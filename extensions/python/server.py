#!/usr/bin/env python3
"""
Python Bridge Server

JSON-RPC server that loads and executes user-defined Python functions.
Communicates via stdin/stdout with newline-delimited JSON.

Usage:
    python server.py --registry path/to/registry.py
"""

import sys
import json
import argparse
import importlib.util
import traceback
from typing import Any, Dict, Optional
from pathlib import Path


class Context:
    """Shared context for function calls."""

    def __init__(self):
        self._data: Dict[str, Any] = {}
        self._steps: Dict[str, Dict[str, Any]] = {}
        self.run_id: str = ""
        self.job_name: str = ""
        self.step_name: str = ""
        self.clock: Optional[Dict[str, Any]] = None

    def now(self):
        """Get current time (respects mock clock if set)."""
        from datetime import datetime
        if self.clock and self.clock.get("virtual_time_ms"):
            return datetime.fromtimestamp(self.clock["virtual_time_ms"] / 1000)
        return datetime.now()

    def is_clock_mocked(self) -> bool:
        """Check if mock clock is active."""
        return self.clock is not None and self.clock.get("virtual_time_ms") is not None

    def get(self, key: str) -> Optional[Any]:
        """Get a value from context."""
        return self._data.get(key)

    def set(self, key: str, value: Any) -> None:
        """Set a value in context."""
        self._data[key] = value

    def remove(self, key: str) -> bool:
        """Remove a value from context."""
        if key in self._data:
            del self._data[key]
            return True
        return False

    def clear(self, pattern: str = "*") -> int:
        """Clear values matching pattern. Returns count of cleared items."""
        import fnmatch
        to_delete = [k for k in self._data.keys() if fnmatch.fnmatch(k, pattern)]
        for key in to_delete:
            del self._data[key]
        return len(to_delete)

    def get_step_output(self, step_id: str, output_name: str) -> Optional[str]:
        """Get output from a previous step."""
        step = self._steps.get(step_id, {})
        return step.get("outputs", {}).get(output_name)


class Registry:
    """Registry of user-defined functions, assertions, and hooks."""

    def __init__(self):
        self.functions: Dict[str, callable] = {}
        self.assertions: Dict[str, callable] = {}
        self.hooks: Dict[str, callable] = {}

    def function(self, name: str = None):
        """Decorator to register a function."""
        def decorator(fn):
            self.functions[name or fn.__name__] = fn
            return fn
        return decorator

    def assertion(self, name: str = None):
        """Decorator to register an assertion."""
        def decorator(fn):
            self.assertions[name or fn.__name__] = fn
            return fn
        return decorator

    def hook(self, name: str):
        """Decorator to register a lifecycle hook."""
        def decorator(fn):
            self.hooks[name] = fn
            return fn
        return decorator


# Global registry instance for user code
registry = Registry()


def load_registry(path: str) -> Registry:
    """Load user's registry module."""
    path = Path(path).resolve()

    if not path.exists():
        raise FileNotFoundError(f"Registry file not found: {path}")

    spec = importlib.util.spec_from_file_location("user_registry", path)
    if spec is None or spec.loader is None:
        raise ImportError(f"Could not load registry from: {path}")

    module = importlib.util.module_from_spec(spec)

    # Make our registry available to the user module
    sys.modules['testing_actions'] = type(sys)('testing_actions')
    sys.modules['testing_actions'].registry = registry
    sys.modules['testing_actions'].Registry = Registry
    sys.modules['testing_actions'].Context = Context

    spec.loader.exec_module(module)

    # Check if user defined their own registry
    if hasattr(module, 'registry') and isinstance(module.registry, Registry):
        return module.registry

    # Check for functions/assertions/hooks dicts
    if hasattr(module, 'functions'):
        for name, fn in module.functions.items():
            registry.functions[name] = fn
    if hasattr(module, 'assertions'):
        for name, fn in module.assertions.items():
            registry.assertions[name] = fn
    if hasattr(module, 'hooks'):
        for name, fn in module.hooks.items():
            registry.hooks[name] = fn

    return registry


def json_rpc_success(id: Any, result: Any) -> str:
    """Create a successful JSON-RPC response."""
    return json.dumps({"jsonrpc": "2.0", "id": id, "result": result})


def json_rpc_error(id: Any, code: int, message: str) -> str:
    """Create an error JSON-RPC response."""
    return json.dumps({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})


