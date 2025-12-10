//! Shared context for Rust bridge function calls

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Clock state for mock time
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClockState {
    /// Virtual time in milliseconds since Unix epoch
    pub virtual_time_ms: Option<i64>,
    /// Virtual time as ISO 8601 string
    pub virtual_time_iso: Option<String>,
    /// Whether time is frozen
    pub frozen: bool,
}

/// Context shared across function calls within a workflow execution
///
/// The context provides a way to share data between function calls,
/// access step outputs, and get information about the current execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Context {
    /// Arbitrary key-value data store
    data: HashMap<String, Value>,

    /// Step outputs (step_id -> output_name -> value)
    step_outputs: HashMap<String, HashMap<String, String>>,

    /// Current run ID
    run_id: String,

    /// Current job name
    job_name: String,

    /// Current step name
    step_name: String,

    /// Mock clock state
    clock: Option<ClockState>,
}

impl Context {
    /// Create a new empty context
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a value from the context data store
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.data.get(key)
    }

    /// Set a value in the context data store
    pub fn set(&mut self, key: &str, value: Value) {
        self.data.insert(key.to_string(), value);
    }

    /// Remove a value from the context data store
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.data.remove(key)
    }

    /// Clear values matching a glob pattern
    ///
    /// Supports simple patterns:
    /// - `*` matches everything
    /// - `prefix*` matches keys starting with prefix
    /// - `*suffix` matches keys ending with suffix
    /// - `prefix*suffix` matches keys with prefix and suffix
    pub fn clear(&mut self, pattern: &str) -> u64 {
        if pattern == "*" {
            let count = self.data.len() as u64;
            self.data.clear();
            return count;
        }

        let keys_to_remove: Vec<String> = self
            .data
            .keys()
            .filter(|k| Self::matches_pattern(k, pattern))
            .cloned()
            .collect();

        let count = keys_to_remove.len() as u64;
        for key in keys_to_remove {
            self.data.remove(&key);
        }
        count
    }

    /// Check if a key matches a glob pattern
    fn matches_pattern(key: &str, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if let Some(prefix) = pattern.strip_suffix('*') {
            if !pattern.contains('*') || pattern.ends_with('*') {
                return key.starts_with(prefix);
            }
        }

        if let Some(suffix) = pattern.strip_prefix('*') {
            return key.ends_with(suffix);
        }

        if let Some(star_pos) = pattern.find('*') {
            let prefix = &pattern[..star_pos];
            let suffix = &pattern[star_pos + 1..];
            return key.starts_with(prefix) && key.ends_with(suffix);
        }

        key == pattern
    }

    /// Get all data in the context
    pub fn data(&self) -> &HashMap<String, Value> {
        &self.data
    }

    /// Get a step output value
    pub fn get_step_output(&self, step_id: &str, output_name: &str) -> Option<&String> {
        self.step_outputs.get(step_id)?.get(output_name)
    }

    /// Get all outputs for a step
    pub fn get_step_outputs(&self, step_id: &str) -> Option<&HashMap<String, String>> {
        self.step_outputs.get(step_id)
    }

    /// Set step outputs (called by the bridge to sync state)
    pub fn set_step_outputs(&mut self, step_id: &str, outputs: HashMap<String, String>) {
        self.step_outputs.insert(step_id.to_string(), outputs);
    }

    /// Get the current run ID
    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    /// Get the current job name
    pub fn job_name(&self) -> &str {
        &self.job_name
    }

    /// Get the current step name
    pub fn step_name(&self) -> &str {
        &self.step_name
    }

    /// Set execution info (called by the bridge)
    pub fn set_execution_info(&mut self, run_id: &str, job_name: &str, step_name: &str) {
        self.run_id = run_id.to_string();
        self.job_name = job_name.to_string();
        self.step_name = step_name.to_string();
    }

    /// Set mock clock state (called by the bridge)
    pub fn set_clock(&mut self, virtual_time_ms: Option<i64>, virtual_time_iso: Option<String>, frozen: bool) {
        self.clock = Some(ClockState {
            virtual_time_ms,
            virtual_time_iso,
            frozen,
        });
    }

    /// Get the mock clock state
    pub fn clock(&self) -> Option<&ClockState> {
        self.clock.as_ref()
    }

    /// Check if the clock is mocked
    pub fn is_clock_mocked(&self) -> bool {
        self.clock.as_ref().map(|c| c.virtual_time_ms.is_some()).unwrap_or(false)
    }

    /// Get current time (respects mock clock if set)
    pub fn now(&self) -> std::time::SystemTime {
        if let Some(clock) = &self.clock {
            if let Some(ms) = clock.virtual_time_ms {
                return std::time::UNIX_EPOCH + std::time::Duration::from_millis(ms as u64);
            }
        }
        std::time::SystemTime::now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_get_set() {
        let mut ctx = Context::new();
        ctx.set("key1", Value::String("value1".to_string()));
        ctx.set("key2", Value::Number(42.into()));

        assert_eq!(
            ctx.get("key1"),
            Some(&Value::String("value1".to_string()))
        );
        assert_eq!(ctx.get("key2"), Some(&Value::Number(42.into())));
        assert_eq!(ctx.get("key3"), None);
    }

    #[test]
    fn test_context_remove() {
        let mut ctx = Context::new();
        ctx.set("key", Value::String("value".to_string()));

        let removed = ctx.remove("key");
        assert_eq!(removed, Some(Value::String("value".to_string())));
        assert_eq!(ctx.get("key"), None);
    }

    #[test]
    fn test_context_clear_all() {
        let mut ctx = Context::new();
        ctx.set("key1", Value::Null);
        ctx.set("key2", Value::Null);
        ctx.set("key3", Value::Null);

        let count = ctx.clear("*");
        assert_eq!(count, 3);
        assert!(ctx.data().is_empty());
    }

    #[test]
    fn test_context_clear_prefix() {
        let mut ctx = Context::new();
        ctx.set("user_1", Value::Null);
        ctx.set("user_2", Value::Null);
        ctx.set("session_1", Value::Null);

        let count = ctx.clear("user_*");
        assert_eq!(count, 2);
        assert_eq!(ctx.data().len(), 1);
        assert!(ctx.get("session_1").is_some());
    }

    #[test]
    fn test_context_clear_suffix() {
        let mut ctx = Context::new();
        ctx.set("test_cache", Value::Null);
        ctx.set("user_cache", Value::Null);
        ctx.set("test_data", Value::Null);

        let count = ctx.clear("*_cache");
        assert_eq!(count, 2);
        assert_eq!(ctx.data().len(), 1);
        assert!(ctx.get("test_data").is_some());
    }

    #[test]
    fn test_step_outputs() {
        let mut ctx = Context::new();
        let mut outputs = HashMap::new();
        outputs.insert("result".to_string(), "42".to_string());
        outputs.insert("status".to_string(), "ok".to_string());

        ctx.set_step_outputs("step1", outputs);

        assert_eq!(ctx.get_step_output("step1", "result"), Some(&"42".to_string()));
        assert_eq!(ctx.get_step_output("step1", "status"), Some(&"ok".to_string()));
        assert_eq!(ctx.get_step_output("step1", "missing"), None);
        assert_eq!(ctx.get_step_output("step2", "result"), None);
    }

    #[test]
    fn test_execution_info() {
        let mut ctx = Context::new();
        ctx.set_execution_info("run-123", "build", "compile");

        assert_eq!(ctx.run_id(), "run-123");
        assert_eq!(ctx.job_name(), "build");
        assert_eq!(ctx.step_name(), "compile");
    }
}
