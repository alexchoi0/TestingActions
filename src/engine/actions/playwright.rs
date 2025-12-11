//! Playwright action implementations

use crate::bridge::PlaywrightBridge;
use crate::engine::error::ExecutorError;
use std::collections::HashMap;

pub async fn execute_page_action(
    bridge: &PlaywrightBridge,
    action: &str,
    page_id: &str,
    params: &HashMap<String, String>,
) -> Result<HashMap<String, String>, ExecutorError> {
    let mut outputs = HashMap::new();

    match action {
        "goto" => {
            let url = params
                .get("url")
                .ok_or_else(|| ExecutorError::MissingParameter("url".to_string()))?;
            bridge.page_goto(page_id, url).await?;
        }
        "reload" => {
            bridge.page_reload(page_id).await?;
        }
        "back" => {
            bridge.page_go_back(page_id).await?;
        }
        "forward" => {
            bridge.page_go_forward(page_id).await?;
        }
        "url" => {
            let url = bridge.page_url(page_id).await?;
            outputs.insert("url".to_string(), url);
        }
        "title" => {
            let title = bridge.page_title(page_id).await?;
            outputs.insert("title".to_string(), title);
        }
        _ => return Err(ExecutorError::UnknownAction(format!("page/{}", action))),
    }

    Ok(outputs)
}

pub async fn execute_element_action(
    bridge: &PlaywrightBridge,
    action: &str,
    page_id: &str,
    params: &HashMap<String, String>,
) -> Result<HashMap<String, String>, ExecutorError> {
    let selector = params
        .get("selector")
        .ok_or_else(|| ExecutorError::MissingParameter("selector".to_string()))?;

    let mut outputs = HashMap::new();

    match action {
        "click" => {
            bridge.element_click(page_id, selector).await?;
        }
        "fill" => {
            let value = params
                .get("value")
                .ok_or_else(|| ExecutorError::MissingParameter("value".to_string()))?;
            bridge.element_fill(page_id, selector, value).await?;
        }
        "type" => {
            let text = params
                .get("text")
                .ok_or_else(|| ExecutorError::MissingParameter("text".to_string()))?;
            let delay = params.get("delay").and_then(|d| d.parse().ok());
            bridge.element_type(page_id, selector, text, delay).await?;
        }
        "select" => {
            let value = params
                .get("value")
                .ok_or_else(|| ExecutorError::MissingParameter("value".to_string()))?;
            bridge.element_select(page_id, selector, value).await?;
        }
        "hover" => {
            bridge.element_hover(page_id, selector).await?;
        }
        "text" => {
            let text = bridge.element_text(page_id, selector).await?;
            outputs.insert("text".to_string(), text);
        }
        "attribute" => {
            let attr = params
                .get("attribute")
                .ok_or_else(|| ExecutorError::MissingParameter("attribute".to_string()))?;
            let value = bridge.element_attribute(page_id, selector, attr).await?;
            outputs.insert("value".to_string(), value.unwrap_or_default());
        }
        _ => return Err(ExecutorError::UnknownAction(format!("element/{}", action))),
    }

    Ok(outputs)
}

