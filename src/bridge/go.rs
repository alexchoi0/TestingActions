//! Go Bridge - Executor-side bridge to communicate with user's Go binary
//!
//! This module provides `GoBridge`, which spawns a Go process running the user's
//! registry binary and communicates with it via JSON-RPC over stdin/stdout.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::process::Stdio;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use super::rpc::{send_request, spawn_communication_task, FunctionInfo, RequestSender};
use super::{AssertionResult, BridgeError};
use crate::engine::ClockSyncState;
use crate::workflow::GoConfig;

/// Bridge to a Go binary for direct function calls
pub struct GoBridge {
    request_tx: RequestSender,
    #[allow(dead_code)]
    child: Child,
}

impl GoBridge {
    /// Create a new Go bridge from configuration
    pub async fn from_config(config: &GoConfig) -> Result<Self, BridgeError> {
        Self::spawn(config).await
    }

    async fn spawn(config: &GoConfig) -> Result<Self, BridgeError> {
        let (binary_path, mut cmd) = if let Some(binary) = &config.binary {
            (binary.clone(), Command::new(binary))
        } else if let Some(go_run) = &config.go_run {
            (go_run.clone(), {
                let mut cmd = Command::new("go");
                cmd.arg("run").arg(go_run);
                cmd
            })
        } else if let Some(go_build) = &config.go_build {
            let binary_name = format!("{}_bridge", go_build.replace(['/', '\\', '.'], "_"));
            let mut build_cmd = Command::new("go");
            build_cmd.arg("build").arg("-o").arg(&binary_name).arg(go_build);

            if let Some(dir) = &config.working_dir {
                build_cmd.current_dir(dir);
            }

            let status = build_cmd.status().await.map_err(|e| {
                BridgeError::StartupFailed(format!("Failed to build Go binary: {}", e))
            })?;

            if !status.success() {
                return Err(BridgeError::StartupFailed("Go build failed".to_string()));
            }

            (binary_name.clone(), Command::new(&binary_name))
        } else {
            return Err(BridgeError::ConfigError(
                "Go config must specify 'binary', 'go_run', or 'go_build'".to_string(),
            ));
        };

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
            BridgeError::StartupFailed(format!("Failed to spawn Go process '{}': {}", binary_path, e))
        })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            BridgeError::StartupFailed("Failed to get stdin of Go process".to_string())
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            BridgeError::StartupFailed("Failed to get stdout of Go process".to_string())
        })?;

        let (request_tx, request_rx) = mpsc::channel(100);
        spawn_communication_task(request_rx, stdin, stdout);

        Ok(Self { request_tx, child })
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value, BridgeError> {
        send_request(&self.request_tx, method, params).await
    }
}

/// Trait for Go bridge operations
#[async_trait]
pub trait GoBridgeOperations: Send + Sync {
    async fn fn_call(&self, name: &str, args: Value) -> Result<Value, BridgeError>;
    async fn ctx_get(&self, key: &str) -> Result<Option<Value>, BridgeError>;
    async fn ctx_set(&self, key: &str, value: Value) -> Result<(), BridgeError>;
    async fn ctx_clear(&self, pattern: &str) -> Result<u64, BridgeError>;
    async fn hook_call(&self, hook_name: &str) -> Result<(), BridgeError>;
    async fn assert_custom(&self, name: &str, params: HashMap<String, Value>) -> Result<AssertionResult, BridgeError>;
    async fn set_execution_info(&self, run_id: &str, job_name: &str, step_name: &str) -> Result<(), BridgeError>;
    async fn sync_step_outputs(&self, step_id: &str, outputs: HashMap<String, String>) -> Result<(), BridgeError>;
    async fn list_functions(&self) -> Result<Vec<FunctionInfo>, BridgeError>;
}

#[async_trait]
impl GoBridgeOperations for GoBridge {
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

    async fn set_execution_info(&self, run_id: &str, job_name: &str, step_name: &str) -> Result<(), BridgeError> {
        self.request(
            "ctx.setExecutionInfo",
            serde_json::json!({ "runId": run_id, "jobName": job_name, "stepName": step_name }),
        )
        .await?;
        Ok(())
    }

    async fn sync_step_outputs(&self, step_id: &str, outputs: HashMap<String, String>) -> Result<(), BridgeError> {
        self.request("ctx.syncStepOutputs", serde_json::json!({ "stepId": step_id, "outputs": outputs })).await?;
        Ok(())
    }

    async fn list_functions(&self) -> Result<Vec<FunctionInfo>, BridgeError> {
        let result = self.request("list_functions", serde_json::json!({})).await?;
        let functions = result
            .get("functions")
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
            .unwrap_or_default();
        Ok(functions)
    }
}

/// Marker trait for Go-specific operations
pub trait GoOperations {}
impl GoOperations for GoBridge {}

impl GoBridge {
    /// Sync the mock clock state to the Go bridge
    pub async fn sync_clock(&self, state: &ClockSyncState) -> Result<(), BridgeError> {
        self.request("clock.sync", serde_json::to_value(state).unwrap())
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::rpc::FunctionInfo;

    #[test]
    fn test_go_config_with_binary() {
        let config = GoConfig {
            binary: Some("./test/registry".to_string()),
            go_run: None,
            go_build: None,
            working_dir: Some("./".to_string()),
            env: HashMap::new(),
            hooks: Default::default(),
        };
        assert_eq!(config.binary, Some("./test/registry".to_string()));
    }

    #[test]
    fn test_go_config_with_go_run() {
        let config = GoConfig {
            binary: None,
            go_run: Some("./cmd/registry/main.go".to_string()),
            go_build: None,
            working_dir: None,
            env: HashMap::new(),
            hooks: Default::default(),
        };
        assert_eq!(config.go_run, Some("./cmd/registry/main.go".to_string()));
    }

    #[test]
    fn test_go_config_with_env() {
        let mut env = HashMap::new();
        env.insert("DATABASE_URL".to_string(), "postgres://localhost/test".to_string());
        let config = GoConfig {
            binary: Some("./registry".to_string()),
            go_run: None,
            go_build: None,
            working_dir: None,
            env,
            hooks: Default::default(),
        };
        assert_eq!(config.env.get("DATABASE_URL"), Some(&"postgres://localhost/test".to_string()));
    }

    #[test]
    fn test_function_info() {
        let info = FunctionInfo::new("test_fn", "A test function");
        assert_eq!(info.name, "test_fn");
        assert_eq!(info.description, "A test function");
    }
}
