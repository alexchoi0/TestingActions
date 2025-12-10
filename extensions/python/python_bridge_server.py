#!/usr/bin/env python3
"""
Python Bridge Server - JSON-RPC server for testing-actions Python integration.

This script provides a template for implementing a Python function registry
that can be called from workflow YAML files using the `py/call` action.

Usage in workflow YAML:
    python:
      script: ./examples/python_bridge_server.py
      interpreter: python3
      # venv: ./.venv  # optional virtual environment

    jobs:
      test:
        steps:
          - platform: python
            uses: py/call
            with:
              function: add_numbers
              args: '{"a": 1, "b": 2}'
"""

import json
import sys
from typing import Any, Callable, Dict, Optional


class Context:
    """Shared context for function calls."""

    def __init__(self):
        self._data: Dict[str, Any] = {}
        self._step_outputs: Dict[str, Dict[str, str]] = {}
        self.run_id: str = ""
        self.job_name: str = ""
        self.step_name: str = ""

    def get(self, key: str) -> Optional[Any]:
        return self._data.get(key)

    def set(self, key: str, value: Any) -> None:
        self._data[key] = value

    def clear(self, pattern: str) -> int:
        """Clear keys matching pattern (* = all, prefix* = starts with, *suffix = ends with)."""
        if pattern == "*":
            count = len(self._data)
            self._data.clear()
            return count

        count = 0
        keys_to_remove = []

        if pattern.startswith("*") and pattern.endswith("*"):
            # Contains
            substr = pattern[1:-1]
            for key in self._data:
                if substr in key:
                    keys_to_remove.append(key)
        elif pattern.startswith("*"):
            # Ends with
            suffix = pattern[1:]
            for key in self._data:
                if key.endswith(suffix):
                    keys_to_remove.append(key)
        elif pattern.endswith("*"):
            # Starts with
            prefix = pattern[:-1]
            for key in self._data:
                if key.startswith(prefix):
                    keys_to_remove.append(key)
        else:
            # Exact match
            if pattern in self._data:
                keys_to_remove.append(pattern)

        for key in keys_to_remove:
            del self._data[key]
            count += 1

        return count


class AssertionResult:
    """Result of a custom assertion."""

    def __init__(
        self,
        success: bool,
        message: Optional[str] = None,
        actual: Optional[Any] = None,
        expected: Optional[Any] = None,
    ):
        self.success = success
        self.message = message
        self.actual = actual
        self.expected = expected

    def to_dict(self) -> Dict[str, Any]:
        result = {"success": self.success}
        if self.message is not None:
            result["message"] = self.message
        if self.actual is not None:
            result["actual"] = self.actual
        if self.expected is not None:
            result["expected"] = self.expected
        return result

    @staticmethod
    def passed(message: Optional[str] = None) -> "AssertionResult":
        return AssertionResult(success=True, message=message)

    @staticmethod
    def failed(
        message: str,
        actual: Optional[Any] = None,
        expected: Optional[Any] = None,
    ) -> "AssertionResult":
        return AssertionResult(
            success=False, message=message, actual=actual, expected=expected
        )


class FunctionInfo:
    """Information about a registered function."""

    def __init__(self, name: str, description: str):
        self.name = name
        self.description = description

    def to_dict(self) -> Dict[str, str]:
        return {"name": self.name, "description": self.description}


class PythonRegistry:
    """Registry for functions, assertions, and hooks."""

    def __init__(self):
        self._functions: Dict[str, Callable] = {}
        self._function_info: Dict[str, FunctionInfo] = {}
        self._assertions: Dict[str, Callable] = {}
        self._assertion_info: Dict[str, FunctionInfo] = {}
        self._hooks: Dict[str, Callable] = {}
        self.context = Context()

    def function(self, name: str, description: str = ""):
        """Decorator to register a function."""

        def decorator(func: Callable) -> Callable:
            self._functions[name] = func
            self._function_info[name] = FunctionInfo(name, description)
            return func

        return decorator

    def assertion(self, name: str, description: str = ""):
        """Decorator to register an assertion."""

        def decorator(func: Callable) -> Callable:
            self._assertions[name] = func
            self._assertion_info[name] = FunctionInfo(name, description)
            return func

        return decorator

    def hook(self, name: str):
        """Decorator to register a lifecycle hook."""

        def decorator(func: Callable) -> Callable:
            self._hooks[name] = func
            return func

        return decorator

    def call_function(self, name: str, args: Any) -> Any:
        if name not in self._functions:
            raise ValueError(f"Unknown function: {name}")
        return self._functions[name](args, self.context)

    def call_assertion(self, name: str, params: Dict[str, Any]) -> AssertionResult:
        if name not in self._assertions:
            return AssertionResult.failed(f"Unknown assertion: {name}")
        return self._assertions[name](params, self.context)

    def call_hook(self, name: str) -> None:
        if name in self._hooks:
            self._hooks[name](self.context)

    def list_functions(self) -> list:
        return [info.to_dict() for info in self._function_info.values()]

    def list_assertions(self) -> list:
        return [info.to_dict() for info in self._assertion_info.values()]


# Global registry instance
registry = PythonRegistry()


