//! Workflow Executor - Runs workflows and manages state
//!
//! This is the main engine that:
//! 1. Parses workflow YAML
//! 2. Resolves dependencies between jobs
//! 3. Executes steps in order
//! 4. Manages context and variable passing
//! 5. Routes actions to appropriate platform

use std::collections::{HashMap, HashSet};
use tracing::{debug, error, info, instrument, warn};

use crate::bridge::{
    Bridge, BridgeConfig, GoBridge, JavaBridge, NodejsBridge, PlaywrightBridge, PythonBridge,
    RustBridge, WebBridge,
};
use crate::engine::actions;
use crate::engine::error::ExecutorError;
use crate::engine::mock_clock::{parse_duration, parse_time, parse_timezone, MockClock};
use crate::engine::result::{JobResult, StepResult, WorkflowResult};
use crate::engine::state_manager::SharedStateManager;
use crate::workflow::expressions::{evaluate_condition, evaluate_params, evaluate_params_json};
use crate::workflow::*;

/// Execution phase - determines clock behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionPhase {
    /// Before hooks - no auto-advance clock
    Before,
    /// Main steps - auto-advance clock
    Steps,
    /// After hooks - no auto-advance clock
    After,
}

/// The platform-aware workflow executor
pub struct Executor {
    bridges: HashMap<Platform, Box<dyn Bridge>>,
    configs: HashMap<Platform, BridgeConfig>,
    context: ExecutionContext,
    state_manager: Option<SharedStateManager>,
    mock_clock: MockClock,
}

impl Executor {
    /// Create a new executor (lazy initialization of bridges)
    pub fn new() -> Self {
        Self {
            bridges: HashMap::new(),
            configs: HashMap::new(),
            context: ExecutionContext::new(),
            state_manager: None,
            mock_clock: MockClock::new(),
        }
    }

    /// Create executor with custom context (for testing/seeding)
    pub fn with_context(context: ExecutionContext) -> Self {
        Self {
            bridges: HashMap::new(),
            configs: HashMap::new(),
            context,
            state_manager: None,
            mock_clock: MockClock::new(),
        }
    }

    /// Get a reference to the mock clock
    pub fn clock(&self) -> &MockClock {
        &self.mock_clock
    }

    /// Set platform configurations from external source (e.g., runner config)
    pub fn with_platforms(mut self, platforms: &PlatformsConfig) -> Self {
        if let Some(config) = &platforms.playwright {
            self.configs.insert(
                Platform::Playwright,
                BridgeConfig::Playwright(config.clone()),
            );
        }
        if let Some(config) = &platforms.nodejs {
            self.configs
                .insert(Platform::Nodejs, BridgeConfig::Nodejs(config.clone()));
        }
        if let Some(config) = &platforms.rust {
            self.configs
                .insert(Platform::Rust, BridgeConfig::Rust(config.clone()));
        }
        if let Some(config) = &platforms.python {
            self.configs
                .insert(Platform::Python, BridgeConfig::Python(config.clone()));
        }
        if let Some(config) = &platforms.java {
            self.configs
                .insert(Platform::Java, BridgeConfig::Java(config.clone()));
        }
        if let Some(config) = &platforms.go {
            self.configs
                .insert(Platform::Go, BridgeConfig::Go(config.clone()));
        }
        if let Some(config) = &platforms.web {
            self.configs
                .insert(Platform::Web, BridgeConfig::Web(config.clone()));
        }
        self
    }

    /// Set environment variables
    pub fn set_env(&mut self, key: &str, value: &str) {
        self.context.env.insert(key.to_string(), value.to_string());
    }

    /// Set secrets
    pub fn set_secret(&mut self, key: &str, value: &str) {
        self.context
            .secrets
            .insert(key.to_string(), value.to_string());
    }

