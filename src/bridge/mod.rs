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

use crate::engine::ClockSyncState;
use crate::workflow::{
    GoConfig, JavaConfig, NodejsConfig, Platform, PlaywrightConfig, PythonConfig, RustConfig,
    WebConfig,
};

pub mod go;
pub mod java;
pub mod nodejs;
pub mod playwright;
pub mod python;
pub mod rpc;
pub mod rust;
pub mod web;

pub use go::GoBridge;
pub use java::JavaBridge;
pub use nodejs::*;
pub use playwright::*;
pub use python::PythonBridge;
pub use rpc::{FunctionInfo, MethodInfo};
pub use rust::RustBridge;
pub use web::{WebBridge, WebResponse};

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

/// Unified Bridge trait for all platform bridges
///
/// This trait provides a common interface for all bridges, allowing them to be
/// stored in a single collection and operated on uniformly. Not all bridges
/// support all operations - unsupported operations return appropriate errors
/// or no-ops by default.
#[async_trait]
pub trait Bridge: Send + Sync {
    /// Returns the platform this bridge supports
    fn platform(&self) -> Platform;

    /// Call a function by name with JSON arguments
    async fn call(&self, name: &str, args: Value) -> Result<Value, BridgeError>;

    /// Get a value from the shared context
    async fn ctx_get(&self, _key: &str) -> Result<Option<Value>, BridgeError> {
        Err(BridgeError::UnsupportedAction("ctx_get".into()))
    }

    /// Set a value in the shared context
    async fn ctx_set(&self, _key: &str, _value: Value) -> Result<(), BridgeError> {
        Err(BridgeError::UnsupportedAction("ctx_set".into()))
    }

    /// Clear context values matching a pattern
    async fn ctx_clear(&self, _pattern: &str) -> Result<u64, BridgeError> {
        Err(BridgeError::UnsupportedAction("ctx_clear".into()))
    }

    /// Set up a mock for a function
    async fn mock_set(&self, _target: &str, _mock_value: Value) -> Result<(), BridgeError> {
        Err(BridgeError::UnsupportedAction("mock_set".into()))
    }

    /// Clear all mocks
    async fn mock_clear(&self) -> Result<(), BridgeError> {
        Err(BridgeError::UnsupportedAction("mock_clear".into()))
    }

    /// Call a lifecycle hook
    async fn hook_call(&self, _hook_name: &str) -> Result<(), BridgeError> {
        Err(BridgeError::UnsupportedAction("hook_call".into()))
    }

    /// Invoke a custom assertion
    async fn assert_custom(
        &self,
        _assertion_name: &str,
        _params: HashMap<String, Value>,
    ) -> Result<AssertionResult, BridgeError> {
        Err(BridgeError::UnsupportedAction("assert_custom".into()))
    }

    /// Update execution context info (run_id, job_name, step_name)
    async fn set_execution_info(
        &self,
        _run_id: &str,
        _job_name: &str,
        _step_name: &str,
    ) -> Result<(), BridgeError> {
        Ok(())
    }

    /// Sync step outputs to the bridge context
    async fn sync_step_outputs(
        &self,
        _step_id: &str,
        _outputs: HashMap<String, String>,
    ) -> Result<(), BridgeError> {
        Ok(())
    }

    /// Sync the mock clock state to this bridge
    async fn sync_clock(&self, _state: &ClockSyncState) -> Result<(), BridgeError> {
        Ok(())
    }

    /// Whether this bridge supports context operations
    fn supports_context(&self) -> bool {
        false
    }

    /// Whether this bridge supports hooks
    fn supports_hooks(&self) -> bool {
        false
    }

    /// Whether this bridge supports mocking
    fn supports_mocking(&self) -> bool {
        false
    }

    /// Whether this bridge supports clock synchronization
    fn supports_clock(&self) -> bool {
        false
    }

    /// Downcast to PlaywrightBridge if this is a Playwright bridge
    fn as_playwright(&self) -> Option<&PlaywrightBridge> {
        None
    }

    /// Downcast to NodejsBridge if this is a Node.js bridge
    fn as_nodejs(&self) -> Option<&NodejsBridge> {
        None
    }

    /// Downcast to RustBridge if this is a Rust bridge
    fn as_rust(&self) -> Option<&RustBridge> {
        None
    }

    /// Downcast to PythonBridge if this is a Python bridge
    fn as_python(&self) -> Option<&PythonBridge> {
        None
    }

    /// Downcast to JavaBridge if this is a Java bridge
    fn as_java(&self) -> Option<&JavaBridge> {
        None
    }

    /// Downcast to GoBridge if this is a Go bridge
    fn as_go(&self) -> Option<&GoBridge> {
        None
    }

    /// Downcast to WebBridge if this is a Web bridge
    fn as_web(&self) -> Option<&WebBridge> {
        None
    }
}

/// Configuration enum for all bridge types
#[derive(Debug, Clone)]
pub enum BridgeConfig {
    Playwright(PlaywrightConfig),
    Nodejs(NodejsConfig),
    Rust(RustConfig),
    Python(PythonConfig),
    Java(JavaConfig),
    Go(GoConfig),
    Web(WebConfig),
}

impl BridgeConfig {
    pub fn platform(&self) -> Platform {
        match self {
            BridgeConfig::Playwright(_) => Platform::Playwright,
            BridgeConfig::Nodejs(_) => Platform::Nodejs,
            BridgeConfig::Rust(_) => Platform::Rust,
            BridgeConfig::Python(_) => Platform::Python,
            BridgeConfig::Java(_) => Platform::Java,
            BridgeConfig::Go(_) => Platform::Go,
            BridgeConfig::Web(_) => Platform::Web,
        }
    }
}
