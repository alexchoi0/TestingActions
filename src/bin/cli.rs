use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use futures::stream::StreamExt;
use futures::SinkExt;
use serde::{Deserialize, Serialize};
use testing_actions::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventType {
    RunStarted,
    RunCompleted,
    WorkflowStarted,
    WorkflowCompleted,
    WorkflowSkipped,
    JobStarted,
    JobCompleted,
    StepStarted,
    StepCompleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunEvent {
    pub event_type: EventType,
    pub run_id: String,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl RunEvent {
    pub fn workflow_completed(run_id: &str, workflow_name: &str, success: bool, error: Option<String>) -> Self {
        Self {
            event_type: EventType::WorkflowCompleted,
            run_id: run_id.to_string(),
            timestamp: Utc::now(),
            workflow_name: Some(workflow_name.to_string()),
            job_name: None,
            step_index: None,
            step_name: None,
            success: Some(success),
            error,
            reason: None,
        }
    }

    pub fn workflow_skipped(run_id: &str, workflow_name: &str, reason: &str) -> Self {
        Self {
            event_type: EventType::WorkflowSkipped,
            run_id: run_id.to_string(),
            timestamp: Utc::now(),
            workflow_name: Some(workflow_name.to_string()),
            job_name: None,
            step_index: None,
            step_name: None,
            success: None,
            error: None,
            reason: Some(reason.to_string()),
        }
    }
}
use testing_actions::workflow::RunnerConfig;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing_subscriber::EnvFilter;
#[cfg(feature = "otel")]
use tracing_subscriber::layer::SubscriberExt;
#[cfg(feature = "otel")]
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Parser)]
#[command(name = "testing-actions")]
#[command(about = "Run declarative test workflows", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Server URL for telemetry reporting (optional)
    #[arg(short, long, global = true)]
    server: Option<String>,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a single workflow file
    Run {
        /// Path to the workflow YAML file
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },

    /// Run all workflows in a directory
    RunDir {
        /// Path to the workflows directory
        #[arg(value_name = "DIR")]
        dir: PathBuf,

        /// Path to runner.yaml config file (default: <DIR>/runner.yaml)
        #[arg(short, long)]
        config: Option<PathBuf>,

        /// Maximum number of parallel workflows (overrides config)
        #[arg(short, long)]
        parallel: Option<usize>,

        /// Stop on first failure (overrides config)
        #[arg(short, long)]
        fail_fast: bool,

        /// Filter workflows by name prefix
        #[arg(short = 'F', long)]
        filter: Option<String>,
    },

    /// List workflows in a directory
    List {
        /// Path to the workflows directory
        #[arg(value_name = "DIR")]
        dir: PathBuf,
    },

    /// Validate workflow files without running them
    Validate {
        /// Path to workflow file or directory
        #[arg(value_name = "PATH")]
        path: PathBuf,
    },
}

#[cfg(feature = "otel")]
fn init_otel_tracing(verbose: bool) {
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::runtime::Tokio;
    use opentelemetry_sdk::trace::TracerProvider;

    let filter = if verbose {
        "testing_actions=debug"
    } else {
        "testing_actions=info"
    };

    let otlp_endpoint =
        std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").unwrap_or_else(|_| "http://localhost:4317".to_string());

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(&otlp_endpoint)
        .build()
        .expect("Failed to create OTLP exporter");

    let provider = TracerProvider::builder()
        .with_batch_exporter(exporter, Tokio)
        .build();

    let tracer = provider.tracer("testing-actions-agent");
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    tracing_subscriber::registry()
        .with(EnvFilter::new(filter))
        .with(tracing_subscriber::fmt::layer())
        .with(otel_layer)
        .init();

    opentelemetry::global::set_tracer_provider(provider);
}

