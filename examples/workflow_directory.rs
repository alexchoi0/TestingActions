//! Workflow Directory Demo
//!
//! This example demonstrates loading and running multiple workflows
//! from a directory, with parallel execution based on dependencies.
//!
//! Run with: cargo run --example workflow_directory

use testing_actions::prelude::*;
use tracing_subscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("testing_actions=info")
        .init();

    println!("\n=== Workflow Directory Demo ===\n");

    let workflows_dir = std::path::Path::new("fixtures/workflows");

    if !workflows_dir.exists() {
        eprintln!("Error: fixtures/workflows directory not found");
        eprintln!("Please run from the project root directory");
        std::process::exit(1);
    }

    println!("Loading workflows from: {}\n", workflows_dir.display());

    let workflows = WorkflowLoader::load_directory(workflows_dir)?;
    println!("Found {} workflow files:\n", workflows.len());
    for w in &workflows {
        if w.depends_on.workflows.is_empty() {
            println!("  - {} (no dependencies)", w.name);
        } else {
            let deps = w.depends_on.workflows.join(", ");
            let always = if w.depends_on.always { " [always]" } else { "" };
            println!("  - {} (depends on: {}{})", w.name, deps, always);
        }
    }

    println!("\nBuilding DAG...\n");
    let dag = WorkflowDAG::build(workflows)?;

    println!("Execution levels (workflows in same level run in parallel):");
    for (i, level) in dag.execution_levels().iter().enumerate() {
        println!("  Level {}: [{}]", i, level.join(", "));
    }

    println!("\nRunning workflows...\n");

    let result = WorkflowDirectoryRunner::new(workflows_dir)
        .parallel(4)
        .run()
        .await?;

    println!("\n=== Results ===\n");
    println!("Overall success: {}\n", if result.success { "YES" } else { "NO" });

    for level in &result.execution_order {
        for name in level {
            if let Some(wr) = result.workflows.get(name) {
                let status = if wr.success { "PASS" } else { "FAIL" };
                println!("  {} [{}]", name, status);
            }
        }
    }

    if !result.skipped.is_empty() {
        println!("\nSkipped workflows: {}", result.skipped.join(", "));
    }

    Ok(())
}
