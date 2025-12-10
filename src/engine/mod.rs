//! Workflow execution engine module
//!
//! This module contains:
//! - `executor` - The main workflow executor
//! - `error` - Executor error types
//! - `result` - Step, job, and workflow result types
//! - `actions` - Platform-specific action implementations
//! - `state_manager` - Shared state management
//! - `workflow_dag` - DAG builder for multi-workflow execution
//! - `directory_runner` - Directory-based workflow runner
//! - `mock_clock` - Mock clock for controlling virtual time

pub mod actions;
pub mod directory_runner;
pub mod error;
pub mod executor;
pub mod mock_clock;
pub mod result;
pub mod state_manager;
pub mod workflow_dag;

pub use directory_runner::{
    run_workflow_directory, DirectoryResult, DirectoryRunError, MultiProfileResult,
    WorkflowDirectoryRunner,
};
pub use error::ExecutorError;
pub use executor::Executor;
pub use mock_clock::{ClockError, ClockSyncState, MockClock};
pub use result::{JobResult, StepResult, WorkflowResult};
pub use state_manager::*;
pub use workflow_dag::{DAGError, WorkflowDAG, WorkflowNode};
