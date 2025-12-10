//! Action types and parsing
//!
//! This module handles parsing and categorizing workflow actions from the "uses" field.

use super::platform::Platform;

/// Categories of actions available
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionCategory {
    // Playwright actions (browser-based)
    Page,    // page/goto, page/reload, etc.
    Element, // element/click, element/fill, etc.
    Assert,  // assert/visible, assert/text, etc. (works on all platforms)
    Wait,    // wait/selector, wait/timeout, etc.
    Browser, // browser/screenshot, browser/pdf, etc.
    Network, // network/intercept, network/mock, etc.

    // Node.js actions (direct function calls)
    Node, // node/call, node/chain - Call registered functions
    Ctx,  // ctx/get, ctx/set, ctx/clear - Shared context operations
    Mock, // mock/set, mock/clear - Mocking support
    Hook, // hook/before, hook/after - Lifecycle hooks

    // Rust actions (direct function calls via separate process)
    Rs, // rs/call - Call registered Rust functions

    // Python actions (direct function calls via separate process)
    Py, // py/call - Call registered Python functions

    // Java actions (direct function calls via separate process)
    Java, // java/call - Call registered Java methods

    // Go actions (direct function calls via separate process)
    Go, // go/call - Call registered Go functions

    // Web/HTTP actions (platform-agnostic HTTP requests)
    Web, // web/get, web/post, web/put, web/patch, web/delete - HTTP requests

    // Testing/utility actions (platform-agnostic)
    Fail, // fail/now - Always fails (for testing)

    // Clock actions (synced across all platforms)
    Clock, // clock/set, clock/forward, clock/forward-until - Mock time control

    // Bash actions (shell commands)
    Bash, // bash/exec, bash/script - Run shell commands or scripts
}

impl ActionCategory {
    /// Returns true if this action category requires Playwright (browser)
    pub fn requires_playwright(&self) -> bool {
        matches!(
            self,
            ActionCategory::Page
                | ActionCategory::Element
                | ActionCategory::Browser
                | ActionCategory::Network
        )
    }

    /// Returns true if this action category requires Node.js bridge
    pub fn requires_nodejs(&self) -> bool {
        matches!(
            self,
            ActionCategory::Node | ActionCategory::Ctx | ActionCategory::Mock | ActionCategory::Hook
        )
    }

    /// Returns true if this action category requires Rust bridge
    pub fn requires_rust(&self) -> bool {
        matches!(self, ActionCategory::Rs)
    }

    /// Returns true if this action category requires Python bridge
    pub fn requires_python(&self) -> bool {
        matches!(self, ActionCategory::Py)
    }

    /// Returns true if this action category requires Java bridge
    pub fn requires_java(&self) -> bool {
        matches!(self, ActionCategory::Java)
    }

    /// Returns true if this action category requires Go bridge
    pub fn requires_go(&self) -> bool {
        matches!(self, ActionCategory::Go)
    }

    /// Returns true if this action category requires Web/HTTP bridge
    pub fn requires_web(&self) -> bool {
        matches!(self, ActionCategory::Web)
    }

    /// Returns true if this action can work on all platforms
    pub fn is_platform_agnostic(&self) -> bool {
        matches!(
            self,
            ActionCategory::Assert
                | ActionCategory::Wait
                | ActionCategory::Fail
                | ActionCategory::Clock
                | ActionCategory::Bash
        )
    }

    /// Infer the required platform from the action category
    pub fn infer_platform(&self) -> Option<Platform> {
        if self.requires_playwright() {
            Some(Platform::Playwright)
        } else if self.requires_nodejs() {
            Some(Platform::Nodejs)
        } else if self.requires_rust() {
            Some(Platform::Rust)
        } else if self.requires_python() {
            Some(Platform::Python)
        } else if self.requires_java() {
            Some(Platform::Java)
        } else if self.requires_go() {
            Some(Platform::Go)
        } else if self.requires_web() {
            Some(Platform::Web)
        } else {
            None // Platform-agnostic (assert, wait)
        }
    }
}

