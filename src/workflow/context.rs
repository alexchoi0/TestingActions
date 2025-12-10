//! Execution context for workflow runtime
//!
//! This module contains the runtime context used during workflow execution
//! for managing environment variables, secrets, and step outputs.

use std::collections::HashMap;

/// Runtime context for expression evaluation
#[derive(Debug, Clone, Default)]
pub struct ExecutionContext {
    /// Environment variables
    pub env: HashMap<String, String>,

    /// Secrets (not logged)
    pub secrets: HashMap<String, String>,

    /// Step outputs (step_id -> output_name -> value)
    pub steps: HashMap<String, HashMap<String, String>>,

    /// Job outputs
    pub jobs: HashMap<String, HashMap<String, String>>,

    /// Current job name
    pub current_job: Option<String>,

    /// Current step ID
    pub current_step: Option<String>,

    /// Run ID
    pub run_id: String,
}

impl ExecutionContext {
    /// Create a new execution context with a generated run ID
    pub fn new() -> Self {
        Self {
            run_id: uuid::Uuid::new_v4().to_string(),
            ..Default::default()
        }
    }

    /// Set a step output
    pub fn set_output(&mut self, step_id: &str, key: &str, value: String) {
        self.steps
            .entry(step_id.to_string())
            .or_default()
            .insert(key.to_string(), value);
    }

    /// Get a step output
    pub fn get_output(&self, step_id: &str, key: &str) -> Option<&String> {
        self.steps.get(step_id)?.get(key)
    }

    /// Set an environment variable
    pub fn set_env(&mut self, key: &str, value: String) {
        self.env.insert(key.to_string(), value);
    }

    /// Get an environment variable
    pub fn get_env(&self, key: &str) -> Option<&String> {
        self.env.get(key)
    }

    /// Set a secret
    pub fn set_secret(&mut self, key: &str, value: String) {
        self.secrets.insert(key.to_string(), value);
    }

    /// Get a secret
    pub fn get_secret(&self, key: &str) -> Option<&String> {
        self.secrets.get(key)
    }

    /// Set a job output
    pub fn set_job_output(&mut self, job_id: &str, key: &str, value: String) {
        self.jobs
            .entry(job_id.to_string())
            .or_default()
            .insert(key.to_string(), value);
    }

    /// Get a job output
    pub fn get_job_output(&self, job_id: &str, key: &str) -> Option<&String> {
        self.jobs.get(job_id)?.get(key)
    }

    /// Get all outputs for a step
    pub fn get_step_outputs(&self, step_id: &str) -> Option<&HashMap<String, String>> {
        self.steps.get(step_id)
    }

    /// Get all outputs for a job
    pub fn get_job_outputs(&self, job_id: &str) -> Option<&HashMap<String, String>> {
        self.jobs.get(job_id)
    }

    /// Merge environment variables from another source
    pub fn merge_env(&mut self, env: &HashMap<String, String>) {
        for (key, value) in env {
            self.env.insert(key.clone(), value.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_context() {
        let ctx = ExecutionContext::new();
        assert!(!ctx.run_id.is_empty());
    }

    #[test]
    fn test_step_outputs() {
        let mut ctx = ExecutionContext::new();
        ctx.set_output("step1", "result", "success".to_string());

        assert_eq!(
            ctx.get_output("step1", "result"),
            Some(&"success".to_string())
        );
        assert_eq!(ctx.get_output("step1", "missing"), None);
        assert_eq!(ctx.get_output("missing", "result"), None);
    }

    #[test]
    fn test_env_vars() {
        let mut ctx = ExecutionContext::new();
        ctx.set_env("MY_VAR", "my_value".to_string());

        assert_eq!(ctx.get_env("MY_VAR"), Some(&"my_value".to_string()));
        assert_eq!(ctx.get_env("MISSING"), None);
    }

    #[test]
    fn test_secrets() {
        let mut ctx = ExecutionContext::new();
        ctx.set_secret("API_KEY", "secret123".to_string());

        assert_eq!(ctx.get_secret("API_KEY"), Some(&"secret123".to_string()));
        assert_eq!(ctx.get_secret("MISSING"), None);
    }

    #[test]
    fn test_job_outputs() {
        let mut ctx = ExecutionContext::new();
        ctx.set_job_output("build", "artifact", "dist.zip".to_string());

        assert_eq!(
            ctx.get_job_output("build", "artifact"),
            Some(&"dist.zip".to_string())
        );
    }

    #[test]
    fn test_merge_env() {
        let mut ctx = ExecutionContext::new();
        ctx.set_env("EXISTING", "value1".to_string());

        let mut new_env = HashMap::new();
        new_env.insert("NEW_VAR".to_string(), "value2".to_string());
        new_env.insert("EXISTING".to_string(), "overwritten".to_string());

        ctx.merge_env(&new_env);

        assert_eq!(ctx.get_env("NEW_VAR"), Some(&"value2".to_string()));
        assert_eq!(ctx.get_env("EXISTING"), Some(&"overwritten".to_string()));
    }
}
