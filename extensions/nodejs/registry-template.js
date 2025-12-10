/**
 * Bridge Registry Template (JavaScript)
 *
 * This file exports the functions, assertions, and hooks that can be called
 * from workflow YAML files using the Node.js platform.
 *
 * Usage:
 * 1. Copy this file to your project (e.g., test/bridge-registry.js)
 * 2. Add your functions, assertions, and hooks
 * 3. Reference it in your workflow:
 *
 *    platform: nodejs
 *    nodejs:
 *      registry: ./test/bridge-registry.js
 */

// Import your service functions
// const { createUser, getUserByEmail, deleteUser } = require('../src/services/user');
// const { processOrder, getCart } = require('../src/services/order');
// const db = require('../src/db');

module.exports = {
  /**
   * Functions that can be called with fn/call action
   *
   * Each function receives:
   * - args: The arguments passed from the workflow (from `with.args`)
   * - ctx: The BridgeContext object for sharing state between steps
   *
   * Functions can be async and should return any JSON-serializable value.
   */
  functions: {
    // Example: Simple function
    add: async (args, ctx) => {
      return args.a + args.b;
    },

    // Example: Function that uses context
    createUser: async (args, ctx) => {
      // const user = await db.users.create(args);
      const user = { id: Date.now(), ...args };
      ctx.set('lastCreatedUser', user);
      return user;
    },

    // Example: Function that reads context
    getLastUser: async (args, ctx) => {
      return ctx.get('lastCreatedUser') || null;
    },

    // Example: Function that can throw
    validateEmail: async (args, ctx) => {
      if (!args.email || !args.email.includes('@')) {
        throw new Error('Invalid email format');
      }
      return { valid: true, email: args.email };
    },
  },

  /**
   * Custom assertions that can be called with assert/<name> action
   *
   * Each assertion receives:
   * - params: All parameters from the workflow step's `with` block
   * - ctx: The BridgeContext object
   *
   * Must return an object with:
   * - success: boolean - whether the assertion passed
   * - message: string (optional) - error message if failed
   * - actual: any (optional) - the actual value found
   * - expected: any (optional) - the expected value
   */
  assertions: {
    // Example: Check if user exists
    userExists: async (params, ctx) => {
      const user = ctx.get('lastCreatedUser');
      const exists = user && user.id === params.userId;
      return {
        success: exists,
        message: exists ? undefined : `User ${params.userId} not found`,
        actual: user?.id,
        expected: params.userId,
      };
    },

    // Example: Check array length
    arrayLength: async (params, ctx) => {
      const arr = params.array || [];
      const expected = params.length;
      return {
        success: arr.length === expected,
        message: arr.length !== expected
          ? `Expected array length ${expected}, got ${arr.length}`
          : undefined,
        actual: arr.length,
        expected: expected,
      };
    },

    // Example: Check value in range
    inRange: async (params, ctx) => {
      const { value, min, max } = params;
      const inRange = value >= min && value <= max;
      return {
        success: inRange,
        message: inRange ? undefined : `${value} is not in range [${min}, ${max}]`,
        actual: value,
        expected: `${min}-${max}`,
      };
    },
  },

  /**
   * Lifecycle hooks called at various points during execution
   *
   * Each hook receives:
   * - ctx: The BridgeContext object
   *
   * Available hooks:
   * - beforeAll: Called once before any jobs run
   * - afterAll: Called once after all jobs complete
   * - beforeEach: Called before each step
   * - afterEach: Called after each step
   */
  hooks: {
    beforeAll: async (ctx) => {
      console.log('[hook] beforeAll - Setting up test environment');
      // await db.connect();
      // await db.migrate();
      ctx.set('testStartTime', Date.now());
    },

    afterAll: async (ctx) => {
      const duration = Date.now() - (ctx.get('testStartTime') || 0);
      console.log(`[hook] afterAll - Tests completed in ${duration}ms`);
      // await db.disconnect();
    },

    beforeEach: async (ctx) => {
      console.log(`[hook] beforeEach - Step: ${ctx.stepName}`);
      // await db.beginTransaction();
    },

    afterEach: async (ctx) => {
      console.log(`[hook] afterEach - Step: ${ctx.stepName}`);
      // await db.rollbackTransaction();
    },
  },
};