/// Parsed action from "uses" field
#[derive(Debug, Clone)]
pub struct ParsedAction {
    pub category: ActionCategory,
    pub action: String,
}

impl ParsedAction {
    /// Parse an action string like "page/goto" into category and action
    pub fn parse(uses: &str) -> Result<Self, String> {
        let parts: Vec<&str> = uses.split('/').collect();
        if parts.len() != 2 {
            return Err(format!(
                "Invalid action format '{}'. Expected 'category/action'",
                uses
            ));
        }

        let category = match parts[0] {
            // Playwright actions
            "page" => ActionCategory::Page,
            "element" => ActionCategory::Element,
            "assert" => ActionCategory::Assert,
            "wait" => ActionCategory::Wait,
            "browser" => ActionCategory::Browser,
            "network" => ActionCategory::Network,
            // Node.js actions
            "node" => ActionCategory::Node,
            "ctx" => ActionCategory::Ctx,
            "mock" => ActionCategory::Mock,
            "hook" => ActionCategory::Hook,
            // Rust actions
            "rs" => ActionCategory::Rs,
            // Python actions
            "py" => ActionCategory::Py,
            // Java actions
            "java" => ActionCategory::Java,
            // Go actions
            "go" => ActionCategory::Go,
            // Web/HTTP actions
            "web" => ActionCategory::Web,
            // Testing/utility actions
            "fail" => ActionCategory::Fail,
            // Clock actions
            "clock" => ActionCategory::Clock,
            // Bash actions
            "bash" => ActionCategory::Bash,
            _ => return Err(format!("Unknown action category: {}", parts[0])),
        };

        Ok(Self {
            category,
            action: parts[1].to_string(),
        })
    }

