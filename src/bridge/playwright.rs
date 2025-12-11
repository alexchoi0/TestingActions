//! Playwright Bridge - Communication with Playwright via JSON-RPC

use std::collections::HashMap;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};

use super::rpc::{new_request, RpcRequest, RpcResponse};
use super::BridgeError;
use crate::workflow::BrowserType;

pub struct PlaywrightBridge {
    request_tx: mpsc::Sender<(
        RpcRequest,
        oneshot::Sender<Result<serde_json::Value, BridgeError>>,
    )>,
    #[allow(dead_code)]
    child: Child,
}

impl PlaywrightBridge {
    pub async fn start() -> Result<Self, BridgeError> {
        let mut child = Command::new("node")
            .arg("extensions/playwright/server.js")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| BridgeError::StartupFailed(e.to_string()))?;

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let (request_tx, mut request_rx) = mpsc::channel::<(
            RpcRequest,
            oneshot::Sender<Result<serde_json::Value, BridgeError>>,
        )>(100);

        tokio::spawn(async move {
            let mut stdin = stdin;
            let mut reader = BufReader::new(stdout);
            let mut pending: HashMap<u64, oneshot::Sender<Result<serde_json::Value, BridgeError>>> =
                HashMap::new();
            let mut line = String::new();

            loop {
                tokio::select! {
                    request = request_rx.recv() => {
                        match request {
                            Some((req, response_tx)) => {
                                let id = req.id;
                                let json = serde_json::to_string(&req).unwrap() + "\n";
                                if stdin.write_all(json.as_bytes()).await.is_err() {
                                    let _ = response_tx.send(Err(BridgeError::Disconnected));
                                    break;
                                }
                                pending.insert(id, response_tx);
                            }
                            None => break,
                        }
                    }

                    result = reader.read_line(&mut line) => {
                        match result {
                            Ok(0) => break,
                            Ok(_) => {
                                if let Ok(response) = serde_json::from_str::<RpcResponse>(&line) {
                                    if let Some(tx) = pending.remove(&response.id) {
                                        let result = match response.error {
                                            Some(err) => Err(BridgeError::ServerError(
                                                format!("[{}] {}", err.code, err.message)
                                            )),
                                            None => Ok(response.result.unwrap_or(serde_json::Value::Null)),
                                        };
                                        let _ = tx.send(result);
                                    }
                                }
                                line.clear();
                            }
                            Err(_) => break,
                        }
                    }
                }
            }
        });

