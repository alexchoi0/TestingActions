package com.playwrightactions.bridge;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.lang.reflect.Method;
import java.util.HashMap;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Java Bridge Server
 *
 * JSON-RPC server that loads and executes user-defined Java methods.
 * Communicates via stdin/stdout with newline-delimited JSON.
 *
 * Usage:
 *     java -cp bridge.jar:user.jar com.playwrightactions.bridge.BridgeServer --registry com.example.MyRegistry
 */
public class BridgeServer {

    private final Registry registry;
    private final Context context;

    public BridgeServer(Registry registry) {
        this.registry = registry;
        this.context = new Context();
    }

    public static void main(String[] args) throws Exception {
        String registryClass = null;

        for (int i = 0; i < args.length; i++) {
            if ("--registry".equals(args[i]) && i + 1 < args.length) {
                registryClass = args[++i];
            }
        }

        if (registryClass == null) {
            System.err.println("Usage: java BridgeServer --registry <class>");
            System.exit(1);
        }

        // Load user's registry class
        Registry registry;
        try {
            Class<?> clazz = Class.forName(registryClass);
            registry = (Registry) clazz.getDeclaredConstructor().newInstance();
            System.err.println("Loaded registry: " + registryClass);
        } catch (Exception e) {
            System.err.println("Failed to load registry: " + e.getMessage());
            e.printStackTrace(System.err);
            System.exit(1);
            return;
        }

        BridgeServer server = new BridgeServer(registry);
        server.run();
    }

    public void run() throws Exception {
        BufferedReader reader = new BufferedReader(new InputStreamReader(System.in));
        PrintStream out = System.out;

        System.err.println("Java bridge server started");

        String line;
        while ((line = reader.readLine()) != null) {
            line = line.trim();
            if (line.isEmpty()) continue;

            try {
                JsonObject request = JsonParser.parse(line);
                Object id = request.get("id");
                String method = (String) request.get("method");
                JsonObject params = (JsonObject) request.getOrDefault("params", new JsonObject());

                try {
                    JsonObject result = handleMethod(method, params);
                    out.println(jsonRpcSuccess(id, result));
                } catch (Exception e) {
                    out.println(jsonRpcError(id, -32000, e.getMessage()));
                }
            } catch (Exception e) {
                System.err.println("Invalid JSON: " + line);
            }
            out.flush();
        }
    }

    private JsonObject handleMethod(String method, JsonObject params) throws Exception {
        switch (method) {
            case "method.call":
                return handleMethodCall(params);
            case "ctx.get":
                return handleCtxGet(params);
            case "ctx.set":
                return handleCtxSet(params);
            case "ctx.clear":
                return handleCtxClear(params);
            case "ctx.setExecutionInfo":
                return handleCtxSetExecutionInfo(params);
            case "ctx.syncStepOutputs":
                return handleCtxSyncStepOutputs(params);
            case "hook.call":
                return handleHookCall(params);
            case "assert.custom":
                return handleAssertCustom(params);
            case "list_methods":
                return handleListMethods(params);
            case "list_assertions":
                return handleListAssertions(params);
            case "clock.sync":
                return handleClockSync(params);
            default:
                throw new RuntimeException("Method not found: " + method);
        }
    }

    private JsonObject handleMethodCall(JsonObject params) throws Exception {
        String name = (String) params.get("name");
        Object args = params.get("args");

        Object result = registry.call(name, args, context);

        JsonObject response = new JsonObject();
        response.put("result", result);
        return response;
    }

    private JsonObject handleCtxGet(JsonObject params) {
        String key = (String) params.get("key");
        Object value = context.get(key);

        JsonObject response = new JsonObject();
        response.put("value", value);
        return response;
    }

    private JsonObject handleCtxSet(JsonObject params) {
        String key = (String) params.get("key");
        Object value = params.get("value");
        context.set(key, value);
        return new JsonObject();
    }

