//! Java bridge action implementations

use std::collections::HashMap;
use crate::bridge::{JavaBridge, JavaBridgeOperations};
use crate::engine::error::ExecutorError;
use crate::engine::result::StepResult;

pub async fn execute_java_call(
    bridge: &JavaBridge,
    action: &str,
    params: &HashMap<String, String>,
    json_params: &HashMap<String, serde_json::Value>,
) -> Result<StepResult, ExecutorError> {
    let mut outputs = HashMap::new();

    match action {
        "call" => {
            let method = params
                .get("method")
                .ok_or_else(|| ExecutorError::MissingParameter("method".to_string()))?;

            let args = json_params
                .get("args")
                .cloned()
                .unwrap_or(serde_json::Value::Null);

            let result = bridge.method_call(method, args).await?;
            outputs.insert("result".to_string(), result.to_string());
        }
        _ => return Err(ExecutorError::UnknownAction(format!("java/{}", action))),
    }

    Ok(StepResult {
        success: true,
        outputs,
        error: None,
        response: None,
    })
}

pub async fn execute_assert_action(
    bridge: &JavaBridge,
    action: &str,
    json_params: &HashMap<String, serde_json::Value>,
) -> Result<StepResult, ExecutorError> {
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

    Ok(StepResult {
        success: true,
        outputs,
        error: None,
        response: None,
    })
}