        Ok(Self { request_tx, child })
    }

    async fn request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, BridgeError> {
        let req = new_request(method, params);
        let (tx, rx) = oneshot::channel();
        self.request_tx
            .send((req, tx))
            .await
            .map_err(|_| BridgeError::Disconnected)?;
        rx.await.map_err(|_| BridgeError::Disconnected)?
    }

    // Browser Actions
    pub async fn browser_launch(
        &self,
        browser_type: BrowserType,
        headless: bool,
    ) -> Result<String, BridgeError> {
        let result = self
            .request(
                "browser.launch",
                serde_json::json!({
                    "browserType": format!("{:?}", browser_type).to_lowercase(),
                    "headless": headless,
                }),
            )
            .await?;
        result["browserId"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| BridgeError::ServerError("No browser ID returned".to_string()))
    }

    pub async fn page_new(&self, browser_id: &str) -> Result<String, BridgeError> {
        let result = self
            .request("page.new", serde_json::json!({ "browserId": browser_id }))
            .await?;
        result["pageId"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| BridgeError::ServerError("No page ID returned".to_string()))
    }

    pub async fn browser_close(&self, browser_id: &str) -> Result<(), BridgeError> {
        self.request(
            "browser.close",
            serde_json::json!({ "browserId": browser_id }),
        )
        .await?;
        Ok(())
    }

    // Page Actions
    pub async fn page_goto(&self, page_id: &str, url: &str) -> Result<(), BridgeError> {
        self.request(
            "page.goto",
            serde_json::json!({ "pageId": page_id, "url": url }),
        )
        .await?;
        Ok(())
    }

    pub async fn page_reload(&self, page_id: &str) -> Result<(), BridgeError> {
        self.request("page.reload", serde_json::json!({ "pageId": page_id }))
            .await?;
        Ok(())
    }

    pub async fn page_go_back(&self, page_id: &str) -> Result<(), BridgeError> {
        self.request("page.goBack", serde_json::json!({ "pageId": page_id }))
            .await?;
        Ok(())
    }

    pub async fn page_go_forward(&self, page_id: &str) -> Result<(), BridgeError> {
        self.request("page.goForward", serde_json::json!({ "pageId": page_id }))
            .await?;
        Ok(())
    }

    pub async fn page_url(&self, page_id: &str) -> Result<String, BridgeError> {
        let result = self
            .request("page.url", serde_json::json!({ "pageId": page_id }))
            .await?;
        result["url"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| BridgeError::ServerError("No URL returned".to_string()))
    }

    pub async fn page_title(&self, page_id: &str) -> Result<String, BridgeError> {
        let result = self
            .request("page.title", serde_json::json!({ "pageId": page_id }))
            .await?;
        result["title"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| BridgeError::ServerError("No title returned".to_string()))
    }

    // Element Actions
    pub async fn element_click(&self, page_id: &str, selector: &str) -> Result<(), BridgeError> {
        self.request(
            "element.click",
            serde_json::json!({ "pageId": page_id, "selector": selector }),
        )
        .await?;
        Ok(())
    }

    pub async fn element_fill(
        &self,
        page_id: &str,
        selector: &str,
        value: &str,
    ) -> Result<(), BridgeError> {
        self.request(
            "element.fill",
            serde_json::json!({ "pageId": page_id, "selector": selector, "value": value }),
        )
        .await?;
        Ok(())
    }

    pub async fn element_type(
        &self,
        page_id: &str,
        selector: &str,
        text: &str,
        delay: Option<u64>,
    ) -> Result<(), BridgeError> {
        self.request("element.type", serde_json::json!({ "pageId": page_id, "selector": selector, "text": text, "delay": delay })).await?;
        Ok(())
    }

    pub async fn element_select(
        &self,
        page_id: &str,
        selector: &str,
        value: &str,
    ) -> Result<(), BridgeError> {
        self.request(
            "element.select",
            serde_json::json!({ "pageId": page_id, "selector": selector, "value": value }),
        )
        .await?;
        Ok(())
    }

    pub async fn element_hover(&self, page_id: &str, selector: &str) -> Result<(), BridgeError> {
        self.request(
            "element.hover",
            serde_json::json!({ "pageId": page_id, "selector": selector }),
        )
        .await?;
        Ok(())
    }

    pub async fn element_text(&self, page_id: &str, selector: &str) -> Result<String, BridgeError> {
        let result = self
            .request(
                "element.textContent",
                serde_json::json!({ "pageId": page_id, "selector": selector }),
            )
            .await?;
        result["text"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| BridgeError::ServerError("No text returned".to_string()))
    }

    pub async fn element_attribute(
        &self,
        page_id: &str,
        selector: &str,
        attribute: &str,
    ) -> Result<Option<String>, BridgeError> {
        let result = self.request("element.getAttribute", serde_json::json!({ "pageId": page_id, "selector": selector, "attribute": attribute })).await?;
        Ok(result["value"].as_str().map(|s| s.to_string()))
    }

    pub async fn element_is_visible(
        &self,
        page_id: &str,
        selector: &str,
    ) -> Result<bool, BridgeError> {
        let result = self
            .request(
                "element.isVisible",
                serde_json::json!({ "pageId": page_id, "selector": selector }),
            )
            .await?;
        Ok(result["visible"].as_bool().unwrap_or(false))
    }

    // Wait Actions
    pub async fn wait_for_selector(
        &self,
        page_id: &str,
        selector: &str,
        timeout: Option<u64>,
    ) -> Result<(), BridgeError> {
        self.request(
            "wait.selector",
            serde_json::json!({ "pageId": page_id, "selector": selector, "timeout": timeout }),
        )
        .await?;
        Ok(())
    }

    pub async fn wait_for_navigation(
        &self,
        page_id: &str,
        timeout: Option<u64>,
    ) -> Result<(), BridgeError> {
        self.request(
            "wait.navigation",
            serde_json::json!({ "pageId": page_id, "timeout": timeout }),
        )
        .await?;
        Ok(())
    }

    pub async fn wait_for_url(
        &self,
        page_id: &str,
        pattern: &str,
        timeout: Option<u64>,
    ) -> Result<(), BridgeError> {
        self.request(
            "wait.url",
            serde_json::json!({ "pageId": page_id, "pattern": pattern, "timeout": timeout }),
        )
        .await?;
        Ok(())
    }

    // Screenshot/PDF
    pub async fn screenshot(
        &self,
        page_id: &str,
        path: &str,
        full_page: bool,
    ) -> Result<(), BridgeError> {
        self.request(
            "page.screenshot",
            serde_json::json!({ "pageId": page_id, "path": path, "fullPage": full_page }),
        )
        .await?;
        Ok(())
    }

    pub async fn pdf(&self, page_id: &str, path: &str) -> Result<(), BridgeError> {
        self.request(
            "page.pdf",
            serde_json::json!({ "pageId": page_id, "path": path }),
        )
        .await?;
        Ok(())
    }
}

impl Drop for PlaywrightBridge {
    fn drop(&mut self) {}
}

use async_trait::async_trait;

#[async_trait]
impl super::Bridge for PlaywrightBridge {
    fn platform(&self) -> crate::workflow::Platform {
        crate::workflow::Platform::Playwright
    }

    async fn call(
        &self,
        action: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, super::BridgeError> {
        self.request(action, args).await
    }

    fn as_playwright(&self) -> Option<&PlaywrightBridge> {
        Some(self)
    }
}
