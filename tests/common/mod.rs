use std::fs;
use std::path::Path;
use tempfile::TempDir;

pub fn create_test_dir() -> TempDir {
    tempfile::tempdir().expect("Failed to create temp directory")
}

pub fn write_workflow(dir: &Path, filename: &str, content: &str) {
    fs::write(dir.join(filename), content).expect("Failed to write workflow file");
}

pub fn write_runner_config(dir: &Path, content: &str) {
    fs::write(dir.join("runner.yaml"), content).expect("Failed to write runner.yaml");
}

pub fn simple_workflow(name: &str) -> String {
    format!(
        r#"
name: {}
jobs:
  test:
    steps:
      - uses: wait/ms
        with:
          duration: "1"
"#,
        name
    )
}

pub fn workflow_with_deps(name: &str, deps: &[&str]) -> String {
    let deps_str = deps
        .iter()
        .map(|d| format!("\"{}\"", d))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        r#"
name: {}
depends_on: [{}]
jobs:
  test:
    steps:
      - uses: wait/ms
        with:
          duration: "1"
"#,
        name, deps_str
    )
}

pub fn workflow_with_always(name: &str, deps: &[&str]) -> String {
    let deps_str = deps
        .iter()
        .map(|d| format!("\"{}\"", d))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        r#"
name: {}
depends_on:
  workflows: [{}]
  always: true
jobs:
  test:
    steps:
      - uses: wait/ms
        with:
          duration: "1"
"#,
        name, deps_str
    )
}

pub fn failing_workflow(name: &str) -> String {
    format!(
        r#"
name: {}
jobs:
  test:
    steps:
      - uses: fail/now
        with:
          message: "Intentional test failure"
"#,
        name
    )
}

pub fn failing_workflow_with_deps(name: &str, deps: &[&str]) -> String {
    let deps_str = deps
        .iter()
        .map(|d| format!("\"{}\"", d))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        r#"
name: {}
depends_on: [{}]
jobs:
  test:
    steps:
      - uses: fail/now
        with:
          message: "Intentional test failure"
"#,
        name, deps_str
    )
}
