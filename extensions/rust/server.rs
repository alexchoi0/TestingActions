//! JSON-RPC server for the Rust bridge
//!
//! This module provides the `serve()` function that users call in their binary
//! to start the JSON-RPC server. The server reads requests from stdin and
//! writes responses to stdout.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{self, BufRead, Write};

#[allow(unused_imports)]
use super::{AssertionResult, Context, FunctionInfo, RustRegistry};

/// JSON-RPC request structure
#[derive(Debug, Deserialize)]
struct RpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: u64,
    method: String,
    #[serde(default)]
    params: Value,
}

/// JSON-RPC response structure
#[derive(Debug, Serialize)]
struct RpcResponse {
    jsonrpc: &'static str,
    id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
}

/// JSON-RPC error structure
#[derive(Debug, Serialize)]
struct RpcError {
    code: i32,
    message: String,
}

impl RpcResponse {
    fn success(id: u64, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: u64, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
            }),
        }
    }
}

/// JSON-RPC error codes
#[allow(dead_code)]
mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const FUNCTION_ERROR: i32 = -32000;
    pub const ASSERTION_ERROR: i32 = -32001;
    pub const HOOK_ERROR: i32 = -32002;
}

/// Start the JSON-RPC server
///
/// This function blocks forever, reading JSON-RPC requests from stdin
/// and writing responses to stdout. It should be called from the `main()`
/// function of a binary that implements `RustRegistry`.
///
/// # Example
///
/// ```ignore
/// use testing_actions::rust_bridge::{serve, RustRegistry, Context, FunctionInfo};
/// use serde_json::Value;
///
/// struct MyRegistry;
///
/// impl RustRegistry for MyRegistry {
///     fn call(&self, name: &str, args: Value, ctx: &mut Context) -> Result<Value, String> {
///         // ... implementation
///     }
///     fn list_functions(&self) -> Vec<FunctionInfo> {
///         vec![]
///     }
/// }
///
/// fn main() {
///     serve(MyRegistry);
/// }
/// ```
pub fn serve<R: RustRegistry>(registry: R) {
    let mut server = RpcServer::new(registry);
    server.run();
}

struct RpcServer<R: RustRegistry> {
    registry: R,
    context: Context,
}

impl<R: RustRegistry> RpcServer<R> {
    fn new(registry: R) -> Self {
        Self {
            registry,
            context: Context::new(),
        }
    }

    fn run(&mut self) {
        let stdin = io::stdin();
        let stdout = io::stdout();
        let mut stdout = stdout.lock();

        for line in stdin.lock().lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };

            if line.trim().is_empty() {
                continue;
            }

            let response = match serde_json::from_str::<RpcRequest>(&line) {
                Ok(request) => self.handle_request(request),
                Err(e) => RpcResponse::error(0, error_codes::PARSE_ERROR, e.to_string()),
            };

