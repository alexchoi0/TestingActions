//! All Platforms Demo
//!
//! This example loads and validates all fixture YAML files, demonstrating
//! the full capabilities of each platform.
//!
//! Run with: cargo run --example all_platforms

use testing_actions::prelude::*;
use tracing_subscriber;

// Load all fixtures at compile time
const SIMPLE_SEARCH: &str = include_str!("../fixtures/simple_search.yaml");
const MULTI_PLATFORM: &str = include_str!("../fixtures/multi_platform_checkout.yaml");
const NODEJS_FN: &str = include_str!("../fixtures/nodejs_functions.yaml");
const RUST_VALIDATION: &str = include_str!("../fixtures/rust_validation.yaml");
const PYTHON_ML: &str = include_str!("../fixtures/python_ml.yaml");
const JAVA_ENTERPRISE: &str = include_str!("../fixtures/java_enterprise.yaml");
const WEB_API: &str = include_str!("../fixtures/web_api.yaml");

struct FixtureInfo {
    name: &'static str,
    yaml: &'static str,
    description: &'static str,
    platforms: &'static [&'static str],
}

const FIXTURES: &[FixtureInfo] = &[
    FixtureInfo {
        name: "simple_search.yaml",
        yaml: SIMPLE_SEARCH,
        description: "Basic Playwright browser automation",
        platforms: &["Playwright"],
    },
    FixtureInfo {
        name: "multi_platform_checkout.yaml",
        yaml: MULTI_PLATFORM,
        description: "Complex e-commerce flow using multiple platforms",
        platforms: &["Playwright", "Node.js", "Rust", "Python", "Java", "Web"],
    },
    FixtureInfo {
        name: "nodejs_functions.yaml",
        yaml: NODEJS_FN,
        description: "Node.js custom functions, mocking, context",
        platforms: &["Node.js"],
    },
    FixtureInfo {
        name: "rust_validation.yaml",
        yaml: RUST_VALIDATION,
        description: "Rust cryptography, validation, performance",
        platforms: &["Rust"],
    },
    FixtureInfo {
        name: "python_ml.yaml",
        yaml: PYTHON_ML,
        description: "Python ML inference, analytics, data science",
        platforms: &["Python"],
    },
    FixtureInfo {
        name: "java_enterprise.yaml",
        yaml: JAVA_ENTERPRISE,
        description: "Java enterprise services, payment gateway, MQ",
        platforms: &["Java"],
    },
    FixtureInfo {
        name: "web_api.yaml",
        yaml: WEB_API,
        description: "HTTP REST API with auth, retry, webhooks",
        platforms: &["Web"],
    },
];

fn print_separator() {
    println!("{}", "─".repeat(70));
}

fn print_workflow_summary(workflow: &Workflow) {
    let total_steps: usize = workflow.jobs.values().map(|j| j.steps.len()).sum();
    let total_jobs = workflow.jobs.len();

    println!("  Workflow: {}", workflow.name);
    println!("  Jobs: {}, Total Steps: {}", total_jobs, total_steps);

    // Collect all platforms used
    let mut platforms_used = std::collections::HashSet::new();
    for job in workflow.jobs.values() {
        for step in &job.steps {
            if let Some(p) = &step.platform {
                platforms_used.insert(format!("{:?}", p));
            }
        }
    }

    if !platforms_used.is_empty() {
        let platforms: Vec<_> = platforms_used.into_iter().collect();
        println!("  Platforms: {}", platforms.join(", "));
    }

    // Show job details
    println!("\n  Jobs:");
    for (name, job) in &workflow.jobs {
        let deps = if job.needs.is_empty() {
            String::new()
        } else {
            format!(" (needs: {})", job.needs.join(", "))
        };
        println!("    • {} [{} steps]{}", name, job.steps.len(), deps);
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("testing_actions=warn")
        .init();

    println!("\n╔══════════════════════════════════════════════════════════════════════╗");
    println!("║           Playwright Actions - All Platforms Demo                    ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝\n");

    println!("This demo loads and validates all workflow fixtures from fixtures/\n");

    let mut passed = 0;
    let mut failed = 0;

    for fixture in FIXTURES {
        print_separator();
        println!("📄 {}", fixture.name);
        println!("   {}", fixture.description);
        println!("   Platforms: {}\n", fixture.platforms.join(", "));

        match serde_yaml::from_str::<Workflow>(fixture.yaml) {
            Ok(workflow) => {
                print_workflow_summary(&workflow);
                println!("\n  ✅ Valid YAML - parsed successfully");
                passed += 1;
            }
            Err(e) => {
                println!("  ❌ Parse Error: {}", e);
                failed += 1;
            }
        }
        println!();
    }

    print_separator();
    println!("\n📊 Summary: {} passed, {} failed out of {} fixtures\n",
             passed, failed, FIXTURES.len());

    if failed > 0 {
        std::process::exit(1);
    }

    // Show platform capabilities
    println!("╔══════════════════════════════════════════════════════════════════════╗");
    println!("║                     Platform Capabilities                            ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝\n");

    println!("🎭 Playwright (Browser Automation)");
    println!("   Actions: page/goto, element/click, element/fill, wait/selector,");
    println!("            browser/screenshot, page/title, network/intercept\n");

    println!("📦 Node.js (Custom Functions)");
    println!("   Actions: node/call, ctx/get, ctx/set, mock/set, mock/clear,");
    println!("            hook/call, assert/custom\n");

    println!("🦀 Rust (High Performance)");
    println!("   Actions: rs/call, assert/custom");
    println!("   Features: Cryptography, validation, batch processing\n");

    println!("🐍 Python (ML & Analytics)");
    println!("   Actions: py/call, assert/custom");
    println!("   Features: ML inference, data analysis, scientific computing\n");

    println!("☕ Java (Enterprise)");
    println!("   Actions: java/call, assert/custom");
    println!("   Features: Payment gateways, message queues, JPA/Hibernate\n");

    println!("🐹 Go (High Performance)");
    println!("   Actions: go/call, assert/custom");
    println!("   Features: Concurrency, networking, system utilities\n");

    println!("🌐 Web (HTTP APIs)");
    println!("   Actions: web/get, web/post, web/put, web/patch, web/delete,");
    println!("            web/request");
    println!("   Features: Auth (Bearer/Basic/API Key), retry, redirects\n");

    Ok(())
}
