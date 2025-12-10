mod common;

use common::*;
use testing_actions::prelude::*;

#[test]
fn test_dag_single_workflow() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "only.yaml", &simple_workflow("only"));

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
    let dag = WorkflowDAG::build(workflows).unwrap();

    assert_eq!(dag.len(), 1);
    assert_eq!(dag.execution_levels().len(), 1);
    assert_eq!(dag.execution_levels()[0], vec!["only"]);
}

#[test]
fn test_dag_linear_chain() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "01.yaml", &simple_workflow("first"));
    write_workflow(dir.path(), "02.yaml", &workflow_with_deps("second", &["first"]));
    write_workflow(dir.path(), "03.yaml", &workflow_with_deps("third", &["second"]));
    write_workflow(dir.path(), "04.yaml", &workflow_with_deps("fourth", &["third"]));

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
    let dag = WorkflowDAG::build(workflows).unwrap();

    assert_eq!(dag.len(), 4);
    assert_eq!(dag.execution_levels().len(), 4);
    assert_eq!(dag.execution_levels()[0], vec!["first"]);
    assert_eq!(dag.execution_levels()[1], vec!["second"]);
    assert_eq!(dag.execution_levels()[2], vec!["third"]);
    assert_eq!(dag.execution_levels()[3], vec!["fourth"]);
}

#[test]
fn test_dag_diamond_pattern() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "01.yaml", &simple_workflow("top"));
    write_workflow(dir.path(), "02.yaml", &workflow_with_deps("left", &["top"]));
    write_workflow(dir.path(), "03.yaml", &workflow_with_deps("right", &["top"]));
    write_workflow(
        dir.path(),
        "04.yaml",
        &workflow_with_deps("bottom", &["left", "right"]),
    );

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
    let dag = WorkflowDAG::build(workflows).unwrap();

    assert_eq!(dag.len(), 4);
    assert_eq!(dag.execution_levels().len(), 3);
    assert_eq!(dag.execution_levels()[0], vec!["top"]);
    assert!(dag.execution_levels()[1].contains(&"left".to_string()));
    assert!(dag.execution_levels()[1].contains(&"right".to_string()));
    assert_eq!(dag.execution_levels()[2], vec!["bottom"]);
}

#[test]
fn test_dag_all_independent() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "a.yaml", &simple_workflow("a"));
    write_workflow(dir.path(), "b.yaml", &simple_workflow("b"));
    write_workflow(dir.path(), "c.yaml", &simple_workflow("c"));
    write_workflow(dir.path(), "d.yaml", &simple_workflow("d"));

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
    let dag = WorkflowDAG::build(workflows).unwrap();

    assert_eq!(dag.len(), 4);
    assert_eq!(dag.execution_levels().len(), 1);
    assert_eq!(dag.execution_levels()[0].len(), 4);
}

#[test]
fn test_dag_fan_out() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "root.yaml", &simple_workflow("root"));
    for i in 1..=5 {
        let name = format!("branch-{}", i);
        write_workflow(
            dir.path(),
            &format!("{}.yaml", name),
            &workflow_with_deps(&name, &["root"]),
        );
    }

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
    let dag = WorkflowDAG::build(workflows).unwrap();

    assert_eq!(dag.len(), 6);
    assert_eq!(dag.execution_levels().len(), 2);
    assert_eq!(dag.execution_levels()[0], vec!["root"]);
    assert_eq!(dag.execution_levels()[1].len(), 5);
}

#[test]
fn test_dag_fan_in() {
    let dir = create_test_dir();
    for i in 1..=5 {
        let name = format!("source-{}", i);
        write_workflow(dir.path(), &format!("{}.yaml", name), &simple_workflow(&name));
    }
    write_workflow(
        dir.path(),
        "sink.yaml",
        &workflow_with_deps(
            "sink",
            &["source-1", "source-2", "source-3", "source-4", "source-5"],
        ),
    );

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
    let dag = WorkflowDAG::build(workflows).unwrap();

    assert_eq!(dag.len(), 6);
    assert_eq!(dag.execution_levels().len(), 2);
    assert_eq!(dag.execution_levels()[0].len(), 5);
    assert_eq!(dag.execution_levels()[1], vec!["sink"]);
}

