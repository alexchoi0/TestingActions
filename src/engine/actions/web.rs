//! Web/HTTP action implementations

use std::collections::HashMap;
use crate::bridge::{WebBridge, WebBridgeOperations};
use crate::engine::error::ExecutorError;
use crate::engine::result::StepResult;

pub async fn execute_web_request(
    bridge: &WebBridge,
    action: &str,
    params: &HashMap<String, String>,
) -> Result<StepResult, ExecutorError> {
    let path = params
        .get("path")
        .or_else(|| params.get("url"))
        .ok_or_else(|| ExecutorError::MissingParameter("path or url".to_string()))?;

    let headers: Option<HashMap<String, String>> = params
        .get("headers")
        .map(|s| serde_json::from_str(s).unwrap_or_default());

    let body: Option<serde_json::Value> = params
        .get("body")
        .map(|s| serde_json::from_str(s).unwrap_or(serde_json::Value::String(s.clone())));

    let query: Option<HashMap<String, String>> = params
        .get("query")
        .map(|s| serde_json::from_str(s).unwrap_or_default());

    let response = match action {
        "get" => bridge.get(path, headers, query).await?,
        "post" => bridge.post(path, body, headers).await?,
        "put" => bridge.put(path, body, headers).await?,
        "patch" => bridge.patch(path, body, headers).await?,
        "delete" => bridge.delete(path, headers).await?,
        "request" => {
            let method = params
                .get("method")
                .ok_or_else(|| ExecutorError::MissingParameter("method".to_string()))?;
            bridge.request(method, path, body, headers, query).await?
        }
        _ => return Err(ExecutorError::UnknownAction(format!("web/{}", action))),
    };

    let mut outputs = HashMap::new();
    outputs.insert("status".to_string(), response.status.to_string());
    outputs.insert("body".to_string(), response.body.to_string());
    outputs.insert("elapsed_ms".to_string(), response.elapsed_ms.to_string());

    if let serde_json::Value::Object(map) = &response.body {
        for (key, value) in map {
            outputs.insert(format!("body.{}", key), value.to_string());
        }
    }

    Ok(StepResult {
        success: response.is_success(),
        outputs,
        error: if response.is_success() {
            None
        } else {
            Some(format!("HTTP {} error", response.status))
        },
        response: Some(response.to_api_response()),
    })
}

pub async fn execute_assert(
    action: &str,
    params: &HashMap<String, String>,
) -> Result<StepResult, ExecutorError> {
    match action {
        "status" => {
            let expected = params
                .get("expected")
                .ok_or_else(|| ExecutorError::MissingParameter("expected".to_string()))?
                .parse::<u16>()
                .map_err(|_| ExecutorError::MissingParameter("expected must be a number".to_string()))?;
            let actual = params
                .get("actual")
                .ok_or_else(|| ExecutorError::MissingParameter("actual".to_string()))?
                .parse::<u16>()
                .map_err(|_| ExecutorError::MissingParameter("actual must be a number".to_string()))?;

            if actual != expected {
                return Err(ExecutorError::AssertionFailed(format!(
                    "Expected status {}, got {}",
                    expected, actual
                )));
            }

            Ok(StepResult {
                success: true,
                outputs: HashMap::new(),
                error: None,
                response: None,
            })
        }
        "json_path" => {
            Err(ExecutorError::UnknownAction("assert/json_path not yet implemented".to_string()))
        }
        _ => Err(ExecutorError::UnknownAction(format!("assert/{}", action))),
    }
}
