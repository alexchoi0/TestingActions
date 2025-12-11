//! Runner configuration
//!
//! Configuration for the workflow directory runner, loaded from runner.yaml.
//!
//! Supports `before:` and `after:` hooks at the runner level, and multiple
//! named profiles that run in parallel with their own hooks:
//!
//! ```yaml
//! parallel: 4
//! fail_fast: false
//!
//! before:
//!   - uses: oci/run
//!     id: postgres
//!     with:
//!       image: postgres:16
//!
//! after:
//!   - uses: oci/stop
//!     with:
//!       all: true
//!
//! profiles:
//!   chrome:
//!     env:
//!       BROWSER: chromium
//!     before:
//!       workflow:
//!         - uses: playwright/launch
//!           with:
//!             browser: chromium
//!     after:
//!       workflow:
//!         - uses: playwright/close
//! ```

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::job::Step;
use super::platform::PlatformsConfig;

/// Database configuration for persistent storage
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DatabaseConfigYaml {
    #[default]
    Memory,
    Sqlite {
        #[serde(default = "default_sqlite_path")]
        path: String,
    },
    Postgres {
        url: String,
    },
    Mysql {
        url: String,
    },
}

fn default_sqlite_path() -> String {
    ".testing-actions/runs.db".to_string()
}

/// Hooks that run at workflow, job, or step level
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileHooks {
    #[serde(default)]
    pub workflow: Vec<Step>,
    #[serde(default)]
    pub job: Vec<Step>,
    #[serde(default)]
    pub step: Vec<Step>,
}

/// A named profile with its own platform settings and hooks
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Profile {
    #[serde(default)]
    pub platforms: PlatformsConfig,

    #[serde(default)]
    pub env: HashMap<String, String>,

    #[serde(default)]
    pub before: ProfileHooks,

    #[serde(default)]
    pub after: ProfileHooks,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerConfig {
    #[serde(default = "default_parallel")]
    pub parallel: usize,

    #[serde(default)]
    pub fail_fast: bool,

    #[serde(default)]
    pub platforms: PlatformsConfig,

    /// Database configuration for persistent storage of runs
    #[serde(default)]
    pub database: DatabaseConfigYaml,

    /// Steps to run before any workflows (e.g., spin up infrastructure)
    #[serde(default)]
    pub before: Vec<Step>,

    /// Steps to run after all workflows complete (e.g., teardown)
    #[serde(default)]
    pub after: Vec<Step>,

    /// Named profiles that run in parallel
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
}

fn default_parallel() -> usize {
    4
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            parallel: default_parallel(),
            fail_fast: false,
            platforms: PlatformsConfig::default(),
            database: DatabaseConfigYaml::default(),
            before: Vec::new(),
            after: Vec::new(),
            profiles: HashMap::new(),
        }
    }
}

impl RunnerConfig {
    /// Check if this config has multiple profiles
    pub fn has_profiles(&self) -> bool {
        !self.profiles.is_empty()
    }

    /// Get the list of profile names (or a single "default" if no profiles defined)
    pub fn profile_names(&self) -> Vec<String> {
        if self.profiles.is_empty() {
            vec!["default".to_string()]
        } else {
            self.profiles.keys().cloned().collect()
        }
    }

    /// Get platforms for a specific profile name
    pub fn platforms_for(&self, profile_name: &str) -> PlatformsConfig {
        if profile_name == "default" || self.profiles.is_empty() {
            self.platforms.clone()
        } else if let Some(profile) = self.profiles.get(profile_name) {
            profile.platforms.clone()
        } else {
            self.platforms.clone()
        }
    }
}

impl RunnerConfig {
    pub fn load(path: impl AsRef<std::path::Path>) -> Result<Self, crate::workflow::LoadError> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)?;
        let config: RunnerConfig =
            serde_yaml::from_str(&content).map_err(|e| crate::workflow::LoadError::Yaml {
                file: path.display().to_string(),
                error: e,
            })?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RunnerConfig::default();
        assert_eq!(config.parallel, 4);
        assert!(!config.fail_fast);
        assert!(!config.has_profiles());
    }

    #[test]
    fn test_parse_config() {
        let yaml = r#"
fail_fast: true
parallel: 8
"#;
        let config: RunnerConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.fail_fast);
        assert_eq!(config.parallel, 8);
    }

    #[test]
    fn test_parse_minimal_config() {
        let yaml = "fail_fast: true";
        let config: RunnerConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.fail_fast);
        assert_eq!(config.parallel, 4);
    }

    #[test]
    fn test_parse_profiles() {
        let yaml = r#"
parallel: 4
fail_fast: false

profiles:
  chrome:
    platforms:
      playwright:
        browser: chromium
  firefox:
    platforms:
      playwright:
        browser: firefox
"#;
        let config: RunnerConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.has_profiles());
        assert_eq!(config.profiles.len(), 2);
        assert!(config.profiles.contains_key("chrome"));
        assert!(config.profiles.contains_key("firefox"));
    }

    #[test]
    fn test_profile_names() {
        let config = RunnerConfig::default();
        assert_eq!(config.profile_names(), vec!["default".to_string()]);

        let yaml = r#"
profiles:
  a: {}
  b: {}
"#;
        let config: RunnerConfig = serde_yaml::from_str(yaml).unwrap();
        let names = config.profile_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"a".to_string()));
        assert!(names.contains(&"b".to_string()));
    }

    #[test]
    fn test_platforms_for() {
        let yaml = r#"
platforms:
  web:
    base_url: "http://default.local"

profiles:
  staging:
    platforms:
      web:
        base_url: "http://staging.local"
"#;
        let config: RunnerConfig = serde_yaml::from_str(yaml).unwrap();

        let default_platforms = config.platforms_for("default");
        assert_eq!(
            &default_platforms.web.as_ref().unwrap().base_url,
            "http://default.local"
        );

        let staging_platforms = config.platforms_for("staging");
        assert_eq!(
            &staging_platforms.web.as_ref().unwrap().base_url,
            "http://staging.local"
        );
    }

    #[test]
    fn test_parse_before_after() {
        let yaml = r#"
parallel: 4

before:
  - uses: oci/run
    id: postgres
    with:
      image: postgres:16

after:
  - uses: oci/stop
    with:
      all: "true"
"#;
        let config: RunnerConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.before.len(), 1);
        assert_eq!(config.after.len(), 1);
        assert_eq!(config.before[0].uses, "oci/run");
        assert_eq!(config.after[0].uses, "oci/stop");
    }

    #[test]
    fn test_parse_profile_hooks() {
        let yaml = r#"
parallel: 4

profiles:
  chrome:
    env:
      BROWSER: chromium
    before:
      workflow:
        - uses: playwright/launch
          with:
            browser: chromium
      job:
        - uses: playwright/new-context
    after:
      workflow:
        - uses: playwright/close
"#;
        let config: RunnerConfig = serde_yaml::from_str(yaml).unwrap();
        let chrome = config.profiles.get("chrome").unwrap();

        assert_eq!(chrome.env.get("BROWSER"), Some(&"chromium".to_string()));
        assert_eq!(chrome.before.workflow.len(), 1);
        assert_eq!(chrome.before.job.len(), 1);
        assert_eq!(chrome.after.workflow.len(), 1);
        assert!(chrome.after.job.is_empty());
    }
}
