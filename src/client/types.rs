use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventType {
    RunStarted,
    RunCompleted,
    WorkflowStarted,
    WorkflowCompleted,
    WorkflowSkipped,
    JobStarted,
    JobCompleted,
    StepStarted,
    StepCompleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunEvent {
    pub event_type: EventType,
    pub run_id: String,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl RunEvent {
    pub fn run_started(run_id: &str) -> Self {
        Self {
            event_type: EventType::RunStarted,
            run_id: run_id.to_string(),
            timestamp: Utc::now(),
            workflow_name: None,
            job_name: None,
            step_index: None,
            step_name: None,
            success: None,
            error: None,
            reason: None,
        }
    }

    pub fn run_completed(run_id: &str, success: bool) -> Self {
        Self {
            event_type: EventType::RunCompleted,
            run_id: run_id.to_string(),
            timestamp: Utc::now(),
            workflow_name: None,
            job_name: None,
            step_index: None,
            step_name: None,
            success: Some(success),
            error: None,
            reason: None,
        }
    }

    pub fn workflow_started(run_id: &str, workflow_name: &str) -> Self {
        Self {
            event_type: EventType::WorkflowStarted,
            run_id: run_id.to_string(),
            timestamp: Utc::now(),
            workflow_name: Some(workflow_name.to_string()),
            job_name: None,
            step_index: None,
            step_name: None,
            success: None,
            error: None,
            reason: None,
        }
    }

    pub fn workflow_completed(run_id: &str, workflow_name: &str, success: bool, error: Option<String>) -> Self {
        Self {
            event_type: EventType::WorkflowCompleted,
            run_id: run_id.to_string(),
            timestamp: Utc::now(),
            workflow_name: Some(workflow_name.to_string()),
            job_name: None,
            step_index: None,
            step_name: None,
            success: Some(success),
            error,
            reason: None,
        }
    }

    pub fn workflow_skipped(run_id: &str, workflow_name: &str, reason: &str) -> Self {
        Self {
            event_type: EventType::WorkflowSkipped,
            run_id: run_id.to_string(),
            timestamp: Utc::now(),
            workflow_name: Some(workflow_name.to_string()),
            job_name: None,
            step_index: None,
            step_name: None,
            success: None,
            error: None,
            reason: Some(reason.to_string()),
        }
    }

    pub fn job_started(run_id: &str, workflow_name: &str, job_name: &str) -> Self {
        Self {
            event_type: EventType::JobStarted,
            run_id: run_id.to_string(),
            timestamp: Utc::now(),
            workflow_name: Some(workflow_name.to_string()),
            job_name: Some(job_name.to_string()),
            step_index: None,
            step_name: None,
            success: None,
            error: None,
            reason: None,
        }
    }

    pub fn job_completed(run_id: &str, workflow_name: &str, job_name: &str, success: bool) -> Self {
        Self {
            event_type: EventType::JobCompleted,
            run_id: run_id.to_string(),
            timestamp: Utc::now(),
            workflow_name: Some(workflow_name.to_string()),
            job_name: Some(job_name.to_string()),
            step_index: None,
            step_name: None,
            success: Some(success),
            error: None,
            reason: None,
        }
    }

    pub fn step_started(run_id: &str, workflow_name: &str, job_name: &str, step_index: usize, step_name: &str) -> Self {
        Self {
            event_type: EventType::StepStarted,
            run_id: run_id.to_string(),
            timestamp: Utc::now(),
            workflow_name: Some(workflow_name.to_string()),
            job_name: Some(job_name.to_string()),
            step_index: Some(step_index),
            step_name: Some(step_name.to_string()),
            success: None,
            error: None,
            reason: None,
        }
    }

    pub fn step_completed(
        run_id: &str,
        workflow_name: &str,
        job_name: &str,
        step_index: usize,
        step_name: &str,
        success: bool,
        error: Option<String>,
    ) -> Self {
        Self {
            event_type: EventType::StepCompleted,
            run_id: run_id.to_string(),
            timestamp: Utc::now(),
            workflow_name: Some(workflow_name.to_string()),
            job_name: Some(job_name.to_string()),
            step_index: Some(step_index),
            step_name: Some(step_name.to_string()),
            success: Some(success),
            error,
            reason: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: String,
    pub status: String,
    pub workflows_dir: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub event_count: i32,
    pub is_paused: bool,
    pub paused_at: Option<String>,
    pub current_workflow: Option<String>,
    pub current_job: Option<String>,
    pub current_step: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunEventResponse {
    pub event_type: String,
    pub run_id: String,
    pub timestamp: String,
    pub workflow_name: Option<String>,
    pub job_name: Option<String>,
    pub step_index: Option<i32>,
    pub step_name: Option<String>,
    pub success: Option<bool>,
    pub error: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GraphQLResponse<T> {
    pub data: Option<T>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize)]
pub struct GraphQLError {
    pub message: String,
}