#[cfg(not(feature = "otel"))]
fn init_tracing(verbose: bool) {
    let filter = if verbose {
        "testing_actions=debug"
    } else {
        "testing_actions=info"
    };

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(filter))
        .init();
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    #[cfg(feature = "otel")]
    init_otel_tracing(cli.verbose);

    #[cfg(not(feature = "otel"))]
    init_tracing(cli.verbose);

    let result = run(cli).await;

    #[cfg(feature = "otel")]
    opentelemetry::global::shutdown_tracer_provider();

    match result {
        Ok(success) => {
            if success {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "Agent failed");
            ExitCode::from(2)
        }
    }
}

async fn run(cli: Cli) -> anyhow::Result<bool> {
    let server_url = cli.server;

    match cli.command {
        Commands::Run { file } => run_single(file, server_url.as_deref()).await,
        Commands::RunDir {
            dir,
            config,
            parallel,
            fail_fast,
            filter,
        } => run_directory(dir, config, parallel, fail_fast, filter, server_url.as_deref()).await,
        Commands::List { dir } => list_workflows(dir).await,
        Commands::Validate { path } => validate(path).await,
    }
}

struct TelemetryReporter {
    client: reqwest::Client,
    server_url: String,
    event_tx: mpsc::Sender<RunEvent>,
    agent_token: String,
}

impl TelemetryReporter {
    fn new(server_url: &str) -> (Arc<Self>, mpsc::Receiver<RunEvent>) {
        let (event_tx, event_rx) = mpsc::channel(1000);
        let agent_token = uuid::Uuid::new_v4().to_string();
        let reporter = Arc::new(Self {
            client: reqwest::Client::new(),
            server_url: server_url.to_string(),
            event_tx,
            agent_token,
        });
        (reporter, event_rx)
    }

    fn agent_token(&self) -> &str {
        &self.agent_token
    }

    #[tracing::instrument(skip(self))]
    async fn register_run(&self, run_id: &str, workflows_dir: &str) -> anyhow::Result<()> {
        let query = format!(
            r#"mutation {{
                registerRun(input: {{
                    runId: "{}",
                    workflowsDir: "{}",
                    startedAt: "{}",
                    agentToken: "{}"
                }}) {{
                    id
                }}
            }}"#,
            run_id,
            workflows_dir,
            Utc::now().to_rfc3339(),
            self.agent_token
        );

        let resp = self
            .client
            .post(format!("{}/graphql", self.server_url))
            .json(&serde_json::json!({ "query": query }))
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                tracing::info!("Run registered successfully");
                Ok(())
            }
            Ok(r) => {
                tracing::error!(
                    status = %r.status(),
                    error.type = "registration_error",
                    "Failed to register run with server"
                );
                Ok(())
            }
            Err(e) => {
                tracing::error!(
                    error = %e,
                    error.type = "connection_error",
                    "Failed to connect to server for run registration"
                );
                Ok(())
            }
        }
    }

    fn send_event(&self, event: RunEvent) {
        let _ = self.event_tx.try_send(event);
    }

    #[allow(dead_code)]
    async fn complete_run(&self, run_id: &str, success: bool) -> anyhow::Result<()> {
        let query = format!(
            r#"mutation {{
                completeRun(input: {{
                    runId: "{}",
                    success: {},
                    completedAt: "{}"
                }})
            }}"#,
            run_id,
            success,
            Utc::now().to_rfc3339()
        );

        let resp = self
            .client
            .post(format!("{}/graphql", self.server_url))
            .json(&serde_json::json!({ "query": query }))
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => Ok(()),
            Ok(r) => {
                tracing::warn!("Failed to complete run: {}", r.status());
                Ok(())
            }
            Err(e) => {
                tracing::warn!("Failed to connect to server: {}", e);
                Ok(())
            }
        }
    }
}