    /// Ensure a bridge is initialized for the given platform
    async fn ensure_bridge(&mut self, platform: Platform) -> Result<(), ExecutorError> {
        if self.bridges.contains_key(&platform) {
            return Ok(());
        }

        let config = self.configs.get(&platform).ok_or_else(|| {
            ExecutorError::ConfigError(format!(
                "{:?} platform requires configuration in workflow",
                platform
            ))
        })?;

        let bridge: Box<dyn Bridge> = match (platform.clone(), config.clone()) {
            (Platform::Playwright, BridgeConfig::Playwright(_)) => {
                info!("Initializing Playwright bridge");
                Box::new(PlaywrightBridge::start().await?)
            }
            (Platform::Nodejs, BridgeConfig::Nodejs(c)) => {
                info!("Initializing Node.js bridge (registry: {})", c.registry);
                Box::new(NodejsBridge::from_config(&c).await?)
            }
            (Platform::Rust, BridgeConfig::Rust(c)) => {
                let binary_info = c
                    .binary
                    .as_ref()
                    .or(c.cargo_bin.as_ref())
                    .map(|s| s.as_str())
                    .unwrap_or("unknown");
                info!("Initializing Rust bridge (binary: {})", binary_info);
                Box::new(RustBridge::from_config(&c).await?)
            }
            (Platform::Python, BridgeConfig::Python(c)) => {
                info!(
                    "Initializing Python bridge (script: {}, interpreter: {})",
                    c.script, c.interpreter
                );
                Box::new(PythonBridge::from_config(&c).await?)
            }
            (Platform::Java, BridgeConfig::Java(c)) => {
                info!("Initializing Java bridge (main_class: {})", c.main_class);
                Box::new(JavaBridge::from_config(&c).await?)
            }
            (Platform::Go, BridgeConfig::Go(c)) => {
                let binary_info = c
                    .binary
                    .as_ref()
                    .or(c.go_run.as_ref())
                    .or(c.go_build.as_ref())
                    .map(|s| s.as_str())
                    .unwrap_or("unknown");
                info!("Initializing Go bridge (binary: {})", binary_info);
                Box::new(GoBridge::from_config(&c).await?)
            }
            (Platform::Web, BridgeConfig::Web(c)) => {
                info!("Initializing Web bridge (base_url: {})", c.base_url);
                Box::new(WebBridge::from_config(&c)?)
            }
            _ => {
                return Err(ExecutorError::ConfigError(format!(
                    "Platform {:?} configuration mismatch",
                    platform
                )));
            }
        };

        self.bridges.insert(platform, bridge);
        Ok(())
    }

    /// Get a reference to the Playwright bridge
    fn get_playwright_bridge(&self) -> Result<&PlaywrightBridge, ExecutorError> {
        self.bridges
            .get(&Platform::Playwright)
            .and_then(|b| b.as_playwright())
            .ok_or_else(|| ExecutorError::ConfigError("Playwright bridge not initialized".into()))
    }

    /// Get a reference to the Web bridge
    fn get_web_bridge(&self) -> Result<&WebBridge, ExecutorError> {
        self.bridges
            .get(&Platform::Web)
            .and_then(|b| b.as_web())
            .ok_or_else(|| ExecutorError::ConfigError("Web bridge not initialized".into()))
    }

    /// Get a reference to the Node.js bridge
    fn get_nodejs_bridge(&self) -> Result<&NodejsBridge, ExecutorError> {
        self.bridges
            .get(&Platform::Nodejs)
            .and_then(|b| b.as_nodejs())
            .ok_or_else(|| ExecutorError::ConfigError("Node.js bridge not initialized".into()))
    }

    /// Get a reference to the Rust bridge
    fn get_rust_bridge(&self) -> Result<&RustBridge, ExecutorError> {
        self.bridges
            .get(&Platform::Rust)
            .and_then(|b| b.as_rust())
            .ok_or_else(|| ExecutorError::ConfigError("Rust bridge not initialized".into()))
    }

    /// Get a reference to the Python bridge
    fn get_python_bridge(&self) -> Result<&PythonBridge, ExecutorError> {
        self.bridges
            .get(&Platform::Python)
            .and_then(|b| b.as_python())
            .ok_or_else(|| ExecutorError::ConfigError("Python bridge not initialized".into()))
    }

    /// Get a reference to the Java bridge
    fn get_java_bridge(&self) -> Result<&JavaBridge, ExecutorError> {
        self.bridges
            .get(&Platform::Java)
            .and_then(|b| b.as_java())
            .ok_or_else(|| ExecutorError::ConfigError("Java bridge not initialized".into()))
    }

    /// Get a reference to the Go bridge
    fn get_go_bridge(&self) -> Result<&GoBridge, ExecutorError> {
        self.bridges
            .get(&Platform::Go)
            .and_then(|b| b.as_go())
            .ok_or_else(|| ExecutorError::ConfigError("Go bridge not initialized".into()))
    }

    /// Get the effective platform for a job (without considering step)
    fn get_job_platform(
        &self,
        job: &Job,
        workflow_platform: &Option<Platform>,
    ) -> Option<Platform> {
        job.platform.clone().or_else(|| workflow_platform.clone())
    }

