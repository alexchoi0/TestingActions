//! # Playwright Actions
//!
//! A GitHub Actions-style declarative workflow engine for browser automation
//! powered by Playwright, with support for multi-platform test execution.
//!
//! ## Features
//!
//! - **Declarative YAML workflows** - Define browser automation like GitHub Actions
//! - **Multi-platform execution** - Run via Playwright, Node.js, Python, Java, Rust, Go, or HTTP
//! - **Parallel execution** - Run independent jobs simultaneously
//! - **Expression syntax** - Use `${{ }}` for dynamic values
//!
//! ## Platforms
//!
//! - **Playwright** (default): Full browser automation via Playwright
//! - **Node.js**: Direct function calls via JSON-RPC bridge
//! - **Python**: Direct function calls via JSON-RPC bridge
//! - **Java**: Direct function calls via JSON-RPC bridge
//! - **Rust**: Direct function calls via JSON-RPC bridge
//! - **Go**: Direct function calls via JSON-RPC bridge
//! - **Web**: HTTP API calls via reqwest
//!
//! ## Quick Start - Playwright Workflow
//!
//! ```rust,no_run
//! use testing_actions::Executor;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let workflow_yaml = r#"
//! name: login-test
//! on:
//!   manual: true
//! jobs:
//!   login:
//!     browser: chromium
//!     headless: false
//!     steps:
//!       - name: Go to login page
//!         uses: page/goto
//!         with:
//!           url: "https://example.com/login"
//! "#;
//!
//!     let mut executor = Executor::new();
//!     let result = executor.run_yaml(workflow_yaml).await?;
//!
//!     println!("Workflow completed: success={}", result.success);
//!     Ok(())
//! }
//! ```

pub mod bridge;
pub mod client;
pub mod engine;
pub mod workflow;

#[path = "../extensions/rust/mod.rs"]
pub mod rust_bridge;

// Re-export main types
pub use bridge::{
    ApiResponse, AssertionResult, Bridge, BridgeConfig, BridgeError, GoBridge, JavaBridge,
    NodejsBridge, PlaywrightBridge, PythonBridge, RustBridge, WebBridge, WebResponse,
};
pub use engine::{
    run_workflow_directory, DAGError, DirectoryResult, DirectoryRunError, ExecutionContextSnapshot,
    Executor, ExecutorError, JobResult, SharedStateManager, StepResult, WorkflowDAG,
    WorkflowDirectoryRunner, WorkflowResult,
};
pub use rust_bridge::{serve as rust_serve, Context as RustContext, FunctionInfo, RustRegistry};
pub use workflow::{
    ActionCategory, BrowserType, ExecutionContext, GoConfig, GoHooksConfig, JavaConfig,
    JavaHooksConfig, Job, NodejsConfig, NodejsHooksConfig, ParsedAction, Platform, PlatformsConfig,
    PlaywrightConfig, PythonConfig, PythonHooksConfig, RustConfig, RustHooksConfig, Step, Viewport,
    WebAuthConfig, WebConfig, WebRetryConfig, Workflow,
};
pub use workflow::{LoadError, WorkflowLoader};

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::bridge::{Bridge, BridgeConfig};
    pub use crate::engine::{
        run_workflow_directory, DAGError, DirectoryResult, DirectoryRunError, Executor,
        SharedStateManager, WorkflowDAG, WorkflowDirectoryRunner, WorkflowResult,
    };
    pub use crate::rust_bridge::{
        serve as rust_serve, AssertionResult as RustAssertionResult, Context as RustContext,
        FunctionInfo, RustRegistry,
    };
    pub use crate::workflow::{
        BrowserType, GoConfig, JavaConfig, Job, LoadError, NodejsConfig, Platform, PlatformsConfig,
        PlaywrightConfig, PythonConfig, RustConfig, Step, WebAuthConfig, WebConfig, WebRetryConfig,
        Workflow, WorkflowLoader,
    };
}
