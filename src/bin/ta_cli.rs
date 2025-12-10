use std::process::ExitCode;
use std::time::Duration;

use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use futures::stream::StreamExt;
use futures::SinkExt;
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

const DEFAULT_SERVER_URL: &str = "http://localhost:3000";

fn get_server_url() -> String {
    std::env::var("TA_SERVER_URL").unwrap_or_else(|_| DEFAULT_SERVER_URL.to_string())
}

fn get_graphql_url() -> String {
    format!("{}/graphql", get_server_url())
}

fn get_ws_url() -> String {
    let server = get_server_url();
    let ws = server
        .replace("http://", "ws://")
        .replace("https://", "wss://");
    format!("{}/graphql/ws", ws)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Run {
    id: String,
    status: String,
    workflows_dir: String,
    started_at: String,
    completed_at: Option<String>,
    event_count: i32,
    is_paused: bool,
    paused_at: Option<String>,
    current_workflow: Option<String>,
    current_job: Option<String>,
    current_step: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunEvent {
    event_type: String,
    run_id: String,
    timestamp: String,
    workflow_name: Option<String>,
    job_name: Option<String>,
    step_index: Option<i32>,
    step_name: Option<String>,
    success: Option<bool>,
    error: Option<String>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphQLResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize)]
struct GraphQLError {
    message: String,
}

#[derive(Parser)]
#[command(name = "ta-cli")]
#[command(about = "CLI for Testing Actions workflow dashboard", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check server health
    Health,

    /// List all runs
    Runs {
        /// Maximum number of runs to return
        #[arg(short, long, default_value = "20")]
        limit: i32,

        /// Output as JSON
        #[arg(short, long)]
        json: bool,
    },

    /// Get details of a specific run
    Run {
        /// The run ID
        run_id: String,

        /// Output as JSON
        #[arg(short, long)]
        json: bool,
    },

    /// Get events for a specific run
    Events {
        /// The run ID
        run_id: String,

        /// Output as JSON
        #[arg(short, long)]
        json: bool,
    },

    /// Watch events for a run in real-time (with timeout)
    Watch {
        /// The run ID
        run_id: String,

        /// Timeout in seconds after last event
        #[arg(short, long, default_value = "30")]
        timeout: u64,

        /// Output as JSON
        #[arg(short, long)]
        json: bool,
    },

    /// Stop a running workflow
    Stop {
        /// The run ID
        run_id: String,
    },

    /// Pause a running workflow
    Pause {
        /// The run ID
        run_id: String,
    },

    /// Resume a paused workflow
    Resume {
        /// The run ID
        run_id: String,
    },
}

async fn graphql_query(query: &str) -> anyhow::Result<serde_json::Value> {
    let client = reqwest::Client::new();
    let response = client
        .post(get_graphql_url())
        .json(&serde_json::json!({ "query": query }))
        .send()
        .await?;

    let result: GraphQLResponse<serde_json::Value> = response.json().await?;

    if let Some(errors) = result.errors {
        let messages: Vec<_> = errors.iter().map(|e| e.message.as_str()).collect();
        anyhow::bail!("GraphQL error: {}", messages.join(", "));
    }

    result.data.ok_or_else(|| anyhow::anyhow!("No data returned"))
}

fn status_icon(status: &str) -> &'static str {
    match status.to_uppercase().as_str() {
        "SUCCESS" => "✓",
        "FAILED" => "✗",
        "RUNNING" => "●",
        "PAUSED" => "⏸",
        "PENDING" => "○",
        "CANCELLED" => "⊘",
        "SKIPPED" => "⊖",
        _ => "?",
    }
}

fn format_timestamp(ts: &str) -> String {
    if let Ok(dt) = ts.parse::<DateTime<Utc>>() {
        dt.format("%Y-%m-%d %H:%M:%S").to_string()
    } else {
        ts.to_string()
    }
}

async fn cmd_health() -> anyhow::Result<()> {
    let data = graphql_query("query { health }").await?;
    let health = data["health"].as_str().unwrap_or("unknown");
    println!("Server: {}", get_server_url());
    println!("Status: {}", health);
    Ok(())
}