    /// Resolve the platform for a specific step
    /// Priority: step > job > workflow > infer from action
    fn resolve_step_platform(
        &self,
        step: &Step,
        job: &Job,
        workflow_platform: &Option<Platform>,
    ) -> Platform {
        step.platform
            .clone()
            .or_else(|| job.platform.clone())
            .or_else(|| workflow_platform.clone())
            .unwrap_or_else(|| {
                ParsedAction::parse(&step.uses)
                    .ok()
                    .and_then(|a| a.category.infer_platform())
                    .unwrap_or(Platform::Playwright)
            })
    }

    /// Run a workflow from YAML string
    #[instrument(skip(self, yaml))]
    pub async fn run_yaml(&mut self, yaml: &str) -> Result<WorkflowResult, ExecutorError> {
        let workflow: Workflow = serde_yaml::from_str(yaml)?;
        self.run(workflow).await
    }

    /// Run a workflow
    #[instrument(skip(self, workflow), fields(workflow_name = %workflow.name))]
    pub async fn run(&mut self, workflow: Workflow) -> Result<WorkflowResult, ExecutorError> {
        info!(
            "Starting workflow: {} (platform: {:?})",
            workflow.name, workflow.platform
        );

        // Store platform configs from consolidated platforms field
        if let Some(config) = &workflow.platforms.playwright {
            self.configs.insert(
                Platform::Playwright,
                BridgeConfig::Playwright(config.clone()),
            );
        }
        if let Some(config) = &workflow.platforms.nodejs {
            self.configs
                .insert(Platform::Nodejs, BridgeConfig::Nodejs(config.clone()));
        }
        if let Some(config) = &workflow.platforms.rust {
            self.configs
                .insert(Platform::Rust, BridgeConfig::Rust(config.clone()));
        }
        if let Some(config) = &workflow.platforms.python {
            self.configs
                .insert(Platform::Python, BridgeConfig::Python(config.clone()));
        }
        if let Some(config) = &workflow.platforms.java {
            self.configs
                .insert(Platform::Java, BridgeConfig::Java(config.clone()));
        }
        if let Some(config) = &workflow.platforms.go {
            self.configs
                .insert(Platform::Go, BridgeConfig::Go(config.clone()));
        }
        if let Some(config) = &workflow.platforms.web {
            self.configs
                .insert(Platform::Web, BridgeConfig::Web(config.clone()));
        }

        // Initialize state manager for this workflow
        self.state_manager = Some(SharedStateManager::new());

        // Merge workflow env into context
        for (key, value) in &workflow.env {
            self.context.env.insert(key.clone(), value.clone());
        }

        let mut results: HashMap<String, JobResult> = HashMap::new();
        let mut all_success = true;

        // Execute workflow before hooks (no clock auto-advance)
        if !workflow.before.is_empty() {
            info!("Running workflow before hooks");
            for step in &workflow.before {
                let result = self
                    .execute_hook_step(step, &workflow.platform, ExecutionPhase::Before)
                    .await;
                if let Err(e) = result {
                    error!("Workflow before hook failed: {}", e);
                    // Before hook failure should abort the workflow
                    return Ok(WorkflowResult {
                        success: false,
                        jobs: results,
                        run_id: self.context.run_id.clone(),
                    });
                }
            }
        }

        // Determine job execution order (topological sort)
        let job_order = self.topological_sort(&workflow.jobs)?;
        debug!("Job execution order: {:?}", job_order);

        // Execute jobs in order
        for job_name in job_order {
            let job = workflow.jobs.get(&job_name).unwrap();

            // Check if dependencies succeeded
            let deps_ok = job
                .needs
                .iter()
                .all(|dep| results.get(dep).map(|r| r.success).unwrap_or(false));

            if !deps_ok && !job.continue_on_error {
                warn!("Skipping job {} due to failed dependencies", job_name);
                results.insert(
                    job_name.clone(),
                    JobResult {
                        success: false,
                        outputs: HashMap::new(),
                        steps: vec![],
                    },
                );
                all_success = false;
                continue;
            }

            // Check job condition
            if let Some(condition) = &job.condition {
                if !evaluate_condition(condition, &self.context)? {
                    info!("Skipping job {} due to condition", job_name);
                    continue;
                }
            }

            // Execute the job
            self.context.current_job = Some(job_name.clone());
            let job_platform = self.get_job_platform(job, &workflow.platform);
            let job_result = self
                .execute_job(&job_name, job, &workflow.platform, &job_platform)
                .await;

            match job_result {
                Ok(result) => {
                    if !result.success {
                        all_success = false;
                    }
                    self.context
                        .jobs
                        .insert(job_name.clone(), result.outputs.clone());
                    results.insert(job_name, result);
                }
                Err(e) => {
                    error!("Job {} failed: {}", job_name, e);
                    all_success = false;
                    results.insert(
                        job_name,
                        JobResult {
                            success: false,
                            outputs: HashMap::new(),
                            steps: vec![],
                        },
                    );
                }
            }
        }

        // Execute workflow after hooks (always, no clock auto-advance)
        if !workflow.after.is_empty() {
            info!("Running workflow after hooks");
            for step in &workflow.after {
                let result = self
                    .execute_hook_step(step, &workflow.platform, ExecutionPhase::After)
                    .await;
                if let Err(e) = result {
                    error!("Workflow after hook failed: {}", e);
                    // After hook failure doesn't change overall success
                }
            }
        }

        Ok(WorkflowResult {
            success: all_success,
            jobs: results,
            run_id: self.context.run_id.clone(),
        })
    }

