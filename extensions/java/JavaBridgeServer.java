/**
 * Java Bridge Server - JSON-RPC server for testing-actions Java integration.
 *
 * This class provides a template for implementing a Java method registry
 * that can be called from workflow YAML files using the `java/call` action.
 *
 * Usage in workflow YAML:
 *     java:
 *       jar: ./target/classes
 *       main_class: JavaBridgeServer
 *       # Or with a JAR file:
 *       # jar: ./target/my-registry.jar
 *       # main_class: com.example.MyRegistry
 *
 *     jobs:
 *       test:
 *         steps:
 *           - platform: java
 *             uses: java/call
 *             with:
 *               method: addNumbers
 *               args: '{"a": 1, "b": 2}'
 *
 * Compile: javac JavaBridgeServer.java
 * Run: java JavaBridgeServer
 */

import java.io.*;
import java.util.*;
import java.util.function.*;
import java.util.regex.*;

public class JavaBridgeServer {

    // =========================================================================
    // Context - Shared state across method calls
    // =========================================================================

    public static class Context {
        private final Map<String, Object> data = new HashMap<>();
        private final Map<String, Map<String, String>> stepOutputs = new HashMap<>();
        private String runId = "";
        private String jobName = "";
        private String stepName = "";

        public Object get(String key) {
            return data.get(key);
        }

        public void set(String key, Object value) {
            data.put(key, value);
        }

        public int clear(String pattern) {
            if (pattern.equals("*")) {
                int count = data.size();
                data.clear();
                return count;
            }

            int count = 0;
            List<String> keysToRemove = new ArrayList<>();

            if (pattern.startsWith("*") && pattern.endsWith("*")) {
                String substr = pattern.substring(1, pattern.length() - 1);
                for (String key : data.keySet()) {
                    if (key.contains(substr)) {
                        keysToRemove.add(key);
                    }
                }
            } else if (pattern.startsWith("*")) {
                String suffix = pattern.substring(1);
                for (String key : data.keySet()) {
                    if (key.endsWith(suffix)) {
                        keysToRemove.add(key);
                    }
                }
            } else if (pattern.endsWith("*")) {
                String prefix = pattern.substring(0, pattern.length() - 1);
                for (String key : data.keySet()) {
                    if (key.startsWith(prefix)) {
                        keysToRemove.add(key);
                    }
                }
            } else {
                if (data.containsKey(pattern)) {
                    keysToRemove.add(pattern);
                }
            }

            for (String key : keysToRemove) {
                data.remove(key);
                count++;
            }

            return count;
        }

        public String getRunId() { return runId; }
        public void setRunId(String runId) { this.runId = runId; }

        public String getJobName() { return jobName; }
        public void setJobName(String jobName) { this.jobName = jobName; }

        public String getStepName() { return stepName; }
        public void setStepName(String stepName) { this.stepName = stepName; }

        public void setStepOutputs(String stepId, Map<String, String> outputs) {
            stepOutputs.put(stepId, outputs);
        }
    }

    // =========================================================================
    // AssertionResult - Result of custom assertions
    // =========================================================================

    public static class AssertionResult {
        public final boolean success;
        public final String message;
        public final Object actual;
        public final Object expected;

        private AssertionResult(boolean success, String message, Object actual, Object expected) {
            this.success = success;
            this.message = message;
            this.actual = actual;
            this.expected = expected;
        }

        public static AssertionResult passed() {
            return new AssertionResult(true, null, null, null);
        }

        public static AssertionResult passed(String message) {
            return new AssertionResult(true, message, null, null);
        }

        public static AssertionResult failed(String message) {
            return new AssertionResult(false, message, null, null);
        }

        public static AssertionResult failed(String message, Object actual, Object expected) {
            return new AssertionResult(false, message, actual, expected);
        }

        public Map<String, Object> toMap() {
            Map<String, Object> result = new HashMap<>();
            result.put("success", success);
            if (message != null) result.put("message", message);
            if (actual != null) result.put("actual", actual);
            if (expected != null) result.put("expected", expected);
            return result;
        }
    }