    private JsonObject handleCtxClear(JsonObject params) {
        String pattern = (String) params.getOrDefault("pattern", "*");
        int cleared = context.clear(pattern);

        JsonObject response = new JsonObject();
        response.put("cleared", cleared);
        return response;
    }

    private JsonObject handleCtxSetExecutionInfo(JsonObject params) {
        context.setRunId((String) params.getOrDefault("runId", ""));
        context.setJobName((String) params.getOrDefault("jobName", ""));
        context.setStepName((String) params.getOrDefault("stepName", ""));
        return new JsonObject();
    }

    private JsonObject handleCtxSyncStepOutputs(JsonObject params) {
        String stepId = (String) params.get("stepId");
        JsonObject outputs = (JsonObject) params.getOrDefault("outputs", new JsonObject());
        context.syncStepOutputs(stepId, outputs);
        return new JsonObject();
    }

    private JsonObject handleHookCall(JsonObject params) {
        String hook = (String) params.get("hook");
        registry.callHook(hook, context);
        return new JsonObject();
    }

    private JsonObject handleAssertCustom(JsonObject params) {
        String name = (String) params.get("name");
        JsonObject assertParams = (JsonObject) params.getOrDefault("params", new JsonObject());

        try {
            AssertionResult result = registry.callAssertion(name, assertParams, context);

            JsonObject response = new JsonObject();
            response.put("success", result.isSuccess());
            response.put("message", result.getMessage());
            response.put("actual", result.getActual());
            response.put("expected", result.getExpected());
            return response;
        } catch (Exception e) {
            JsonObject response = new JsonObject();
            response.put("success", false);
            response.put("message", e.getMessage());
            return response;
        }
    }

    private JsonObject handleListMethods(JsonObject params) {
        JsonObject response = new JsonObject();
        response.put("methods", registry.listMethods());
        return response;
    }

    private JsonObject handleListAssertions(JsonObject params) {
        JsonObject response = new JsonObject();
        response.put("assertions", registry.listAssertions());
        return response;
    }

    private JsonObject handleClockSync(JsonObject params) {
        ClockState clock = new ClockState();
        Object virtualTimeMs = params.get("virtual_time_ms");
        if (virtualTimeMs instanceof Number) {
            clock.virtualTimeMs = ((Number) virtualTimeMs).longValue();
        }
        clock.virtualTimeIso = (String) params.get("virtual_time_iso");
        Object frozen = params.get("frozen");
        clock.frozen = frozen instanceof Boolean && (Boolean) frozen;
        context.setClock(clock);
        return new JsonObject();
    }

    private String jsonRpcSuccess(Object id, Object result) {
        JsonObject response = new JsonObject();
        response.put("jsonrpc", "2.0");
        response.put("id", id);
        response.put("result", result);
        return response.toJson();
    }

    private String jsonRpcError(Object id, int code, String message) {
        JsonObject error = new JsonObject();
        error.put("code", code);
        error.put("message", message);

        JsonObject response = new JsonObject();
        response.put("jsonrpc", "2.0");
        response.put("id", id);
        response.put("error", error);
        return response.toJson();
    }
}

/**
 * Simple JSON object implementation (no external dependencies).
 */
class JsonObject extends HashMap<String, Object> {
    public String toJson() {
        StringBuilder sb = new StringBuilder("{");
        boolean first = true;
        for (Map.Entry<String, Object> entry : entrySet()) {
            if (!first) sb.append(",");
            first = false;
            sb.append("\"").append(escape(entry.getKey())).append("\":");
            sb.append(valueToJson(entry.getValue()));
        }
        sb.append("}");
        return sb.toString();
    }

