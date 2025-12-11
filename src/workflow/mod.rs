//! Workflow types and definitions
//!
//! This module contains all types for defining and parsing workflows:
//! - `platform` - Platform enum and all platform-specific configurations
//! - `job` - Workflow, Job, Step, and Triggers
//! - `action` - ActionCategory and ParsedAction for parsing "uses" fields
//! - `context` - ExecutionContext for runtime state
//! - `expressions` - Expression evaluation for `${{ }}` syntax
//! - `loader` - Load workflows from files and directories

pub mod action;
pub mod context;
pub mod expressions;
pub mod job;
pub mod loader;
pub mod platform;
pub mod runner_config;

// Re-export all public types for convenience
pub use action::{ActionCategory, ParsedAction};
pub use context::ExecutionContext;
pub use expressions::{evaluate as evaluate_expression, evaluate_params_json};
pub use job::{DependsOn, Job, JobDefaults, RetryConfig, Step, Workflow};
pub use loader::{LoadError, WorkflowLoader};
pub use platform::{
    BrowserType, GoConfig, GoHooksConfig, JavaConfig, JavaHooksConfig, NodejsConfig,
    NodejsHooksConfig, Platform, PlatformsConfig, PlaywrightConfig, PythonConfig,
    PythonHooksConfig, RustConfig, RustHooksConfig, Viewport, WebAuthConfig, WebConfig,
    WebRetryConfig,
};
pub use runner_config::{DatabaseConfigYaml, Profile, ProfileHooks, RunnerConfig};