    // =========================================================================
    // MethodInfo - Information about registered methods
    // =========================================================================

    public static class MethodInfo {
        public final String name;
        public final String description;

        public MethodInfo(String name, String description) {
            this.name = name;
            this.description = description;
        }

        public Map<String, String> toMap() {
            Map<String, String> result = new HashMap<>();
            result.put("name", name);
            result.put("description", description);
            return result;
        }
    }

    // =========================================================================
    // Registry - Method and assertion registration
    // =========================================================================

    private final Map<String, BiFunction<Map<String, Object>, Context, Object>> methods = new HashMap<>();
    private final Map<String, MethodInfo> methodInfos = new HashMap<>();
    private final Map<String, BiFunction<Map<String, Object>, Context, AssertionResult>> assertions = new HashMap<>();
    private final Map<String, MethodInfo> assertionInfos = new HashMap<>();
    private final Map<String, Consumer<Context>> hooks = new HashMap<>();
    private final Context context = new Context();

    public void registerMethod(String name, String description,
                               BiFunction<Map<String, Object>, Context, Object> handler) {
        methods.put(name, handler);
        methodInfos.put(name, new MethodInfo(name, description));
    }

    public void registerAssertion(String name, String description,
                                  BiFunction<Map<String, Object>, Context, AssertionResult> handler) {
        assertions.put(name, handler);
        assertionInfos.put(name, new MethodInfo(name, description));
    }

    public void registerHook(String name, Consumer<Context> handler) {
        hooks.put(name, handler);
    }

    // =========================================================================
    // Example Methods - Replace with your own implementations
    // =========================================================================

    private void registerExampleMethods() {
        registerMethod("addNumbers", "Add two numbers together", (args, ctx) -> {
            int a = ((Number) args.getOrDefault("a", 0)).intValue();
            int b = ((Number) args.getOrDefault("b", 0)).intValue();
            Map<String, Object> result = new HashMap<>();
            result.put("result", a + b);
            return result;
        });

        registerMethod("greet", "Generate a greeting message", (args, ctx) -> {
            String name = (String) args.getOrDefault("name", "World");
            Map<String, Object> result = new HashMap<>();
            result.put("message", "Hello, " + name + "!");
            return result;
        });

        registerMethod("storeValue", "Store a value in context", (args, ctx) -> {
            String key = (String) args.get("key");
            Object value = args.get("value");
            ctx.set(key, value);
            Map<String, Object> result = new HashMap<>();
            result.put("stored", true);
            return result;
        });

        registerMethod("getValue", "Get a value from context", (args, ctx) -> {
            String key = (String) args.get("key");
            Map<String, Object> result = new HashMap<>();
            result.put("value", ctx.get(key));
            return result;
        });
    }

    // =========================================================================
    // Example Assertions
    // =========================================================================

    private void registerExampleAssertions() {
        registerAssertion("equals", "Assert two values are equal", (params, ctx) -> {
            Object actual = params.get("actual");
            Object expected = params.get("expected");
            if (Objects.equals(actual, expected)) {
                return AssertionResult.passed();
            }
            return AssertionResult.failed(
                "Expected " + expected + ", got " + actual,
                actual, expected
            );
        });

        registerAssertion("contains", "Assert string contains substring", (params, ctx) -> {
            String haystack = (String) params.getOrDefault("haystack", "");
            String needle = (String) params.getOrDefault("needle", "");
            if (haystack.contains(needle)) {
                return AssertionResult.passed();
            }
            return AssertionResult.failed(
                "'" + haystack + "' does not contain '" + needle + "'",
                haystack, "*" + needle + "*"
            );
        });
    }

    // =========================================================================
    // Example Hooks
    // =========================================================================

