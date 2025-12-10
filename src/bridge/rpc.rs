//! Shared JSON-RPC types for bridge communication
//!
//! This module contains common types used by all bridges that communicate
//! via JSON-RPC over stdin/stdout with external processes.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::ChildStdin;
use tokio::process::ChildStdout;
use tokio::sync::{mpsc, oneshot};

use super::BridgeError;

/// JSON-RPC request
#[derive(Debug, Serialize)]
pub struct RpcRequest {
    pub jsonrpc: &'static str,
    pub id: u64,
    pub method: String,
    pub params: Value,
}

/// JSON-RPC response
#[derive(Debug, Deserialize)]
pub struct RpcResponse {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: u64,
    pub result: Option<Value>,
    pub error: Option<RpcError>,
}

/// JSON-RPC error
#[derive(Debug, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

/// Information about a registered function (used by Python, Go, Rust bridges)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionInfo {
    pub name: String,
    pub description: String,
}

impl FunctionInfo {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
        }
    }
}

/// Information about a registered method (used by Java bridge)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodInfo {
    pub name: String,
    pub description: String,
}

impl MethodInfo {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
        }
    }
}

/// Request sender type alias
pub type RequestSender = mpsc::Sender<(RpcRequest, oneshot::Sender<Result<Value, BridgeError>>)>;

/// Global request ID counter
static REQUEST_ID: AtomicU64 = AtomicU64::new(1);

/// Create a new RPC request with auto-incremented ID
pub fn new_request(method: &str, params: Value) -> RpcRequest {
    RpcRequest {
        jsonrpc: "2.0",
        id: REQUEST_ID.fetch_add(1, Ordering::SeqCst),
        method: method.to_string(),
        params,
    }
}

/// Send an RPC request and wait for response
pub async fn send_request(
    request_tx: &RequestSender,
    method: &str,
    params: Value,
) -> Result<Value, BridgeError> {
    let req = new_request(method, params);
    let (tx, rx) = oneshot::channel();

    request_tx
        .send((req, tx))
        .await
        .map_err(|_| BridgeError::Disconnected)?;

    rx.await.map_err(|_| BridgeError::Disconnected)?
}

/// Spawn the background communication task for JSON-RPC over stdin/stdout
pub fn spawn_communication_task(
    mut request_rx: mpsc::Receiver<(RpcRequest, oneshot::Sender<Result<Value, BridgeError>>)>,
    stdin: ChildStdin,
    stdout: ChildStdout,
) {
    tokio::spawn(async move {
        let mut stdin = stdin;
        let mut reader = BufReader::new(stdout);
        let mut pending: HashMap<u64, oneshot::Sender<Result<Value, BridgeError>>> = HashMap::new();
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
                                        None => Ok(response.result.unwrap_or(Value::Null)),
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
}
