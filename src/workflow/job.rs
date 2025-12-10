//! Workflow, Job, and Step definitions
//!
//! This module contains the core workflow structure types that mirror GitHub Actions concepts.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::platform::{
    BrowserType, GoConfig, JavaConfig, NodejsConfig, Platform, PlatformsConfig,
    PlaywrightConfig, PythonConfig, RustConfig, Viewport, WebConfig,
};

// ============================================================================
// Workflow
// ============================================================================

/// A complete workflow definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    /// Workflow name (required)
    pub name: String,

    /// Workflows this workflow depends on
    #[serde(default, deserialize_with = "deserialize_depends_on")]
    pub depends_on: DependsOn,

    /// Default execution platform for all jobs/steps
    #[serde(default)]
    pub platform: Option<Platform>,

    /// Consolidated platform configurations
    #[serde(default)]
    pub platforms: PlatformsConfig,

    /// Environment variables available to all jobs
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Default settings for all jobs
    #[serde(default)]
    pub defaults: Option<JobDefaults>,

    /// Steps to run before any jobs in this workflow
    #[serde(default)]
    pub before: Vec<Step>,

    /// Steps to run after all jobs in this workflow complete
    #[serde(default)]
    pub after: Vec<Step>,

    /// Jobs to execute
    pub jobs: HashMap<String, Job>,
}

/// Dependency specification for a workflow
#[derive(Debug, Clone, Default, Serialize)]
pub struct DependsOn {
    /// List of workflow names this depends on
    pub workflows: Vec<String>,
    /// If true, run even if dependencies fail/skip
    pub always: bool,
}

impl<'de> Deserialize<'de> for DependsOn {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum DependsOnHelper {
            List(Vec<String>),
            Full { workflows: Vec<String>, #[serde(default)] always: bool },
        }

        match DependsOnHelper::deserialize(deserializer)? {
            DependsOnHelper::List(workflows) => Ok(DependsOn { workflows, always: false }),
            DependsOnHelper::Full { workflows, always } => Ok(DependsOn { workflows, always }),
        }
    }
}

fn deserialize_depends_on<'de, D>(deserializer: D) -> Result<DependsOn, D::Error>
where
    D: serde::Deserializer<'de>,
{
    DependsOn::deserialize(deserializer)
}

impl Workflow {
    /// Get Playwright configuration
    pub fn playwright(&self) -> Option<&PlaywrightConfig> {
        self.platforms.playwright.as_ref()
    }

    /// Get Node.js configuration
    pub fn nodejs(&self) -> Option<&NodejsConfig> {
        self.platforms.nodejs.as_ref()
    }

    /// Get Rust configuration
    pub fn rust(&self) -> Option<&RustConfig> {
        self.platforms.rust.as_ref()
    }

    /// Get Python configuration
    pub fn python(&self) -> Option<&PythonConfig> {
        self.platforms.python.as_ref()
    }

    /// Get Java configuration
    pub fn java(&self) -> Option<&JavaConfig> {
        self.platforms.java.as_ref()
    }

    /// Get Go configuration
    pub fn go(&self) -> Option<&GoConfig> {
        self.platforms.go.as_ref()
    }

    /// Get Web configuration
    pub fn web(&self) -> Option<&WebConfig> {
        self.platforms.web.as_ref()
    }
}

// ============================================================================
// Job
// ============================================================================

/// Default settings for jobs
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JobDefaults {
    /// Default browser
    pub browser: Option<BrowserType>,

    /// Default timeout in milliseconds
    pub timeout: Option<u64>,

    /// Default headless mode
    pub headless: Option<bool>,
}

/// A job contains multiple steps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    /// Human-readable name
    pub name: Option<String>,

    /// Override execution platform for this job
    pub platform: Option<Platform>,

    /// Browser to use (only for playwright platform)
    #[serde(default)]
    pub browser: BrowserType,

    /// Run headless (only for playwright platform)
    #[serde(default = "default_headless")]
    pub headless: bool,

    /// Viewport configuration (only for playwright platform)
    pub viewport: Option<Viewport>,

    /// Jobs this job depends on
    #[serde(default)]
    pub needs: Vec<String>,

    /// Condition to run this job
    #[serde(rename = "if")]
    pub condition: Option<String>,

    /// Job-level environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Steps to run before the main steps
    #[serde(default)]
    pub before: Vec<Step>,

    /// Steps to run after the main steps complete
    #[serde(default)]
    pub after: Vec<Step>,

    /// Steps to execute
    pub steps: Vec<Step>,

    /// Continue even if a step fails
    #[serde(default)]
    pub continue_on_error: bool,

    /// Timeout for the entire job in milliseconds
    pub timeout: Option<u64>,
}

