//! Rust registry trait and related types

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::Context;

/// Information about a registered function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionInfo {
    /// Function name (used in `rs/call` with: function: <name>)
    pub name: String,
    /// Human-readable description
    pub description: String,
}

impl FunctionInfo {
    /// Create a new function info
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
        }
    }
}

/// Result of a custom assertion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResult {
    /// Whether the assertion passed
    pub success: bool,
    /// Optional message (usually set on failure)
    pub message: Option<String>,
    /// Actual value that was found
    pub actual: Option<Value>,
    /// Expected value
    pub expected: Option<Value>,
}

impl AssertionResult {
    /// Create a successful assertion result
    pub fn pass() -> Self {
        Self {
            success: true,
            message: None,
            actual: None,
            expected: None,
        }
    }

    /// Create a successful assertion result with values
    pub fn pass_with_values(actual: Value, expected: Value) -> Self {
        Self {
            success: true,
            message: None,
            actual: Some(actual),
            expected: Some(expected),
        }
    }

    /// Create a failed assertion result
    pub fn fail(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: Some(message.into()),
            actual: None,
            expected: None,
        }
    }

    /// Create a failed assertion result with values
    pub fn fail_with_values(
        message: impl Into<String>,
        actual: Value,
        expected: Value,
    ) -> Self {
        Self {
            success: false,
            message: Some(message.into()),
            actual: Some(actual),
            expected: Some(expected),
        }
    }

    /// Create an error result (for when the assertion itself fails to execute)
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: Some(message.into()),
            actual: None,
            expected: None,
        }
    }
}

/// Trait that users implement to register their Rust functions for workflow calls
///
/// This trait is the main extension point for the Rust bridge. Users implement
/// this trait in their own crate, then create a binary that calls `serve(registry)`.
///
/// ## Example
///
/// ```ignore
/// use testing_actions::rust_bridge::{RustRegistry, Context, FunctionInfo, AssertionResult};
/// use serde_json::Value;
///
/// struct MyRegistry {
///     db: Database,
/// }
///
/// impl RustRegistry for MyRegistry {
///     fn call(&self, name: &str, args: Value, ctx: &mut Context) -> Result<Value, String> {
///         match name {
///             "create_user" => {
///                 let email = args.get("email")
///                     .and_then(|v| v.as_str())
///                     .ok_or("email required")?;
///                 let user = self.db.create_user(email)?;
///                 ctx.set("last_user_id", user.id.into());
///                 Ok(serde_json::to_value(user).unwrap())
///             }
///             _ => Err(format!("Unknown function: {}", name))
///         }
///     }
///
///     fn list_functions(&self) -> Vec<FunctionInfo> {
///         vec![
///             FunctionInfo::new("create_user", "Create a new user"),
///         ]
///     }
/// }
/// ```
pub trait RustRegistry: Send + Sync {
    /// Call a function by name
    ///
    /// # Arguments
    /// * `name` - The function name from the workflow step's `function` parameter
    /// * `args` - The arguments from the workflow step's `args` parameter (as JSON)
    /// * `ctx` - Mutable context for sharing data between calls
    ///
    /// # Returns
    /// * `Ok(Value)` - The function's return value (will be available in step outputs)
    /// * `Err(String)` - An error message (will fail the step)
    fn call(&self, name: &str, args: Value, ctx: &mut Context) -> Result<Value, String>;

    /// List all available functions
    ///
    /// This is called during workflow validation and can be used for
    /// auto-completion or documentation generation.
    fn list_functions(&self) -> Vec<FunctionInfo>;

    /// Call a custom assertion by name
    ///
    /// Custom assertions allow developers to define domain-specific checks
    /// that can be used in workflows with `assert/custom`.
    ///
    /// # Arguments
    /// * `name` - The assertion name from the workflow step
    /// * `params` - Parameters for the assertion (as JSON)
    /// * `ctx` - Read-only context for accessing shared data
    ///
    /// # Returns
    /// * `AssertionResult` indicating success/failure with optional message
    ///
    /// # Default Implementation
    /// Returns an error for unknown assertions.
    fn call_assertion(&self, name: &str, _params: Value, _ctx: &Context) -> AssertionResult {
        AssertionResult::error(format!("Unknown assertion: {}", name))
    }

    /// Call a lifecycle hook
    ///
    /// Hooks are called at specific points during workflow execution:
    /// - `before_all`: Before any jobs run
    /// - `after_all`: After all jobs complete
    /// - `before_each`: Before each step
    /// - `after_each`: After each step
    ///
    /// # Arguments
    /// * `hook` - The hook name
    /// * `ctx` - Mutable context
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(String)` to abort the workflow
    ///
    /// # Default Implementation
    /// No-op (returns Ok).
    fn call_hook(&self, _hook: &str, _ctx: &mut Context) -> Result<(), String> {
        Ok(())
    }

    /// List all available assertions
    ///
    /// Override this to provide assertion discovery for validation
    /// and documentation.
    ///
    /// # Default Implementation
    /// Returns an empty list.
    fn list_assertions(&self) -> Vec<FunctionInfo> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_info() {
        let info = FunctionInfo::new("test_fn", "A test function");
        assert_eq!(info.name, "test_fn");
        assert_eq!(info.description, "A test function");
    }

    #[test]
    fn test_assertion_result_pass() {
        let result = AssertionResult::pass();
        assert!(result.success);
        assert!(result.message.is_none());
    }

    #[test]
    fn test_assertion_result_fail() {
        let result = AssertionResult::fail("expected true");
        assert!(!result.success);
        assert_eq!(result.message, Some("expected true".to_string()));
    }

    #[test]
    fn test_assertion_result_with_values() {
        let result = AssertionResult::fail_with_values(
            "values differ",
            Value::Number(1.into()),
            Value::Number(2.into()),
        );
        assert!(!result.success);
        assert_eq!(result.actual, Some(Value::Number(1.into())));
        assert_eq!(result.expected, Some(Value::Number(2.into())));
    }

    struct TestRegistry;

    impl RustRegistry for TestRegistry {
        fn call(&self, name: &str, _args: Value, _ctx: &mut Context) -> Result<Value, String> {
            match name {
                "ping" => Ok(Value::String("pong".to_string())),
                _ => Err(format!("Unknown: {}", name)),
            }
        }

        fn list_functions(&self) -> Vec<FunctionInfo> {
            vec![FunctionInfo::new("ping", "Returns pong")]
        }
    }

    #[test]
    fn test_registry_call() {
        let reg = TestRegistry;
        let mut ctx = Context::new();

        let result = reg.call("ping", Value::Null, &mut ctx);
        assert_eq!(result, Ok(Value::String("pong".to_string())));

        let result = reg.call("unknown", Value::Null, &mut ctx);
        assert!(result.is_err());
    }

    #[test]
    fn test_registry_list_functions() {
        let reg = TestRegistry;
        let funcs = reg.list_functions();
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "ping");
    }

    #[test]
    fn test_registry_default_assertion() {
        let reg = TestRegistry;
        let ctx = Context::new();
        let result = reg.call_assertion("any", Value::Null, &ctx);
        assert!(!result.success);
        assert!(result.message.unwrap().contains("Unknown assertion"));
    }

    #[test]
    fn test_registry_default_hook() {
        let reg = TestRegistry;
        let mut ctx = Context::new();
        let result = reg.call_hook("before_all", &mut ctx);
        assert!(result.is_ok());
    }
}