    /// Check if this action is compatible with the given platform
    pub fn is_compatible_with(&self, platform: &Platform) -> bool {
        match platform {
            Platform::Playwright => {
                self.category.requires_playwright() || self.category.is_platform_agnostic()
            }
            Platform::Nodejs => {
                self.category.requires_nodejs() || self.category.is_platform_agnostic()
            }
            Platform::Rust => {
                self.category.requires_rust() || self.category.is_platform_agnostic()
            }
            Platform::Python => {
                self.category.requires_python() || self.category.is_platform_agnostic()
            }
            Platform::Java => {
                self.category.requires_java() || self.category.is_platform_agnostic()
            }
            Platform::Go => {
                self.category.requires_go() || self.category.is_platform_agnostic()
            }
            Platform::Web => {
                self.category.requires_web() || self.category.is_platform_agnostic()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_action() {
        let action = ParsedAction::parse("page/goto").unwrap();
        assert_eq!(action.category, ActionCategory::Page);
        assert_eq!(action.action, "goto");

        let action = ParsedAction::parse("element/click").unwrap();
        assert_eq!(action.category, ActionCategory::Element);
        assert_eq!(action.action, "click");
    }

    #[test]
    fn test_parse_nodejs_actions() {
        let action = ParsedAction::parse("node/call").unwrap();
        assert_eq!(action.category, ActionCategory::Node);

        let action = ParsedAction::parse("ctx/set").unwrap();
        assert_eq!(action.category, ActionCategory::Ctx);

        let action = ParsedAction::parse("mock/set").unwrap();
        assert_eq!(action.category, ActionCategory::Mock);

        let action = ParsedAction::parse("hook/before").unwrap();
        assert_eq!(action.category, ActionCategory::Hook);
    }

    #[test]
    fn test_parse_rust_actions() {
        let action = ParsedAction::parse("rs/call").unwrap();
        assert_eq!(action.category, ActionCategory::Rs);
    }

    #[test]
    fn test_parse_python_actions() {
        let action = ParsedAction::parse("py/call").unwrap();
        assert_eq!(action.category, ActionCategory::Py);
    }

    #[test]
    fn test_parse_java_actions() {
        let action = ParsedAction::parse("java/call").unwrap();
        assert_eq!(action.category, ActionCategory::Java);
    }

    #[test]
    fn test_parse_go_actions() {
        let action = ParsedAction::parse("go/call").unwrap();
        assert_eq!(action.category, ActionCategory::Go);
    }

    #[test]
    fn test_parse_web_actions() {
        let action = ParsedAction::parse("web/get").unwrap();
        assert_eq!(action.category, ActionCategory::Web);
    }

    #[test]
    fn test_parse_action_invalid() {
        assert!(ParsedAction::parse("invalid").is_err());
        assert!(ParsedAction::parse("unknown/action").is_err());
    }

    #[test]
    fn test_action_platform_compatibility() {
        let page_action = ParsedAction::parse("page/goto").unwrap();
        assert!(page_action.is_compatible_with(&Platform::Playwright));
        assert!(!page_action.is_compatible_with(&Platform::Web));

        let web_action = ParsedAction::parse("web/get").unwrap();
        assert!(!web_action.is_compatible_with(&Platform::Playwright));
        assert!(web_action.is_compatible_with(&Platform::Web));

        // Assert works on all platforms
        let assert_action = ParsedAction::parse("assert/visible").unwrap();
        assert!(assert_action.is_compatible_with(&Platform::Playwright));
        assert!(assert_action.is_compatible_with(&Platform::Nodejs));
        assert!(assert_action.is_compatible_with(&Platform::Web));
    }

    #[test]
    fn test_infer_platform() {
        assert_eq!(
            ActionCategory::Page.infer_platform(),
            Some(Platform::Playwright)
        );
        assert_eq!(
            ActionCategory::Node.infer_platform(),
            Some(Platform::Nodejs)
        );
        assert_eq!(ActionCategory::Rs.infer_platform(), Some(Platform::Rust));
        assert_eq!(ActionCategory::Py.infer_platform(), Some(Platform::Python));
        assert_eq!(ActionCategory::Java.infer_platform(), Some(Platform::Java));
        assert_eq!(ActionCategory::Go.infer_platform(), Some(Platform::Go));
        assert_eq!(ActionCategory::Web.infer_platform(), Some(Platform::Web));
        assert_eq!(ActionCategory::Assert.infer_platform(), None);
    }

    #[test]
    fn test_parse_clock_actions() {
        let action = ParsedAction::parse("clock/set").unwrap();
        assert_eq!(action.category, ActionCategory::Clock);
        assert_eq!(action.action, "set");

        let action = ParsedAction::parse("clock/forward").unwrap();
        assert_eq!(action.category, ActionCategory::Clock);
        assert_eq!(action.action, "forward");

        let action = ParsedAction::parse("clock/forward-until").unwrap();
        assert_eq!(action.category, ActionCategory::Clock);
        assert_eq!(action.action, "forward-until");
    }

    #[test]
    fn test_clock_is_platform_agnostic() {
        assert!(ActionCategory::Clock.is_platform_agnostic());
        assert_eq!(ActionCategory::Clock.infer_platform(), None);

        let clock_action = ParsedAction::parse("clock/forward").unwrap();
        assert!(clock_action.is_compatible_with(&Platform::Playwright));
        assert!(clock_action.is_compatible_with(&Platform::Nodejs));
        assert!(clock_action.is_compatible_with(&Platform::Python));
    }

    #[test]
    fn test_parse_bash_actions() {
        let action = ParsedAction::parse("bash/exec").unwrap();
        assert_eq!(action.category, ActionCategory::Bash);
        assert_eq!(action.action, "exec");
    }

    #[test]
    fn test_bash_is_platform_agnostic() {
        assert!(ActionCategory::Bash.is_platform_agnostic());
        assert_eq!(ActionCategory::Bash.infer_platform(), None);

        let bash_action = ParsedAction::parse("bash/exec").unwrap();
        assert!(bash_action.is_compatible_with(&Platform::Playwright));
        assert!(bash_action.is_compatible_with(&Platform::Nodejs));
        assert!(bash_action.is_compatible_with(&Platform::Python));
        assert!(bash_action.is_compatible_with(&Platform::Web));
    }
}
