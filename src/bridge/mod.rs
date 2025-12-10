//! Bridge modules for external service communication
//!
//! This module provides bridges to different execution platforms:
//! - `playwright`: Browser automation via Playwright
//! - `nodejs`: Direct Node.js function calls
//! - `rust`: Direct Rust function calls via separate process
//! - `python`: Direct Python function calls via separate process
//! - `java`: Direct Java function calls via separate process
//! - `go`: Direct Go function calls via separate process
//! - `web`: Platform-agnostic HTTP requests

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

pub mod rpc;
pub mod playwright;
pub mod nodejs;
pub mod rust;
pub mod python;
pub mod java;
pub mod go;
pub mod web;

pub use rpc::{FunctionInfo, MethodInfo};
pub use playwright::*;
pub use nodejs::*;
pub use rust::{RustBridge, RustBridgeOperations, RustOperations};
pub use python::{PythonBridge, PythonBridgeOperations, PythonOperations};
pub use java::{JavaBridge, JavaBridgeOperations, JavaOperations};
pub use go::{GoBridge, GoBridgeOperations, GoOperations};
pub use web::{WebBridge, WebBridgeOperations, WebOperations, WebResponse};

/// Common error type for bridge operations
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("Failed to start server: {0}")]
    StartupFailed(String),

    #[error("Server disconnected")]
    Disconnected,

    #[error("Request timed out")]
    Timeout,

    #[error("Server error: {0}")]
    ServerError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Unsupported action: {0}")]
    UnsupportedAction(String),

    #[error("HTTP error: {status} - {message}")]
    HttpError { status: u16, message: String },

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Assertion failed: {0}")]
    AssertionFailed(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Result of a custom assertion
#[derive(Debug, Clone)]
pub struct AssertionResult {
    pub success: bool,
    pub message: Option<String>,
    pub actual: Option<Value>,
    pub expected: Option<Value>,
}

/// Result of an action execution
#[derive(Debug, Clone, Default)]
pub struct ActionResult {
    pub outputs: HashMap<String, String>,
    pub response: Option<ApiResponse>,
}

/// API response from HTTP requests
#[derive(Debug, Clone)]
pub struct ApiResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Value,
}

/// Marker trait for Playwright-specific operations
pub trait PlaywrightOperations {
    fn supports_browser(&self) -> bool {
        true
    }
}

/// Trait for Node.js bridge operations (direct function calls)
#[async_trait]
pub trait NodejsBridgeOperations: Send + Sync {
    /// Call a registered function by name
    async fn fn_call(&self, name: &str, args: Value) -> Result<Value, BridgeError>;

    /// Get a value from the shared context
    async fn ctx_get(&self, key: &str) -> Result<Option<Value>, BridgeError>;

    /// Set a value in the shared context
    async fn ctx_set(&self, key: &str, value: Value) -> Result<(), BridgeError>;

    /// Clear context values matching a pattern
    async fn ctx_clear(&self, pattern: &str) -> Result<u64, BridgeError>;

    /// Set up a mock for a function
    async fn mock_set(&self, target: &str, mock_value: Value) -> Result<(), BridgeError>;

    /// Clear all mocks
    async fn mock_clear(&self) -> Result<(), BridgeError>;

    /// Call a lifecycle hook
    async fn hook_call(&self, hook_name: &str) -> Result<(), BridgeError>;

    /// Invoke a custom assertion
    async fn assert_custom(
        &self,
        assertion_name: &str,
        params: HashMap<String, Value>,
    ) -> Result<AssertionResult, BridgeError>;

    /// Update execution context info (run_id, job_name, step_name)
    async fn set_execution_info(
        &self,
        run_id: &str,
        job_name: &str,
        step_name: &str,
    ) -> Result<(), BridgeError>;

    /// Sync step outputs to the bridge context
    async fn sync_step_outputs(
        &self,
        step_id: &str,
        outputs: HashMap<String, String>,
    ) -> Result<(), BridgeError>;
}

/// Marker trait for Node.js-specific operations
pub trait NodejsOperations {
    fn supports_functions(&self) -> bool {
        true
    }
    fn supports_context(&self) -> bool {
        true
    }
    fn supports_mocking(&self) -> bool {
        true
    }
    fn supports_hooks(&self) -> bool {
        true
    }
}
