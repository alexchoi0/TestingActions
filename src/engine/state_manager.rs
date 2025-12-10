//! Shared State Manager - Unified state management across platforms
//!
//! This module manages state that flows between different platform bridges:
//! - ExecutionContext (step outputs, environment)
//! - Platform-specific cached state

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::workflow::{ExecutionContext, Platform};

/// Snapshot of ExecutionContext for serialization
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionContextSnapshot {
    /// Step outputs: step_id -> { output_name -> value }
    pub step_outputs: HashMap<String, HashMap<String, String>>,

    /// Environment variables
    pub env: HashMap<String, String>,

    /// Secrets (names only, not values - for validation)
    pub secret_names: Vec<String>,

    /// Job outputs
    pub job_outputs: HashMap<String, HashMap<String, String>>,

    /// Run ID
    pub run_id: String,
}

impl From<&ExecutionContext> for ExecutionContextSnapshot {
    fn from(ctx: &ExecutionContext) -> Self {
        Self {
            step_outputs: ctx.steps.clone(),
            env: ctx.env.clone(),
            secret_names: ctx.secrets.keys().cloned().collect(),
            job_outputs: ctx.jobs.clone(),
            run_id: ctx.run_id.clone(),
        }
    }
}

/// Manages shared state across all platform bridges
#[derive(Debug)]
pub struct SharedStateManager {
    /// Current step index
    current_step_index: usize,

    /// Platforms that have been used in this execution
    used_platforms: Vec<Platform>,
}

impl SharedStateManager {
    /// Create a new state manager
    pub fn new() -> Self {
        Self {
            current_step_index: 0,
            used_platforms: Vec::new(),
        }
    }

    /// Get the current step index
    pub fn current_step_index(&self) -> usize {
        self.current_step_index
    }

    /// Advance to the next step
    pub fn advance_step(&mut self, platform: Option<&Platform>) {
        self.current_step_index += 1;

        if let Some(p) = platform {
            if !self.used_platforms.contains(p) {
                self.used_platforms.push(p.clone());
            }
        }
    }

    /// Record that a platform was used
    pub fn record_platform_usage(&mut self, platform: &Platform) {
        if !self.used_platforms.contains(platform) {
            self.used_platforms.push(platform.clone());
        }
    }

    /// Get platforms that have been used
    pub fn used_platforms(&self) -> &[Platform] {
        &self.used_platforms
    }

    /// Check if a specific platform has been used
    pub fn has_used_platform(&self, platform: &Platform) -> bool {
        self.used_platforms.contains(platform)
    }

    /// Clear all cached state (for cleanup)
    pub fn clear(&mut self) {
        self.used_platforms.clear();
        self.current_step_index = 0;
    }
}

impl Default for SharedStateManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_manager_creation() {
        let manager = SharedStateManager::new();

        assert_eq!(manager.current_step_index(), 0);
        assert!(manager.used_platforms().is_empty());
    }

    #[test]
    fn test_step_advancement() {
        let mut manager = SharedStateManager::new();

        manager.advance_step(Some(&Platform::Playwright));
        assert_eq!(manager.current_step_index(), 1);
        assert!(manager.has_used_platform(&Platform::Playwright));
    }

    #[test]
    fn test_platform_tracking() {
        let mut manager = SharedStateManager::new();

        manager.advance_step(Some(&Platform::Playwright));
        manager.advance_step(Some(&Platform::Nodejs));
        manager.advance_step(Some(&Platform::Web));

        assert!(manager.has_used_platform(&Platform::Playwright));
        assert!(manager.has_used_platform(&Platform::Nodejs));
        assert!(manager.has_used_platform(&Platform::Web));
        assert_eq!(manager.used_platforms().len(), 3);
    }
}
