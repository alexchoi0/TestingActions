//! Platform types and configurations
//!
//! This module contains all platform-specific configuration types for:
//! - Playwright (browser automation)
//! - Node.js (function calls)
//! - Rust (native function calls)
//! - Python (script execution)
//! - Java (JVM integration)
//! - Go (native function calls)
//! - Web (HTTP API calls)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Platform Enum
// ============================================================================

/// Execution platform for workflows
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    /// Browser automation via Playwright (default)
    #[default]
    Playwright,
    /// Direct Node.js function calls
    Nodejs,
    /// Direct Rust function calls via separate process
    Rust,
    /// Direct Python function calls via separate process
    Python,
    /// Direct Java function calls via separate process
    Java,
    /// Direct Go function calls via separate process
    Go,
    /// Generic HTTP API calls (platform-agnostic web requests)
    Web,
}

// ============================================================================
// Playwright Configuration
// ============================================================================

/// Playwright browser automation configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlaywrightConfig {
    /// Browser to use
    #[serde(default)]
    pub browser: BrowserType,

    /// Run in headless mode
    #[serde(default = "default_headless")]
    pub headless: bool,

    /// Viewport configuration
    pub viewport: Option<Viewport>,

    /// Default timeout in milliseconds
    pub timeout: Option<u64>,
}

/// Browser types supported
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BrowserType {
    #[default]
    Chromium,
    Firefox,
    Webkit,
}

/// Viewport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub device_scale_factor: Option<f64>,
    #[serde(default)]
    pub is_mobile: bool,
}

fn default_headless() -> bool {
    true
}

// ============================================================================
// Node.js Configuration
// ============================================================================

/// Node.js direct function call configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodejsConfig {
    /// Path to the registry file that exports callable functions
    pub registry: String,

    /// Working directory for the Node.js process
    pub working_dir: Option<String>,

    /// Path to .env file to load
    pub env_file: Option<String>,

    /// Enable TypeScript support (uses tsx/ts-node)
    #[serde(default)]
    pub typescript: bool,

    /// Lifecycle hooks configuration
    #[serde(default)]
    pub hooks: NodejsHooksConfig,
}

/// Lifecycle hooks for Node.js bridge
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodejsHooksConfig {
    pub before_all: Option<String>,
    pub after_all: Option<String>,
    pub before_each: Option<String>,
    pub after_each: Option<String>,
}

// ============================================================================
// Rust Configuration
// ============================================================================

/// Rust direct function call configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RustConfig {
    /// Path to the compiled binary that implements RustRegistry
    pub binary: Option<String>,

    /// Alternative: use `cargo run --bin <name>` to build and run
    pub cargo_bin: Option<String>,

    /// Working directory for the Rust process
    pub working_dir: Option<String>,

    /// Environment variables to pass to the binary
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Lifecycle hooks configuration
    #[serde(default)]
    pub hooks: RustHooksConfig,
}

/// Lifecycle hooks for Rust bridge
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RustHooksConfig {
    pub before_all: Option<String>,
    pub after_all: Option<String>,
    pub before_each: Option<String>,
    pub after_each: Option<String>,
}

// ============================================================================
// Python Configuration
// ============================================================================

/// Python direct function call configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PythonConfig {
    /// Path to the Python script that implements the registry
    pub script: String,

    /// Python interpreter to use (default: "python3")
    #[serde(default = "default_python_interpreter")]
    pub interpreter: String,

    /// Working directory for the Python process
    pub working_dir: Option<String>,

    /// Path to virtual environment (activates before running)
    pub venv: Option<String>,

    /// Environment variables to pass to the process
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Lifecycle hooks configuration
    #[serde(default)]
    pub hooks: PythonHooksConfig,
}

fn default_python_interpreter() -> String {
    "python3".to_string()
}

/// Lifecycle hooks for Python bridge
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PythonHooksConfig {
    pub before_all: Option<String>,
    pub after_all: Option<String>,
    pub before_each: Option<String>,
    pub after_each: Option<String>,
}

// ============================================================================
// Java Configuration
// ============================================================================

/// Java direct function call configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JavaConfig {
    /// Path to the JAR file or class directory
    pub jar: Option<String>,

    /// Main class name that implements the registry
    pub main_class: String,

    /// Classpath entries (additional JARs or directories)
    #[serde(default)]
    pub classpath: Vec<String>,

    /// Java executable to use (default: "java")
    #[serde(default = "default_java_executable")]
    pub java_home: String,

    /// JVM arguments (e.g., -Xmx512m)
    #[serde(default)]
    pub jvm_args: Vec<String>,

    /// Working directory for the Java process
    pub working_dir: Option<String>,

    /// Environment variables to pass to the process
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Lifecycle hooks configuration
    #[serde(default)]
    pub hooks: JavaHooksConfig,
}

fn default_java_executable() -> String {
    "java".to_string()
}

/// Lifecycle hooks for Java bridge
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JavaHooksConfig {
    pub before_all: Option<String>,
    pub after_all: Option<String>,
    pub before_each: Option<String>,
    pub after_each: Option<String>,
}

// ============================================================================
// Go Configuration
// ============================================================================