fn default_headless() -> bool {
    true
}

// ============================================================================
// Step
// ============================================================================

/// A single step in a job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    /// Step name (for logging)
    pub name: Option<String>,

    /// Platform for this specific step (overrides job/workflow default)
    pub platform: Option<Platform>,

    /// Action to use (e.g., "page/goto", "element/click")
    pub uses: String,

    /// Action parameters
    #[serde(default)]
    pub with: HashMap<String, serde_yaml::Value>,

    /// Step-level environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Condition to run this step
    #[serde(rename = "if")]
    pub condition: Option<String>,

    /// ID for referencing outputs
    pub id: Option<String>,

    /// Timeout for this step in milliseconds
    pub timeout: Option<u64>,

    /// Continue workflow if this step fails
    #[serde(default)]
    pub continue_on_error: bool,

    /// Retry configuration
    pub retry: Option<RetryConfig>,
}

/// Retry configuration for steps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of attempts
    pub max_attempts: u32,

    /// Delay between attempts in milliseconds
    #[serde(default = "default_retry_delay")]
    pub delay: u64,
}

fn default_retry_delay() -> u64 {
    1000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_deserialize() {
        let yaml = r#"
name: test-workflow
jobs:
  test:
    browser: chromium
    steps:
      - name: Go to page
        uses: page/goto
        with:
          url: "https://example.com"
"#;

        let workflow: Workflow = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(workflow.name, "test-workflow");
        assert!(workflow.jobs.contains_key("test"));
    }

    #[test]
    fn test_mixed_platform_workflow() {
        let yaml = r#"
name: mixed-test
platform: playwright
platforms:
  web:
    base_url: "http://localhost:3000"
jobs:
  setup:
    platform: web
    steps:
      - uses: web/post
        with:
          path: /api/seed
  visual:
    platform: playwright
    browser: chromium
    steps:
      - uses: page/goto
        with:
          url: "http://localhost:3000"
"#;

        let workflow: Workflow = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(workflow.platform, Some(Platform::Playwright));

        let setup_job = workflow.jobs.get("setup").unwrap();
        assert_eq!(setup_job.platform, Some(Platform::Web));

        let visual_job = workflow.jobs.get("visual").unwrap();
        assert_eq!(visual_job.platform, Some(Platform::Playwright));
    }

    #[test]
    fn test_step_level_platform() {
        let yaml = r#"
name: step-platform-test
platform: playwright
jobs:
  mixed:
    steps:
      - name: Browser step
        uses: page/goto
        with:
          url: "http://localhost:3000"
      - name: API step
        platform: web
        uses: web/get
        with:
          path: /api/health
      - name: Function step
        platform: nodejs
        uses: node/call
        with:
          function: getData
"#;

        let workflow: Workflow = serde_yaml::from_str(yaml).unwrap();
        let job = workflow.jobs.get("mixed").unwrap();

        assert_eq!(job.steps[0].platform, None);
        assert_eq!(job.steps[1].platform, Some(Platform::Web));
        assert_eq!(job.steps[2].platform, Some(Platform::Nodejs));
    }

    #[test]
    fn test_workflow_before_after() {
        let yaml = r#"
name: test-hooks
before:
  - uses: web/post
    with:
      path: /api/reset
after:
  - uses: web/post
    with:
      path: /api/cleanup
jobs:
  test:
    steps:
      - uses: wait/ms
        with:
          duration: "1"
"#;

        let workflow: Workflow = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(workflow.before.len(), 1);
        assert_eq!(workflow.after.len(), 1);
        assert_eq!(workflow.before[0].uses, "web/post");
        assert_eq!(workflow.after[0].uses, "web/post");
    }

    #[test]
    fn test_job_before_after() {
        let yaml = r#"
name: test-job-hooks
jobs:
  test:
    before:
      - uses: web/post
        with:
          path: /api/seed
    after:
      - uses: web/post
        with:
          path: /api/clear
    steps:
      - uses: wait/ms
        with:
          duration: "1"
"#;

        let workflow: Workflow = serde_yaml::from_str(yaml).unwrap();
        let job = workflow.jobs.get("test").unwrap();
        assert_eq!(job.before.len(), 1);
        assert_eq!(job.after.len(), 1);
        assert_eq!(job.before[0].uses, "web/post");
        assert_eq!(job.after[0].uses, "web/post");
    }
}
