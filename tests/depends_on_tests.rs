mod common;

use testing_actions::workflow::{DependsOn, Workflow};

#[test]
fn test_depends_on_empty() {
    let yaml = r#"
name: test
jobs:
  test:
    steps:
      - uses: wait/ms
        with:
          duration: "1"
"#;
    let workflow: Workflow = serde_yaml::from_str(yaml).unwrap();
    assert!(workflow.depends_on.workflows.is_empty());
    assert!(!workflow.depends_on.always);
}

#[test]
fn test_depends_on_simple_list() {
    let yaml = r#"
name: test
depends_on: [setup, init]
jobs:
  test:
    steps:
      - uses: wait/ms
        with:
          duration: "1"
"#;
    let workflow: Workflow = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(workflow.depends_on.workflows, vec!["setup", "init"]);
    assert!(!workflow.depends_on.always);
}

#[test]
fn test_depends_on_single_item() {
    let yaml = r#"
name: test
depends_on: [setup]
jobs:
  test:
    steps:
      - uses: wait/ms
        with:
          duration: "1"
"#;
    let workflow: Workflow = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(workflow.depends_on.workflows, vec!["setup"]);
    assert!(!workflow.depends_on.always);
}

#[test]
fn test_depends_on_with_always_true() {
    let yaml = r#"
name: cleanup
depends_on:
  workflows: [test-api, test-e2e]
  always: true
jobs:
  test:
    steps:
      - uses: wait/ms
        with:
          duration: "1"
"#;
    let workflow: Workflow = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(workflow.depends_on.workflows, vec!["test-api", "test-e2e"]);
    assert!(workflow.depends_on.always);
}

#[test]
fn test_depends_on_with_always_false() {
    let yaml = r#"
name: test
depends_on:
  workflows: [setup]
  always: false
jobs:
  test:
    steps:
      - uses: wait/ms
        with:
          duration: "1"
"#;
    let workflow: Workflow = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(workflow.depends_on.workflows, vec!["setup"]);
    assert!(!workflow.depends_on.always);
}

#[test]
fn test_depends_on_workflows_only_in_object_form() {
    let yaml = r#"
name: test
depends_on:
  workflows: [a, b, c]
jobs:
  test:
    steps:
      - uses: wait/ms
        with:
          duration: "1"
"#;
    let workflow: Workflow = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(workflow.depends_on.workflows, vec!["a", "b", "c"]);
    assert!(!workflow.depends_on.always); // default is false
}

#[test]
fn test_depends_on_empty_list() {
    let yaml = r#"
name: test
depends_on: []
jobs:
  test:
    steps:
      - uses: wait/ms
        with:
          duration: "1"
"#;
    let workflow: Workflow = serde_yaml::from_str(yaml).unwrap();
    assert!(workflow.depends_on.workflows.is_empty());
}

#[test]
fn test_depends_on_object_empty_workflows() {
    let yaml = r#"
name: test
depends_on:
  workflows: []
  always: true
jobs:
  test:
    steps:
      - uses: wait/ms
        with:
          duration: "1"
"#;
    let workflow: Workflow = serde_yaml::from_str(yaml).unwrap();
    assert!(workflow.depends_on.workflows.is_empty());
    assert!(workflow.depends_on.always);
}

#[test]
fn test_depends_on_special_characters_in_names() {
    let yaml = r#"
name: test
depends_on: [setup-db, init_cache, test.api]
jobs:
  test:
    steps:
      - uses: wait/ms
        with:
          duration: "1"
"#;
    let workflow: Workflow = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(
        workflow.depends_on.workflows,
        vec!["setup-db", "init_cache", "test.api"]
    );
}

#[test]
fn test_depends_on_many_dependencies() {
    let deps: Vec<String> = (0..20).map(|i| format!("workflow-{}", i)).collect();
    let deps_yaml = deps
        .iter()
        .map(|d| format!("\"{}\"", d))
        .collect::<Vec<_>>()
        .join(", ");

    let yaml = format!(
        r#"
name: test
depends_on: [{}]
jobs:
  test:
    steps:
      - uses: wait/ms
        with:
          duration: "1"
"#,
        deps_yaml
    );

    let workflow: Workflow = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(workflow.depends_on.workflows.len(), 20);
}