def main():
    parser = argparse.ArgumentParser(description="Python Bridge Server")
    parser.add_argument("--registry", required=True, help="Path to registry.py")
    args = parser.parse_args()

    # Load user registry
    try:
        user_registry = load_registry(args.registry)
        print(f"Loaded registry from: {args.registry}", file=sys.stderr)
        print(f"  Functions: {', '.join(user_registry.functions.keys()) or '(none)'}", file=sys.stderr)
        print(f"  Assertions: {', '.join(user_registry.assertions.keys()) or '(none)'}", file=sys.stderr)
        print(f"  Hooks: {', '.join(user_registry.hooks.keys()) or '(none)'}", file=sys.stderr)
    except Exception as e:
        print(f"Failed to load registry: {e}", file=sys.stderr)
        sys.exit(1)

    # Shared context
    ctx = Context()

    # Method handlers
    def handle_fn_call(params: dict) -> dict:
        name = params.get("name")
        args = params.get("args")

        fn = user_registry.functions.get(name)
        if not fn:
            available = ", ".join(user_registry.functions.keys())
            raise ValueError(f"Function not found: {name}. Available: {available}")

        result = fn(args, ctx)
        return {"result": result}

    def handle_ctx_get(params: dict) -> dict:
        key = params.get("key")
        value = ctx.get(key)
        return {"value": value}

    def handle_ctx_set(params: dict) -> dict:
        key = params.get("key")
        value = params.get("value")
        ctx.set(key, value)
        return {}

    def handle_ctx_clear(params: dict) -> dict:
        pattern = params.get("pattern", "*")
        cleared = ctx.clear(pattern)
        return {"cleared": cleared}

    def handle_ctx_set_execution_info(params: dict) -> dict:
        ctx.run_id = params.get("runId", "")
        ctx.job_name = params.get("jobName", "")
        ctx.step_name = params.get("stepName", "")
        return {}

    def handle_ctx_sync_step_outputs(params: dict) -> dict:
        step_id = params.get("stepId")
        outputs = params.get("outputs", {})
        if step_id not in ctx._steps:
            ctx._steps[step_id] = {"outputs": {}}
        ctx._steps[step_id]["outputs"].update(outputs)
        return {}

    def handle_hook_call(params: dict) -> dict:
        hook_name = params.get("hook")
        hook_fn = user_registry.hooks.get(hook_name)
        if hook_fn:
            hook_fn(ctx)
        return {}

    def handle_assert_custom(params: dict) -> dict:
        name = params.get("name")
        assertion_params = params.get("params", {})

        assertion_fn = user_registry.assertions.get(name)
        if not assertion_fn:
            available = ", ".join(user_registry.assertions.keys())
            return {
                "success": False,
                "message": f"Assertion not found: {name}. Available: {available}"
            }

        try:
            result = assertion_fn(assertion_params, ctx)
            if isinstance(result, dict):
                return {
                    "success": result.get("success", True),
                    "message": result.get("message"),
                    "actual": result.get("actual"),
                    "expected": result.get("expected"),
                }
            elif isinstance(result, bool):
                return {"success": result}
            else:
                return {"success": bool(result)}
        except Exception as e:
            return {"success": False, "message": str(e)}

    def handle_list_functions(params: dict) -> dict:
        functions = [
            {"name": name, "description": getattr(fn, "__doc__", "") or ""}
            for name, fn in user_registry.functions.items()
        ]
        return {"functions": functions}

    def handle_list_assertions(params: dict) -> dict:
        assertions = [
            {"name": name, "description": getattr(fn, "__doc__", "") or ""}
            for name, fn in user_registry.assertions.items()
        ]
        return {"assertions": assertions}

    def handle_clock_sync(params: dict) -> dict:
        ctx.clock = {
            "virtual_time_ms": params.get("virtual_time_ms"),
            "virtual_time_iso": params.get("virtual_time_iso"),
            "frozen": params.get("frozen"),
        }
        return {}

    methods = {
        "fn.call": handle_fn_call,
        "ctx.get": handle_ctx_get,
        "ctx.set": handle_ctx_set,
        "ctx.clear": handle_ctx_clear,
        "ctx.setExecutionInfo": handle_ctx_set_execution_info,
        "ctx.syncStepOutputs": handle_ctx_sync_step_outputs,
        "hook.call": handle_hook_call,
        "assert.custom": handle_assert_custom,
        "list_functions": handle_list_functions,
        "list_assertions": handle_list_assertions,
        "clock.sync": handle_clock_sync,
    }

    print("Python bridge server started", file=sys.stderr)

    # Main loop
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue

        try:
            request = json.loads(line)
        except json.JSONDecodeError as e:
            print(f"Invalid JSON: {line}", file=sys.stderr)
            continue

        request_id = request.get("id")
        method = request.get("method")
        params = request.get("params", {})

        handler = methods.get(method)
        if not handler:
            print(json_rpc_error(request_id, -32601, f"Method not found: {method}"))
            sys.stdout.flush()
            continue

        try:
            result = handler(params)
            print(json_rpc_success(request_id, result))
        except Exception as e:
            traceback.print_exc(file=sys.stderr)
            print(json_rpc_error(request_id, -32000, str(e)))

        sys.stdout.flush()


if __name__ == "__main__":
    main()
