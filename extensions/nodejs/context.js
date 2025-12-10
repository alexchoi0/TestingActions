/**
 * BridgeContext - Shared context object for Node.js bridge
 *
 * This context is passed to all functions and provides:
 * - Key-value storage for sharing data between steps
 * - Step outputs (read-only, synced from Rust executor)
 * - Environment variables
 * - Execution info (run ID, job name, step name)
 */

class BridgeContext {
    constructor() {
        // Private storage
        this._store = new Map();

        // Step outputs (synced from executor)
        this.steps = {};

        // Environment (read-only from process.env)
        this.env = { ...process.env };

        // Execution info
        this.runId = '';
        this.jobName = '';
        this.stepName = '';

        // Mock clock (null means use real time)
        this.clock = null;
    }

    /**
     * Get current time (respects mock clock if set)
     * @returns {Date} Current date/time
     */
    now() {
        if (this.clock && this.clock.virtualTimeMs) {
            return new Date(this.clock.virtualTimeMs);
        }
        return new Date();
    }

    /**
     * Check if mock clock is active
     * @returns {boolean} True if using virtual time
     */
    isClockMocked() {
        return this.clock && this.clock.virtualTimeMs != null;
    }

    /**
     * Get a value from the context store
     * @param {string} key - The key to retrieve
     * @returns {any} The value, or undefined if not found
     */
    get(key) {
        return this._store.get(key);
    }

    /**
     * Set a value in the context store
     * @param {string} key - The key to set
     * @param {any} value - The value to store
     */
    set(key, value) {
        this._store.set(key, value);
    }

    /**
     * Check if a key exists in the context store
     * @param {string} key - The key to check
     * @returns {boolean} True if the key exists
     */
    has(key) {
        return this._store.has(key);
    }

    /**
     * Delete a key from the context store
     * @param {string} key - The key to delete
     * @returns {boolean} True if the key was deleted
     */
    delete(key) {
        return this._store.delete(key);
    }

    /**
     * Clear context values matching a pattern
     * @param {string} [pattern] - Glob pattern (* for wildcard), or clear all if not provided
     * @returns {number} Number of entries cleared
     */
    clear(pattern) {
        if (!pattern || pattern === '*') {
            const count = this._store.size;
            this._store.clear();
            return count;
        }

        // Convert glob pattern to regex
        const regex = new RegExp('^' + pattern.replace(/\*/g, '.*') + '$');
        let cleared = 0;

        for (const key of this._store.keys()) {
            if (regex.test(key)) {
                this._store.delete(key);
                cleared++;
            }
        }

        return cleared;
    }

    /**
     * Get all keys in the context store
     * @returns {string[]} Array of keys
     */
    keys() {
        return Array.from(this._store.keys());
    }

    /**
     * Get all entries in the context store
     * @returns {[string, any][]} Array of [key, value] pairs
     */
    entries() {
        return Array.from(this._store.entries());
    }

    /**
     * Get the size of the context store
     * @returns {number} Number of entries
     */
    get size() {
        return this._store.size;
    }

    /**
     * Get step output value
     * @param {string} stepId - The step ID
     * @param {string} outputKey - The output key
     * @returns {any} The output value, or undefined
     */
    getStepOutput(stepId, outputKey) {
        return this.steps[stepId]?.outputs?.[outputKey];
    }

    /**
     * Serialize context for debugging
     * @returns {object} Serialized context
     */
    toJSON() {
        return {
            store: Object.fromEntries(this._store),
            steps: this.steps,
            runId: this.runId,
            jobName: this.jobName,
            stepName: this.stepName,
        };
    }
}

module.exports = BridgeContext;