    private String valueToJson(Object value) {
        if (value == null) return "null";
        if (value instanceof String) return "\"" + escape((String) value) + "\"";
        if (value instanceof Number) return value.toString();
        if (value instanceof Boolean) return value.toString();
        if (value instanceof JsonObject) return ((JsonObject) value).toJson();
        if (value instanceof java.util.List) {
            StringBuilder sb = new StringBuilder("[");
            boolean first = true;
            for (Object item : (java.util.List<?>) value) {
                if (!first) sb.append(",");
                first = false;
                sb.append(valueToJson(item));
            }
            sb.append("]");
            return sb.toString();
        }
        return "\"" + escape(value.toString()) + "\"";
    }

    private String escape(String s) {
        return s.replace("\\", "\\\\")
                .replace("\"", "\\\"")
                .replace("\n", "\\n")
                .replace("\r", "\\r")
                .replace("\t", "\\t");
    }
}

/**
 * Simple JSON parser (no external dependencies).
 */
class JsonParser {
    private final String json;
    private int pos;

    private JsonParser(String json) {
        this.json = json;
        this.pos = 0;
    }

    public static JsonObject parse(String json) {
        return new JsonParser(json).parseObject();
    }

    private JsonObject parseObject() {
        JsonObject obj = new JsonObject();
        skipWhitespace();
        expect('{');
        skipWhitespace();

        if (peek() != '}') {
            do {
                skipWhitespace();
                String key = parseString();
                skipWhitespace();
                expect(':');
                skipWhitespace();
                Object value = parseValue();
                obj.put(key, value);
                skipWhitespace();
            } while (tryConsume(','));
        }

        expect('}');
        return obj;
    }

    private Object parseValue() {
        skipWhitespace();
        char c = peek();

        if (c == '"') return parseString();
        if (c == '{') return parseObject();
        if (c == '[') return parseArray();
        if (c == 't') { consume("true"); return true; }
        if (c == 'f') { consume("false"); return false; }
        if (c == 'n') { consume("null"); return null; }
        if (c == '-' || Character.isDigit(c)) return parseNumber();

        throw new RuntimeException("Unexpected character: " + c);
    }

    private String parseString() {
        expect('"');
        StringBuilder sb = new StringBuilder();
        while (peek() != '"') {
            char c = next();
            if (c == '\\') {
                char escaped = next();
                switch (escaped) {
                    case 'n': sb.append('\n'); break;
                    case 'r': sb.append('\r'); break;
                    case 't': sb.append('\t'); break;
                    case '"': sb.append('"'); break;
                    case '\\': sb.append('\\'); break;
                    default: sb.append(escaped);
                }
            } else {
                sb.append(c);
            }
        }
        expect('"');
        return sb.toString();
    }

    private java.util.List<Object> parseArray() {
        java.util.List<Object> list = new java.util.ArrayList<>();
        expect('[');
        skipWhitespace();

        if (peek() != ']') {
            do {
                skipWhitespace();
                list.add(parseValue());
                skipWhitespace();
            } while (tryConsume(','));
        }

        expect(']');
        return list;
    }

    private Number parseNumber() {
        int start = pos;
        if (peek() == '-') pos++;
        while (pos < json.length() && (Character.isDigit(json.charAt(pos)) || json.charAt(pos) == '.' || json.charAt(pos) == 'e' || json.charAt(pos) == 'E' || json.charAt(pos) == '+' || json.charAt(pos) == '-')) {
            pos++;
        }
        String numStr = json.substring(start, pos);
        if (numStr.contains(".") || numStr.contains("e") || numStr.contains("E")) {
            return Double.parseDouble(numStr);
        }
        return Long.parseLong(numStr);
    }

    private void skipWhitespace() {
        while (pos < json.length() && Character.isWhitespace(json.charAt(pos))) pos++;
    }

    private char peek() {
        return pos < json.length() ? json.charAt(pos) : '\0';
    }

    private char next() {
        return json.charAt(pos++);
    }

    private void expect(char c) {
        if (next() != c) throw new RuntimeException("Expected '" + c + "'");
    }

    private boolean tryConsume(char c) {
        if (peek() == c) { pos++; return true; }
        return false;
    }

