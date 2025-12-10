//! Rust Bridge - Direct function calls via JSON-RPC over stdin/stdout

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
use crate::workflow::RustConfig;

pub struct RustBridge {
    request_tx: RequestSender,
    #[allow(dead_code)]
    child: Child,
}

impl RustBridge {
    pub async fn from_config(config: &RustConfig) -> Result<Self, BridgeError> {
        let binary_path = if let Some(bin) = &config.binary {
            bin.clone()
        } else if let Some(cargo_bin) = &config.cargo_bin {
            Self::cargo_build(cargo_bin, config.working_dir.as_deref()).await?
        } else {
            return Err(BridgeError::StartupFailed(
                "Rust config requires either 'binary' or 'cargo_bin'".to_string(),
            ));
        };

        Self::spawn(&binary_path, config).await
    }

    async fn cargo_build(bin_name: &str, working_dir: Option<&str>) -> Result<String, BridgeError> {
        let mut cmd = Command::new("cargo");
        cmd.args(["build", "--bin", bin_name]);

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        let status = cmd.status().await.map_err(|e| {
            BridgeError::StartupFailed(format!("Failed to run cargo build: {}", e))
        })?;

        if !status.success() {
            return Err(BridgeError::StartupFailed(format!("cargo build --bin {} failed", bin_name)));
        }

        let mut metadata_cmd = Command::new("cargo");
        metadata_cmd.args(["metadata", "--format-version", "1", "--no-deps"]);

        if let Some(dir) = working_dir {
            metadata_cmd.current_dir(dir);
        }

        let output = metadata_cmd.output().await.map_err(|e| {
            BridgeError::StartupFailed(format!("Failed to get cargo metadata: {}", e))
        })?;

        if !output.status.success() {
            return Err(BridgeError::StartupFailed("cargo metadata failed".to_string()));
        }

        let metadata: Value = serde_json::from_slice(&output.stdout).map_err(|e| {
            BridgeError::StartupFailed(format!("Failed to parse cargo metadata: {}", e))
        })?;

        let target_dir = metadata
            .get("target_directory")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BridgeError::StartupFailed("No target_directory in metadata".to_string()))?;

        let binary_path = format!("{}/debug/{}", target_dir, bin_name);

        if !Path::new(&binary_path).exists() {
            return Err(BridgeError::StartupFailed(format!("Binary not found at {}", binary_path)));
        }

        Ok(binary_path)
    }

    async fn spawn(binary: &str, config: &RustConfig) -> Result<Self, BridgeError> {
        let mut cmd = Command::new(binary);

        if let Some(dir) = &config.working_dir {
            cmd.current_dir(dir);
        }

        for (k, v) in &config.env {
            cmd.env(k, v);
        }

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let mut child = cmd.spawn().map_err(|e| {
            BridgeError::StartupFailed(format!("Failed to spawn Rust binary '{}': {}", binary, e))
        })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            BridgeError::StartupFailed("Failed to get stdin".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            BridgeError::StartupFailed("Failed to get stdout".to_string())
        })?;

        let (request_tx, request_rx) = mpsc::channel(100);
        spawn_communication_task(request_rx, stdin, stdout);

        Ok(Self { request_tx, child })
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value, BridgeError> {
        send_request(&self.request_tx, method, params).await
    }
}

#[async_trait]
pub trait RustBridgeOperations: Send + Sync {
    async fn fn_call(&self, name: &str, args: Value) -> Result<Value, BridgeError>;
    async fn ctx_get(&self, key: &str) -> Result<Option<Value>, BridgeError>;
    async fn ctx_set(&self, key: &str, value: Value) -> Result<(), BridgeError>;
    async fn ctx_clear(&self, pattern: &str) -> Result<u64, BridgeError>;
    async fn hook_call(&self, hook_name: &str) -> Result<(), BridgeError>;
    async fn assert_custom(&self, name: &str, params: HashMap<String, Value>) -> Result<AssertionResult, BridgeError>;
    async fn set_execution_info(&self, run_id: &str, job_name: &str, step_name: &str) -> Result<(), BridgeError>;
    async fn sync_step_outputs(&self, step_id: &str, outputs: HashMap<String, String>) -> Result<(), BridgeError>;
    async fn list_functions(&self) -> Result<Vec<FunctionInfo>, BridgeError>;
    async fn list_assertions(&self) -> Result<Vec<FunctionInfo>, BridgeError>;
}