    /// Execute a single job
    #[instrument(skip(self, job))]
    async fn execute_job(
        &mut self,
        job_name: &str,
        job: &Job,
        workflow_platform: &Option<Platform>,
        job_platform: &Option<Platform>,
    ) -> Result<JobResult, ExecutorError> {
        info!(
            "Executing job: {} (default platform: {:?})",
            job_name, job_platform
        );

        // Merge job env
        for (key, value) in &job.env {
            self.context.env.insert(key.clone(), value.clone());
        }

        // Track browser state for playwright steps
        let mut browser_id: Option<String> = None;
        let mut page_id: Option<String> = None;

        let mut step_results = vec![];
        let job_outputs = HashMap::new();
        let mut all_success = true;

        // Execute job before hooks (no clock auto-advance)
        if !job.before.is_empty() {
            info!("Running job before hooks for: {}", job_name);
            for step in &job.before {
                let result = self
                    .execute_hook_step(step, workflow_platform, ExecutionPhase::Before)
                    .await;
                if let Err(e) = result {
                    error!("Job before hook failed: {}", e);
                    // Before hook failure should abort the job
                    return Ok(JobResult {
                        success: false,
                        outputs: job_outputs,
                        steps: step_results,
                    });
                }
            }
        }

        // Execute main steps (with clock auto-advance)
        for (idx, step) in job.steps.iter().enumerate() {
            let step_name = step
                .name
                .clone()
                .unwrap_or_else(|| format!("Step {}", idx + 1));

            // Check step condition
            if let Some(condition) = &step.condition {
                if !evaluate_condition(condition, &self.context)? {
                    info!("Skipping step '{}' due to condition", step_name);
                    continue;
                }
            }

            if let Some(step_id) = &step.id {
                self.context.current_step = Some(step_id.clone());
            }

            // Parse action first to check if it needs a platform
            let action = ParsedAction::parse(&step.uses).map_err(ExecutorError::UnknownAction)?;

            // For platform-agnostic actions (wait, etc.), execute directly without bridge
            let result = if action.category.is_platform_agnostic()
                && step.platform.is_none()
                && job.platform.is_none()
                && workflow_platform.is_none()
            {
                self.execute_platform_agnostic_action(&action, step).await
            } else {
                // Resolve platform for this specific step
                let step_platform = self.resolve_step_platform(step, job, workflow_platform);

                // Ensure the appropriate bridge is ready
                self.ensure_bridge(step_platform.clone()).await?;

                // Special handling for Playwright: manage browser/page lifecycle
                if step_platform == Platform::Playwright && browser_id.is_none() {
                    let bridge = self.get_playwright_bridge()?;
                    browser_id = Some(
                        bridge
                            .browser_launch(job.browser.clone(), job.headless)
                            .await?,
                    );
                    page_id = Some(bridge.page_new(browser_id.as_ref().unwrap()).await?);
                }

                self.execute_step(step, &step_platform, page_id.as_deref())
                    .await
            };

            // Auto-advance clock after each step (if clock is active) - only during Steps phase
            if self.mock_clock.is_active().await {
                self.mock_clock.auto_advance_step().await;
                // Sync to all active bridges - ignore errors as some bridges may not be active
                let _ = self.sync_clock_to_bridges().await;
            }

            match result {
                Ok(step_result) => {
                    if let Some(step_id) = &step.id {
                        for (key, value) in &step_result.outputs {
                            self.context.set_output(step_id, key, value.clone());
                        }
                    }

                    if !step_result.success {
                        all_success = false;
                        if !step.continue_on_error && !job.continue_on_error {
                            step_results.push(step_result);
                            break;
                        }
                    }

                    step_results.push(step_result);
                }
                Err(e) => {
                    error!("Step '{}' failed: {}", step_name, e);
                    all_success = false;

                    step_results.push(StepResult {
                        success: false,
                        outputs: HashMap::new(),
                        error: Some(e.to_string()),
                        response: None,
                    });

                    if !step.continue_on_error && !job.continue_on_error {
                        break;
                    }
                }
            }
        }

        // Execute job after hooks (always, no clock auto-advance)
        if !job.after.is_empty() {
            info!("Running job after hooks for: {}", job_name);
            for step in &job.after {
                let result = self
                    .execute_hook_step(step, workflow_platform, ExecutionPhase::After)
                    .await;
                if let Err(e) = result {
                    error!("Job after hook failed: {}", e);
                    // After hook failure doesn't change overall success
                }
            }
        }

        // Cleanup
        if let Some(browser_id) = browser_id {
            if let Ok(bridge) = self.get_playwright_bridge() {
                let _ = bridge.browser_close(&browser_id).await;
            }
        }

        Ok(JobResult {
            success: all_success,
            outputs: job_outputs,
            steps: step_results,
        })
    }