    private void registerExampleHooks() {
        registerHook("before_all", ctx -> {
            ctx.set("test_started", true);
        });

        registerHook("after_all", ctx -> {
            ctx.set("test_ended", true);
        });

        registerHook("before_each", ctx -> {
            // Called before each step
        });

        registerHook("after_each", ctx -> {
            // Called after each step
        });
    }

    // =========================================================================
    // JSON-RPC Server Implementation
    // =========================================================================

    public JavaBridgeServer() {
        registerExampleMethods();
        registerExampleAssertions();
        registerExampleHooks();
    }

    @SuppressWarnings("unchecked")
    public Map<String, Object> handleRequest(Map<String, Object> request) {
        String method = (String) request.get("method");
        Map<String, Object> params = (Map<String, Object>) request.getOrDefault("params", new HashMap<>());
        Object requestId = request.get("id");

        try {
            Object result = dispatchMethod(method, params);
            Map<String, Object> response = new HashMap<>();
            response.put("jsonrpc", "2.0");
            response.put("id", requestId);
            response.put("result", result);
            return response;
        } catch (Exception e) {
            Map<String, Object> response = new HashMap<>();
            response.put("jsonrpc", "2.0");
            response.put("id", requestId);
            Map<String, Object> error = new HashMap<>();
            error.put("code", -32000);
            error.put("message", e.getMessage());
            response.put("error", error);
            return response;
        }
    }

    @SuppressWarnings("unchecked")
    private Object dispatchMethod(String method, Map<String, Object> params) throws Exception {
        switch (method) {
            case "method.call": {
                String name = (String) params.get("name");
                Map<String, Object> args = (Map<String, Object>) params.getOrDefault("args", new HashMap<>());
                BiFunction<Map<String, Object>, Context, Object> handler = methods.get(name);
                if (handler == null) {
                    throw new Exception("Unknown method: " + name);
                }
                Object result = handler.apply(args, context);
                Map<String, Object> response = new HashMap<>();
                response.put("result", result);
                return response;
            }

            case "ctx.get": {
                String key = (String) params.get("key");
                Map<String, Object> response = new HashMap<>();
                response.put("value", context.get(key));
                return response;
            }

            case "ctx.set": {
                String key = (String) params.get("key");
                Object value = params.get("value");
                context.set(key, value);
                return new HashMap<>();
            }

            case "ctx.clear": {
                String pattern = (String) params.getOrDefault("pattern", "*");
                int cleared = context.clear(pattern);
                Map<String, Object> response = new HashMap<>();
                response.put("cleared", cleared);
                return response;
            }

            case "ctx.setExecutionInfo": {
                context.setRunId((String) params.getOrDefault("runId", ""));
                context.setJobName((String) params.getOrDefault("jobName", ""));
                context.setStepName((String) params.getOrDefault("stepName", ""));
                return new HashMap<>();
            }

            case "ctx.syncStepOutputs": {
                String stepId = (String) params.get("stepId");
                Map<String, String> outputs = (Map<String, String>) params.getOrDefault("outputs", new HashMap<>());
                context.setStepOutputs(stepId, outputs);
                return new HashMap<>();
            }

            case "hook.call": {
                String hookName = (String) params.get("hook");
                Consumer<Context> handler = hooks.get(hookName);
                if (handler != null) {
                    handler.accept(context);
                }
                return new HashMap<>();
            }

            case "assert.custom": {
                String name = (String) params.get("name");
                Map<String, Object> assertParams = (Map<String, Object>) params.getOrDefault("params", new HashMap<>());
                BiFunction<Map<String, Object>, Context, AssertionResult> handler = assertions.get(name);
                if (handler == null) {
                    return AssertionResult.failed("Unknown assertion: " + name).toMap();
                }
                return handler.apply(assertParams, context).toMap();
            }

            case "list_methods": {
                List<Map<String, String>> methodList = new ArrayList<>();
                for (MethodInfo info : methodInfos.values()) {
                    methodList.add(info.toMap());
                }
                Map<String, Object> response = new HashMap<>();
                response.put("methods", methodList);
                return response;
            }

            case "list_assertions": {
                List<Map<String, String>> assertionList = new ArrayList<>();
                for (MethodInfo info : assertionInfos.values()) {
                    assertionList.add(info.toMap());
                }
                Map<String, Object> response = new HashMap<>();
                response.put("assertions", assertionList);
                return response;
            }

            default:
                throw new Exception("Method not found: " + method);
        }
    }

