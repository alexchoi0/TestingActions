//! Execution result types

use std::collections::HashMap;
use crate::bridge::ApiResponse;

/// Result of a step execution
#[derive(Debug, Clone)]
pub struct StepResult {
    pub success: bool,
    pub outputs: HashMap<String, String>,
    pub error: Option<String>,
    pub response: Option<ApiResponse>,
}

impl Default for StepResult {
    fn default() -> Self {
        Self {
            success: true,
            outputs: HashMap::new(),
            error: None,
            response: None,
        }
    }
}

/// Result of a job execution
#[derive(Debug, Clone)]
pub struct JobResult {
    pub success: bool,
    pub outputs: HashMap<String, String>,
    pub steps: Vec<StepResult>,
}

/// Result of a workflow execution
#[derive(Debug, Clone)]
pub struct WorkflowResult {
    pub success: bool,
    pub jobs: HashMap<String, JobResult>,
    pub run_id: String,
}