fn start_command_listener(
    server_url: &str,
    run_id: &str,
    agent_token: &str,
    cancelled: Arc<AtomicBool>,
) -> tokio::task::JoinHandle<()> {
    let ws_url = server_url
        .replace("http://", "ws://")
        .replace("https://", "wss://");
    let ws_url = format!("{}/graphql/ws", ws_url);

    let run_id = run_id.to_string();
    let agent_token = agent_token.to_string();

    tokio::spawn(async move {
        let mut backoff_ms = 1000u64;
        const MAX_BACKOFF_MS: u64 = 30000;

        loop {
            if cancelled.load(Ordering::SeqCst) {
                break;
            }

            match connect_async(&ws_url).await {
                Ok((ws_stream, _)) => {
                    backoff_ms = 1000;
                    tracing::debug!("Connected to command channel");

                    let (mut write, mut read) = ws_stream.split();

                    let init_msg = serde_json::json!({
                        "type": "connection_init",
                        "payload": {}
                    });
                    if write
                        .send(Message::Text(init_msg.to_string()))
                        .await
                        .is_err()
                    {
                        tracing::warn!("Failed to send init message, reconnecting...");
                        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                        continue;
                    }

                    let mut connection_active = true;
                    while connection_active {
                        if cancelled.load(Ordering::SeqCst) {
                            break;
                        }

                        match read.next().await {
                            Some(Ok(Message::Text(text))) => {
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                                    if json.get("type").and_then(|t| t.as_str()) == Some("connection_ack") {
                                        let subscribe_msg = serde_json::json!({
                                            "id": "1",
                                            "type": "subscribe",
                                            "payload": {
                                                "query": format!(
                                                    "subscription {{ commandsForRun(runId: \"{}\") {{ commandType runId timestamp agentToken }} }}",
                                                    run_id
                                                )
                                            }
                                        });
                                        if write
                                            .send(Message::Text(subscribe_msg.to_string()))
                                            .await
                                            .is_err()
                                        {
                                            connection_active = false;
                                        }
                                    } else if json.get("type").and_then(|t| t.as_str()) == Some("next") {
                                        if let Some(payload) = json.get("payload") {
                                            if let Some(data) = payload.get("data") {
                                                if let Some(cmd) = data.get("commandsForRun") {
                                                    let cmd_run_id = cmd.get("runId").and_then(|r| r.as_str());
                                                    let cmd_token = cmd.get("agentToken").and_then(|t| t.as_str());
                                                    let cmd_type = cmd.get("commandType").and_then(|c| c.as_str());

                                                    if cmd_run_id != Some(run_id.as_str()) {
                                                        tracing::warn!(
                                                            "Ignoring command for different run: {:?}",
                                                            cmd_run_id
                                                        );
                                                        continue;
                                                    }

                                                    if cmd_token != Some(agent_token.as_str()) {
                                                        tracing::warn!(
                                                            "Ignoring command with invalid agent token"
                                                        );
                                                        continue;
                                                    }

                                                    match cmd_type {
                                                        Some("STOP") => {
                                                            tracing::info!("Received authenticated STOP command from server");
                                                            cancelled.store(true, Ordering::SeqCst);
                                                            return;
                                                        }
                                                        Some("PAUSE") | Some("RESUME") => {
                                                            tracing::debug!("Received {:?} command (not implemented)", cmd_type);
                                                        }
                                                        Some(other) => {
                                                            tracing::warn!("Ignoring unknown command type: {}", other);
                                                        }
                                                        None => {
                                                            tracing::warn!("Received command with no type");
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Some(Ok(_)) => {}
                            Some(Err(e)) => {
                                tracing::warn!("WebSocket error: {}, reconnecting...", e);
                                connection_active = false;
                            }
                            None => {
                                tracing::debug!("WebSocket connection closed, reconnecting...");
                                connection_active = false;
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!("Failed to connect to command channel: {}", e);
                }
            }

            if cancelled.load(Ordering::SeqCst) {
                break;
            }

            tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
            backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_MS);
        }
    })
}

#[tracing::instrument(skip(client, events), fields(event_count = events.len()))]
async fn flush_events(
    client: &reqwest::Client,
    server_url: &str,
    events: Vec<RunEvent>,
) -> anyhow::Result<()> {
    if events.is_empty() {
        return Ok(());
    }

    let event_type_map = |e: &RunEvent| -> &str {
        match e.event_type {
            EventType::RunStarted => "RUN_STARTED",
            EventType::RunCompleted => "RUN_COMPLETED",
            EventType::WorkflowStarted => "WORKFLOW_STARTED",
            EventType::WorkflowCompleted => "WORKFLOW_COMPLETED",
            EventType::WorkflowSkipped => "WORKFLOW_SKIPPED",
            EventType::JobStarted => "JOB_STARTED",
            EventType::JobCompleted => "JOB_COMPLETED",
            EventType::StepStarted => "STEP_STARTED",
            EventType::StepCompleted => "STEP_COMPLETED",
        }
    };

    let events_str: Vec<String> = events
        .iter()
        .map(|e| {
            let mut parts = vec![
                format!("eventType: {}", event_type_map(e)),
                format!("runId: \"{}\"", e.run_id),
                format!("timestamp: \"{}\"", e.timestamp.to_rfc3339()),
            ];
            if let Some(ref wn) = e.workflow_name {
                parts.push(format!("workflowName: \"{}\"", wn));
            }
            if let Some(ref jn) = e.job_name {
                parts.push(format!("jobName: \"{}\"", jn));
            }
            if let Some(si) = e.step_index {
                parts.push(format!("stepIndex: {}", si));
            }
            if let Some(ref sn) = e.step_name {
                parts.push(format!("stepName: \"{}\"", sn));
            }
            if let Some(s) = e.success {
                parts.push(format!("success: {}", s));
            }
            if let Some(ref err) = e.error {
                parts.push(format!("error: \"{}\"", err.replace("\"", "\\\"")));
            }
            if let Some(ref r) = e.reason {
                parts.push(format!("reason: \"{}\"", r.replace("\"", "\\\"")));
            }
            format!("{{ {} }}", parts.join(", "))
        })
        .collect();

    let query = format!(
        r#"mutation {{ reportEvents(events: [{}]) }}"#,
        events_str.join(", ")
    );

    let resp = client
        .post(format!("{}/graphql", server_url))
        .json(&serde_json::json!({ "query": query }))
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => Ok(()),
        Ok(r) => {
            tracing::error!(
                status = %r.status(),
                error.type = "http_error",
                "Failed to report events to server"
            );
            Ok(())
        }
        Err(e) => {
            tracing::error!(
                error = %e,
                error.type = "connection_error",
                "Failed to connect to telemetry server"
            );
            Ok(())
        }
    }
}

async fn run_single(file: PathBuf, _server_url: Option<&str>) -> anyhow::Result<bool> {
    if !file.exists() {
        anyhow::bail!("Workflow file not found: {}", file.display());
    }

    println!("Running workflow: {}\n", file.display());

    let workflow = WorkflowLoader::load_file(&file)?;
    let mut executor = Executor::new();
    let result = executor.run(workflow).await?;

    print_workflow_result(&result);
    Ok(result.success)
}

#[tracing::instrument(skip(server_url), fields(workflows_dir = %dir.display()))]
async fn run_directory(
    dir: PathBuf,
    config_path: Option<PathBuf>,
    parallel: Option<usize>,
    fail_fast: bool,
    filter: Option<String>,
    server_url: Option<&str>,
) -> anyhow::Result<bool> {
    if !dir.exists() {
        tracing::error!(path = %dir.display(), "Directory not found");
        anyhow::bail!("Directory not found: {}", dir.display());
    }

    println!("Running workflows from: {}\n", dir.display());

    let config_file = config_path.unwrap_or_else(|| dir.join("runner.yaml"));
    let mut config = if config_file.exists() {
        println!("Using config: {}\n", config_file.display());
        RunnerConfig::load(&config_file)?
    } else {
        RunnerConfig::default()
    };

    if let Some(p) = parallel {
        config.parallel = p;
    }
    if fail_fast {
        config.fail_fast = true;
    }

    let run_id = uuid::Uuid::new_v4().to_string();
    let cancelled = Arc::new(AtomicBool::new(false));

    let telemetry = if let Some(server) = server_url {
        let (reporter, mut event_rx) = TelemetryReporter::new(server);

        let _ = reporter
            .register_run(&run_id, &dir.display().to_string())
            .await;

        let command_handle = start_command_listener(server, &run_id, reporter.agent_token(), cancelled.clone());

        let flush_server_url = server.to_string();
        let flush_cancelled = cancelled.clone();
        let flush_handle = tokio::spawn(async move {
            let client = reqwest::Client::new();
            let mut buffer = Vec::new();
            let mut interval = tokio::time::interval(Duration::from_millis(500));

            loop {
                if flush_cancelled.load(Ordering::SeqCst) {
                    if !buffer.is_empty() {
                        let _ = flush_events(&client, &flush_server_url, buffer).await;
                    }
                    break;
                }

                tokio::select! {
                    _ = interval.tick() => {
                        if !buffer.is_empty() {
                            let events = std::mem::take(&mut buffer);
                            let _ = flush_events(&client, &flush_server_url, events).await;
                        }
                    }
                    event = event_rx.recv() => {
                        match event {
                            Some(e) => buffer.push(e),
                            None => {
                                if !buffer.is_empty() {
                                    let _ = flush_events(&client, &flush_server_url, buffer).await;
                                }
                                break;
                            }
                        }
                    }
                }
            }
        });

        Some((reporter, flush_handle, command_handle, server.to_string()))
    } else {
        None
    };

    let has_profiles = config.has_profiles();
    let runner = WorkflowDirectoryRunner::with_config(&dir, config);

    let result = if has_profiles {
        let result = runner.run_multi().await?;

        println!("\n=== Results ===\n");
        println!(
            "Overall: {}\n",
            if result.success { "PASS" } else { "FAIL" }
        );

        let mut profile_names: Vec<_> = result.profiles.keys().collect();
        profile_names.sort();

        for profile_name in profile_names {
            let dir_result = &result.profiles[profile_name];
            let status = if dir_result.success { "✓" } else { "✗" };
            println!("  {} Profile: {}", status, profile_name);

            for level in &dir_result.execution_order {
                for name in level {
                    if let Some(wr) = dir_result.workflows.get(name) {
                        let wf_status = if wr.success { "✓" } else { "✗" };
                        println!("      {} {}", wf_status, name);

                        if let Some((ref reporter, _, _, _)) = telemetry {
                            reporter.send_event(RunEvent::workflow_completed(
                                &run_id,
                                name,
                                wr.success,
                                None,
                            ));
                        }
                    }
                }
            }

            for name in &dir_result.skipped {
                if let Some((ref reporter, _, _, _)) = telemetry {
                    reporter.send_event(RunEvent::workflow_skipped(
                        &run_id,
                        name,
                        "Dependency failed",
                    ));
                }
            }

            if !dir_result.skipped.is_empty() {
                println!("      Skipped: {}", dir_result.skipped.join(", "));
            }
            println!();
        }

        result.success
    } else {
        let mut runner = runner;
        if let Some(prefix) = filter {
            runner = runner.filter(move |name| name.starts_with(&prefix));
        }

        let result = runner.run().await?;

        println!("\n=== Results ===\n");
        println!(
            "Overall: {}\n",
            if result.success { "PASS" } else { "FAIL" }
        );

        for level in &result.execution_order {
            for name in level {
                if let Some(wr) = result.workflows.get(name) {
                    let status = if wr.success { "✓" } else { "✗" };
                    println!("  {} {}", status, name);

                    if let Some((ref reporter, _, _, _)) = telemetry {
                        reporter.send_event(RunEvent::workflow_completed(
                            &run_id,
                            name,
                            wr.success,
                            None,
                        ));
                    }
                }
            }
        }

        for name in &result.skipped {
            if let Some((ref reporter, _, _, _)) = telemetry {
                reporter.send_event(RunEvent::workflow_skipped(
                    &run_id,
                    name,
                    "Dependency failed",
                ));
            }
        }

        if !result.skipped.is_empty() {
            println!("\nSkipped: {}", result.skipped.join(", "));
        }

        result.success
    };

    let was_cancelled = cancelled.load(Ordering::SeqCst);
    let final_result = if was_cancelled {
        println!("\n=== Run cancelled by server ===\n");
        false
    } else {
        result
    };

    if let Some((reporter, flush_handle, command_handle, server)) = telemetry {
        drop(reporter);
        let _ = flush_handle.await;
        command_handle.abort();

        let client = reqwest::Client::new();
        let query = format!(
            r#"mutation {{
                completeRun(input: {{
                    runId: "{}",
                    success: {},
                    completedAt: "{}"
                }})
            }}"#,
            run_id,
            final_result,
            Utc::now().to_rfc3339()
        );

        let _ = client
            .post(format!("{}/graphql", server))
            .json(&serde_json::json!({ "query": query }))
            .send()
            .await;
    }

    Ok(final_result)
}

async fn list_workflows(dir: PathBuf) -> anyhow::Result<bool> {
    if !dir.exists() {
        anyhow::bail!("Directory not found: {}", dir.display());
    }

    let workflows = WorkflowLoader::load_directory(&dir)?;

    if workflows.is_empty() {
        println!("No workflows found in: {}", dir.display());
        return Ok(true);
    }

    println!("Workflows in {}:\n", dir.display());

    let dag = WorkflowDAG::build(workflows.clone())?;

    for w in &workflows {
        if w.depends_on.workflows.is_empty() {
            println!("  {} (no dependencies)", w.name);
        } else {
            let deps = w.depends_on.workflows.join(", ");
            let always = if w.depends_on.always { " [always]" } else { "" };
            println!("  {} (depends on: {}{})", w.name, deps, always);
        }
    }

    println!("\nExecution order:");
    for (i, level) in dag.execution_levels().iter().enumerate() {
        println!("  Level {}: [{}]", i, level.join(", "));
    }

    Ok(true)
}

async fn validate(path: PathBuf) -> anyhow::Result<bool> {
    if !path.exists() {
        anyhow::bail!("Path not found: {}", path.display());
    }

    let is_dir = path.is_dir();

    if is_dir {
        let workflows = WorkflowLoader::load_directory(&path)?;
        if workflows.is_empty() {
            println!("No workflows found in: {}", path.display());
            return Ok(true);
        }

        let dag = WorkflowDAG::build(workflows.clone())?;
        println!(
            "✓ {} workflows validated, {} execution levels",
            workflows.len(),
            dag.execution_levels().len()
        );
    } else {
        let _workflow = WorkflowLoader::load_file(&path)?;
        println!("✓ {} is valid", path.display());
    }

    Ok(true)
}

fn print_workflow_result(result: &WorkflowResult) {
    println!("\n=== Workflow Result ===\n");
    println!("Success: {}", if result.success { "YES" } else { "NO" });
    println!("Run ID: {}\n", result.run_id);

    for (job_name, job_result) in &result.jobs {
        let status = if job_result.success { "✓" } else { "✗" };
        println!("{} Job: {}", status, job_name);

        for (i, step) in job_result.steps.iter().enumerate() {
            let step_status = if step.success { "  ✓" } else { "  ✗" };
            println!("  {} Step {}", step_status, i + 1);
            if let Some(err) = &step.error {
                println!("      Error: {}", err);
            }
        }
    }
}
