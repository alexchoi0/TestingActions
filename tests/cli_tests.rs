mod common;

use common::*;
use std::process::Command;

fn cli_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_testing-actions"))
}

#[test]
fn test_cli_help() {
    let output = cli_command().arg("--help").output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Run declarative test workflows"));
    assert!(stdout.contains("run"));
    assert!(stdout.contains("run-dir"));
    assert!(stdout.contains("list"));
    assert!(stdout.contains("validate"));
}

#[test]
fn test_cli_version() {
    let output = cli_command().arg("--version").output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("testing-actions"));
}

#[test]
fn test_cli_run_help() {
    let output = cli_command().args(["run", "--help"]).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Run a single workflow file"));
}

#[test]
fn test_cli_run_dir_help() {
    let output = cli_command().args(["run-dir", "--help"]).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Run all workflows in a directory"));
    assert!(stdout.contains("--parallel"));
    assert!(stdout.contains("--fail-fast"));
    assert!(stdout.contains("--filter"));
    assert!(stdout.contains("--config"));
}

#[test]
fn test_cli_list_help() {
    let output = cli_command().args(["list", "--help"]).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("List workflows in a directory"));
}

#[test]
fn test_cli_validate_help() {
    let output = cli_command().args(["validate", "--help"]).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Validate workflow files"));
}

