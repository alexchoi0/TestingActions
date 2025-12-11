//! Web/HTTP Bridge - Platform-agnostic HTTP requests via the runner
//!
//! This bridge executes HTTP requests directly using reqwest, without
//! needing a separate process. It supports:
//! - GET, POST, PUT, PATCH, DELETE methods
//! - Authentication (Bearer, Basic, API Key, OAuth2)
//! - Retry with exponential backoff
//! - Custom headers
//! - Response parsing

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, info, warn};

use super::{ApiResponse, BridgeError};
use crate::workflow::{WebAuthConfig, WebConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebRequest {
    pub method: String,
    pub path: String,
    pub headers: Option<HashMap<String, String>>,
    pub body: Option<Value>,
    pub query: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Value,
    pub elapsed_ms: u64,
}

#[derive(Debug)]
pub struct WebBridge {
    config: WebConfig,
    client: reqwest::Client,
}

impl WebBridge {
    pub fn new(config: WebConfig) -> Result<Self, BridgeError> {
        let mut client_builder = reqwest::Client::builder()
            .timeout(Duration::from_millis(config.timeout))
            .redirect(if config.follow_redirects {
                reqwest::redirect::Policy::default()
            } else {
                reqwest::redirect::Policy::none()
            });

        if !config.validate_ssl {
            client_builder = client_builder.danger_accept_invalid_certs(true);
        }

        let client = client_builder
            .build()
            .map_err(|e| BridgeError::StartupFailed(e.to_string()))?;

        Ok(Self { config, client })
    }

    pub fn from_config(config: &WebConfig) -> Result<Self, BridgeError> {
        Self::new(config.clone())
    }

    fn build_url(&self, path: &str) -> String {
        let base = self.config.base_url.trim_end_matches('/');
        let path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{}", path)
        };
        format!("{}{}", base, path)
    }

    fn apply_auth(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.config.auth {
            Some(WebAuthConfig::Bearer { token }) => {
                request.header("Authorization", format!("Bearer {}", token))
            }
            Some(WebAuthConfig::Basic { username, password }) => {
                request.basic_auth(username, Some(password))
            }
            Some(WebAuthConfig::ApiKey { header, key }) => request.header(header, key),
            Some(WebAuthConfig::OAuth2 { .. }) => request,
            None => request,
        }
    }

    fn apply_headers(
        &self,
        mut request: reqwest::RequestBuilder,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> reqwest::RequestBuilder {
        for (key, value) in &self.config.headers {
            request = request.header(key, value);
        }

        if let Some(headers) = extra_headers {
            for (key, value) in headers {
                request = request.header(key, value);
            }
        }

        request
    }

    fn should_retry(&self, status: u16, attempt: u32) -> bool {
        if let Some(ref retry) = self.config.retry {
            if attempt < retry.max_attempts {
                return retry.retry_on_status.contains(&status);
            }
        }
        false
    }

    fn get_retry_delay(&self, attempt: u32) -> Duration {
        if let Some(ref retry) = self.config.retry {
            let delay = retry.initial_delay * 2u64.pow(attempt.saturating_sub(1));
            let delay = delay.min(retry.max_delay);
            Duration::from_millis(delay)
        } else {
            Duration::from_millis(1000)
        }
    }

    async fn execute_with_retry(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        headers: Option<&HashMap<String, String>>,
        query: Option<&HashMap<String, String>>,
    ) -> Result<WebResponse, BridgeError> {
        let url = self.build_url(path);
        let max_attempts = self
            .config
            .retry
            .as_ref()
            .map(|r| r.max_attempts)
            .unwrap_or(1);

        let mut last_error = None;

        for attempt in 0..max_attempts {
            if attempt > 0 {
                let delay = self.get_retry_delay(attempt);
                warn!(
                    "Retrying request (attempt {}/{}) after {:?}",
                    attempt + 1,
                    max_attempts,
                    delay
                );
                tokio::time::sleep(delay).await;
            }

            let start = std::time::Instant::now();

            let mut request = match method.to_uppercase().as_str() {
                "GET" => self.client.get(&url),
                "POST" => self.client.post(&url),
                "PUT" => self.client.put(&url),
                "PATCH" => self.client.patch(&url),
                "DELETE" => self.client.delete(&url),
                "HEAD" => self.client.head(&url),
                _ => {
                    return Err(BridgeError::UnsupportedAction(format!(
                        "Unknown HTTP method: {}",
                        method
                    )))
                }
            };

            if let Some(query_params) = query {
                request = request.query(query_params);
            }

            request = self.apply_auth(request);
            request = self.apply_headers(request, headers);

            if let Some(ref body_value) = body {
                request = request.json(body_value);
            }

            debug!("Executing {} {}", method, url);

            match request.send().await {
                Ok(response) => {
                    let status = response.status().as_u16();
                    let elapsed_ms = start.elapsed().as_millis() as u64;

                    let response_headers: HashMap<String, String> = response
                        .headers()
                        .iter()
                        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                        .collect();

                    if self.should_retry(status, attempt) {
                        last_error = Some(BridgeError::HttpError {
                            status,
                            message: format!("Retryable status code: {}", status),
                        });
                        continue;
                    }

                    let body_text = response
                        .text()
                        .await
                        .map_err(|e| BridgeError::ServerError(e.to_string()))?;

                    let body: Value = if body_text.is_empty() {
                        Value::Null
                    } else {
                        serde_json::from_str(&body_text).unwrap_or(Value::String(body_text))
                    };

                    info!("{} {} -> {} ({}ms)", method, url, status, elapsed_ms);

                    return Ok(WebResponse {
                        status,
                        headers: response_headers,
                        body,
                        elapsed_ms,
                    });
                }
                Err(e) => {
                    warn!("Request failed: {}", e);
                    last_error = Some(BridgeError::ServerError(e.to_string()));

                    if e.is_connect() || e.is_timeout() {
                        continue;
                    }

                    return Err(BridgeError::ServerError(e.to_string()));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| BridgeError::ServerError("Request failed".to_string())))
    }

    pub async fn get(
        &self,
        path: &str,
        headers: Option<HashMap<String, String>>,
        query: Option<HashMap<String, String>>,
    ) -> Result<WebResponse, BridgeError> {
        self.execute_with_retry("GET", path, None, headers.as_ref(), query.as_ref())
            .await
    }

    pub async fn post(
        &self,
        path: &str,
        body: Option<Value>,
        headers: Option<HashMap<String, String>>,
    ) -> Result<WebResponse, BridgeError> {
        self.execute_with_retry("POST", path, body, headers.as_ref(), None)
            .await
    }

    pub async fn put(
        &self,
        path: &str,
        body: Option<Value>,
        headers: Option<HashMap<String, String>>,
    ) -> Result<WebResponse, BridgeError> {
        self.execute_with_retry("PUT", path, body, headers.as_ref(), None)
            .await
    }

    pub async fn patch(
        &self,
        path: &str,
        body: Option<Value>,
        headers: Option<HashMap<String, String>>,
    ) -> Result<WebResponse, BridgeError> {
        self.execute_with_retry("PATCH", path, body, headers.as_ref(), None)
            .await
    }

    pub async fn delete(
        &self,
        path: &str,
        headers: Option<HashMap<String, String>>,
    ) -> Result<WebResponse, BridgeError> {
        self.execute_with_retry("DELETE", path, None, headers.as_ref(), None)
            .await
    }

    pub async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        headers: Option<HashMap<String, String>>,
        query: Option<HashMap<String, String>>,
    ) -> Result<WebResponse, BridgeError> {
        self.execute_with_retry(method, path, body, headers.as_ref(), query.as_ref())
            .await
    }
}

#[async_trait]
impl super::Bridge for WebBridge {
    fn platform(&self) -> crate::workflow::Platform {
        crate::workflow::Platform::Web
    }

    async fn call(&self, method: &str, args: Value) -> Result<Value, BridgeError> {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("/");
        let body = args.get("body").cloned();
        let headers: Option<HashMap<String, String>> = args
            .get("headers")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let query: Option<HashMap<String, String>> = args
            .get("query")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        let response = self.request(method, path, body, headers, query).await?;
        Ok(serde_json::to_value(&response).unwrap_or(Value::Null))
    }

    fn as_web(&self) -> Option<&WebBridge> {
        Some(self)
    }
}

impl WebResponse {
    pub fn to_api_response(&self) -> ApiResponse {
        ApiResponse {
            status: self.status,
            headers: self.headers.clone(),
            body: self.body.clone(),
        }
    }

    pub fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }

    pub fn is_client_error(&self) -> bool {
        self.status >= 400 && self.status < 500
    }

    pub fn is_server_error(&self) -> bool {
        self.status >= 500
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::WebRetryConfig;

    fn make_test_config() -> WebConfig {
        WebConfig {
            base_url: "https://api.example.com".to_string(),
            headers: HashMap::new(),
            timeout: 30000,
            auth: None,
            retry: None,
            follow_redirects: true,
            validate_ssl: true,
        }
    }

    #[test]
    fn test_build_url() {
        let config = make_test_config();
        let bridge = WebBridge::new(config).unwrap();

        assert_eq!(bridge.build_url("/users"), "https://api.example.com/users");
        assert_eq!(bridge.build_url("users"), "https://api.example.com/users");
    }

    #[test]
    fn test_build_url_with_trailing_slash() {
        let mut config = make_test_config();
        config.base_url = "https://api.example.com/".to_string();
        let bridge = WebBridge::new(config).unwrap();

        assert_eq!(bridge.build_url("/users"), "https://api.example.com/users");
    }

    #[test]
    fn test_web_response_status_checks() {
        let response = WebResponse {
            status: 200,
            headers: HashMap::new(),
            body: Value::Null,
            elapsed_ms: 100,
        };
        assert!(response.is_success());
        assert!(!response.is_client_error());
        assert!(!response.is_server_error());

        let response = WebResponse {
            status: 404,
            headers: HashMap::new(),
            body: Value::Null,
            elapsed_ms: 100,
        };
        assert!(!response.is_success());
        assert!(response.is_client_error());

        let response = WebResponse {
            status: 500,
            headers: HashMap::new(),
            body: Value::Null,
            elapsed_ms: 100,
        };
        assert!(!response.is_success());
        assert!(response.is_server_error());
    }

    #[test]
    fn test_config_with_auth() {
        let mut config = make_test_config();
        config.auth = Some(WebAuthConfig::Bearer {
            token: "test-token".to_string(),
        });
        let bridge = WebBridge::new(config).unwrap();
        assert!(bridge.config.auth.is_some());
    }

    #[test]
    fn test_config_with_retry() {
        let mut config = make_test_config();
        config.retry = Some(WebRetryConfig {
            max_attempts: 3,
            initial_delay: 1000,
            max_delay: 10000,
            retry_on_status: vec![429, 500, 502, 503, 504],
        });
        let bridge = WebBridge::new(config).unwrap();

        assert!(bridge.should_retry(429, 0));
        assert!(bridge.should_retry(500, 1));
        assert!(!bridge.should_retry(404, 0));
        assert!(!bridge.should_retry(429, 3));
    }

    #[test]
    fn test_retry_delay_exponential() {
        let mut config = make_test_config();
        config.retry = Some(WebRetryConfig {
            max_attempts: 5,
            initial_delay: 1000,
            max_delay: 10000,
            retry_on_status: vec![500],
        });
        let bridge = WebBridge::new(config).unwrap();

        assert_eq!(bridge.get_retry_delay(0), Duration::from_millis(1000));
        assert_eq!(bridge.get_retry_delay(1), Duration::from_millis(1000));
        assert_eq!(bridge.get_retry_delay(2), Duration::from_millis(2000));
        assert_eq!(bridge.get_retry_delay(3), Duration::from_millis(4000));
        assert_eq!(bridge.get_retry_delay(4), Duration::from_millis(8000));
        assert_eq!(bridge.get_retry_delay(5), Duration::from_millis(10000));
    }
}