    public void serve() throws IOException {
        BufferedReader reader = new BufferedReader(new InputStreamReader(System.in));
        PrintWriter writer = new PrintWriter(System.out, true);

        String line;
        while ((line = reader.readLine()) != null) {
            if (line.trim().isEmpty()) continue;

            try {
                Map<String, Object> request = parseJson(line);
                Map<String, Object> response = handleRequest(request);
                writer.println(toJson(response));
            } catch (Exception e) {
                Map<String, Object> response = new HashMap<>();
                response.put("jsonrpc", "2.0");
                response.put("id", null);
                Map<String, Object> error = new HashMap<>();
                error.put("code", -32700);
                error.put("message", "Parse error: " + e.getMessage());
                response.put("error", error);
                writer.println(toJson(response));
            }
        }
    }

    // =========================================================================
    // Simple JSON Parser/Writer (no external dependencies)
    // =========================================================================

    @SuppressWarnings("unchecked")
    private static Map<String, Object> parseJson(String json) {
        json = json.trim();
        if (!json.startsWith("{")) {
            throw new RuntimeException("Expected object");
        }
        return (Map<String, Object>) parseValue(json, new int[]{0});
    }

    private static Object parseValue(String json, int[] pos) {
        skipWhitespace(json, pos);
        char c = json.charAt(pos[0]);

        if (c == '{') {
            return parseObject(json, pos);
        } else if (c == '[') {
            return parseArray(json, pos);
        } else if (c == '"') {
            return parseString(json, pos);
        } else if (c == 't' || c == 'f') {
            return parseBoolean(json, pos);
        } else if (c == 'n') {
            return parseNull(json, pos);
        } else if (c == '-' || Character.isDigit(c)) {
            return parseNumber(json, pos);
        }
        throw new RuntimeException("Unexpected character: " + c);
    }

    private static Map<String, Object> parseObject(String json, int[] pos) {
        Map<String, Object> map = new HashMap<>();
        pos[0]++; // skip {
        skipWhitespace(json, pos);

        if (json.charAt(pos[0]) == '}') {
            pos[0]++;
            return map;
        }

        while (true) {
            skipWhitespace(json, pos);
            String key = parseString(json, pos);
            skipWhitespace(json, pos);
            if (json.charAt(pos[0]) != ':') throw new RuntimeException("Expected :");
            pos[0]++;
            Object value = parseValue(json, pos);
            map.put(key, value);
            skipWhitespace(json, pos);
            if (json.charAt(pos[0]) == '}') {
                pos[0]++;
                return map;
            }
            if (json.charAt(pos[0]) != ',') throw new RuntimeException("Expected , or }");
            pos[0]++;
        }
    }

    private static List<Object> parseArray(String json, int[] pos) {
        List<Object> list = new ArrayList<>();
        pos[0]++; // skip [
        skipWhitespace(json, pos);

        if (json.charAt(pos[0]) == ']') {
            pos[0]++;
            return list;
        }

        while (true) {
            list.add(parseValue(json, pos));
            skipWhitespace(json, pos);
            if (json.charAt(pos[0]) == ']') {
                pos[0]++;
                return list;
            }
            if (json.charAt(pos[0]) != ',') throw new RuntimeException("Expected , or ]");
            pos[0]++;
        }
    }

