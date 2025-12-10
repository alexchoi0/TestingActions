/**
 * Node.js Bridge JSON-RPC Server
 *
 * This server provides direct JavaScript function calls without HTTP overhead.
 * It loads a registry file that exports callable functions, assertions, and hooks.
 *
 * Protocol: JSON-RPC 2.0 over stdin/stdout (newline-delimited)
 */

const readline = require('readline');
const path = require('path');
const fs = require('fs');

// Parse command line arguments
const args = process.argv.slice(2);
let registryPath = './registry.js';
let typescript = false;
let envFile = null;

for (let i = 0; i < args.length; i++) {
    switch (args[i]) {
        case '--registry':
            registryPath = args[++i];
            break;
        case '--typescript':
            typescript = true;
            break;
        case '--env-file':
            envFile = args[++i];
            break;
    }
}

// Load environment file if specified
if (envFile && fs.existsSync(envFile)) {
    try {
        const envContent = fs.readFileSync(envFile, 'utf-8');
        for (const line of envContent.split('\n')) {
            const trimmed = line.trim();
            if (trimmed && !trimmed.startsWith('#')) {
                const [key, ...valueParts] = trimmed.split('=');
                if (key) {
                    process.env[key.trim()] = valueParts.join('=').trim();
                }
            }
        }
        console.error(`Loaded env file: ${envFile}`);
    } catch (e) {
        console.error(`Warning: Failed to load env file: ${e.message}`);
    }
}

// Enable TypeScript if needed
if (typescript) {
    try {
        // Try tsx first (faster)
        require('tsx/cjs');
        console.error('TypeScript enabled via tsx');
    } catch (e1) {
        try {
            // Fall back to ts-node
            require('ts-node/register');
            console.error('TypeScript enabled via ts-node');
        } catch (e2) {
            console.error('Warning: TypeScript requested but neither tsx nor ts-node is available');
            console.error('Install with: npm install -D tsx or npm install -D ts-node typescript');
        }
    }
}

// Load the registry
let registry = { functions: {}, assertions: {}, hooks: {} };
try {
    const absPath = path.resolve(registryPath);
    registry = require(absPath);
    console.error(`Loaded registry from: ${absPath}`);
    console.error(`  Functions: ${Object.keys(registry.functions || {}).join(', ') || '(none)'}`);
    console.error(`  Assertions: ${Object.keys(registry.assertions || {}).join(', ') || '(none)'}`);
    console.error(`  Hooks: ${Object.keys(registry.hooks || {}).join(', ') || '(none)'}`);
} catch (e) {
    console.error(`Failed to load registry from ${registryPath}: ${e.message}`);
    process.exit(1);
}

// Shared context
const BridgeContext = require('./context');
const ctx = new BridgeContext();

// Mock storage
const mocks = new Map();

// JSON-RPC helpers
function success(id, result) {
    console.log(JSON.stringify({ jsonrpc: '2.0', id, result }));
}

function error(id, code, message) {
    console.log(JSON.stringify({ jsonrpc: '2.0', id, error: { code, message } }));
}

// Method handlers
const methods = {
    // Function calls
    'fn.call': async ({ name, args }) => {
        // Check for mock first
        if (mocks.has(name)) {
            const mockValue = mocks.get(name);
            if (typeof mockValue === 'function') {
                return { result: await mockValue(args, ctx) };
            }
            return { result: mockValue };
        }

        const fn = registry.functions?.[name];
        if (!fn) {
            throw new Error(`Function not found: ${name}. Available: ${Object.keys(registry.functions || {}).join(', ')}`);
        }

        if (typeof fn !== 'function') {
            throw new Error(`'${name}' is not a function`);
        }

        const result = await fn(args, ctx);
        return { result };
    },

    // Context operations
    'ctx.get': async ({ key }) => {
        return { value: ctx.get(key) ?? null };
    },

    'ctx.set': async ({ key, value }) => {
        ctx.set(key, value);
        return {};
    },

    'ctx.clear': async ({ pattern }) => {
        const cleared = ctx.clear(pattern);
        return { cleared };
    },

    'ctx.setExecutionInfo': async ({ runId, jobName, stepName }) => {
        ctx.runId = runId;
        ctx.jobName = jobName;
        ctx.stepName = stepName;
        return {};
    },

    'ctx.syncStepOutputs': async ({ stepId, outputs }) => {
        if (!ctx.steps[stepId]) {
            ctx.steps[stepId] = { outputs: {} };
        }
        ctx.steps[stepId].outputs = { ...ctx.steps[stepId].outputs, ...outputs };
        return {};
    },

    // Mock operations
    'mock.set': async ({ target, value }) => {
        mocks.set(target, value);
        return {};
    },

    'mock.clear': async () => {
        mocks.clear();
        return {};
    },

    // Hook calls
    'hook.call': async ({ hook }) => {
        const hookFn = registry.hooks?.[hook];
        if (hookFn && typeof hookFn === 'function') {
            await hookFn(ctx);
        }
        return {};
    },

    // Custom assertions
    'assert.custom': async ({ name, params }) => {
        const assertFn = registry.assertions?.[name];
        if (!assertFn) {
            return {
                success: false,
                message: `Assertion not found: ${name}. Available: ${Object.keys(registry.assertions || {}).join(', ')}`,
            };
        }

        if (typeof assertFn !== 'function') {
            return {
                success: false,
                message: `'${name}' is not a function`,
            };
        }

        try {
            const result = await assertFn(params, ctx);
            return {
                success: result.success !== false,
                message: result.message,
                actual: result.actual,
                expected: result.expected,
            };
        } catch (e) {
            return {
                success: false,
                message: `Assertion threw error: ${e.message}`,
            };
        }
    },

    // Registry info
    'registry.info': async () => {
        return {
            functions: Object.keys(registry.functions || {}),
            assertions: Object.keys(registry.assertions || {}),
            hooks: Object.keys(registry.hooks || {}),
        };
    },

    // Clock sync
    'clock.sync': async ({ virtual_time_ms, virtual_time_iso, frozen }) => {
        ctx.clock = {
            virtualTimeMs: virtual_time_ms,
            virtualTimeIso: virtual_time_iso,
            frozen,
            now: () => virtual_time_ms ? new Date(virtual_time_ms) : new Date(),
        };
        return {};
    },
};

// Main loop
const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
    terminal: false,
});

rl.on('line', async (line) => {
    let request;
    try {
        request = JSON.parse(line);
    } catch (e) {
        console.error('Invalid JSON:', line);
        return;
    }

    const { id, method, params } = request;

    const handler = methods[method];
    if (!handler) {
        error(id, -32601, `Method not found: ${method}`);
        return;
    }

    try {
        const result = await handler(params || {});
        success(id, result);
    } catch (e) {
        error(id, -32000, e.message);
    }
});

rl.on('close', () => {
    process.exit(0);
});

// Handle cleanup
process.on('SIGINT', () => process.exit(0));
process.on('SIGTERM', () => process.exit(0));

console.error('Node.js bridge server started');
