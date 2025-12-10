//! Node.js Bridge - Direct function calls without HTTP overhead
//!
//! This module provides the fastest test execution by calling JavaScript
//! functions directly via a Node.js process and JSON-RPC communication.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use super::rpc::{send_request, spawn_communication_task, RequestSender};
use super::{AssertionResult, BridgeError, NodejsBridgeOperations, NodejsOperations};
use crate::engine::ClockSyncState;
use crate::workflow::NodejsConfig;

/// Bridge to Node.js for direct function calls
pub struct NodejsBridge {
    request_tx: RequestSender,
    #[allow(dead_code)]
    child: Child,
}

impl NodejsBridge {
    /// Create a new Node.js bridge from configuration
    pub async fn from_config(config: &NodejsConfig) -> Result<Self, BridgeError> {
        let working_dir = config.working_dir.clone().unwrap_or_else(|| ".".to_string());
        Self::new(&config.registry, &working_dir, config.typescript, config.env_file.as_deref())
            .await
    }

    /// Create a new Node.js bridge
    pub async fn new(
        registry_path: &str,
        working_dir: &str,
        typescript: bool,
        env_file: Option<&str>,
    ) -> Result<Self, BridgeError> {
        let registry_abs = if Path::new(registry_path).is_absolute() {
            registry_path.to_string()
        } else {
            Path::new(working_dir)
                .join(registry_path)
                .to_string_lossy()
                .to_string()
        };

        let mut args = vec![
            "extensions/nodejs/server.js".to_string(),
            "--registry".to_string(),
            registry_abs,
        ];

        if typescript {
            args.push("--typescript".to_string());
        }

        if let Some(env) = env_file {
            args.push("--env-file".to_string());
            args.push(env.to_string());
        }

        let mut child = Command::new("node")
            .args(&args)
            .current_dir(working_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| BridgeError::StartupFailed(format!("Failed to spawn Node.js: {}", e)))?;

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let (request_tx, request_rx) = mpsc::channel(100);
        spawn_communication_task(request_rx, stdin, stdout);

        Ok(Self { request_tx, child })
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value, BridgeError> {
        send_request(&self.request_tx, method, params).await
    }
}

#[async_trait]
impl NodejsBridgeOperations for NodejsBridge {
    async fn fn_call(&self, name: &str, args: Value) -> Result<Value, BridgeError> {
        let result = self
            .request("fn.call", serde_json::json!({ "name": name, "args": args }))
            .await?;
        Ok(result.get("result").cloned().unwrap_or(Value::Null))
    }

    async fn ctx_get(&self, key: &str) -> Result<Option<Value>, BridgeError> {
        let result = self.request("ctx.get", serde_json::json!({ "key": key })).await?;
        let value = result.get("value").cloned().unwrap_or(Value::Null);
        Ok(if value.is_null() { None } else { Some(value) })
    }

    async fn ctx_set(&self, key: &str, value: Value) -> Result<(), BridgeError> {
        self.request("ctx.set", serde_json::json!({ "key": key, "value": value })).await?;
        Ok(())
    }

    async fn ctx_clear(&self, pattern: &str) -> Result<u64, BridgeError> {
        let result = self.request("ctx.clear", serde_json::json!({ "pattern": pattern })).await?;
        Ok(result.get("cleared").and_then(|v| v.as_u64()).unwrap_or(0))
    }

    async fn mock_set(&self, target: &str, mock_value: Value) -> Result<(), BridgeError> {
        self.request("mock.set", serde_json::json!({ "target": target, "value": mock_value })).await?;
        Ok(())
    }

    async fn mock_clear(&self) -> Result<(), BridgeError> {
        self.request("mock.clear", serde_json::json!({})).await?;
        Ok(())
    }

    async fn hook_call(&self, hook_name: &str) -> Result<(), BridgeError> {
        self.request("hook.call", serde_json::json!({ "hook": hook_name })).await?;
        Ok(())
    }

    async fn assert_custom(
        &self,
        assertion_name: &str,
        params: HashMap<String, Value>,
    ) -> Result<AssertionResult, BridgeError> {
        let result = self
            .request("assert.custom", serde_json::json!({ "name": assertion_name, "params": params }))
            .await?;

        Ok(AssertionResult {
            success: result.get("success").and_then(|v| v.as_bool()).unwrap_or(false),
            message: result.get("message").and_then(|v| v.as_str()).map(|s| s.to_string()),
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
}

impl NodejsOperations for NodejsBridge {}

impl NodejsBridge {
    /// Sync the mock clock state to the Node.js bridge
    pub async fn sync_clock(&self, state: &ClockSyncState) -> Result<(), BridgeError> {
        self.request("clock.sync", serde_json::to_value(state).unwrap())
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::NodejsHooksConfig;

    #[test]
    fn test_nodejs_config_basic() {
        let config = NodejsConfig {
            registry: "./test/registry.js".to_string(),
            working_dir: Some("./".to_string()),
            env_file: None,
            typescript: false,
            hooks: NodejsHooksConfig::default(),
        };

        assert_eq!(config.registry, "./test/registry.js");
        assert!(!config.typescript);
    }

    #[test]
    fn test_nodejs_config_typescript() {
        let config = NodejsConfig {
            registry: "./test/registry.ts".to_string(),
            working_dir: None,
            env_file: Some(".env.test".to_string()),
            typescript: true,
            hooks: NodejsHooksConfig::default(),
        };

        assert!(config.typescript);
        assert_eq!(config.env_file, Some(".env.test".to_string()));
    }
}
