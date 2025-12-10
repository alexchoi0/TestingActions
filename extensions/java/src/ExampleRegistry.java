package com.playwrightactions.example;

import com.playwrightactions.bridge.*;
import java.util.Arrays;
import java.util.List;

/**
 * Example Java Registry
 *
 * This file demonstrates how to implement the Registry interface
 * for the Java bridge.
 *
 * Usage in workflow YAML:
 *     platforms:
 *       java:
 *         classpath: ./target/classes:./lib/*
 *         registry: com.playwrightactions.example.ExampleRegistry
 */
public class ExampleRegistry implements Registry {

    @Override
    public Object call(String name, Object args, Context ctx) throws Exception {
        JsonObject params = args instanceof JsonObject ? (JsonObject) args : new JsonObject();

        switch (name) {
            case "add": {
                Number a = (Number) params.getOrDefault("a", 0);
                Number b = (Number) params.getOrDefault("b", 0);
                return a.doubleValue() + b.doubleValue();
            }

            case "multiply": {
                Number a = (Number) params.getOrDefault("a", 0);
                Number b = (Number) params.getOrDefault("b", 0);
                return a.doubleValue() * b.doubleValue();
            }

            case "greet": {
                String greeting = (String) params.getOrDefault("name", "World");
                return "Hello, " + greeting + "!";
            }

            case "store_and_retrieve": {
                String key = (String) params.get("key");
                Object value = params.get("value");
                ctx.set(key, value);
                return ctx.get(key);
            }

            default:
                throw new RuntimeException("Unknown function: " + name);
        }
    }

    @Override
    public List<String> listMethods() {
        return Arrays.asList("add", "multiply", "greet", "store_and_retrieve");
    }

    @Override
    public List<String> listAssertions() {
        return Arrays.asList("is_positive", "equals");
    }

    @Override
    public AssertionResult callAssertion(String name, JsonObject params, Context ctx) {
        switch (name) {
            case "is_positive": {
                Number value = (Number) params.getOrDefault("value", 0);
                boolean success = value.doubleValue() > 0;
                return new AssertionResult(
                    success,
                    "Value must be positive",
                    value,
                    "> 0"
                );
            }

            case "equals": {
                Object actual = params.get("actual");
                Object expected = params.get("expected");
                boolean success = actual != null && actual.equals(expected);
                return new AssertionResult(
                    success,
                    success ? null : "Values do not match",
                    actual,
                    expected
                );
            }

            default:
                return AssertionResult.fail("Unknown assertion: " + name);
        }
    }

    @Override
    public void callHook(String hook, Context ctx) {
        switch (hook) {
            case "before_all":
                System.err.println("Setting up test environment...");
                ctx.set("setup_complete", true);
                break;

            case "after_all":
                System.err.println("Tearing down test environment...");
                ctx.clear("*");
                break;
        }
    }
}