/// Go direct function call configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoConfig {
    /// Path to the compiled Go binary or plugin (.so file)
    pub binary: Option<String>,

    /// Alternative: use `go run` to build and run a Go file
    pub go_run: Option<String>,

    /// Alternative: use `go build` to compile and run
    pub go_build: Option<String>,

    /// Working directory for the Go process
    pub working_dir: Option<String>,

    /// Environment variables to pass to the process
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Lifecycle hooks configuration
    #[serde(default)]
    pub hooks: GoHooksConfig,
}

/// Lifecycle hooks for Go bridge
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GoHooksConfig {
    pub before_all: Option<String>,
    pub after_all: Option<String>,
    pub before_each: Option<String>,
    pub after_each: Option<String>,
}

// ============================================================================
// Web/HTTP Configuration
// ============================================================================

/// Web/HTTP API configuration for platform-agnostic HTTP requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebConfig {
    /// Base URL for API requests (e.g., "https://api.example.com")
    pub base_url: String,

    /// Default headers to include in all requests
    #[serde(default)]
    pub headers: HashMap<String, String>,

    /// Default timeout in milliseconds
    #[serde(default = "default_web_timeout")]
    pub timeout: u64,

    /// Authentication configuration
    pub auth: Option<WebAuthConfig>,

    /// Retry configuration for failed requests
    pub retry: Option<WebRetryConfig>,

    /// Whether to follow redirects (default: true)
    #[serde(default = "default_follow_redirects")]
    pub follow_redirects: bool,

    /// Whether to validate SSL certificates (default: true)
    #[serde(default = "default_validate_ssl")]
    pub validate_ssl: bool,
}

fn default_web_timeout() -> u64 {
    30000
}

fn default_follow_redirects() -> bool {
    true
}

fn default_validate_ssl() -> bool {
    true
}

/// Authentication configuration for web requests
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebAuthConfig {
    /// Bearer token authentication
    Bearer { token: String },
    /// Basic authentication
    Basic { username: String, password: String },
    /// API key authentication
    ApiKey { header: String, key: String },
    /// OAuth2 client credentials
    OAuth2 {
        token_url: String,
        client_id: String,
        client_secret: String,
        #[serde(default)]
        scope: Option<String>,
    },
}

/// Retry configuration for web requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebRetryConfig {
    /// Maximum number of retry attempts
    #[serde(default = "default_max_retries")]
    pub max_attempts: u32,

    /// Initial delay between retries in milliseconds
    #[serde(default = "default_retry_initial_delay")]
    pub initial_delay: u64,

    /// Maximum delay between retries in milliseconds
    #[serde(default = "default_retry_max_delay")]
    pub max_delay: u64,

    /// HTTP status codes that should trigger a retry
    #[serde(default = "default_retry_status_codes")]
    pub retry_on_status: Vec<u16>,
}

fn default_max_retries() -> u32 {
    3
}

fn default_retry_initial_delay() -> u64 {
    1000
}

fn default_retry_max_delay() -> u64 {
    10000
}

fn default_retry_status_codes() -> Vec<u16> {
    vec![429, 500, 502, 503, 504]
}

// ============================================================================
// Unified Platforms Container
// ============================================================================

/// Unified platform configurations container
///
/// This struct consolidates all platform-specific configurations under a single key.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlatformsConfig {
    pub playwright: Option<PlaywrightConfig>,
    pub nodejs: Option<NodejsConfig>,
    pub rust: Option<RustConfig>,
    pub python: Option<PythonConfig>,
    pub java: Option<JavaConfig>,
    pub go: Option<GoConfig>,
    pub web: Option<WebConfig>,
}

impl PlatformsConfig {
    /// Check if a specific platform is configured
    pub fn has_platform(&self, platform: &Platform) -> bool {
        match platform {
            Platform::Playwright => self.playwright.is_some(),
            Platform::Nodejs => self.nodejs.is_some(),
            Platform::Rust => self.rust.is_some(),
            Platform::Python => self.python.is_some(),
            Platform::Java => self.java.is_some(),
            Platform::Go => self.go.is_some(),
            Platform::Web => self.web.is_some(),
        }
    }

    /// Get list of configured platforms
    pub fn configured_platforms(&self) -> Vec<Platform> {
        let mut platforms = Vec::new();
        if self.playwright.is_some() {
            platforms.push(Platform::Playwright);
        }
        if self.nodejs.is_some() {
            platforms.push(Platform::Nodejs);
        }
        if self.rust.is_some() {
            platforms.push(Platform::Rust);
        }
        if self.python.is_some() {
            platforms.push(Platform::Python);
        }
        if self.java.is_some() {
            platforms.push(Platform::Java);
        }
        if self.go.is_some() {
            platforms.push(Platform::Go);
        }
        if self.web.is_some() {
            platforms.push(Platform::Web);
        }
        platforms
    }

    /// Check if empty (no platforms configured)
    pub fn is_empty(&self) -> bool {
        self.playwright.is_none()
            && self.nodejs.is_none()
            && self.rust.is_none()
            && self.python.is_none()
            && self.java.is_none()
            && self.go.is_none()
            && self.web.is_none()
    }
}