#[test]
fn test_dag_complex_mixed() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "a.yaml", &simple_workflow("a"));
    write_workflow(dir.path(), "b.yaml", &simple_workflow("b"));
    write_workflow(dir.path(), "c.yaml", &workflow_with_deps("c", &["a"]));
    write_workflow(dir.path(), "d.yaml", &workflow_with_deps("d", &["a", "b"]));
    write_workflow(dir.path(), "e.yaml", &workflow_with_deps("e", &["c", "d"]));
    write_workflow(dir.path(), "f.yaml", &workflow_with_deps("f", &["d"]));

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
    let dag = WorkflowDAG::build(workflows).unwrap();

    assert_eq!(dag.len(), 6);

    // Level 0: a, b (no deps)
    assert!(dag.execution_levels()[0].contains(&"a".to_string()));
    assert!(dag.execution_levels()[0].contains(&"b".to_string()));
}

#[test]
fn test_dag_missing_dependency_error() {
    let dir = create_test_dir();
    write_workflow(
        dir.path(),
        "orphan.yaml",
        &workflow_with_deps("orphan", &["nonexistent"]),
    );

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
    let result = WorkflowDAG::build(workflows);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("nonexistent"));
}

#[test]
fn test_dag_self_dependency_error() {
    let dir = create_test_dir();
    write_workflow(
        dir.path(),
        "self.yaml",
        &workflow_with_deps("self-ref", &["self-ref"]),
    );

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
    let result = WorkflowDAG::build(workflows);

    // Self-reference means the dependency doesn't exist at build time
    // or creates a cycle
    assert!(result.is_err());
}

#[test]
fn test_dag_simple_cycle_error() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "a.yaml", &workflow_with_deps("a", &["b"]));
    write_workflow(dir.path(), "b.yaml", &workflow_with_deps("b", &["a"]));

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
    let result = WorkflowDAG::build(workflows);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Cyclic"));
}

#[test]
fn test_dag_three_node_cycle_error() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "a.yaml", &workflow_with_deps("a", &["c"]));
    write_workflow(dir.path(), "b.yaml", &workflow_with_deps("b", &["a"]));
    write_workflow(dir.path(), "c.yaml", &workflow_with_deps("c", &["b"]));

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
    let result = WorkflowDAG::build(workflows);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Cyclic"));
}

#[test]
fn test_dag_duplicate_name_error() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "first.yaml", &simple_workflow("duplicate"));
    write_workflow(dir.path(), "second.yaml", &simple_workflow("duplicate"));

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
    let result = WorkflowDAG::build(workflows);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Duplicate"));
}

#[test]
fn test_dag_get_workflow() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "test.yaml", &simple_workflow("my-workflow"));

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
    let dag = WorkflowDAG::build(workflows).unwrap();

    assert!(dag.get_workflow("my-workflow").is_some());
    assert!(dag.get_workflow("nonexistent").is_none());
}

#[test]
fn test_dag_get_node() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "a.yaml", &simple_workflow("a"));
    write_workflow(dir.path(), "b.yaml", &workflow_with_always("b", &["a"]));

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
    let dag = WorkflowDAG::build(workflows).unwrap();

    let node_a = dag.get_node("a").unwrap();
    assert!(node_a.dependencies.is_empty());
    assert!(!node_a.always);

    let node_b = dag.get_node("b").unwrap();
    assert_eq!(node_b.dependencies, vec!["a"]);
    assert!(node_b.always);
}

#[test]
fn test_dag_workflow_names() {
    let dir = create_test_dir();
    write_workflow(dir.path(), "x.yaml", &simple_workflow("x"));
    write_workflow(dir.path(), "y.yaml", &simple_workflow("y"));
    write_workflow(dir.path(), "z.yaml", &simple_workflow("z"));

    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
    let dag = WorkflowDAG::build(workflows).unwrap();

    let names = dag.workflow_names();
    assert_eq!(names.len(), 3);
    assert!(names.contains(&"x"));
    assert!(names.contains(&"y"));
    assert!(names.contains(&"z"));
}

#[test]
fn test_dag_is_empty() {
    let dir = create_test_dir();
    let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();

    // Can't build DAG from empty, but we can check the workflows vec
    assert!(workflows.is_empty());
}