pub async fn execute_assert(
    bridge: &PlaywrightBridge,
    action: &str,
    page_id: &str,
    params: &HashMap<String, String>,
) -> Result<HashMap<String, String>, ExecutorError> {
    match action {
        "visible" => {
            let selector = params
                .get("selector")
                .ok_or_else(|| ExecutorError::MissingParameter("selector".to_string()))?;
            let visible = bridge.element_is_visible(page_id, selector).await?;
            if !visible {
                return Err(ExecutorError::AssertionFailed(format!(
                    "Element '{}' is not visible",
                    selector
                )));
            }
        }
        "hidden" => {
            let selector = params
                .get("selector")
                .ok_or_else(|| ExecutorError::MissingParameter("selector".to_string()))?;
            let visible = bridge.element_is_visible(page_id, selector).await?;
            if visible {
                return Err(ExecutorError::AssertionFailed(format!(
                    "Element '{}' is visible but expected hidden",
                    selector
                )));
            }
        }
        "text_contains" => {
            let selector = params
                .get("selector")
                .ok_or_else(|| ExecutorError::MissingParameter("selector".to_string()))?;
            let expected = params
                .get("text")
                .ok_or_else(|| ExecutorError::MissingParameter("text".to_string()))?;
            let actual = bridge.element_text(page_id, selector).await?;
            if !actual.contains(expected) {
                return Err(ExecutorError::AssertionFailed(format!(
                    "Text '{}' does not contain '{}'",
                    actual, expected
                )));
            }
        }
        "url_contains" => {
            let pattern = params
                .get("pattern")
                .ok_or_else(|| ExecutorError::MissingParameter("pattern".to_string()))?;
            let url = bridge.page_url(page_id).await?;
            if !url.contains(pattern) {
                return Err(ExecutorError::AssertionFailed(format!(
                    "URL '{}' does not contain '{}'",
                    url, pattern
                )));
            }
        }
        "title_is" => {
            let expected = params
                .get("title")
                .ok_or_else(|| ExecutorError::MissingParameter("title".to_string()))?;
            let actual = bridge.page_title(page_id).await?;
            if &actual != expected {
                return Err(ExecutorError::AssertionFailed(format!(
                    "Title '{}' does not match '{}'",
                    actual, expected
                )));
            }
        }
        _ => return Err(ExecutorError::UnknownAction(format!("assert/{}", action))),
    }

    Ok(HashMap::new())
}

pub async fn execute_wait_action(
    bridge: &PlaywrightBridge,
    action: &str,
    page_id: &str,
    params: &HashMap<String, String>,
) -> Result<HashMap<String, String>, ExecutorError> {
    let timeout = params.get("timeout").and_then(|t| t.parse().ok());

    match action {
        "selector" => {
            let selector = params
                .get("selector")
                .ok_or_else(|| ExecutorError::MissingParameter("selector".to_string()))?;
            bridge.wait_for_selector(page_id, selector, timeout).await?;
        }
        "navigation" => {
            bridge.wait_for_navigation(page_id, timeout).await?;
        }
        "url" => {
            let pattern = params
                .get("pattern")
                .ok_or_else(|| ExecutorError::MissingParameter("pattern".to_string()))?;
            bridge.wait_for_url(page_id, pattern, timeout).await?;
        }
        "timeout" => {
            let ms = params
                .get("ms")
                .ok_or_else(|| ExecutorError::MissingParameter("ms".to_string()))?
                .parse::<u64>()
                .map_err(|_| ExecutorError::MissingParameter("ms must be a number".to_string()))?;
            tokio::time::sleep(tokio::time::Duration::from_millis(ms)).await;
        }
        _ => return Err(ExecutorError::UnknownAction(format!("wait/{}", action))),
    }

    Ok(HashMap::new())
}

pub async fn execute_browser_action(
    bridge: &PlaywrightBridge,
    action: &str,
    page_id: &str,
    params: &HashMap<String, String>,
) -> Result<HashMap<String, String>, ExecutorError> {
    match action {
        "screenshot" => {
            let path = params
                .get("path")
                .ok_or_else(|| ExecutorError::MissingParameter("path".to_string()))?;
            let full_page = params.get("fullPage").map(|v| v == "true").unwrap_or(false);
            bridge.screenshot(page_id, path, full_page).await?;
        }
        "pdf" => {
            let path = params
                .get("path")
                .ok_or_else(|| ExecutorError::MissingParameter("path".to_string()))?;
            bridge.pdf(page_id, path).await?;
        }
        _ => return Err(ExecutorError::UnknownAction(format!("browser/{}", action))),
    }

    Ok(HashMap::new())
}

pub async fn execute_network_action(
    action: &str,
    _page_id: &str,
    _params: &HashMap<String, String>,
) -> Result<HashMap<String, String>, ExecutorError> {
    match action {
        "intercept" | "mock" | "wait_for_response" => {
            tracing::warn!("Network action '{}' not yet implemented", action);
        }
        _ => return Err(ExecutorError::UnknownAction(format!("network/{}", action))),
    }

    Ok(HashMap::new())
}