#[async_trait]
impl RustBridgeOperations for RustBridge {
    async fn fn_call(&self, name: &str, args: Value) -> Result<Value, BridgeError> {
        let result = self.request("fn.call", serde_json::json!({ "name": name, "args": args })).await?;
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

    async fn hook_call(&self, hook_name: &str) -> Result<(), BridgeError> {
        self.request("hook.call", serde_json::json!({ "hook": hook_name })).await?;
        Ok(())
    }

    async fn assert_custom(&self, name: &str, params: HashMap<String, Value>) -> Result<AssertionResult, BridgeError> {
        let result = self.request("assert.custom", serde_json::json!({ "name": name, "params": params })).await?;
        Ok(AssertionResult {
            success: result.get("success").and_then(|v| v.as_bool()).unwrap_or(false),
            message: result.get("message").and_then(|v| v.as_str()).map(|s| s.to_string()),
            actual: result.get("actual").cloned(),
            expected: result.get("expected").cloned(),
        })
    }

    async fn set_execution_info(&self, run_id: &str, job_name: &str, step_name: &str) -> Result<(), BridgeError> {
        self.request("ctx.setExecutionInfo", serde_json::json!({ "runId": run_id, "jobName": job_name, "stepName": step_name })).await?;
        Ok(())
    }

    async fn sync_step_outputs(&self, step_id: &str, outputs: HashMap<String, String>) -> Result<(), BridgeError> {
        self.request("ctx.syncStepOutputs", serde_json::json!({ "stepId": step_id, "outputs": outputs })).await?;
        Ok(())
    }

    async fn list_functions(&self) -> Result<Vec<FunctionInfo>, BridgeError> {
        let result = self.request("list_functions", serde_json::json!({})).await?;
        Ok(parse_function_list(&result, "functions"))
    }

    async fn list_assertions(&self) -> Result<Vec<FunctionInfo>, BridgeError> {
        let result = self.request("list_assertions", serde_json::json!({})).await?;
        Ok(parse_function_list(&result, "assertions"))
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

pub trait RustOperations {}
impl RustOperations for RustBridge {}

impl RustBridge {
    /// Sync the mock clock state to the Rust bridge
    pub async fn sync_clock(&self, state: &ClockSyncState) -> Result<(), BridgeError> {
        self.request("clock.sync", serde_json::to_value(state).unwrap())
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_config_with_binary() {
        let config = RustConfig {
            binary: Some("./target/debug/test-registry".to_string()),
            cargo_bin: None,
            working_dir: Some("./".to_string()),
            env: HashMap::new(),
            hooks: Default::default(),
        };
        assert_eq!(config.binary, Some("./target/debug/test-registry".to_string()));
    }

    #[test]
    fn test_rust_config_with_cargo_bin() {
        let config = RustConfig {
            binary: None,
            cargo_bin: Some("my-registry".to_string()),
            working_dir: None,
            env: HashMap::new(),
            hooks: Default::default(),
        };
        assert_eq!(config.cargo_bin, Some("my-registry".to_string()));
    }

    #[test]
    fn test_rust_config_with_env() {
        let mut env = HashMap::new();
        env.insert("DATABASE_URL".to_string(), "postgres://localhost/test".to_string());
        let config = RustConfig {
            binary: Some("./registry".to_string()),
            cargo_bin: None,
            working_dir: None,
            env,
            hooks: Default::default(),
        };
        assert_eq!(config.env.get("DATABASE_URL"), Some(&"postgres://localhost/test".to_string()));
    }
}