#[test]
fn test_cli_run_single_workflow() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "test.yaml", &simple_workflow("test"));

    let output = cli_command()
        .args(["run", dir.path().join("test.yaml").to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Success: YES"));
}

#[test]
fn test_cli_run_nonexistent_file() {
    let output = cli_command()
        .args(["run", "/nonexistent/workflow.yaml"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found") || stderr.contains("Error"));
}

#[test]
fn test_cli_run_dir_success() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "a.yaml", &simple_workflow("a"));
    write_workflow(dir.path(), "b.yaml", &simple_workflow("b"));

    let output = cli_command()
        .args(["run-dir", dir.path().to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Overall: PASS"));
    assert!(stdout.contains("✓ a"));
    assert!(stdout.contains("✓ b"));
}

#[test]
fn test_cli_run_dir_with_failure() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "pass.yaml", &simple_workflow("pass"));
    write_workflow(dir.path(), "fail.yaml", &failing_workflow("fail"));

    let output = cli_command()
        .args(["run-dir", dir.path().to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Overall: FAIL"));
}

#[test]
fn test_cli_run_dir_empty() {
    let dir = create_test_dir();

    let output = cli_command()
        .args(["run-dir", dir.path().to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Overall: PASS"));
}

#[test]
fn test_cli_run_dir_nonexistent() {
    let output = cli_command()
        .args(["run-dir", "/nonexistent/directory"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found") || stderr.contains("Error"));
}

#[test]
fn test_cli_run_dir_with_parallel() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "a.yaml", &simple_workflow("a"));
    write_workflow(dir.path(), "b.yaml", &simple_workflow("b"));

    let output = cli_command()
        .args(["run-dir", dir.path().to_str().unwrap(), "--parallel", "1"])
        .output()
        .unwrap();

    assert!(output.status.success());
}

#[test]
fn test_cli_run_dir_with_fail_fast() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "fail.yaml", &failing_workflow("fail"));
    write_workflow(
        dir.path(),
        "skip.yaml",
        &workflow_with_deps("skip", &["fail"]),
    );

    let output = cli_command()
        .args(["run-dir", dir.path().to_str().unwrap(), "--fail-fast"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Skipped"));
}

#[test]
fn test_cli_run_dir_with_filter() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "test-a.yaml", &simple_workflow("test-a"));
    write_workflow(dir.path(), "test-b.yaml", &simple_workflow("test-b"));
    write_workflow(dir.path(), "other.yaml", &simple_workflow("other"));

    let output = cli_command()
        .args([
            "run-dir",
            dir.path().to_str().unwrap(),
            "--filter",
            "test-",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test-a"));
    assert!(stdout.contains("test-b"));
    assert!(!stdout.contains("✓ other"));
}

#[test]
fn test_cli_run_dir_with_config() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "test.yaml", &simple_workflow("test"));
    write_runner_config(dir.path(), "parallel: 2\nfail_fast: true");

    let output = cli_command()
        .args(["run-dir", dir.path().to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Using config:"));
}

#[test]
fn test_cli_run_dir_with_explicit_config() {
    let dir = create_test_dir();
    let config_dir = create_test_dir();

    write_workflow(dir.path(), "test.yaml", &simple_workflow("test"));
    write_runner_config(config_dir.path(), "parallel: 8");

    let output = cli_command()
        .args([
            "run-dir",
            dir.path().to_str().unwrap(),
            "--config",
            config_dir.path().join("runner.yaml").to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
}

#[test]
fn test_cli_list_workflows() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "setup.yaml", &simple_workflow("setup"));
    write_workflow(
        dir.path(),
        "test.yaml",
        &workflow_with_deps("test", &["setup"]),
    );

    let output = cli_command()
        .args(["list", dir.path().to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("setup"));
    assert!(stdout.contains("test"));
    assert!(stdout.contains("depends on"));
    assert!(stdout.contains("Execution order"));
}

#[test]
fn test_cli_list_empty_directory() {
    let dir = create_test_dir();

    let output = cli_command()
        .args(["list", dir.path().to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No workflows found"));
}

#[test]
fn test_cli_list_with_always() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "setup.yaml", &simple_workflow("setup"));
    write_workflow(
        dir.path(),
        "cleanup.yaml",
        &workflow_with_always("cleanup", &["setup"]),
    );

    let output = cli_command()
        .args(["list", dir.path().to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[always]"));
}

#[test]
fn test_cli_validate_single_file() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "valid.yaml", &simple_workflow("valid"));

    let output = cli_command()
        .args(["validate", dir.path().join("valid.yaml").to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("✓"));
    assert!(stdout.contains("valid"));
}

#[test]
fn test_cli_validate_directory() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "a.yaml", &simple_workflow("a"));
    write_workflow(dir.path(), "b.yaml", &workflow_with_deps("b", &["a"]));

    let output = cli_command()
        .args(["validate", dir.path().to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("2 workflows validated"));
}

#[test]
fn test_cli_validate_invalid_yaml() {
    let dir = create_test_dir();
    std::fs::write(dir.path().join("invalid.yaml"), "not: valid: yaml: [")
        .expect("Failed to write");

    let output = cli_command()
        .args(["validate", dir.path().join("invalid.yaml").to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
}

#[test]
fn test_cli_validate_missing_dependency() {
    let dir = create_test_dir();
    write_workflow(
        dir.path(),
        "orphan.yaml",
        &workflow_with_deps("orphan", &["nonexistent"]),
    );

    let output = cli_command()
        .args(["validate", dir.path().to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("nonexistent") || stderr.contains("Error"));
}

#[test]
fn test_cli_validate_cyclic_dependency() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "a.yaml", &workflow_with_deps("a", &["b"]));
    write_workflow(dir.path(), "b.yaml", &workflow_with_deps("b", &["a"]));

    let output = cli_command()
        .args(["validate", dir.path().to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
}

#[test]
fn test_cli_verbose_flag() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "test.yaml", &simple_workflow("test"));

    let output = cli_command()
        .args(["-v", "run", dir.path().join("test.yaml").to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success());
    // Verbose mode should show debug output
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Debug logs go to stderr
    assert!(stderr.len() > 0 || output.stdout.len() > 0);
}

#[test]
fn test_cli_unknown_command() {
    let output = cli_command().args(["unknown-command"]).output().unwrap();

    assert!(!output.status.success());
}

#[test]
fn test_cli_run_dir_skipped_workflows() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "fail.yaml", &failing_workflow("fail"));
    write_workflow(
        dir.path(),
        "skip1.yaml",
        &workflow_with_deps("skip1", &["fail"]),
    );
    write_workflow(
        dir.path(),
        "skip2.yaml",
        &workflow_with_deps("skip2", &["fail"]),
    );

    let output = cli_command()
        .args(["run-dir", dir.path().to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Skipped:"));
}
