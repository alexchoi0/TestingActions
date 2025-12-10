//! Multi-Platform Workflow Example
//!
//! This example demonstrates a complex e-commerce test workflow that uses ALL platforms:
//! - Playwright: Browser automation for UI testing
//! - Next.js: Direct server interaction for API and Server Actions
//! - Node.js: Custom JavaScript functions for data generation
//! - Rust: High-performance data validation and cryptography
//! - Python: ML-based fraud detection and data analysis
//! - Java: Enterprise service integration (payment gateway)
//! - Web: External third-party API calls
//!
//! Run with: cargo run --example multi_platform_workflow

use testing_actions::prelude::*;
use tracing_subscriber;

const WORKFLOW_YAML: &str = include_str!("../fixtures/multi_platform_checkout.yaml");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("testing_actions=info")
        .init();

    println!("=== Multi-Platform E-Commerce Checkout Test ===\n");
    println!("This workflow demonstrates all 7 platforms working together:\n");
    println!("  🎭 Playwright  - Browser UI automation");
    println!("  ⚡ Next.js     - Server actions, API routes, database");
    println!("  📦 Node.js    - Custom JS functions, data generation");
    println!("  🦀 Rust       - High-perf validation, cryptography");
    println!("  🐍 Python     - ML fraud detection, analytics");
    println!("  ☕ Java       - Payment gateway integration");
    println!("  🌐 Web        - External API calls (Stripe, etc.)\n");

    // Parse and display workflow structure
    let workflow: Workflow = serde_yaml::from_str(WORKFLOW_YAML)?;

    println!("Workflow: {}\n", workflow.name);
    println!("Jobs ({}):", workflow.jobs.len());

    for (job_name, job) in &workflow.jobs {
        let step_count = job.steps.len();
        let platforms: Vec<String> = job
            .steps
            .iter()
            .filter_map(|s| s.platform.as_ref())
            .map(|p| format!("{:?}", p))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        println!("  📋 {} ({} steps)", job_name, step_count);
        if let Some(name) = &job.name {
            println!("     Name: {}", name);
        }
        if !job.needs.is_empty() {
            println!("     Depends on: {:?}", job.needs);
        }
        if !platforms.is_empty() {
            println!("     Platforms: {}", platforms.join(", "));
        }
    }

    println!("\n--- Workflow YAML parsed successfully ---");
    println!("\nNote: This example shows workflow definition only.");
    println!("Actual execution requires running backend services.\n");

    // To actually run, you would need:
    // let mut executor = Executor::new();
    // executor.set_secret("STRIPE_TEST_KEY", "sk_test_...");
    // executor.set_secret("INVENTORY_API_KEY", "...");
    // let result = executor.run_yaml(WORKFLOW_YAML).await?;

    Ok(())
}
