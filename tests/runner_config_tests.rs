mod common;

use common::*;
use testing_actions::workflow::RunnerConfig;

#[test]
fn test_default_runner_config() {
    let config = RunnerConfig::default();
    assert_eq!(config.parallel, 4);
    assert!(!config.fail_fast);
    assert!(config.platforms.is_empty());
}

#[test]
fn test_load_minimal_config() {
    let dir = create_test_dir();
    write_runner_config(dir.path(), "parallel: 8");

    let config = RunnerConfig::load(dir.path().join("runner.yaml")).unwrap();
    assert_eq!(config.parallel, 8);
    assert!(!config.fail_fast);
}

#[test]
fn test_load_full_config() {
    let dir = create_test_dir();
    write_runner_config(
        dir.path(),
        r#"
parallel: 16
fail_fast: true
platforms:
  web:
    base_url: "http://localhost:3000"
    timeout: 5000
"#,
    );

    let config = RunnerConfig::load(dir.path().join("runner.yaml")).unwrap();
    assert_eq!(config.parallel, 16);
    assert!(config.fail_fast);
    assert!(config.platforms.web.is_some());

    let web = config.platforms.web.unwrap();
    assert_eq!(web.base_url, "http://localhost:3000");
    assert_eq!(web.timeout, 5000);
}

#[test]
fn test_load_config_with_multiple_platforms() {
    let dir = create_test_dir();
    write_runner_config(
        dir.path(),
        r#"
parallel: 4
platforms:
  web:
    base_url: "http://api.example.com"
  nodejs:
    registry: "./registry.js"
    typescript: true
  rust:
    binary: "./target/debug/test-server"
"#,
    );

    let config = RunnerConfig::load(dir.path().join("runner.yaml")).unwrap();

    assert!(config.platforms.web.is_some());
    assert!(config.platforms.nodejs.is_some());
    assert!(config.platforms.rust.is_some());
    assert!(config.platforms.python.is_none());
    assert!(config.platforms.java.is_none());
    assert!(config.platforms.go.is_none());

    let nodejs = config.platforms.nodejs.unwrap();
    assert_eq!(nodejs.registry, "./registry.js");
    assert!(nodejs.typescript);
}

#[test]
fn test_load_config_file_not_found() {
    let result = RunnerConfig::load("/nonexistent/path/runner.yaml");
    assert!(result.is_err());
}

#[test]
fn test_load_config_invalid_yaml() {
    let dir = create_test_dir();
    write_runner_config(dir.path(), "invalid: yaml: syntax: [");

    let result = RunnerConfig::load(dir.path().join("runner.yaml"));
    assert!(result.is_err());
}

#[test]
fn test_load_config_wrong_types() {
    let dir = create_test_dir();
    write_runner_config(dir.path(), "parallel: not_a_number");

    let result = RunnerConfig::load(dir.path().join("runner.yaml"));
    assert!(result.is_err());
}

#[test]
fn test_config_with_all_platforms() {
    let dir = create_test_dir();
    write_runner_config(
        dir.path(),
        r#"
platforms:
  playwright:
    browser: firefox
    headless: false
  web:
    base_url: "http://localhost"
  nodejs:
    registry: "./reg.js"
  rust:
    binary: "./bin"
  python:
    script: "./script.py"
  java:
    main_class: "com.example.Main"
  go:
    binary: "./go-bin"
"#,
    );

    let config = RunnerConfig::load(dir.path().join("runner.yaml")).unwrap();

    assert!(config.platforms.playwright.is_some());
    assert!(config.platforms.web.is_some());
    assert!(config.platforms.nodejs.is_some());
    assert!(config.platforms.rust.is_some());
    assert!(config.platforms.python.is_some());
    assert!(config.platforms.java.is_some());
    assert!(config.platforms.go.is_some());

    let playwright = config.platforms.playwright.unwrap();
    assert!(!playwright.headless);
}

#[test]
fn test_empty_config_file() {
    let dir = create_test_dir();
    write_runner_config(dir.path(), "");

    let config = RunnerConfig::load(dir.path().join("runner.yaml")).unwrap();
    assert_eq!(config.parallel, 4); // default
    assert!(!config.fail_fast); // default
}
