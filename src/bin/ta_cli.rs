use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};
use futures::stream::StreamExt;
use futures::SinkExt;
use testing_actions::client::{
    format_timestamp, status_icon, subscriptions, ws_url, GraphQLClient, Run, RunEventResponse,
};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

const DEFAULT_SERVER_URL: &str = "http://localhost:3000";

fn get_server_url() -> String {
    std::env::var("TA_SERVER_URL").unwrap_or_else(|_| DEFAULT_SERVER_URL.to_string())
}

#[derive(Parser)]
#[command(name = "ta-ctl")]
#[command(about = "Control and query Testing Actions workflow server", long_about = None)]
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
        #[arg(short, long, default_value = "20")]
        limit: i32,

        #[arg(short, long)]
        json: bool,
    },

    /// Get details of a specific run
    Run {
        run_id: String,

        #[arg(short, long)]
        json: bool,
    },

    /// Get events for a specific run
    Events {
        run_id: String,

        #[arg(short, long)]
        json: bool,
    },

    /// Watch events for a run in real-time (with timeout)
    Watch {
        run_id: String,

        #[arg(short, long, default_value = "30")]
        timeout: u64,

        #[arg(short, long)]
        json: bool,
    },

    /// Stop a running workflow
    Stop { run_id: String },

    /// Pause a running workflow
    Pause { run_id: String },

    /// Resume a paused workflow
    Resume { run_id: String },
}

fn print_run(run: &Run) {
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
        print_position(
            run.current_workflow.as_deref(),
            run.current_job.as_deref(),
            run.current_step,
        );
    }
}

fn print_run_summary(run: &Run) {
    let icon = status_icon(&run.status);
    let completed = run
        .completed_at
        .as_ref()
        .map(|c| format!(" → {}", format_timestamp(c)))
        .unwrap_or_default();
    println!("  {} {}", icon, run.id);
    println!("    Status: {}", run.status);
    println!("    Dir: {}", run.workflows_dir);
    println!(
        "    Started: {}{}",
        format_timestamp(&run.started_at),
        completed
    );
    println!("    Events: {}", run.event_count);
    if run.is_paused {
        if let Some(ref paused_at) = run.paused_at {
            println!("    Paused: {}", format_timestamp(paused_at));
        }
        if run.current_workflow.is_some() {
            print!("    ");
            print_position(
                run.current_workflow.as_deref(),
                run.current_job.as_deref(),
                run.current_step,
            );
        }
    }
    println!();
}

fn print_position(workflow: Option<&str>, job: Option<&str>, step: Option<i32>) {
    if let Some(wf) = workflow {
        print!("Position: {}", wf);
        if let Some(j) = job {
            print!(" > {}", j);
        }
        if let Some(s) = step {
            print!(" > step {}", s);
        }
        println!();
    }
}

fn print_event(event: &RunEventResponse) {
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

async fn cmd_health() -> anyhow::Result<()> {
    let server_url = get_server_url();
    let client = GraphQLClient::new(&server_url);
    let health = client.health().await?;
    println!("Server: {}", server_url);
    println!("Status: {}", health);
    Ok(())
}

async fn cmd_runs(limit: i32, json_output: bool) -> anyhow::Result<()> {
    let client = GraphQLClient::new(&get_server_url());
    let runs = client.get_runs(limit).await?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&runs)?);
    } else if runs.is_empty() {
        println!("No runs found.");
    } else {
        println!("Runs:\n");
        for run in runs {
            print_run_summary(&run);
        }
    }

    Ok(())
}

async fn cmd_run(run_id: &str, json_output: bool) -> anyhow::Result<()> {
    let client = GraphQLClient::new(&get_server_url());
    let run = client
        .get_run(run_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Run not found: {}", run_id))?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&run)?);
    } else {
        print_run(&run);
    }

    Ok(())
}

async fn cmd_events(run_id: &str, json_output: bool) -> anyhow::Result<()> {
    let client = GraphQLClient::new(&get_server_url());
    let events = client.get_run_events(run_id).await?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&events)?);
    } else if events.is_empty() {
        println!("No events found.");
    } else {
        println!("Events ({}):\n", events.len());
        for event in events {
            print_event(&event);
        }
    }

    Ok(())
}

async fn cmd_watch(run_id: &str, timeout_secs: u64, json_output: bool) -> anyhow::Result<()> {
    eprintln!("Watching run {} (timeout: {}s)...", run_id, timeout_secs);

    let ws = ws_url(&get_server_url());
    let (ws_stream, _) = connect_async(&ws).await?;
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
                                    "query": subscriptions::events_for_run(run_id)
                                }
                            });
                            write.send(Message::Text(subscribe_msg.to_string())).await?;
                            subscribed = true;
                        }
                        Some("next") => {
                            if let Some(payload) = json.get("payload") {
                                if let Some(data) = payload.get("data") {
                                    if let Some(event_val) = data.get("eventsForRun") {
                                        if let Ok(event) = serde_json::from_value::<RunEventResponse>(
                                            event_val.clone(),
                                        ) {
                                            if json_output {
                                                println!("{}", serde_json::to_string(&event)?);
                                            } else {
                                                print_event(&event);
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
    let client = GraphQLClient::new(&get_server_url());
    let result = client.stop_run(run_id).await?;

    if result {
        println!("Stop command sent to run: {}", run_id);
    } else {
        println!("Failed to stop run: {}", run_id);
    }

    Ok(())
}

async fn cmd_pause(run_id: &str) -> anyhow::Result<()> {
    let client = GraphQLClient::new(&get_server_url());
    let result = client.pause_run(run_id).await?;

    if result {
        println!("Pause command sent to run: {}", run_id);
    } else {
        println!("Failed to pause run: {}", run_id);
    }

    Ok(())
}

async fn cmd_resume(run_id: &str) -> anyhow::Result<()> {
    let client = GraphQLClient::new(&get_server_url());
    let result = client.resume_run(run_id).await?;

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
        Commands::Watch {
            run_id,
            timeout,
            json,
        } => cmd_watch(&run_id, timeout, json).await,
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
