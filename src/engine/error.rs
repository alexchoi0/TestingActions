//! Executor error types

use crate::bridge::BridgeError;
use crate::workflow::expressions::ExpressionError;

/// Errors that can occur during workflow execution
#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    #[error("Parse error: {0}")]
    ParseError(#[from] serde_yaml::Error),

    #[error("Expression error: {0}")]
    ExpressionError(#[from] ExpressionError),

    #[error("Bridge error: {0}")]
    BridgeError(#[from] BridgeError),

    #[error("Job not found: {0}")]
    JobNotFound(String),

    #[error("Circular dependency detected: {0:?}")]
    CircularDependency(Vec<String>),

    #[error("Unknown action: {0}")]
    UnknownAction(String),

    #[error("Missing required parameter: {0}")]
    MissingParameter(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Step failed: {0}")]
    StepFailed(String),

    #[error("Job failed: {0}")]
    JobFailed(String),

    #[error("Assertion failed: {0}")]
    AssertionFailed(String),

    #[error("Platform mismatch: {0}")]
    PlatformMismatch(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}