    /// Execute a single step
    #[instrument(skip(self, step))]
    async fn execute_step(
        &mut self,
        step: &Step,
        platform: &Platform,
        page_id: Option<&str>,
    ) -> Result<StepResult, ExecutorError> {
        let step_name = step.name.clone().unwrap_or_else(|| step.uses.clone());
        info!("Executing step: {}", step_name);

        // Parse action
        let action = ParsedAction::parse(&step.uses).map_err(ExecutorError::UnknownAction)?;

        // Check platform compatibility
        if !action.is_compatible_with(platform) {
            return Err(ExecutorError::PlatformMismatch(format!(
                "Action '{}' is not compatible with {:?} platform",
                step.uses, platform
            )));
        }

        // Evaluate parameters (string form for simple actions)
        let params = evaluate_params(&step.with, &self.context)?;
        // Evaluate parameters (JSON form for function calls, preserving structure)
        let json_params = evaluate_params_json(&step.with, &self.context)?;

        // Execute based on platform and action category
        match platform {
            Platform::Playwright => {
                self.execute_playwright_action(&action, page_id, &params)
                    .await
            }
            Platform::Nodejs => {
                self.execute_nodejs_action(&action, &params, &json_params)
                    .await
            }
            Platform::Rust => {
                self.execute_rust_action(&action, &params, &json_params)
                    .await
            }
            Platform::Python => {
                self.execute_python_action(&action, &params, &json_params)
                    .await
            }
            Platform::Java => {
                self.execute_java_action(&action, &params, &json_params)
                    .await
            }
            Platform::Go => self.execute_go_action(&action, &params, &json_params).await,
            Platform::Web => self.execute_web_action(&action, &params).await,
        }
    }

    /// Execute a hook step (before/after) without clock auto-advance
    ///
    /// Hook steps execute in Before or After phase, which means:
    /// - No automatic clock advancement
    /// - Used for setup/teardown operations
    #[instrument(skip(self, step))]
    async fn execute_hook_step(
        &mut self,
        step: &Step,
        workflow_platform: &Option<Platform>,
        _phase: ExecutionPhase,
    ) -> Result<StepResult, ExecutorError> {
        let step_name = step.name.clone().unwrap_or_else(|| step.uses.clone());
        info!("Executing hook step: {}", step_name);

        // Parse action
        let action = ParsedAction::parse(&step.uses).map_err(ExecutorError::UnknownAction)?;

        // For platform-agnostic actions, execute directly
        if action.category.is_platform_agnostic() && step.platform.is_none() {
            return self.execute_platform_agnostic_action(&action, step).await;
        }

        // Resolve platform
        let platform = step
            .platform
            .clone()
            .or_else(|| workflow_platform.clone())
            .unwrap_or_else(|| {
                action
                    .category
                    .infer_platform()
                    .unwrap_or(Platform::Playwright)
            });

        // Ensure bridge is ready
        self.ensure_bridge(platform.clone()).await?;

        // Execute step (no clock auto-advance for hooks)
        self.execute_step(step, &platform, None).await
    }

