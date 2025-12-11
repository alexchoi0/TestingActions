//! Bash/shell action implementations
//!
//! Executes shell commands or scripts directly on the host system.
//!
//! Actions:
//! - `bash/exec` - Execute a command or script
//!
//! Example:
//! ```yaml
//! - uses: bash/exec
//!   with:
//!     command: "echo Hello World"
//!
//! - uses: bash/exec
//!   with:
//!     script: "./setup.sh"
//!     args: ["--env", "test"]
//!
//! - uses: bash/exec
//!   with:
//!     command: "npm install"
//!     working_dir: "./frontend"
//! ```

use std::collections::HashMap;
use std::process::Stdio;
use tokio::process::Command;
use tracing::info;

use crate::engine::error::ExecutorError;
use crate::engine::result::StepResult;

/// Execute a bash action
pub async fn execute_bash_action(
    action: &str,
    params: &HashMap<String, String>,
) -> Result<StepResult, ExecutorError> {
    match action {
        "exec" => execute_exec(params).await,
        _ => Err(ExecutorError::UnknownAction(format!(
            "Unknown bash action: {}",
            action
        ))),
    }
}

/// Execute a command or script
async fn execute_exec(params: &HashMap<String, String>) -> Result<StepResult, ExecutorError> {
    let command = params.get("command");
    let script = params.get("script");
    let working_dir = params.get("working_dir");

    let (shell_cmd, args) = if let Some(cmd) = command {
        // Run command through shell
        info!("Executing bash command: {}", cmd);
        ("sh".to_string(), vec!["-c".to_string(), cmd.clone()])
    } else if let Some(script_path) = script {
        // Run script file
        info!("Executing bash script: {}", script_path);

        // Parse additional args if provided
        let mut script_args = vec![script_path.clone()];
        if let Some(args_str) = params.get("args") {
            // Try to parse as JSON array, fallback to space-separated
            if let Ok(parsed) = serde_json::from_str::<Vec<String>>(args_str) {
                script_args.extend(parsed);
            } else {
                script_args.extend(args_str.split_whitespace().map(String::from));
            }
        }

        ("bash".to_string(), script_args)
    } else {
        return Err(ExecutorError::ConfigError(
            "bash/exec requires either 'command' or 'script' parameter".to_string(),
        ));
    };

    let mut cmd = Command::new(&shell_cmd);
    cmd.args(&args);

    // Set working directory if specified
    if let Some(dir) = working_dir {
        cmd.current_dir(dir);
    }

    // Capture stdout and stderr
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Execute
    let output = cmd
        .output()
        .await
        .map_err(|e| ExecutorError::StepFailed(format!("Failed to execute command: {}", e)))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    let mut outputs = HashMap::new();
    outputs.insert("stdout".to_string(), stdout.trim().to_string());
    outputs.insert("stderr".to_string(), stderr.trim().to_string());
    outputs.insert(
        "exit_code".to_string(),
        output.status.code().unwrap_or(-1).to_string(),
    );

    if output.status.success() {
        info!("Command completed successfully");
        Ok(StepResult {
            success: true,
            outputs,
            error: None,
            response: None,
        })
    } else {
        let error_msg = if stderr.is_empty() {
            format!(
                "Command exited with code {}",
                output.status.code().unwrap_or(-1)
            )
        } else {
            stderr.trim().to_string()
        };

        Err(ExecutorError::StepFailed(error_msg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bash_exec_command() {
        let mut params = HashMap::new();
        params.insert("command".to_string(), "echo hello".to_string());

        let result = execute_bash_action("exec", &params).await.unwrap();
        assert!(result.success);
        assert_eq!(result.outputs.get("stdout"), Some(&"hello".to_string()));
        assert_eq!(result.outputs.get("exit_code"), Some(&"0".to_string()));
    }

    #[tokio::test]
    async fn test_bash_exec_failing_command() {
        let mut params = HashMap::new();
        params.insert("command".to_string(), "exit 1".to_string());

        let result = execute_bash_action("exec", &params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_bash_exec_with_working_dir() {
        let mut params = HashMap::new();
        params.insert("command".to_string(), "pwd".to_string());
        params.insert("working_dir".to_string(), "/tmp".to_string());

        let result = execute_bash_action("exec", &params).await.unwrap();
        assert!(result.success);
        // On macOS /tmp is a symlink to /private/tmp
        let stdout = result.outputs.get("stdout").unwrap();
        assert!(stdout == "/tmp" || stdout == "/private/tmp");
    }

    #[tokio::test]
    async fn test_bash_exec_missing_params() {
        let params = HashMap::new();
        let result = execute_bash_action("exec", &params).await;
        assert!(result.is_err());
    }
}
