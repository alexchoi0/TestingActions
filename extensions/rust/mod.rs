//! Rust Bridge - Direct Rust function calls via separate process
//!
//! This module provides the infrastructure for calling Rust functions directly
//! from workflows. Users implement the `RustRegistry` trait in their own binary,
//! then use `serve()` to start the JSON-RPC server.
//!
//! ## Usage
//!
//! 1. Implement `RustRegistry` in your code:
//!
//! ```ignore
//! use testing_actions::rust_bridge::{RustRegistry, Context, FunctionInfo, AssertionResult};
//! use serde_json::Value;
//!
//! struct MyRegistry;
//!
//! impl RustRegistry for MyRegistry {
//!     fn call(&self, name: &str, args: Value, ctx: &mut Context) -> Result<Value, String> {
//!         match name {
//!             "hello" => Ok(Value::String("world".to_string())),
//!             _ => Err(format!("Unknown function: {}", name))
//!         }
//!     }
//!
//!     fn list_functions(&self) -> Vec<FunctionInfo> {
//!         vec![FunctionInfo::new("hello", "Returns 'world'")]
//!     }
//! }
//! ```
//!
//! 2. Create a binary that serves the registry:
//!
//! ```ignore
//! use testing_actions::rust_bridge::serve;
//!
//! fn main() {
//!     let registry = MyRegistry;
//!     serve(registry);
//! }
//! ```
//!
//! 3. Reference the binary in your workflow:
//!
//! ```yaml
//! rust:
//!   binary: ./target/debug/my-registry
//! ```

mod context;
mod registry;
mod server;

pub use context::Context;
pub use registry::{AssertionResult, FunctionInfo, RustRegistry};
pub use server::serve;
