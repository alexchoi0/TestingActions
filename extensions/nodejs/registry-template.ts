/**
 * Bridge Registry Template (TypeScript)
 *
 * This file exports the functions, assertions, and hooks that can be called
 * from workflow YAML files using the Node.js platform.
 *
 * Usage:
 * 1. Copy this file to your project (e.g., test/bridge-registry.ts)
 * 2. Add your functions, assertions, and hooks
 * 3. Reference it in your workflow:
 *
 *    platform: nodejs
 *    nodejs:
 *      registry: ./test/bridge-registry.ts
 *      typescript: true
 */

// Type definitions
interface BridgeContext {
  get(key: string): unknown;
  set(key: string, value: unknown): void;
  has(key: string): boolean;
  delete(key: string): boolean;
  clear(pattern?: string): number;
  steps: Record<string, { outputs: Record<string, unknown> }>;
  env: Record<string, string>;
  runId: string;
  jobName: string;
  stepName: string;
}

interface AssertionResult {
  success: boolean;
  message?: string;
  actual?: unknown;
  expected?: unknown;
}

type FunctionHandler = (args: Record<string, unknown>, ctx: BridgeContext) => Promise<unknown>;
type AssertionHandler = (params: Record<string, unknown>, ctx: BridgeContext) => Promise<AssertionResult>;
type HookHandler = (ctx: BridgeContext) => Promise<void>;

interface BridgeRegistry {
  functions: Record<string, FunctionHandler>;
  assertions: Record<string, AssertionHandler>;
  hooks: Record<string, HookHandler>;
}

// Import your service functions
// import { createUser, getUserByEmail, deleteUser } from '../src/services/user';
// import { processOrder, getCart } from '../src/services/order';
// import { db } from '../src/db';

// Example types for your domain
interface User {
  id: number;
  email: string;
  name: string;
  role?: string;
}

const registry: BridgeRegistry = {
  /**
   * Functions that can be called with fn/call action
   */
  functions: {
    add: async (args, ctx) => {
      const { a, b } = args as { a: number; b: number };
      return a + b;
    },

    createUser: async (args, ctx) => {
      const { email, name } = args as { email: string; name: string };
      // const user = await db.users.create({ email, name });
      const user: User = { id: Date.now(), email, name };
      ctx.set('lastCreatedUser', user);
      return user;
    },

    getLastUser: async (args, ctx) => {
      return ctx.get('lastCreatedUser') as User | undefined;
    },

    getUserById: async (args, ctx) => {
      const { id } = args as { id: number };
      // return await db.users.findUnique({ where: { id } });
      const lastUser = ctx.get('lastCreatedUser') as User | undefined;
      return lastUser?.id === id ? lastUser : null;
    },

    validateEmail: async (args, ctx) => {
      const { email } = args as { email: string };
      if (!email || !email.includes('@')) {
        throw new Error('Invalid email format');
      }
      return { valid: true, email };
    },
  },

  /**
   * Custom assertions
   */
  assertions: {
    userExists: async (params, ctx) => {
      const { userId } = params as { userId: number };
      const user = ctx.get('lastCreatedUser') as User | undefined;
      const exists = user && user.id === userId;
      return {
        success: exists,
        message: exists ? undefined : `User ${userId} not found`,
        actual: user?.id,
        expected: userId,
      };
    },

    userHasRole: async (params, ctx) => {
      const { userId, role } = params as { userId: number; role: string };
      const user = ctx.get('lastCreatedUser') as User | undefined;
      const hasRole = user && user.id === userId && user.role === role;
      return {
        success: hasRole,
        message: hasRole ? undefined : `User ${userId} does not have role '${role}'`,
        actual: user?.role,
        expected: role,
      };
    },

    arrayLength: async (params, ctx) => {
      const { array, length } = params as { array: unknown[]; length: number };
      const arr = array || [];
      return {
        success: arr.length === length,
        message: arr.length !== length
          ? `Expected array length ${length}, got ${arr.length}`
          : undefined,
        actual: arr.length,
        expected: length,
      };
    },

    inRange: async (params, ctx) => {
      const { value, min, max } = params as { value: number; min: number; max: number };
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
   * Lifecycle hooks
   */
  hooks: {
    beforeAll: async (ctx) => {
      console.log('[hook] beforeAll - Setting up test environment');
      // await db.connect();
      ctx.set('testStartTime', Date.now());
    },

    afterAll: async (ctx) => {
      const startTime = ctx.get('testStartTime') as number | undefined;
      const duration = Date.now() - (startTime || 0);
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

export = registry;