# ==============================================================================
# Example functions - Replace these with your own implementations
# ==============================================================================


@registry.function("add_numbers", "Add two numbers together")
def add_numbers(args: Dict[str, Any], ctx: Context) -> Any:
    a = args.get("a", 0)
    b = args.get("b", 0)
    return {"result": a + b}


@registry.function("greet", "Generate a greeting message")
def greet(args: Dict[str, Any], ctx: Context) -> Any:
    name = args.get("name", "World")
    return {"message": f"Hello, {name}!"}


@registry.function("store_value", "Store a value in context")
def store_value(args: Dict[str, Any], ctx: Context) -> Any:
    key = args.get("key")
    value = args.get("value")
    ctx.set(key, value)
    return {"stored": True}


@registry.function("get_value", "Get a value from context")
def get_value(args: Dict[str, Any], ctx: Context) -> Any:
    key = args.get("key")
    return {"value": ctx.get(key)}


# ==============================================================================
# Example assertions
# ==============================================================================


@registry.assertion("equals", "Assert two values are equal")
def assert_equals(params: Dict[str, Any], ctx: Context) -> AssertionResult:
    actual = params.get("actual")
    expected = params.get("expected")
    if actual == expected:
        return AssertionResult.passed()
    return AssertionResult.failed(
        f"Expected {expected}, got {actual}", actual=actual, expected=expected
    )


@registry.assertion("contains", "Assert string contains substring")
def assert_contains(params: Dict[str, Any], ctx: Context) -> AssertionResult:
    haystack = params.get("haystack", "")
    needle = params.get("needle", "")
    if needle in haystack:
        return AssertionResult.passed()
    return AssertionResult.failed(
        f"'{haystack}' does not contain '{needle}'",
        actual=haystack,
        expected=f"*{needle}*",
    )


# ==============================================================================
# Example hooks
# ==============================================================================


@registry.hook("before_all")
def before_all(ctx: Context) -> None:
    ctx.set("test_started", True)


@registry.hook("after_all")
def after_all(ctx: Context) -> None:
    ctx.set("test_ended", True)


@registry.hook("before_each")
def before_each(ctx: Context) -> None:
    pass


@registry.hook("after_each")
def after_each(ctx: Context) -> None:
    pass


# ==============================================================================
# JSON-RPC Server Implementation
# ==============================================================================


class JsonRpcError(Exception):
    def __init__(self, code: int, message: str):
        self.code = code
        self.message = message


def handle_request(request: Dict[str, Any]) -> Dict[str, Any]:
    """Handle a single JSON-RPC request."""
    method = request.get("method", "")
    params = request.get("params", {})
    request_id = request.get("id")

    try:
        result = dispatch_method(method, params)
        return {"jsonrpc": "2.0", "id": request_id, "result": result}
    except JsonRpcError as e:
        return {
            "jsonrpc": "2.0",
            "id": request_id,
            "error": {"code": e.code, "message": e.message},
        }
    except Exception as e:
        return {
            "jsonrpc": "2.0",
            "id": request_id,
            "error": {"code": -32000, "message": str(e)},
        }


def dispatch_method(method: str, params: Dict[str, Any]) -> Any:
    """Dispatch to the appropriate handler based on method name."""
    if method == "fn.call":
        name = params.get("name")
        args = params.get("args", {})
        result = registry.call_function(name, args)
        return {"result": result}

    elif method == "ctx.get":
        key = params.get("key")
        value = registry.context.get(key)
        return {"value": value}

    elif method == "ctx.set":
        key = params.get("key")
        value = params.get("value")
        registry.context.set(key, value)
        return {}

    elif method == "ctx.clear":
        pattern = params.get("pattern", "*")
        cleared = registry.context.clear(pattern)
        return {"cleared": cleared}

    elif method == "ctx.setExecutionInfo":
        registry.context.run_id = params.get("runId", "")
        registry.context.job_name = params.get("jobName", "")
        registry.context.step_name = params.get("stepName", "")
        return {}

    elif method == "ctx.syncStepOutputs":
        step_id = params.get("stepId")
        outputs = params.get("outputs", {})
        registry.context._step_outputs[step_id] = outputs
        return {}

    elif method == "hook.call":
        hook_name = params.get("hook")
        registry.call_hook(hook_name)
        return {}

    elif method == "assert.custom":
        name = params.get("name")
        assertion_params = params.get("params", {})
        result = registry.call_assertion(name, assertion_params)
        return result.to_dict()

    elif method == "list_functions":
        return {"functions": registry.list_functions()}

    elif method == "list_assertions":
        return {"assertions": registry.list_assertions()}

    else:
        raise JsonRpcError(-32601, f"Method not found: {method}")


def serve():
    """Main server loop - reads JSON-RPC from stdin, writes to stdout."""
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue

        try:
            request = json.loads(line)
            response = handle_request(request)
            print(json.dumps(response), flush=True)
        except json.JSONDecodeError as e:
            error_response = {
                "jsonrpc": "2.0",
                "id": None,
                "error": {"code": -32700, "message": f"Parse error: {e}"},
            }
            print(json.dumps(error_response), flush=True)


if __name__ == "__main__":
    serve()