            let json = serde_json::to_string(&response).unwrap();
            if writeln!(stdout, "{}", json).is_err() {
                break;
            }
            if stdout.flush().is_err() {
                break;
            }
        }
    }

    fn handle_request(&mut self, req: RpcRequest) -> RpcResponse {
        match req.method.as_str() {
            "fn.call" => self.handle_fn_call(req.id, req.params),
            "ctx.get" => self.handle_ctx_get(req.id, req.params),
            "ctx.set" => self.handle_ctx_set(req.id, req.params),
            "ctx.clear" => self.handle_ctx_clear(req.id, req.params),
            "ctx.setExecutionInfo" => self.handle_set_execution_info(req.id, req.params),
            "ctx.syncStepOutputs" => self.handle_sync_step_outputs(req.id, req.params),
            "hook.call" => self.handle_hook_call(req.id, req.params),
            "assert.custom" => self.handle_assert_custom(req.id, req.params),
            "list_functions" => self.handle_list_functions(req.id),
            "list_assertions" => self.handle_list_assertions(req.id),
            "clock.sync" => self.handle_clock_sync(req.id, req.params),
            _ => RpcResponse::error(req.id, error_codes::METHOD_NOT_FOUND, "Method not found"),
        }
    }

    fn handle_fn_call(&mut self, id: u64, params: Value) -> RpcResponse {
        let name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => {
                return RpcResponse::error(id, error_codes::INVALID_PARAMS, "Missing 'name' param")
            }
        };

        let args = params.get("args").cloned().unwrap_or(Value::Null);

        match self.registry.call(name, args, &mut self.context) {
            Ok(result) => RpcResponse::success(id, serde_json::json!({ "result": result })),
            Err(e) => RpcResponse::error(id, error_codes::FUNCTION_ERROR, e),
        }
    }

    fn handle_ctx_get(&self, id: u64, params: Value) -> RpcResponse {
        let key = match params.get("key").and_then(|v| v.as_str()) {
            Some(k) => k,
            None => {
                return RpcResponse::error(id, error_codes::INVALID_PARAMS, "Missing 'key' param")
            }
        };

        let value = self.context.get(key).cloned().unwrap_or(Value::Null);
        RpcResponse::success(id, serde_json::json!({ "value": value }))
    }

    fn handle_ctx_set(&mut self, id: u64, params: Value) -> RpcResponse {
        let key = match params.get("key").and_then(|v| v.as_str()) {
            Some(k) => k,
            None => {
                return RpcResponse::error(id, error_codes::INVALID_PARAMS, "Missing 'key' param")
            }
        };

        let value = params.get("value").cloned().unwrap_or(Value::Null);
        self.context.set(key, value);
        RpcResponse::success(id, serde_json::json!({ "ok": true }))
    }

    fn handle_ctx_clear(&mut self, id: u64, params: Value) -> RpcResponse {
        let pattern = params
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("*");

        let cleared = self.context.clear(pattern);
        RpcResponse::success(id, serde_json::json!({ "cleared": cleared }))
    }

    fn handle_set_execution_info(&mut self, id: u64, params: Value) -> RpcResponse {
        let run_id = params.get("runId").and_then(|v| v.as_str()).unwrap_or("");
        let job_name = params.get("jobName").and_then(|v| v.as_str()).unwrap_or("");
        let step_name = params
            .get("stepName")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        self.context.set_execution_info(run_id, job_name, step_name);
        RpcResponse::success(id, serde_json::json!({ "ok": true }))
    }

    fn handle_sync_step_outputs(&mut self, id: u64, params: Value) -> RpcResponse {
        let step_id = match params.get("stepId").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                return RpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing 'stepId' param",
                )
            }
        };

        let outputs: HashMap<String, String> = params
            .get("outputs")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        self.context.set_step_outputs(step_id, outputs);
        RpcResponse::success(id, serde_json::json!({ "ok": true }))
    }

    fn handle_hook_call(&mut self, id: u64, params: Value) -> RpcResponse {
        let hook = match params.get("hook").and_then(|v| v.as_str()) {
            Some(h) => h,
            None => {
                return RpcResponse::error(id, error_codes::INVALID_PARAMS, "Missing 'hook' param")
            }
        };

        match self.registry.call_hook(hook, &mut self.context) {
            Ok(()) => RpcResponse::success(id, serde_json::json!({ "ok": true })),
            Err(e) => RpcResponse::error(id, error_codes::HOOK_ERROR, e),
        }
    }

    fn handle_assert_custom(&self, id: u64, params: Value) -> RpcResponse {
        let name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => {
                return RpcResponse::error(id, error_codes::INVALID_PARAMS, "Missing 'name' param")
            }
        };

        let assertion_params = params.get("params").cloned().unwrap_or(Value::Null);

        let result = self
            .registry
            .call_assertion(name, assertion_params, &self.context);

        RpcResponse::success(
            id,
            serde_json::json!({
                "success": result.success,
                "message": result.message,
                "actual": result.actual,
                "expected": result.expected,
            }),
        )
    }

    fn handle_list_functions(&self, id: u64) -> RpcResponse {
        let functions: Vec<Value> = self
            .registry
            .list_functions()
            .into_iter()
            .map(|f| {
                serde_json::json!({
                    "name": f.name,
                    "description": f.description,
                })
            })
            .collect();

        RpcResponse::success(id, serde_json::json!({ "functions": functions }))
    }

    fn handle_list_assertions(&self, id: u64) -> RpcResponse {
        let assertions: Vec<Value> = self
            .registry
            .list_assertions()
            .into_iter()
            .map(|f| {
                serde_json::json!({
                    "name": f.name,
                    "description": f.description,
                })
            })
            .collect();

        RpcResponse::success(id, serde_json::json!({ "assertions": assertions }))
    }

    fn handle_clock_sync(&mut self, id: u64, params: Value) -> RpcResponse {
        let virtual_time_ms = params.get("virtual_time_ms").and_then(|v| v.as_i64());
        let virtual_time_iso = params
            .get("virtual_time_iso")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let frozen = params
            .get("frozen")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        self.context
            .set_clock(virtual_time_ms, virtual_time_iso, frozen);
        RpcResponse::success(id, serde_json::json!({ "ok": true }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestRegistry;

    impl RustRegistry for TestRegistry {
        fn call(&self, name: &str, args: Value, ctx: &mut Context) -> Result<Value, String> {
            match name {
                "echo" => Ok(args),
                "set_ctx" => {
                    let key = args.get("key").and_then(|v| v.as_str()).unwrap_or("test");
                    let val = args.get("value").cloned().unwrap_or(Value::Null);
                    ctx.set(key, val);
                    Ok(Value::Bool(true))
                }
                "fail" => Err("intentional failure".to_string()),
                _ => Err(format!("Unknown function: {}", name)),
            }
        }

        fn list_functions(&self) -> Vec<FunctionInfo> {
            vec![
                FunctionInfo::new("echo", "Echo the input"),
                FunctionInfo::new("set_ctx", "Set a context value"),
                FunctionInfo::new("fail", "Always fails"),
            ]
        }

        fn call_assertion(&self, name: &str, params: Value, _ctx: &Context) -> AssertionResult {
            match name {
                "is_true" => {
                    let value = params
                        .get("value")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if value {
                        AssertionResult::pass()
                    } else {
                        AssertionResult::fail("Expected true")
                    }
                }
                _ => AssertionResult::error(format!("Unknown assertion: {}", name)),
            }
        }

        fn list_assertions(&self) -> Vec<FunctionInfo> {
            vec![FunctionInfo::new("is_true", "Check if value is true")]
        }
    }

    #[test]
    fn test_rpc_response_success() {
        let resp = RpcResponse::success(1, serde_json::json!({"result": "ok"}));
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_rpc_response_error() {
        let resp = RpcResponse::error(1, -32600, "Invalid request");
        assert!(resp.result.is_none());
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32600);
    }

    #[test]
    fn test_server_fn_call() {
        let mut server = RpcServer::new(TestRegistry);
        let resp = server.handle_fn_call(
            1,
            serde_json::json!({
                "name": "echo",
                "args": {"hello": "world"}
            }),
        );
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert_eq!(
            result.get("result").unwrap().get("hello"),
            Some(&Value::String("world".to_string()))
        );
    }

    #[test]
    fn test_server_fn_call_error() {
        let mut server = RpcServer::new(TestRegistry);
        let resp = server.handle_fn_call(
            1,
            serde_json::json!({
                "name": "fail",
                "args": {}
            }),
        );
        assert!(resp.error.is_some());
        assert!(resp.error.unwrap().message.contains("intentional failure"));
    }

    #[test]
    fn test_server_ctx_operations() {
        let mut server = RpcServer::new(TestRegistry);

        // Set a value
        let resp = server.handle_ctx_set(
            1,
            serde_json::json!({
                "key": "test_key",
                "value": 42
            }),
        );
        assert!(resp.result.is_some());

        // Get the value
        let resp = server.handle_ctx_get(
            2,
            serde_json::json!({
                "key": "test_key"
            }),
        );
        assert_eq!(
            resp.result.unwrap().get("value"),
            Some(&Value::Number(42.into()))
        );

        // Clear
        let resp = server.handle_ctx_clear(3, serde_json::json!({ "pattern": "*" }));
        assert_eq!(
            resp.result.unwrap().get("cleared"),
            Some(&Value::Number(1.into()))
        );
    }

    #[test]
    fn test_server_assert_custom() {
        let server = RpcServer::new(TestRegistry);

        // Passing assertion
        let resp = server.handle_assert_custom(
            1,
            serde_json::json!({
                "name": "is_true",
                "params": { "value": true }
            }),
        );
        let result = resp.result.unwrap();
        assert_eq!(result.get("success"), Some(&Value::Bool(true)));

        // Failing assertion
        let resp = server.handle_assert_custom(
            2,
            serde_json::json!({
                "name": "is_true",
                "params": { "value": false }
            }),
        );
        let result = resp.result.unwrap();
        assert_eq!(result.get("success"), Some(&Value::Bool(false)));
    }

    #[test]
    fn test_server_list_functions() {
        let server = RpcServer::new(TestRegistry);
        let resp = server.handle_list_functions(1);
        let result = resp.result.unwrap();
        let functions = result.get("functions").unwrap().as_array().unwrap();
        assert_eq!(functions.len(), 3);
    }

    #[test]
    fn test_server_list_assertions() {
        let server = RpcServer::new(TestRegistry);
        let resp = server.handle_list_assertions(1);
        let result = resp.result.unwrap();
        let assertions = result.get("assertions").unwrap().as_array().unwrap();
        assert_eq!(assertions.len(), 1);
    }
}