    private static String parseString(String json, int[] pos) {
        pos[0]++; // skip opening "
        StringBuilder sb = new StringBuilder();
        while (pos[0] < json.length()) {
            char c = json.charAt(pos[0]);
            if (c == '"') {
                pos[0]++;
                return sb.toString();
            }
            if (c == '\\') {
                pos[0]++;
                c = json.charAt(pos[0]);
                switch (c) {
                    case '"': case '\\': case '/': sb.append(c); break;
                    case 'n': sb.append('\n'); break;
                    case 'r': sb.append('\r'); break;
                    case 't': sb.append('\t'); break;
                    default: sb.append(c);
                }
            } else {
                sb.append(c);
            }
            pos[0]++;
        }
        throw new RuntimeException("Unterminated string");
    }

    private static Number parseNumber(String json, int[] pos) {
        int start = pos[0];
        boolean isFloat = false;
        if (json.charAt(pos[0]) == '-') pos[0]++;
        while (pos[0] < json.length() && Character.isDigit(json.charAt(pos[0]))) pos[0]++;
        if (pos[0] < json.length() && json.charAt(pos[0]) == '.') {
            isFloat = true;
            pos[0]++;
            while (pos[0] < json.length() && Character.isDigit(json.charAt(pos[0]))) pos[0]++;
        }
        if (pos[0] < json.length() && (json.charAt(pos[0]) == 'e' || json.charAt(pos[0]) == 'E')) {
            isFloat = true;
            pos[0]++;
            if (json.charAt(pos[0]) == '+' || json.charAt(pos[0]) == '-') pos[0]++;
            while (pos[0] < json.length() && Character.isDigit(json.charAt(pos[0]))) pos[0]++;
        }
        String numStr = json.substring(start, pos[0]);
        return isFloat ? Double.parseDouble(numStr) : Long.parseLong(numStr);
    }

    private static Boolean parseBoolean(String json, int[] pos) {
        if (json.substring(pos[0]).startsWith("true")) {
            pos[0] += 4;
            return true;
        }
        if (json.substring(pos[0]).startsWith("false")) {
            pos[0] += 5;
            return false;
        }
        throw new RuntimeException("Invalid boolean");
    }

    private static Object parseNull(String json, int[] pos) {
        if (json.substring(pos[0]).startsWith("null")) {
            pos[0] += 4;
            return null;
        }
        throw new RuntimeException("Invalid null");
    }

    private static void skipWhitespace(String json, int[] pos) {
        while (pos[0] < json.length() && Character.isWhitespace(json.charAt(pos[0]))) {
            pos[0]++;
        }
    }

    @SuppressWarnings("unchecked")
    private static String toJson(Object obj) {
        if (obj == null) {
            return "null";
        } else if (obj instanceof Boolean) {
            return obj.toString();
        } else if (obj instanceof Number) {
            return obj.toString();
        } else if (obj instanceof String) {
            return "\"" + escapeString((String) obj) + "\"";
        } else if (obj instanceof List) {
            List<Object> list = (List<Object>) obj;
            StringBuilder sb = new StringBuilder("[");
            for (int i = 0; i < list.size(); i++) {
                if (i > 0) sb.append(",");
                sb.append(toJson(list.get(i)));
            }
            sb.append("]");
            return sb.toString();
        } else if (obj instanceof Map) {
            Map<String, Object> map = (Map<String, Object>) obj;
            StringBuilder sb = new StringBuilder("{");
            boolean first = true;
            for (Map.Entry<String, Object> entry : map.entrySet()) {
                if (!first) sb.append(",");
                first = false;
                sb.append("\"").append(escapeString(entry.getKey())).append("\":");
                sb.append(toJson(entry.getValue()));
            }
            sb.append("}");
            return sb.toString();
        }
        return "\"" + escapeString(obj.toString()) + "\"";
    }

    private static String escapeString(String s) {
        return s.replace("\\", "\\\\")
                .replace("\"", "\\\"")
                .replace("\n", "\\n")
                .replace("\r", "\\r")
                .replace("\t", "\\t");
    }

    // =========================================================================
    // Main Entry Point
    // =========================================================================

    public static void main(String[] args) throws IOException {
        JavaBridgeServer server = new JavaBridgeServer();
        server.serve();
    }
}