    /// Execute a Playwright action
    async fn execute_playwright_action(
        &self,
        action: &ParsedAction,
        page_id: Option<&str>,
        params: &HashMap<String, String>,
    ) -> Result<StepResult, ExecutorError> {
        let page_id = page_id.ok_or_else(|| {
            ExecutorError::ConfigError("Playwright action requires page_id".to_string())
        })?;
        let bridge = self.get_playwright_bridge()?;

        let outputs = match action.category {
            ActionCategory::Page => {
                actions::playwright::execute_page_action(bridge, &action.action, page_id, params)
                    .await?
            }
            ActionCategory::Element => {
                actions::playwright::execute_element_action(bridge, &action.action, page_id, params)
                    .await?
            }
            ActionCategory::Assert => {
                actions::playwright::execute_assert(bridge, &action.action, page_id, params).await?
            }
            ActionCategory::Wait => {
                actions::playwright::execute_wait_action(bridge, &action.action, page_id, params)
                    .await?
            }
            ActionCategory::Browser => {
                actions::playwright::execute_browser_action(bridge, &action.action, page_id, params)
                    .await?
            }
            ActionCategory::Network => {
                actions::playwright::execute_network_action(&action.action, page_id, params).await?
            }
            _ => {
                return Err(ExecutorError::PlatformMismatch(format!(
                    "Action category {:?} not supported on Playwright",
                    action.category
                )));
            }
        };

        Ok(StepResult {
            success: true,
            outputs,
            error: None,
            response: None,
        })
    }

    /// Execute a Node.js action
    async fn execute_nodejs_action(
        &self,
        action: &ParsedAction,
        params: &HashMap<String, String>,
        json_params: &HashMap<String, serde_json::Value>,
    ) -> Result<StepResult, ExecutorError> {
        let bridge = self.get_nodejs_bridge()?;

        match action.category {
            ActionCategory::Node => {
                actions::nodejs::execute_node_action(bridge, &action.action, params, json_params)
                    .await
            }
            ActionCategory::Ctx => {
                actions::nodejs::execute_ctx_action(bridge, &action.action, params).await
            }
            ActionCategory::Mock => {
                actions::nodejs::execute_mock_action(bridge, &action.action, params).await
            }
            ActionCategory::Hook => {
                actions::nodejs::execute_hook_action(bridge, &action.action, params).await
            }
            ActionCategory::Assert => {
                actions::nodejs::execute_assert_action(bridge, &action.action, json_params).await
            }
            ActionCategory::Wait => actions::nodejs::execute_wait(&action.action, params).await,
            _ => Err(ExecutorError::PlatformMismatch(format!(
                "Action category {:?} not supported on Node.js",
                action.category
            ))),
        }
    }

    /// Execute a Rust action
    async fn execute_rust_action(
        &self,
        action: &ParsedAction,
        params: &HashMap<String, String>,
        json_params: &HashMap<String, serde_json::Value>,
    ) -> Result<StepResult, ExecutorError> {
        let bridge = self.get_rust_bridge()?;

        match action.category {
            ActionCategory::Rs => {
                actions::rust::execute_rs_action(bridge, &action.action, params, json_params).await
            }
            ActionCategory::Assert => {
                actions::rust::execute_assert_action(bridge, &action.action, json_params).await
            }
            ActionCategory::Wait => actions::nodejs::execute_wait(&action.action, params).await,
            _ => Err(ExecutorError::PlatformMismatch(format!(
                "Action category {:?} not supported on Rust",
                action.category
            ))),
        }
    }

    /// Execute a Python action
    async fn execute_python_action(
        &self,
        action: &ParsedAction,
        params: &HashMap<String, String>,
        json_params: &HashMap<String, serde_json::Value>,
    ) -> Result<StepResult, ExecutorError> {
        let bridge = self.get_python_bridge()?;

        match action.category {
            ActionCategory::Py => {
                actions::python::execute_py_action(bridge, &action.action, params, json_params)
                    .await
            }
            ActionCategory::Assert => {
                actions::python::execute_assert_action(bridge, &action.action, json_params).await
            }
            ActionCategory::Wait => actions::nodejs::execute_wait(&action.action, params).await,
            _ => Err(ExecutorError::PlatformMismatch(format!(
                "Action category {:?} not supported on Python",
                action.category
            ))),
        }
    }

