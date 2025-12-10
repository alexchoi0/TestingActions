# Playwright Actions: GitHub Actions-Style Browser Automation in Rust

A declarative workflow engine for browser automation that provides:

- **GitHub Actions-style YAML syntax** for defining browser automation
- **Flow DAG optimization** for fastest test execution
- **Checkpoint system** to save/restore browser state
- **Parallel execution** of independent test branches

## Concept Overview

This library provides a **declarative workflow DSL** inspired by GitHub Actions that compiles to Playwright browser automation commands.

## Core Innovation: DAG-Based Checkpoint Optimization

Most test suites have shared prefixes (e.g., login → dashboard). Without optimization:

```
Test A: Login(5s) → Dashboard(2s) → Settings(1s) → Change Password(3s) = 11s
Test B: Login(5s) → Dashboard(2s) → Settings(1s) → Notifications(2s) = 10s  
Test C: Login(5s) → Dashboard(2s) → Profile(1s) → Edit Profile(2s) = 10s
                                                            Total: 31s
```

With checkpoint optimization:

```
Login(5s) → checkpoint "logged_in"
├── Restore → Dashboard(2s) → checkpoint "at_dashboard"
│   ├── Restore → Settings(1s) → checkpoint "at_settings"
│   │   ├── Restore → Change Password(3s)
│   │   └── Restore → Notifications(2s)
│   └── Restore → Profile(1s) → Edit Profile(2s)
                                                            Total: 16s (48% faster!)
```

## Quick Start

### Simple Workflow

```rust
use testing_actions::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let workflow = r#"
name: login-test
on:
  manual: true
jobs:
  login:
    browser: chromium
    steps:
      - uses: page/goto
        with:
          url: "https://example.com/login"
      - uses: element/fill
        with:
          selector: "#email"
          value: "${{ secrets.EMAIL }}"
      - uses: element/click
        with:
          selector: "button[type=submit]"
"#;

    let mut executor = Executor::new().await?;
    executor.set_secret("EMAIL", "user@test.com");
    
    let result = executor.run_yaml(workflow).await?;
    println!("Success: {}", result.success);
    Ok(())
}
```

### Flow DAG with Checkpoints

```rust
use testing_actions::checkpoint::{Flow, FlowDag, ExecutionPlanner};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Define flows (see examples/flow-dag.yml for YAML format)
    let flows = vec![
        Flow { id: "login".into(), requires: vec![], ... },
        Flow { id: "dashboard".into(), requires: vec!["login".into()], ... },
        Flow { id: "settings".into(), requires: vec!["dashboard".into()], ... },
    ];
    
    // Build DAG and plan execution
    let dag = FlowDag::build(flows)?;
    let planner = ExecutionPlanner::new();
    let plan = planner.plan_full_suite(&dag, &existing_checkpoints);
    
    println!("Time saved: {}ms", plan.savings_ms);
    println!("Speedup: {:.1}x", plan.baseline_time_ms / plan.estimated_time_ms);
    
    Ok(())
}
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         PLAYWRIGHT ACTIONS                              │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐     │
│  │   YAML Parser   │───▶│   Flow DAG      │───▶│   Execution     │     │
│  │   (serde_yaml)  │    │   Builder       │    │   Planner       │     │
│  └─────────────────┘    └─────────────────┘    └─────────────────┘     │
│                                                         │               │
│                                                         ▼               │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐     │
│  │   Checkpoint    │◀──▶│   Flow          │───▶│   Playwright    │     │
│  │   Store         │    │   Executor      │    │   Bridge        │     │
│  └─────────────────┘    └─────────────────┘    └─────────────────┘     │
│                                                         │               │
│                                                         ▼               │
│                                               ┌─────────────────┐       │
│                                               │   Node.js       │       │
│                                               │   Playwright    │       │
│                                               └─────────────────┘       │
└─────────────────────────────────────────────────────────────────────────┘
```

## Project Structure

```
testing-actions/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public API
│   ├── workflow/           # YAML parsing, types, expressions
│   ├── engine/             # Workflow execution
│   ├── checkpoint/         # DAG, checkpoints, planner
│   │   ├── dag.rs          # Flow DAG construction
│   │   ├── store.rs        # Checkpoint storage
│   │   ├── planner.rs      # Execution optimization
│   │   └── executor.rs     # Parallel flow execution
│   └── bridge/             # Playwright communication
└── playwright-server/      # Node.js sidecar
```

## Available Actions

| Category | Action | Parameters | Description |
|----------|--------|------------|-------------|
| `page/` | `goto` | `url` | Navigate to URL |
| | `reload` | - | Reload page |
| | `back` | - | Go back |
| | `url` | - | Get current URL |
| | `title` | - | Get page title |
| `element/` | `click` | `selector` | Click element |
| | `fill` | `selector`, `value` | Fill input |
| | `type` | `selector`, `text`, `delay` | Type with delay |
| | `select` | `selector`, `value` | Select option |
| | `hover` | `selector` | Hover element |
| `assert/` | `visible` | `selector` | Assert visible |
| | `hidden` | `selector` | Assert hidden |
| | `text_contains` | `selector`, `text` | Assert text |
| | `url_contains` | `pattern` | Assert URL |
| `wait/` | `selector` | `selector`, `timeout` | Wait for element |
| | `navigation` | `timeout` | Wait for navigation |
| | `url` | `pattern`, `timeout` | Wait for URL |
| `browser/` | `screenshot` | `path`, `fullPage` | Take screenshot |
| | `pdf` | `path` | Generate PDF |

## Expression Syntax

Use `${{ }}` for dynamic values:

- `${{ env.BASE_URL }}` - Environment variable
- `${{ secrets.PASSWORD }}` - Secret (not logged)
- `${{ steps.login.outputs.token }}` - Step output
- `${{ jobs.setup.outputs.user_id }}` - Job output

## License

MIT
