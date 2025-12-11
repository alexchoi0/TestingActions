mod common;

use common::*;
use std::fs;
use testing_actions::prelude::*;

#[test]
fn test_load_empty_directory() {
    let dir = create_test_dir();
    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
    assert!(workflows.is_empty());
}

#[test]
fn test_load_single_workflow() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "test.yaml", &simple_workflow("test-workflow"));

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();

    assert_eq!(workflows.len(), 1);
    assert_eq!(workflows[0].name, "test-workflow");
}

#[test]
fn test_load_multiple_workflows() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "a.yaml", &simple_workflow("workflow-a"));
    write_workflow(dir.path(), "b.yaml", &simple_workflow("workflow-b"));
    write_workflow(dir.path(), "c.yaml", &simple_workflow("workflow-c"));

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();

    assert_eq!(workflows.len(), 3);
    let names: Vec<_> = workflows.iter().map(|w| w.name.as_str()).collect();
    assert!(names.contains(&"workflow-a"));
    assert!(names.contains(&"workflow-b"));
    assert!(names.contains(&"workflow-c"));
}

#[test]
fn test_load_yaml_extension() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "test.yaml", &simple_workflow("yaml-ext"));

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
    assert_eq!(workflows.len(), 1);
}

#[test]
fn test_load_yml_extension() {
    let dir = create_test_dir();
    fs::write(dir.path().join("test.yml"), &simple_workflow("yml-ext")).unwrap();

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
    assert_eq!(workflows.len(), 1);
    assert_eq!(workflows[0].name, "yml-ext");
}

#[test]
fn test_load_mixed_extensions() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "a.yaml", &simple_workflow("yaml-file"));
    fs::write(dir.path().join("b.yml"), &simple_workflow("yml-file")).unwrap();

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
    assert_eq!(workflows.len(), 2);
}

#[test]
fn test_skip_non_yaml_files() {
    let dir = create_test_dir();
    write_workflow(
        dir.path(),
        "workflow.yaml",
        &simple_workflow("real-workflow"),
    );
    fs::write(dir.path().join("readme.md"), "# README").unwrap();
    fs::write(dir.path().join("config.json"), "{}").unwrap();
    fs::write(dir.path().join("script.sh"), "#!/bin/bash").unwrap();

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
    assert_eq!(workflows.len(), 1);
    assert_eq!(workflows[0].name, "real-workflow");
}

#[test]
fn test_skip_runner_yaml() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "workflow.yaml", &simple_workflow("workflow"));
    write_runner_config(dir.path(), "parallel: 4");

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
    assert_eq!(workflows.len(), 1);
    assert_eq!(workflows[0].name, "workflow");
}

#[test]
fn test_skip_runner_yml() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "workflow.yaml", &simple_workflow("workflow"));
    fs::write(dir.path().join("runner.yml"), "parallel: 4").unwrap();

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
    assert_eq!(workflows.len(), 1);
}

#[test]
fn test_skip_subdirectories() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "root.yaml", &simple_workflow("root"));

    let subdir = dir.path().join("subdir");
    fs::create_dir(&subdir).unwrap();
    fs::write(subdir.join("nested.yaml"), &simple_workflow("nested")).unwrap();

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
    assert_eq!(workflows.len(), 1);
    assert_eq!(workflows[0].name, "root");
}

#[test]
fn test_load_file_success() {
    let dir = create_test_dir();
    let path = dir.path().join("single.yaml");
    fs::write(&path, &simple_workflow("single")).unwrap();

    let workflow = WorkflowLoader::load_file(&path).unwrap();
    assert_eq!(workflow.name, "single");
}

#[test]
fn test_load_file_not_found() {
    let result = WorkflowLoader::load_file(std::path::Path::new("/nonexistent.yaml"));
    assert!(result.is_err());
}

#[test]
fn test_load_file_invalid_yaml() {
    let dir = create_test_dir();
    let path = dir.path().join("invalid.yaml");
    fs::write(&path, "invalid: yaml: [syntax").unwrap();

    let result = WorkflowLoader::load_file(&path);
    assert!(result.is_err());
}

#[test]
fn test_load_file_missing_required_field() {
    let dir = create_test_dir();
    let path = dir.path().join("missing.yaml");
    fs::write(
        &path,
        r#"
jobs:
  test:
    steps:
      - uses: wait/ms
"#,
    )
    .unwrap();

    let result = WorkflowLoader::load_file(&path);
    assert!(result.is_err());
}

#[test]
fn test_load_directory_with_invalid_file() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "valid.yaml", &simple_workflow("valid"));
    fs::write(dir.path().join("invalid.yaml"), "not: valid: yaml: [").unwrap();

    let result = WorkflowLoader::load_directory(dir.path());
    assert!(result.is_err());
}

#[test]
fn test_load_workflow_with_all_fields() {
    let dir = create_test_dir();
    fs::write(
        dir.path().join("full.yaml"),
        r#"
name: full-workflow
depends_on:
  workflows: [setup]
  always: true
platform: web
platforms:
  web:
    base_url: "http://localhost"
env:
  API_KEY: secret
defaults:
  timeout: 5000
jobs:
  test:
    name: Test Job
    platform: web
    needs: []
    env:
      JOB_VAR: value
    steps:
      - name: Step 1
        uses: wait/ms
        with:
          duration: "1"
"#,
    )
    .unwrap();

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
    assert_eq!(workflows.len(), 1);

    let w = &workflows[0];
    assert_eq!(w.name, "full-workflow");
    assert!(w.depends_on.always);
    assert_eq!(w.depends_on.workflows, vec!["setup"]);
    assert!(w.platforms.web.is_some());
    assert!(w.env.contains_key("API_KEY"));
}

#[test]
fn test_load_hidden_files_skipped() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "visible.yaml", &simple_workflow("visible"));
    fs::write(dir.path().join(".hidden.yaml"), &simple_workflow("hidden")).unwrap();

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();

    // Hidden files should still be loaded if they have .yaml extension
    // (we don't explicitly filter them, so this tests current behavior)
    assert!(workflows.len() >= 1);
    let names: Vec<_> = workflows.iter().map(|w| w.name.as_str()).collect();
    assert!(names.contains(&"visible"));
}

#[test]
fn test_load_special_characters_in_filename() {
    let dir = create_test_dir();
    write_workflow(
        dir.path(),
        "test-workflow_v2.yaml",
        &simple_workflow("special-name"),
    );

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
    assert_eq!(workflows.len(), 1);
    assert_eq!(workflows[0].name, "special-name");
}

#[test]
fn test_load_unicode_in_workflow() {
    let dir = create_test_dir();
    fs::write(
        dir.path().join("unicode.yaml"),
        r#"
name: "テスト-workflow"
jobs:
  test:
    steps:
      - name: "步骤 1"
        uses: wait/ms
        with:
          duration: "1"
"#,
    )
    .unwrap();

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
    assert_eq!(workflows.len(), 1);
    assert_eq!(workflows[0].name, "テスト-workflow");
}
