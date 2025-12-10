"""
Example Python Registry

This file demonstrates how to define functions, assertions, and hooks
for the Python bridge.

Usage in workflow YAML:
    platforms:
      python:
        script: extensions/python/server.py
        args: ["--registry", "./my_registry.py"]
"""

# You can define functions as a dict
functions = {
    "add": lambda args, ctx: args.get("a", 0) + args.get("b", 0),
    "multiply": lambda args, ctx: args.get("a", 0) * args.get("b", 0),
}


# Or define them as regular functions
def greet(args, ctx):
    """Greet a user by name."""
    name = args.get("name", "World")
    return f"Hello, {name}!"


def store_and_retrieve(args, ctx):
    """Store a value in context and return it."""
    key = args.get("key")
    value = args.get("value")
    ctx.set(key, value)
    return ctx.get(key)


# Add to the functions dict
functions["greet"] = greet
functions["store_and_retrieve"] = store_and_retrieve


# Define assertions
assertions = {
    "is_positive": lambda params, ctx: {
        "success": params.get("value", 0) > 0,
        "message": "Value must be positive",
        "actual": params.get("value"),
        "expected": "> 0"
    },
    "equals": lambda params, ctx: {
        "success": params.get("actual") == params.get("expected"),
        "actual": params.get("actual"),
        "expected": params.get("expected"),
    },
}


# Define hooks
def setup(ctx):
    """Called before tests run."""
    print("Setting up test environment...")
    ctx.set("setup_complete", True)


def teardown(ctx):
    """Called after tests complete."""
    print("Tearing down test environment...")
    ctx.clear("*")


hooks = {
    "before_all": setup,
    "after_all": teardown,
}
