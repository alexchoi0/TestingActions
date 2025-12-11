//! Python Bridge - Direct function calls via JSON-RPC over stdin/stdout

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use super::rpc::{send_request, spawn_communication_task, FunctionInfo, RequestSender};
use super::{AssertionResult, BridgeError};
use crate::engine::ClockSyncState;
use crate::workflow::PythonConfig;

pub struct PythonBridge {
    request_tx: RequestSender,
    #[allow(dead_code)]
    child: Child,
}

impl PythonBridge {
    pub async fn from_config(config: &PythonConfig) -> Result<Self, BridgeError> {
        Self::spawn(config).await
    }

    async fn spawn(config: &PythonConfig) -> Result<Self, BridgeError> {
        let mut cmd = if let Some(venv) = &config.venv {
            let venv_python = if cfg!(windows) {
                Path::new(venv).join("Scripts").join("python.exe")
            } else {
                Path::new(venv).join("bin").join("python")
            };

            if venv_python.exists() {
                Command::new(venv_python)
            } else {
                return Err(BridgeError::StartupFailed(format!(
                    "Virtual environment Python not found at {:?}",
                    venv_python
                )));
            }
        } else {
            Command::new(&config.interpreter)
        };

        cmd.arg(&config.script);

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
            .map_err(|e| BridgeError::StartupFailed(format!("Failed to spawn Python: {}", e)))?;

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

    pub async fn fn_call(&self, name: &str, args: Value) -> Result<Value, BridgeError> {
        let result = self
            .request("fn.call", serde_json::json!({ "name": name, "args": args }))
            .await?;
        Ok(result.get("result").cloned().unwrap_or(Value::Null))
    }

    pub async fn list_functions(&self) -> Result<Vec<FunctionInfo>, BridgeError> {
        let result = self
            .request("list_functions", serde_json::json!({}))
            .await?;
        Ok(parse_function_list(&result, "functions"))
    }

    pub async fn list_assertions(&self) -> Result<Vec<FunctionInfo>, BridgeError> {
        let result = self
            .request("list_assertions", serde_json::json!({}))
            .await?;
        Ok(parse_function_list(&result, "assertions"))
    }

    pub async fn sync_clock(&self, state: &ClockSyncState) -> Result<(), BridgeError> {
        self.request("clock.sync", serde_json::to_value(state).unwrap())
            .await?;
        Ok(())
    }
}

fn parse_function_list(result: &Value, key: &str) -> Vec<FunctionInfo> {
    result
        .get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    let name = v.get("name")?.as_str()?;
                    let desc = v.get("description").and_then(|d| d.as_str()).unwrap_or("");
                    Some(FunctionInfo::new(name, desc))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[async_trait]
impl super::Bridge for PythonBridge {
    fn platform(&self) -> crate::workflow::Platform {
        crate::workflow::Platform::Python
    }

    async fn call(&self, name: &str, args: Value) -> Result<Value, BridgeError> {
        self.fn_call(name, args).await
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
        PythonBridge::sync_clock(self, state).await
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

    fn as_python(&self) -> Option<&PythonBridge> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::rpc::FunctionInfo;

    #[test]
    fn test_python_config_basic() {
        let config = PythonConfig {
            script: "./test/registry.py".to_string(),
            interpreter: "python3".to_string(),
            working_dir: Some("./".to_string()),
            venv: None,
            env: HashMap::new(),
            hooks: Default::default(),
        };
        assert_eq!(config.script, "./test/registry.py");
    }

    #[test]
    fn test_python_config_with_venv() {
        let config = PythonConfig {
            script: "./test/registry.py".to_string(),
            interpreter: "python3".to_string(),
            working_dir: None,
            venv: Some("./.venv".to_string()),
            env: HashMap::new(),
            hooks: Default::default(),
        };
        assert_eq!(config.venv, Some("./.venv".to_string()));
    }

    #[test]
    fn test_python_config_with_env() {
        let mut env = HashMap::new();
        env.insert(
            "DATABASE_URL".to_string(),
            "postgres://localhost/test".to_string(),
        );
        let config = PythonConfig {
            script: "./registry.py".to_string(),
            interpreter: "python3".to_string(),
            working_dir: None,
            venv: None,
            env,
            hooks: Default::default(),
        };
        assert_eq!(
            config.env.get("DATABASE_URL"),
            Some(&"postgres://localhost/test".to_string())
        );
    }

    #[test]
    fn test_function_info() {
        let info = FunctionInfo::new("test_fn", "A test function");
        assert_eq!(info.name, "test_fn");
    }
}