    /// Execute a Java action
    async fn execute_java_action(
        &self,
        action: &ParsedAction,
        params: &HashMap<String, String>,
        json_params: &HashMap<String, serde_json::Value>,
    ) -> Result<StepResult, ExecutorError> {
        let bridge = self.get_java_bridge()?;

        match action.category {
            ActionCategory::Java => {
                actions::java::execute_java_call(bridge, &action.action, params, json_params).await
            }
            ActionCategory::Assert => {
                actions::java::execute_assert_action(bridge, &action.action, json_params).await
            }
            ActionCategory::Wait => actions::nodejs::execute_wait(&action.action, params).await,
            _ => Err(ExecutorError::PlatformMismatch(format!(
                "Action category {:?} not supported on Java",
                action.category
            ))),
        }
    }

    /// Execute a Go action
    async fn execute_go_action(
        &self,
        action: &ParsedAction,
        params: &HashMap<String, String>,
        json_params: &HashMap<String, serde_json::Value>,
    ) -> Result<StepResult, ExecutorError> {
        let bridge = self.get_go_bridge()?;

        match action.category {
            ActionCategory::Go => {
                actions::go::execute_go_call(bridge, &action.action, params, json_params).await
            }
            ActionCategory::Assert => {
                actions::go::execute_assert_action(bridge, &action.action, json_params).await
            }
            ActionCategory::Wait => actions::nodejs::execute_wait(&action.action, params).await,
            _ => Err(ExecutorError::PlatformMismatch(format!(
                "Action category {:?} not supported on Go",
                action.category
            ))),
        }
    }

    /// Execute a Web/HTTP action
    async fn execute_web_action(
        &self,
        action: &ParsedAction,
        params: &HashMap<String, String>,
    ) -> Result<StepResult, ExecutorError> {
        let bridge = self.get_web_bridge()?;

        match action.category {
            ActionCategory::Web => {
                actions::web::execute_web_request(bridge, &action.action, params).await
            }
            ActionCategory::Assert => actions::web::execute_assert(&action.action, params).await,
            ActionCategory::Wait => actions::nodejs::execute_wait(&action.action, params).await,
            _ => Err(ExecutorError::PlatformMismatch(format!(
                "Action category {:?} not supported on Web",
                action.category
            ))),
        }
    }

    /// Execute a platform-agnostic action (wait, assert without platform context)
    async fn execute_platform_agnostic_action(
        &self,
        action: &ParsedAction,
        step: &Step,
    ) -> Result<StepResult, ExecutorError> {
        let params = evaluate_params(&step.with, &self.context)?;

        match action.category {
            ActionCategory::Wait => actions::nodejs::execute_wait(&action.action, &params).await,
            ActionCategory::Assert => Err(ExecutorError::ConfigError(
                "Assert actions require a platform context".to_string(),
            )),
            ActionCategory::Fail => {
                let message = params
                    .get("message")
                    .map(|v| v.as_str())
                    .unwrap_or("Intentional failure");
                Err(ExecutorError::StepFailed(message.to_string()))
            }
            ActionCategory::Clock => self.execute_clock_action(&action.action, &params).await,
            ActionCategory::Bash => {
                actions::bash::execute_bash_action(&action.action, &params).await
            }
            _ => Err(ExecutorError::PlatformMismatch(format!(
                "Action category {:?} is not platform-agnostic",
                action.category
            ))),
        }
    }

