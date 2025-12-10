//! Node.js action implementations

use std::collections::HashMap;
use crate::bridge::{NodejsBridge, NodejsBridgeOperations};
use crate::engine::error::ExecutorError;
use crate::engine::result::StepResult;

/// Execute a wait action (common across platforms)
pub async fn execute_wait(
    action: &str,
    params: &HashMap<String, String>,
) -> Result<StepResult, ExecutorError> {
    match action {
        "timeout" | "delay" | "ms" => {
            let ms: u64 = params
                .get("ms")
                .or_else(|| params.get("timeout"))
                .or_else(|| params.get("duration"))
                .ok_or_else(|| ExecutorError::MissingParameter("ms or duration".to_string()))?
                .parse()
                .map_err(|_| ExecutorError::InvalidParameter("ms/duration must be a number".to_string()))?;

            tokio::time::sleep(tokio::time::Duration::from_millis(ms)).await;
        }
        _ => return Err(ExecutorError::UnknownAction(format!("wait/{}", action))),
    }

    Ok(StepResult {
        success: true,
        outputs: HashMap::new(),
        error: None,
        response: None,
    })
}

pub async fn execute_node_action(
    bridge: &NodejsBridge,
    action: &str,
    params: &HashMap<String, String>,
    json_params: &HashMap<String, serde_json::Value>,
) -> Result<StepResult, ExecutorError> {
    let mut outputs = HashMap::new();

    match action {
        "call" => {
            let function = params
                .get("function")
                .ok_or_else(|| ExecutorError::MissingParameter("function".to_string()))?;

            let args = json_params
                .get("args")
                .cloned()
                .unwrap_or(serde_json::Value::Null);

            let result = bridge.fn_call(function, args).await?;
            outputs.insert("result".to_string(), result.to_string());
        }
        "chain" => {
            let functions: Vec<String> = json_params
                .get("functions")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();

            let mut current_value = json_params
                .get("initial")
                .cloned()
                .unwrap_or(serde_json::Value::Null);

            for func in functions {
                current_value = bridge.fn_call(&func, current_value).await?;
            }

            outputs.insert("result".to_string(), current_value.to_string());
        }
        _ => return Err(ExecutorError::UnknownAction(format!("node/{}", action))),
    }

    Ok(StepResult {
        success: true,
        outputs,
        error: None,
        response: None,
    })
}

pub async fn execute_ctx_action(
    bridge: &NodejsBridge,
    action: &str,
    params: &HashMap<String, String>,
) -> Result<StepResult, ExecutorError> {
    let mut outputs = HashMap::new();

    match action {
        "get" => {
            let key = params
                .get("key")
                .ok_or_else(|| ExecutorError::MissingParameter("key".to_string()))?;

            let value = bridge.ctx_get(key).await?;
            outputs.insert(
                "value".to_string(),
                value
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "null".to_string()),
            );
        }
        "set" => {
            let key = params
                .get("key")
                .ok_or_else(|| ExecutorError::MissingParameter("key".to_string()))?;

            let value: serde_json::Value = params
                .get("value")
                .map(|s| serde_json::from_str(s).unwrap_or(serde_json::Value::String(s.clone())))
                .unwrap_or(serde_json::Value::Null);

            bridge.ctx_set(key, value).await?;
        }
        "clear" => {
            let pattern = params
                .get("pattern")
                .map(|s| s.as_str())
                .unwrap_or("*");

            let cleared = bridge.ctx_clear(pattern).await?;
            outputs.insert("cleared".to_string(), cleared.to_string());
        }
        _ => return Err(ExecutorError::UnknownAction(format!("ctx/{}", action))),
    }

    Ok(StepResult {
        success: true,
        outputs,
        error: None,
        response: None,
    })
}

pub async fn execute_mock_action(
    bridge: &NodejsBridge,
    action: &str,
    params: &HashMap<String, String>,
) -> Result<StepResult, ExecutorError> {
    match action {
        "set" => {
            let target = params
                .get("target")
                .ok_or_else(|| ExecutorError::MissingParameter("target".to_string()))?;

            let value: serde_json::Value = params
                .get("value")
                .map(|s| serde_json::from_str(s).unwrap_or(serde_json::Value::String(s.clone())))
                .unwrap_or(serde_json::Value::Null);

            bridge.mock_set(target, value).await?;
        }
        "clear" => {
            bridge.mock_clear().await?;
        }
        _ => return Err(ExecutorError::UnknownAction(format!("mock/{}", action))),
    }

    Ok(StepResult {
        success: true,
        outputs: HashMap::new(),
        error: None,
        response: None,
    })
}

