# Testing Actions

A GitHub Actions-style declarative workflow engine for multi-platform test automation.

## Features

- **Declarative YAML workflows** - Define test automation like GitHub Actions
- **Workflow dependencies** - DAG-based execution with `depends_on`
- **Multi-platform execution** - Playwright, Node.js, Python, Java, Rust, Go, HTTP
- **Parallel execution** - Run independent workflows simultaneously
- **Expression syntax** - Use `${{ }}` for dynamic values
- **Web dashboard** - ReactFlow-based DAG visualization
- **Real-time control** - Pause, resume, and stop workflows remotely

## Quick Start

### Installation

```bash
cargo install --path .
```

This installs two binaries:
- **`ta-run`** - Execute workflows
- **`ta-ctl`** - Control and query the server

### Start the server

```bash
cd web
npm install
npm run dev
```

### Run workflows

```bash
# Run all workflows in a directory (connects to server by default)
ta-run run-dir workflows/

# Run a single workflow
ta-run run workflow.yaml

# Run offline (no server connection)
ta-run --offline run-dir workflows/

# List workflows and execution order
ta-run list workflows/

# Validate workflows
ta-run validate workflows/
```

### Query and control workflows

```bash
# Check server health
ta-ctl health

# List all runs
ta-ctl runs

# Get run details
ta-ctl run <run-id>

# Watch events in real-time
ta-ctl watch <run-id>

# Control execution
ta-ctl pause <run-id>
ta-ctl resume <run-id>
ta-ctl stop <run-id>
```

### Configuration

Both `ta-run` and `ta-ctl` use the `TA_SERVER_URL` environment variable:

```bash
export TA_SERVER_URL=http://localhost:3000
```

## Workflow Syntax

```yaml
name: api-tests
on:
  manual: true
depends_on:
  - setup

jobs:
  test:
    steps:
      - name: Health check
        uses: web/get
        with:
          url: "https://api.example.com/health"

      - name: Run tests
        uses: bash/exec
        with:
          command: cargo test
```

### Workflow Dependencies

Workflows can depend on other workflows using `depends_on`:

```yaml
# workflows/setup.yaml
name: setup
jobs:
  init:
    steps:
      - uses: bash/exec
        with:
          command: docker-compose up -d

# workflows/test.yaml
name: test
depends_on:
  - setup
jobs:
  run:
    steps:
      - uses: bash/exec
        with:
          command: npm test
```

The engine builds a DAG and executes workflows in the correct order, running independent workflows in parallel.

### Runner Configuration

Create `runner.yaml` in your workflows directory:

```yaml
parallel: 4        # Max parallel workflows
fail_fast: false   # Continue on failure
```

## Platforms

### Bash

```yaml
- uses: bash/exec
  with:
    command: echo "Hello"
    working_dir: /tmp
```

### Web/HTTP

```yaml
- uses: web/get
  with:
    url: "https://api.example.com/users"
    headers:
      Authorization: "Bearer ${{ env.TOKEN }}"

- uses: web/post
  with:
    url: "https://api.example.com/users"
    body:
      name: "John"
      email: "john@example.com"
```

### Playwright

```yaml
jobs:
  e2e:
    browser: chromium
    headless: true
    steps:
      - uses: page/goto
        with:
          url: "https://example.com"

      - uses: element/fill
        with:
          selector: "#email"
          value: "${{ secrets.EMAIL }}"

      - uses: element/click
        with:
          selector: "button[type=submit]"
```

### Node.js, Python, Java, Rust, Go

Each platform supports calling registered functions via JSON-RPC:

```yaml
platforms:
  nodejs:
    script: ./functions.js

jobs:
  process:
    platform: nodejs
    steps:
      - uses: myFunction
        with:
          arg1: value1
```

## Expression Syntax

Use `${{ }}` for dynamic values:

- `${{ env.VAR }}` - Environment variable
- `${{ secrets.PASSWORD }}` - Secret value
- `${{ steps.step_id.outputs.result }}` - Step output
- `${{ jobs.job_id.outputs.value }}` - Job output

## Web Dashboard

The `web/` directory contains a Next.js dashboard with:

- ReactFlow DAG visualization
- Real-time workflow status
- GraphQL API with subscriptions
- Google SSO authentication

```bash
cd web
npm install
npm run dev
```

## Project Structure

```
testing-actions/
├── src/
│   ├── bin/
│   │   ├── cli.rs         # ta-run binary
│   │   └── ta_cli.rs      # ta-ctl binary
│   ├── lib.rs             # Library exports
│   ├── client/            # Shared GraphQL client
│   ├── workflow/          # YAML parsing, types
│   ├── engine/            # Execution engine, DAG
│   └── bridge/            # Platform bridges
├── workflows/             # Example workflows
├── web/                   # Next.js dashboard
└── extensions/            # Platform extensions
    ├── nodejs/
    ├── python/
    ├── java/
    ├── rust/
    └── go/
```

## CLI Reference

### ta-run

```
ta-run [OPTIONS] <COMMAND>

Commands:
  run       Run a single workflow file
  run-dir   Run all workflows in a directory
  list      List workflows in a directory
  validate  Validate workflow files

Options:
  -s, --server <URL>  Server URL [env: TA_SERVER_URL]
      --offline       Disable server connection
  -v, --verbose       Enable verbose output
  -h, --help          Print help
  -V, --version       Print version
```

### ta-run run-dir

```
ta-run run-dir <DIR> [OPTIONS]

Options:
  -c, --config <FILE>    Runner config file
  -p, --parallel <N>     Max parallel workflows
  -f, --fail-fast        Stop on first failure
  -F, --filter <PREFIX>  Filter by name prefix
```

### ta-ctl

```
ta-ctl <COMMAND>

Commands:
  health  Check server health
  runs    List all runs
  run     Get details of a specific run
  events  Get events for a specific run
  watch   Watch events in real-time
  stop    Stop a running workflow
  pause   Pause a running workflow
  resume  Resume a paused workflow

Environment:
  TA_SERVER_URL  Server URL (default: http://localhost:3000)
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Copyright (c) 2025 Alex Choi
