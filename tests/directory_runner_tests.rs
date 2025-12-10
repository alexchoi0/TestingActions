mod common;

use common::*;
use testing_actions::prelude::*;
use testing_actions::workflow::RunnerConfig;

#[tokio::test]
async fn test_run_empty_directory() {
    let dir = create_test_dir();

    let result = run_workflow_directory(dir.path()).await.unwrap();

    assert!(result.success);
    assert!(result.workflows.is_empty());
    assert!(result.execution_order.is_empty());
    assert!(result.skipped.is_empty());
}

#[tokio::test]
async fn test_run_single_workflow() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "only.yaml", &simple_workflow("only"));

    let result = run_workflow_directory(dir.path()).await.unwrap();

    assert!(result.success);
    assert_eq!(result.workflows.len(), 1);
    assert!(result.workflows.contains_key("only"));
    assert!(result.workflows.get("only").unwrap().success);
}

#[tokio::test]
async fn test_run_linear_chain() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "01.yaml", &simple_workflow("first"));
    write_workflow(dir.path(), "02.yaml", &workflow_with_deps("second", &["first"]));
    write_workflow(dir.path(), "03.yaml", &workflow_with_deps("third", &["second"]));

    let result = run_workflow_directory(dir.path()).await.unwrap();

    assert!(result.success);
    assert_eq!(result.workflows.len(), 3);
    assert!(result.workflows.get("first").unwrap().success);
    assert!(result.workflows.get("second").unwrap().success);
    assert!(result.workflows.get("third").unwrap().success);
}

#[tokio::test]
async fn test_run_parallel_workflows() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "a.yaml", &simple_workflow("a"));
    write_workflow(dir.path(), "b.yaml", &simple_workflow("b"));
    write_workflow(dir.path(), "c.yaml", &simple_workflow("c"));

    let result = WorkflowDirectoryRunner::new(dir.path())
        .parallel(3)
        .run()
        .await
        .unwrap();

    assert!(result.success);
    assert_eq!(result.workflows.len(), 3);
    // All should be in level 0 (parallel)
    assert_eq!(result.execution_order.len(), 1);
    assert_eq!(result.execution_order[0].len(), 3);
}

#[tokio::test]
async fn test_run_with_filter() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "test-a.yaml", &simple_workflow("test-a"));
    write_workflow(dir.path(), "test-b.yaml", &simple_workflow("test-b"));
    write_workflow(dir.path(), "other.yaml", &simple_workflow("other"));

    let result = WorkflowDirectoryRunner::new(dir.path())
        .filter(|name| name.starts_with("test-"))
        .run()
        .await
        .unwrap();

    assert!(result.success);
    assert_eq!(result.workflows.len(), 2);
    assert!(result.workflows.contains_key("test-a"));
    assert!(result.workflows.contains_key("test-b"));
    assert!(!result.workflows.contains_key("other"));
}

#[tokio::test]
async fn test_run_filter_all() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "a.yaml", &simple_workflow("a"));
    write_workflow(dir.path(), "b.yaml", &simple_workflow("b"));

    let result = WorkflowDirectoryRunner::new(dir.path())
        .filter(|_| false)
        .run()
        .await
        .unwrap();

    assert!(result.success);
    assert!(result.workflows.is_empty());
}

#[tokio::test]
async fn test_run_with_config() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "a.yaml", &simple_workflow("a"));
    write_workflow(dir.path(), "b.yaml", &simple_workflow("b"));

    let config = RunnerConfig {
        parallel: 1,
        fail_fast: false,
        platforms: Default::default(),
        database: Default::default(),
        before: Default::default(),
        after: Default::default(),
        profiles: Default::default(),
    };

    let result = WorkflowDirectoryRunner::with_config(dir.path(), config)
        .run()
        .await
        .unwrap();

    assert!(result.success);
    assert_eq!(result.workflows.len(), 2);
}

#[tokio::test]
async fn test_run_diamond_pattern() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "setup.yaml", &simple_workflow("setup"));
    write_workflow(
        dir.path(),
        "api.yaml",
        &workflow_with_deps("api-tests", &["setup"]),
    );
    write_workflow(
        dir.path(),
        "e2e.yaml",
        &workflow_with_deps("e2e-tests", &["setup"]),
    );
    write_workflow(
        dir.path(),
        "cleanup.yaml",
        &workflow_with_deps("cleanup", &["api-tests", "e2e-tests"]),
    );

    let result = run_workflow_directory(dir.path()).await.unwrap();

    assert!(result.success);
    assert_eq!(result.workflows.len(), 4);
    assert_eq!(result.execution_order.len(), 3);
}

#[tokio::test]
async fn test_skip_workflow_on_dependency_failure() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "fail.yaml", &failing_workflow("fail"));
    write_workflow(
        dir.path(),
        "skip.yaml",
        &workflow_with_deps("should-skip", &["fail"]),
    );

    let result = run_workflow_directory(dir.path()).await.unwrap();

    assert!(!result.success);
    assert!(!result.workflows.get("fail").unwrap().success);
    assert!(result.skipped.contains(&"should-skip".to_string()));
}

#[tokio::test]
async fn test_always_runs_on_dependency_failure() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "fail.yaml", &failing_workflow("fail"));
    write_workflow(
        dir.path(),
        "cleanup.yaml",
        &workflow_with_always("cleanup", &["fail"]),
    );

    let result = run_workflow_directory(dir.path()).await.unwrap();

    assert!(!result.success);
    assert!(!result.workflows.get("fail").unwrap().success);
    // cleanup should run despite fail's failure
    assert!(result.workflows.contains_key("cleanup"));
    assert!(result.workflows.get("cleanup").unwrap().success);
    assert!(!result.skipped.contains(&"cleanup".to_string()));
}

