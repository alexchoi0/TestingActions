//! Java Bridge - Direct method calls via JSON-RPC over stdin/stdout

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::process::Stdio;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use super::rpc::{send_request, spawn_communication_task, MethodInfo, RequestSender};
use super::{AssertionResult, BridgeError};
use crate::engine::ClockSyncState;
use crate::workflow::JavaConfig;

pub struct JavaBridge {
    request_tx: RequestSender,
    #[allow(dead_code)]
    child: Child,
}

impl JavaBridge {
    pub async fn from_config(config: &JavaConfig) -> Result<Self, BridgeError> {
        Self::spawn(config).await
    }

    async fn spawn(config: &JavaConfig) -> Result<Self, BridgeError> {
        let mut classpath_parts = config.classpath.clone();
        if let Some(jar) = &config.jar {
            classpath_parts.insert(0, jar.clone());
        }

        let classpath = if classpath_parts.is_empty() {
            ".".to_string()
        } else {
            let separator = if cfg!(windows) { ";" } else { ":" };
            classpath_parts.join(separator)
        };

        let mut cmd = Command::new(&config.java_home);

        for arg in &config.jvm_args {
            cmd.arg(arg);
        }

        cmd.arg("-cp").arg(&classpath).arg(&config.main_class);

        if let Some(dir) = &config.working_dir {
            cmd.current_dir(dir);
        }

        for (k, v) in &config.env {
            cmd.env(k, v);
        }

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let mut child = cmd
            .spawn()
            .map_err(|e| BridgeError::StartupFailed(format!("Failed to spawn Java: {}", e)))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| BridgeError::StartupFailed("Failed to get stdin".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| BridgeError::StartupFailed("Failed to get stdout".to_string()))?;

        let (request_tx, request_rx) = mpsc::channel(100);
        spawn_communication_task(request_rx, stdin, stdout);

        Ok(Self { request_tx, child })
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value, BridgeError> {
        send_request(&self.request_tx, method, params).await
    }

    pub async fn method_call(&self, name: &str, args: Value) -> Result<Value, BridgeError> {
        let result = self
            .request(
                "method.call",
                serde_json::json!({ "name": name, "args": args }),
            )
            .await?;
        Ok(result.get("result").cloned().unwrap_or(Value::Null))
    }

    pub async fn list_methods(&self) -> Result<Vec<MethodInfo>, BridgeError> {
        let result = self.request("list_methods", serde_json::json!({})).await?;
        Ok(parse_method_list(&result, "methods"))
    }

    pub async fn list_assertions(&self) -> Result<Vec<MethodInfo>, BridgeError> {
        let result = self
            .request("list_assertions", serde_json::json!({}))
            .await?;
        Ok(parse_method_list(&result, "assertions"))
    }

    pub async fn sync_clock(&self, state: &ClockSyncState) -> Result<(), BridgeError> {
        self.request("clock.sync", serde_json::to_value(state).unwrap())
            .await?;
        Ok(())
    }
}

fn parse_method_list(result: &Value, key: &str) -> Vec<MethodInfo> {
    result
        .get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    let name = v.get("name")?.as_str()?;
                    let desc = v.get("description").and_then(|d| d.as_str()).unwrap_or("");
                    Some(MethodInfo::new(name, desc))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[async_trait]
impl super::Bridge for JavaBridge {
    fn platform(&self) -> crate::workflow::Platform {
        crate::workflow::Platform::Java
    }

    async fn call(&self, name: &str, args: Value) -> Result<Value, BridgeError> {
        self.method_call(name, args).await
    }

    async fn ctx_get(&self, key: &str) -> Result<Option<Value>, BridgeError> {
        let result = self
            .request("ctx.get", serde_json::json!({ "key": key }))
            .await?;
        let value = result.get("value").cloned().unwrap_or(Value::Null);
        Ok(if value.is_null() { None } else { Some(value) })
    }

    async fn ctx_set(&self, key: &str, value: Value) -> Result<(), BridgeError> {
        self.request("ctx.set", serde_json::json!({ "key": key, "value": value }))
            .await?;
        Ok(())
    }

    async fn ctx_clear(&self, pattern: &str) -> Result<u64, BridgeError> {
        let result = self
            .request("ctx.clear", serde_json::json!({ "pattern": pattern }))
            .await?;
        Ok(result.get("cleared").and_then(|v| v.as_u64()).unwrap_or(0))
    }

    async fn hook_call(&self, hook_name: &str) -> Result<(), BridgeError> {
        self.request("hook.call", serde_json::json!({ "hook": hook_name }))
            .await?;
        Ok(())
    }

    async fn assert_custom(
        &self,
        name: &str,
        params: HashMap<String, Value>,
    ) -> Result<AssertionResult, BridgeError> {
        let result = self
            .request(
                "assert.custom",
                serde_json::json!({ "name": name, "params": params }),
            )
            .await?;
        Ok(AssertionResult {
            success: result
                .get("success")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            message: result
                .get("message")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            actual: result.get("actual").cloned(),
            expected: result.get("expected").cloned(),
        })
    }

    async fn set_execution_info(
        &self,
        run_id: &str,
        job_name: &str,
        step_name: &str,
    ) -> Result<(), BridgeError> {
        self.request(
            "ctx.setExecutionInfo",
            serde_json::json!({ "runId": run_id, "jobName": job_name, "stepName": step_name }),
        )
        .await?;
        Ok(())
    }

    async fn sync_step_outputs(
        &self,
        step_id: &str,
        outputs: HashMap<String, String>,
    ) -> Result<(), BridgeError> {
        self.request(
            "ctx.syncStepOutputs",
            serde_json::json!({ "stepId": step_id, "outputs": outputs }),
        )
        .await?;
        Ok(())
    }

    async fn sync_clock(&self, state: &ClockSyncState) -> Result<(), BridgeError> {
        JavaBridge::sync_clock(self, state).await
    }

    fn supports_context(&self) -> bool {
        true
    }

    fn supports_hooks(&self) -> bool {
        true
    }

    fn supports_clock(&self) -> bool {
        true
    }

    fn as_java(&self) -> Option<&JavaBridge> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::rpc::MethodInfo;

    #[test]
    fn test_java_config_basic() {
        let config = JavaConfig {
            jar: Some("./target/test-registry.jar".to_string()),
            main_class: "com.example.TestRegistry".to_string(),
            classpath: vec![],
            java_home: "java".to_string(),
            jvm_args: vec![],
            working_dir: Some("./".to_string()),
            env: HashMap::new(),
            hooks: Default::default(),
        };
        assert_eq!(config.main_class, "com.example.TestRegistry");
    }

    #[test]
    fn test_java_config_with_classpath() {
        let config = JavaConfig {
            jar: None,
            main_class: "com.example.TestRegistry".to_string(),
            classpath: vec!["./lib/dep1.jar".to_string(), "./lib/dep2.jar".to_string()],
            java_home: "java".to_string(),
            jvm_args: vec!["-Xmx512m".to_string()],
            working_dir: None,
            env: HashMap::new(),
            hooks: Default::default(),
        };
        assert_eq!(config.classpath.len(), 2);
        assert_eq!(config.jvm_args, vec!["-Xmx512m"]);
    }

    #[test]
    fn test_java_config_with_env() {
        let mut env = HashMap::new();
        env.insert(
            "DATABASE_URL".to_string(),
            "jdbc:postgresql://localhost/test".to_string(),
        );
        let config = JavaConfig {
            jar: Some("./registry.jar".to_string()),
            main_class: "com.example.Registry".to_string(),
            classpath: vec![],
            java_home: "java".to_string(),
            jvm_args: vec![],
            working_dir: None,
            env,
            hooks: Default::default(),
        };
        assert_eq!(
            config.env.get("DATABASE_URL"),
            Some(&"jdbc:postgresql://localhost/test".to_string())
        );
    }

    #[test]
    fn test_method_info() {
        let info = MethodInfo::new("testMethod", "A test method");
        assert_eq!(info.name, "testMethod");
    }
}