    private void consume(String s) {
        for (char c : s.toCharArray()) expect(c);
    }
}

/**
 * Interface that users implement to register their functions.
 */
interface Registry {
    /**
     * Call a registered function by name.
     */
    Object call(String name, Object args, Context ctx) throws Exception;

    /**
     * List all registered function names.
     */
    java.util.List<String> listMethods();

    /**
     * List all registered assertion names.
     */
    default java.util.List<String> listAssertions() {
        return java.util.Collections.emptyList();
    }

    /**
     * Call a lifecycle hook.
     */
    default void callHook(String hook, Context ctx) {
        // Default: no-op
    }

    /**
     * Call a custom assertion.
     */
    default AssertionResult callAssertion(String name, JsonObject params, Context ctx) {
        return new AssertionResult(false, "Assertion not found: " + name, null, null);
    }
}

/**
 * Shared context for function calls.
 */
class ClockState {
    public Long virtualTimeMs;
    public String virtualTimeIso;
    public boolean frozen;
}

class Context {
    private final ConcurrentHashMap<String, Object> data = new ConcurrentHashMap<>();
    private final ConcurrentHashMap<String, JsonObject> stepOutputs = new ConcurrentHashMap<>();
    private String runId = "";
    private String jobName = "";
    private String stepName = "";
    private ClockState clock = null;

    public ClockState getClock() { return clock; }
    public void setClock(ClockState clock) { this.clock = clock; }

    public java.util.Date now() {
        if (clock != null && clock.virtualTimeMs != null) {
            return new java.util.Date(clock.virtualTimeMs);
        }
        return new java.util.Date();
    }

    public boolean isClockMocked() {
        return clock != null && clock.virtualTimeMs != null;
    }

    public Object get(String key) {
        return data.get(key);
    }

    public void set(String key, Object value) {
        data.put(key, value);
    }

    public boolean remove(String key) {
        return data.remove(key) != null;
    }

    public int clear(String pattern) {
        if ("*".equals(pattern)) {
            int size = data.size();
            data.clear();
            return size;
        }
        String regex = pattern.replace("*", ".*").replace("?", ".");
        java.util.List<String> toRemove = new java.util.ArrayList<>();
        for (String key : data.keySet()) {
            if (key.matches(regex)) {
                toRemove.add(key);
            }
        }
        for (String key : toRemove) {
            data.remove(key);
        }
        return toRemove.size();
    }

    public String getRunId() { return runId; }
    public void setRunId(String runId) { this.runId = runId; }

    public String getJobName() { return jobName; }
    public void setJobName(String jobName) { this.jobName = jobName; }

    public String getStepName() { return stepName; }
    public void setStepName(String stepName) { this.stepName = stepName; }

    public void syncStepOutputs(String stepId, JsonObject outputs) {
        stepOutputs.put(stepId, outputs);
    }

    public Object getStepOutput(String stepId, String outputName) {
        JsonObject outputs = stepOutputs.get(stepId);
        if (outputs != null) {
            return outputs.get(outputName);
        }
        return null;
    }
}

/**
 * Result of a custom assertion.
 */
class AssertionResult {
    private final boolean success;
    private final String message;
    private final Object actual;
    private final Object expected;

    public AssertionResult(boolean success, String message, Object actual, Object expected) {
        this.success = success;
        this.message = message;
        this.actual = actual;
        this.expected = expected;
    }

    public boolean isSuccess() { return success; }
    public String getMessage() { return message; }
    public Object getActual() { return actual; }
    public Object getExpected() { return expected; }

    public static AssertionResult ok() {
        return new AssertionResult(true, null, null, null);
    }

    public static AssertionResult ok(String message) {
        return new AssertionResult(true, message, null, null);
    }

    public static AssertionResult fail(String message) {
        return new AssertionResult(false, message, null, null);
    }

    public static AssertionResult fail(String message, Object actual, Object expected) {
        return new AssertionResult(false, message, actual, expected);
    }
}