async fn cmd_runs(limit: i32, json_output: bool) -> anyhow::Result<()> {
    let query = format!(
        r#"query {{
            runs(limit: {}) {{
                id
                status
                workflowsDir
                startedAt
                completedAt
                eventCount
                isPaused
                pausedAt
                currentWorkflow
                currentJob
                currentStep
            }}
        }}"#,
        limit
    );

    let data = graphql_query(&query).await?;
    let runs: Vec<Run> = serde_json::from_value(data["runs"].clone())?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&runs)?);
    } else if runs.is_empty() {
        println!("No runs found.");
    } else {
        println!("Runs:\n");
        for run in runs {
            let icon = status_icon(&run.status);
            let completed = run
                .completed_at
                .map(|c| format!(" → {}", format_timestamp(&c)))
                .unwrap_or_default();
            println!("  {} {}", icon, run.id);
            println!("    Status: {}", run.status);
            println!("    Dir: {}", run.workflows_dir);
            println!("    Started: {}{}", format_timestamp(&run.started_at), completed);
            println!("    Events: {}", run.event_count);
            if run.is_paused {
                if let Some(ref paused_at) = run.paused_at {
                    println!("    Paused: {}", format_timestamp(paused_at));
                }
                if let Some(ref wf) = run.current_workflow {
                    print!("    Position: {}", wf);
                    if let Some(ref job) = run.current_job {
                        print!(" > {}", job);
                    }
                    if let Some(step) = run.current_step {
                        print!(" > step {}", step);
                    }
                    println!();
                }
            }
            println!();
        }
    }

    Ok(())
}

async fn cmd_run(run_id: &str, json_output: bool) -> anyhow::Result<()> {
    let query = format!(
        r#"query {{
            run(id: "{}") {{
                id
                status
                workflowsDir
                startedAt
                completedAt
                eventCount
                isPaused
                pausedAt
                currentWorkflow
                currentJob
                currentStep
            }}
        }}"#,
        run_id
    );

    let data = graphql_query(&query).await?;

    if data["run"].is_null() {
        anyhow::bail!("Run not found: {}", run_id);
    }

    let run: Run = serde_json::from_value(data["run"].clone())?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&run)?);
    } else {
        let icon = status_icon(&run.status);
        println!("Run: {}", run.id);
        println!("Status: {} {}", icon, run.status);
        println!("Directory: {}", run.workflows_dir);
        println!("Started: {}", format_timestamp(&run.started_at));
        if let Some(completed) = &run.completed_at {
            println!("Completed: {}", format_timestamp(completed));
        }
        println!("Events: {}", run.event_count);
        if run.is_paused {
            if let Some(ref paused_at) = run.paused_at {
                println!("Paused: {}", format_timestamp(paused_at));
            }
            if let Some(ref wf) = run.current_workflow {
                print!("Position: {}", wf);
                if let Some(ref job) = run.current_job {
                    print!(" > {}", job);
                }
                if let Some(step) = run.current_step {
                    print!(" > step {}", step);
                }
                println!();
            }
        }
    }

    Ok(())
}

async fn cmd_events(run_id: &str, json_output: bool) -> anyhow::Result<()> {
    let query = format!(
        r#"query {{
            runEvents(runId: "{}") {{
                eventType
                runId
                timestamp
                workflowName
                jobName
                stepIndex
                stepName
                success
                error
                reason
            }}
        }}"#,
        run_id
    );

    let data = graphql_query(&query).await?;
    let events: Vec<RunEvent> = serde_json::from_value(data["runEvents"].clone())?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&events)?);
    } else if events.is_empty() {
        println!("No events found.");
    } else {
        println!("Events ({}):\n", events.len());
        for event in events {
            print_event_text(&event);
        }
    }

    Ok(())
}

fn print_event_text(event: &RunEvent) {
    let ts = format_timestamp(&event.timestamp);
    let mut detail = String::new();

    if let Some(ref wn) = event.workflow_name {
        detail.push_str(&format!(" workflow={}", wn));
    }
    if let Some(ref jn) = event.job_name {
        detail.push_str(&format!(" job={}", jn));
    }
    if let Some(ref sn) = event.step_name {
        detail.push_str(&format!(" step={}", sn));
    }
    if let Some(si) = event.step_index {
        detail.push_str(&format!(" index={}", si));
    }
    if let Some(success) = event.success {
        detail.push_str(&format!(" success={}", success));
    }
    if let Some(ref err) = event.error {
        detail.push_str(&format!(" error=\"{}\"", err));
    }
    if let Some(ref reason) = event.reason {
        detail.push_str(&format!(" reason=\"{}\"", reason));
    }

    println!("[{}] {}{}", ts, event.event_type, detail);
}