pub async fn execute_hook_action(
    bridge: &NodejsBridge,
    action: &str,
    params: &HashMap<String, String>,
) -> Result<StepResult, ExecutorError> {
    match action {
        "call" => {
            let hook = params
                .get("hook")
                .ok_or_else(|| ExecutorError::MissingParameter("hook".to_string()))?;

            bridge.hook_call(hook).await?;
        }
        "before" | "after" => {
            let hook = params.get("hook").map(|s| s.as_str()).unwrap_or(action);
            bridge.hook_call(hook).await?;
        }
        _ => return Err(ExecutorError::UnknownAction(format!("hook/{}", action))),
    }

    Ok(StepResult {
        success: true,
        outputs: HashMap::new(),
        error: None,
        response: None,
    })
}

pub async fn execute_assert_action(
    bridge: &NodejsBridge,
    action: &str,
    json_params: &HashMap<String, serde_json::Value>,
) -> Result<StepResult, ExecutorError> {
    match action {
        "returns" => {
            let function = json_params
                .get("function")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ExecutorError::MissingParameter("function".to_string()))?;

            let args = json_params
                .get("args")
                .cloned()
                .unwrap_or(serde_json::Value::Null);

            let expected = json_params
                .get("expected")
                .ok_or_else(|| ExecutorError::MissingParameter("expected".to_string()))?;

            let actual = bridge.fn_call(function, args).await?;

            if actual != *expected {
                return Err(ExecutorError::AssertionFailed(format!(
                    "Function '{}' returned {:?}, expected {:?}",
                    function, actual, expected
                )));
            }

            return Ok(StepResult {
                success: true,
                outputs: HashMap::new(),
                error: None,
                response: None,
            });
        }
        "throws" => {
            let function = json_params
                .get("function")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ExecutorError::MissingParameter("function".to_string()))?;

            let args = json_params
                .get("args")
                .cloned()
                .unwrap_or(serde_json::Value::Null);

            let result = bridge.fn_call(function, args).await;

            if result.is_ok() {
                return Err(ExecutorError::AssertionFailed(format!(
                    "Function '{}' did not throw an error",
                    function
                )));
            }

            return Ok(StepResult {
                success: true,
                outputs: HashMap::new(),
                error: None,
                response: None,
            });
        }
        "ctx_equals" => {
            let key = json_params
                .get("key")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ExecutorError::MissingParameter("key".to_string()))?;

            let expected = json_params
                .get("value")
                .ok_or_else(|| ExecutorError::MissingParameter("value".to_string()))?;

            let actual = bridge.ctx_get(key).await?;
            let actual_value = actual.unwrap_or(serde_json::Value::Null);

            if actual_value != *expected {
                return Err(ExecutorError::AssertionFailed(format!(
                    "Context '{}': expected {:?}, got {:?}",
                    key, expected, actual_value
                )));
            }

            return Ok(StepResult {
                success: true,
                outputs: HashMap::new(),
                error: None,
                response: None,
            });
        }
        _ => {
            let assertion_params = json_params
                .get("params")
                .and_then(|v| v.as_object())
                .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default();

            let result = bridge.assert_custom(action, assertion_params).await?;

            if !result.success {
                let message = result.message.unwrap_or_else(|| {
                    format!("Custom assertion '{}' failed", action)
                });
                return Err(ExecutorError::AssertionFailed(message));
            }

            let mut outputs = HashMap::new();
            if let Some(actual) = &result.actual {
                outputs.insert("actual".to_string(), actual.to_string());
            }
            if let Some(expected) = &result.expected {
                outputs.insert("expected".to_string(), expected.to_string());
            }

            return Ok(StepResult {
                success: true,
                outputs,
                error: None,
                response: None,
            });
        }
    }
}
