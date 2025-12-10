//! Simple workflow example
//!
//! Run with: cargo run --example simple_workflow

use testing_actions::prelude::*;
use tracing_subscriber;

const WORKFLOW_YAML: &str = include_str!("../fixtures/simple_search.yaml");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("testing_actions=debug")
        .init();

    // Create executor
    let mut executor = Executor::new();

    // Optionally set secrets
    // executor.set_secret("API_KEY", "your-api-key");

    // Run the workflow
    println!("Starting workflow execution...");
    let result = executor.run_yaml(WORKFLOW_YAML).await?;

    // Report results
    println!("\n=== Workflow Results ===");
    println!("Run ID: {}", result.run_id);
    println!("Success: {}", result.success);
    println!();

    for (job_name, job_result) in &result.jobs {
        println!("Job: {}", job_name);
        println!("  Success: {}", job_result.success);
        println!("  Steps completed: {}", job_result.steps.len());

        for (idx, step) in job_result.steps.iter().enumerate() {
            let status = if step.success { "✓" } else { "✗" };
            println!("    [{}] Step {}", status, idx + 1);

            if !step.outputs.is_empty() {
                for (key, value) in &step.outputs {
                    println!("        {} = {}", key, value);
                }
            }

            if let Some(error) = &step.error {
                println!("        Error: {}", error);
            }
        }
    }

    Ok(())
}