async fn cmd_watch(run_id: &str, timeout_secs: u64, json_output: bool) -> anyhow::Result<()> {
    eprintln!("Watching run {} (timeout: {}s)...", run_id, timeout_secs);

    let ws_url = get_ws_url();
    let (ws_stream, _) = connect_async(&ws_url).await?;
    let (mut write, mut read) = ws_stream.split();

    let init_msg = serde_json::json!({
        "type": "connection_init",
        "payload": {}
    });
    write.send(Message::Text(init_msg.to_string())).await?;

    let mut subscribed = false;
    let timeout = Duration::from_secs(timeout_secs);

    loop {
        let result = tokio::time::timeout(timeout, read.next()).await;

        match result {
            Ok(Some(Ok(Message::Text(text)))) => {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                    let msg_type = json.get("type").and_then(|t| t.as_str());

                    match msg_type {
                        Some("connection_ack") if !subscribed => {
                            let subscribe_msg = serde_json::json!({
                                "id": "1",
                                "type": "subscribe",
                                "payload": {
                                    "query": format!(
                                        r#"subscription {{
                                            eventsForRun(runId: "{}") {{
                                                eventType
                                                runId
                                                timestamp
                                                workflowName
                                                jobName
                                                stepIndex
                                                stepName
                                                success
                                                error
                                                reason
                                            }}
                                        }}"#,
                                        run_id
                                    )
                                }
                            });
                            write.send(Message::Text(subscribe_msg.to_string())).await?;
                            subscribed = true;
                        }
                        Some("next") => {
                            if let Some(payload) = json.get("payload") {
                                if let Some(data) = payload.get("data") {
                                    if let Some(event_val) = data.get("eventsForRun") {
                                        if let Ok(event) =
                                            serde_json::from_value::<RunEvent>(event_val.clone())
                                        {
                                            if json_output {
                                                println!("{}", serde_json::to_string(&event)?);
                                            } else {
                                                print_event_text(&event);
                                            }

                                            if event.event_type == "RUN_COMPLETED" {
                                                eprintln!("Run completed.");
                                                return Ok(());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Some("complete") => {
                            eprintln!("Subscription completed.");
                            return Ok(());
                        }
                        Some("error") => {
                            eprintln!("Subscription error: {:?}", json.get("payload"));
                            return Ok(());
                        }
                        _ => {}
                    }
                }
            }
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(e))) => {
                anyhow::bail!("WebSocket error: {}", e);
            }
            Ok(None) => {
                eprintln!("Connection closed.");
                return Ok(());
            }
            Err(_) => {
                eprintln!("Timeout - no events for {}s.", timeout_secs);
                return Ok(());
            }
        }
    }
}

async fn cmd_stop(run_id: &str) -> anyhow::Result<()> {
    let query = format!(
        r#"mutation {{
            stopRun(runId: "{}")
        }}"#,
        run_id
    );

    let data = graphql_query(&query).await?;
    let result = data["stopRun"].as_bool().unwrap_or(false);

    if result {
        println!("Stop command sent to run: {}", run_id);
    } else {
        println!("Failed to stop run: {}", run_id);
    }

    Ok(())
}

async fn cmd_pause(run_id: &str) -> anyhow::Result<()> {
    let query = format!(
        r#"mutation {{
            pauseRun(runId: "{}")
        }}"#,
        run_id
    );

    let data = graphql_query(&query).await?;
    let result = data["pauseRun"].as_bool().unwrap_or(false);

    if result {
        println!("Pause command sent to run: {}", run_id);
    } else {
        println!("Failed to pause run: {}", run_id);
    }

    Ok(())
}

async fn cmd_resume(run_id: &str) -> anyhow::Result<()> {
    let query = format!(
        r#"mutation {{
            resumeRun(runId: "{}")
        }}"#,
        run_id
    );

    let data = graphql_query(&query).await?;
    let result = data["resumeRun"].as_bool().unwrap_or(false);

    if result {
        println!("Resume command sent to run: {}", run_id);
    } else {
        println!("Failed to resume run: {}", run_id);
    }

    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Health => cmd_health().await,
        Commands::Runs { limit, json } => cmd_runs(limit, json).await,
        Commands::Run { run_id, json } => cmd_run(&run_id, json).await,
        Commands::Events { run_id, json } => cmd_events(&run_id, json).await,
        Commands::Watch { run_id, timeout, json } => cmd_watch(&run_id, timeout, json).await,
        Commands::Stop { run_id } => cmd_stop(&run_id).await,
        Commands::Pause { run_id } => cmd_pause(&run_id).await,
        Commands::Resume { run_id } => cmd_resume(&run_id).await,
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::from(1)
        }
    }
}