    /// Execute a clock action and sync to all active bridges
    async fn execute_clock_action(
        &self,
        action: &str,
        params: &HashMap<String, String>,
    ) -> Result<StepResult, ExecutorError> {
        match action {
            "set" => {
                let time_str = params.get("time").ok_or_else(|| {
                    ExecutorError::ConfigError("clock/set requires 'time' parameter".to_string())
                })?;
                let time = parse_time(time_str).map_err(|e| {
                    ExecutorError::ConfigError(format!("Invalid time format: {}", e))
                })?;
                self.mock_clock.set(time).await;

                // Handle optional timezone parameter
                if let Some(tz_str) = params.get("timezone") {
                    let offset_secs = parse_timezone(tz_str).map_err(|e| {
                        ExecutorError::ConfigError(format!("Invalid timezone format: {}", e))
                    })?;
                    self.mock_clock.set_timezone(offset_secs).await;
                    info!("Clock set to: {} (timezone: {})", time, tz_str);
                } else {
                    info!("Clock set to: {}", time);
                }
            }
            "timezone" => {
                let tz_str = params.get("timezone").ok_or_else(|| {
                    ExecutorError::ConfigError(
                        "clock/timezone requires 'timezone' parameter".to_string(),
                    )
                })?;
                let offset_secs = parse_timezone(tz_str).map_err(|e| {
                    ExecutorError::ConfigError(format!("Invalid timezone format: {}", e))
                })?;
                self.mock_clock.set_timezone(offset_secs).await;
                info!("Clock timezone set to: {}", tz_str);
            }
            "forward" => {
                let duration_str = params.get("duration").ok_or_else(|| {
                    ExecutorError::ConfigError(
                        "clock/forward requires 'duration' parameter".to_string(),
                    )
                })?;
                let duration = parse_duration(duration_str).map_err(|e| {
                    ExecutorError::ConfigError(format!("Invalid duration format: {}", e))
                })?;
                self.mock_clock.forward(duration).await;
                info!("Clock forwarded by: {:?}", duration);
            }
            "forward-until" => {
                let time_str = params.get("time").ok_or_else(|| {
                    ExecutorError::ConfigError(
                        "clock/forward-until requires 'time' parameter".to_string(),
                    )
                })?;
                let time = parse_time(time_str).map_err(|e| {
                    ExecutorError::ConfigError(format!("Invalid time format: {}", e))
                })?;
                self.mock_clock.forward_until(time).await.map_err(|e| {
                    ExecutorError::ConfigError(format!("Cannot forward clock: {}", e))
                })?;
                info!("Clock forwarded until: {}", time);
            }
            "reset" => {
                self.mock_clock.reset().await;
                info!("Clock reset to real time");
            }
            _ => {
                return Err(ExecutorError::UnknownAction(format!(
                    "Unknown clock action: {}",
                    action
                )));
            }
        }

        // Sync clock to all active bridges
        self.sync_clock_to_bridges().await?;

        let clock_state = self.mock_clock.get_sync_state().await;
        let mut outputs = HashMap::new();
        if let Some(time_iso) = clock_state.virtual_time_iso {
            outputs.insert("time".to_string(), time_iso);
        }
        if let Some(time_ms) = clock_state.virtual_time_ms {
            outputs.insert("time_ms".to_string(), time_ms.to_string());
        }

        Ok(StepResult {
            success: true,
            outputs,
            error: None,
            response: None,
        })
    }

    /// Sync the current clock state to all active bridges
    async fn sync_clock_to_bridges(&self) -> Result<(), ExecutorError> {
        let clock_state = self.mock_clock.get_sync_state().await;

        for (platform, bridge) in &self.bridges {
            if bridge.supports_clock() {
                if let Err(e) = bridge.sync_clock(&clock_state).await {
                    warn!("Failed to sync clock to {:?} bridge: {}", platform, e);
                }
            }
        }

        Ok(())
    }

    /// Topological sort of jobs based on dependencies
    fn topological_sort(&self, jobs: &HashMap<String, Job>) -> Result<Vec<String>, ExecutorError> {
        let mut result = vec![];
        let mut visited = HashSet::new();
        let mut temp_visited = HashSet::new();

        fn visit(
            name: &str,
            jobs: &HashMap<String, Job>,
            visited: &mut HashSet<String>,
            temp_visited: &mut HashSet<String>,
            result: &mut Vec<String>,
            path: &mut Vec<String>,
        ) -> Result<(), ExecutorError> {
            if temp_visited.contains(name) {
                return Err(ExecutorError::CircularDependency(path.clone()));
            }
            if visited.contains(name) {
                return Ok(());
            }

            temp_visited.insert(name.to_string());
            path.push(name.to_string());

            if let Some(job) = jobs.get(name) {
                for dep in &job.needs {
                    if !jobs.contains_key(dep) {
                        return Err(ExecutorError::JobNotFound(dep.clone()));
                    }
                    visit(dep, jobs, visited, temp_visited, result, path)?;
                }
            }

            path.pop();
            temp_visited.remove(name);
            visited.insert(name.to_string());
            result.push(name.to_string());

            Ok(())
        }

        for name in jobs.keys() {
            visit(
                name,
                jobs,
                &mut visited,
                &mut temp_visited,
                &mut result,
                &mut vec![],
            )?;
        }

        Ok(result)
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}