#[tokio::test]
async fn test_fail_fast_stops_pending() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "fail.yaml", &failing_workflow("fail"));
    write_workflow(dir.path(), "a.yaml", &workflow_with_deps("a", &["fail"]));
    write_workflow(dir.path(), "b.yaml", &workflow_with_deps("b", &["fail"]));

    let result = WorkflowDirectoryRunner::new(dir.path())
        .fail_fast(true)
        .run()
        .await
        .unwrap();

    assert!(!result.success);
    // With fail_fast, dependent workflows should be skipped
    assert!(result.skipped.contains(&"a".to_string()));
    assert!(result.skipped.contains(&"b".to_string()));
}

#[tokio::test]
async fn test_fail_fast_always_still_runs() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "fail.yaml", &failing_workflow("fail"));
    write_workflow(
        dir.path(),
        "cleanup.yaml",
        &workflow_with_always("cleanup", &["fail"]),
    );

    let result = WorkflowDirectoryRunner::new(dir.path())
        .fail_fast(true)
        .run()
        .await
        .unwrap();

    assert!(!result.success);
    // cleanup with always: true should still run
    assert!(result.workflows.contains_key("cleanup"));
    assert!(!result.skipped.contains(&"cleanup".to_string()));
}

#[tokio::test]
async fn test_cascade_skip_on_failure() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "fail.yaml", &failing_workflow("fail"));
    write_workflow(dir.path(), "a.yaml", &workflow_with_deps("a", &["fail"]));
    write_workflow(dir.path(), "b.yaml", &workflow_with_deps("b", &["a"]));
    write_workflow(dir.path(), "c.yaml", &workflow_with_deps("c", &["b"]));

    let result = run_workflow_directory(dir.path()).await.unwrap();

    assert!(!result.success);
    assert!(!result.workflows.get("fail").unwrap().success);
    // All dependents should be skipped
    assert!(result.skipped.contains(&"a".to_string()));
    assert!(result.skipped.contains(&"b".to_string()));
    assert!(result.skipped.contains(&"c".to_string()));
}

#[tokio::test]
async fn test_partial_success() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "pass.yaml", &simple_workflow("pass"));
    write_workflow(dir.path(), "fail.yaml", &failing_workflow("fail"));

    let result = run_workflow_directory(dir.path()).await.unwrap();

    assert!(!result.success);
    assert!(result.workflows.get("pass").unwrap().success);
    assert!(!result.workflows.get("fail").unwrap().success);
}

#[tokio::test]
async fn test_skips_runner_yaml() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "test.yaml", &simple_workflow("test"));
    write_runner_config(dir.path(), "parallel: 2");

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();

    assert_eq!(workflows.len(), 1);
    assert_eq!(workflows[0].name, "test");
}

#[tokio::test]
async fn test_run_with_runner_config() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "a.yaml", &simple_workflow("a"));
    write_workflow(dir.path(), "b.yaml", &simple_workflow("b"));
    write_runner_config(
        dir.path(),
        r#"
parallel: 1
fail_fast: true
"#,
    );

    let config = RunnerConfig::load(dir.path().join("runner.yaml")).unwrap();
    let result = WorkflowDirectoryRunner::with_config(dir.path(), config)
        .run()
        .await
        .unwrap();

    assert!(result.success);
    assert_eq!(result.workflows.len(), 2);
}

#[tokio::test]
async fn test_eager_scheduling() {
    let dir = create_test_dir();
    // Create a pattern where notify can start before e2e finishes
    // setup -> (api-tests, e2e-tests[slow])
    // api-tests -> notify
    // (api-tests, e2e-tests) -> cleanup

    write_workflow(dir.path(), "setup.yaml", &simple_workflow("setup"));
    write_workflow(
        dir.path(),
        "api.yaml",
        &workflow_with_deps("api-tests", &["setup"]),
    );
    write_workflow(
        dir.path(),
        "e2e.yaml",
        &format!(
            r#"
name: e2e-tests
depends_on: [setup]
jobs:
  test:
    steps:
      - uses: wait/ms
        with:
          duration: "100"
"#
        ),
    );
    write_workflow(
        dir.path(),
        "notify.yaml",
        &workflow_with_deps("notify", &["api-tests"]),
    );
    write_workflow(
        dir.path(),
        "cleanup.yaml",
        &workflow_with_deps("cleanup", &["api-tests", "e2e-tests"]),
    );

    let result = WorkflowDirectoryRunner::new(dir.path())
        .parallel(4)
        .run()
        .await
        .unwrap();

    assert!(result.success);
    assert_eq!(result.workflows.len(), 5);
}

#[tokio::test]
async fn test_max_parallel_respected() {
    let dir = create_test_dir();
    // Create 10 independent workflows
    for i in 0..10 {
        write_workflow(
            dir.path(),
            &format!("w{}.yaml", i),
            &simple_workflow(&format!("workflow-{}", i)),
        );
    }

    let result = WorkflowDirectoryRunner::new(dir.path())
        .parallel(2) // Only 2 at a time
        .run()
        .await
        .unwrap();

    assert!(result.success);
    assert_eq!(result.workflows.len(), 10);
}
