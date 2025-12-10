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

pub mod workflow;
pub mod engine;
pub mod bridge;

#[path = "../extensions/rust/mod.rs"]
pub mod rust_bridge;

// Re-export main types
pub use workflow::{
    Workflow, Job, Step, BrowserType, Viewport,
    ExecutionContext, ParsedAction, ActionCategory,
    Platform, NodejsConfig, NodejsHooksConfig,
    PlaywrightConfig, RustConfig, RustHooksConfig,
    PythonConfig, PythonHooksConfig, JavaConfig, JavaHooksConfig,
    GoConfig, GoHooksConfig,
    WebConfig, WebAuthConfig, WebRetryConfig, PlatformsConfig,
};
pub use engine::{
    Executor, ExecutorError, WorkflowResult, JobResult, StepResult,
    SharedStateManager, ExecutionContextSnapshot,
    run_workflow_directory, DirectoryResult, DirectoryRunError, WorkflowDirectoryRunner,
    WorkflowDAG, DAGError,
};
pub use workflow::{LoadError, WorkflowLoader};
pub use bridge::{
    PlaywrightBridge, NodejsBridge, RustBridge, PythonBridge, JavaBridge, GoBridge,
    WebBridge, WebBridgeOperations, WebOperations, WebResponse,
    BridgeError, NodejsBridgeOperations, RustBridgeOperations,
    PythonBridgeOperations, JavaBridgeOperations, GoBridgeOperations,
    ApiResponse, AssertionResult,
};
pub use rust_bridge::{RustRegistry, Context as RustContext, FunctionInfo, serve as rust_serve};

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::workflow::{
        Workflow, Job, Step, BrowserType, Platform, PlatformsConfig,
        NodejsConfig, PlaywrightConfig, RustConfig, PythonConfig, JavaConfig, GoConfig,
        WebConfig, WebAuthConfig, WebRetryConfig, WorkflowLoader, LoadError,
    };
    pub use crate::engine::{
        Executor, WorkflowResult, SharedStateManager,
        run_workflow_directory, DirectoryResult, DirectoryRunError, WorkflowDirectoryRunner,
        WorkflowDAG, DAGError,
    };
    pub use crate::rust_bridge::{RustRegistry, Context as RustContext, FunctionInfo, AssertionResult as RustAssertionResult, serve as rust_serve};
    pub use crate::bridge::{PythonBridgeOperations, JavaBridgeOperations, GoBridgeOperations, WebBridgeOperations};
}
