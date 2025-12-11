use crate::client::types::{GraphQLResponse, Run, RunEvent, RunEventResponse};

const RUN_FIELDS: &str = r#"
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
"#;

const RUN_EVENT_FIELDS: &str = r#"
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
"#;

pub fn graphql_url(server_url: &str) -> String {
    format!("{}/graphql", server_url)
}

pub fn ws_url(server_url: &str) -> String {
    let ws = server_url
        .replace("http://", "ws://")
        .replace("https://", "wss://");
    format!("{}/graphql/ws", ws)
}

pub mod queries {
    use super::*;

    pub fn health() -> String {
        "query { health }".to_string()
    }

    pub fn runs(limit: i32) -> String {
        format!(
            "query {{ runs(limit: {}) {{ {} }} }}",
            limit,
            RUN_FIELDS.trim()
        )
    }

    pub fn run(id: &str) -> String {
        format!(
            r#"query {{ run(id: "{}") {{ {} }} }}"#,
            id,
            RUN_FIELDS.trim()
        )
    }

    pub fn run_events(run_id: &str) -> String {
        format!(
            r#"query {{ runEvents(runId: "{}") {{ {} }} }}"#,
            run_id,
            RUN_EVENT_FIELDS.trim()
        )
    }
}

pub mod mutations {
    pub fn register_run(
        run_id: &str,
        workflows_dir: &str,
        started_at: &str,
        agent_token: &str,
    ) -> String {
        format!(
            r#"mutation {{
                registerRun(input: {{
                    runId: "{}"
                    workflowsDir: "{}"
                    startedAt: "{}"
                    agentToken: "{}"
                }}) {{
                    id
                    status
                }}
            }}"#,
            run_id, workflows_dir, started_at, agent_token
        )
    }

    pub fn complete_run(run_id: &str, success: bool, completed_at: &str) -> String {
        format!(
            r#"mutation {{
                completeRun(input: {{
                    runId: "{}"
                    success: {}
                    completedAt: "{}"
                }})
            }}"#,
            run_id, success, completed_at
        )
    }

    pub fn report_events(events_json: &str) -> String {
        format!(
            r#"mutation {{
                reportEvents(events: {})
            }}"#,
            events_json
        )
    }

    pub fn stop_run(run_id: &str) -> String {
        format!(r#"mutation {{ stopRun(runId: "{}") }}"#, run_id)
    }

    pub fn pause_run(run_id: &str) -> String {
        format!(r#"mutation {{ pauseRun(runId: "{}") }}"#, run_id)
    }

    pub fn resume_run(run_id: &str) -> String {
        format!(r#"mutation {{ resumeRun(runId: "{}") }}"#, run_id)
    }

    pub fn cancel_run(run_id: &str) -> String {
        format!(r#"mutation {{ cancelRun(runId: "{}") }}"#, run_id)
    }
}

pub mod subscriptions {
    use super::*;

    pub fn events_for_run(run_id: &str) -> String {
        format!(
            r#"subscription {{ eventsForRun(runId: "{}") {{ {} }} }}"#,
            run_id,
            RUN_EVENT_FIELDS.trim()
        )
    }

    pub fn commands_for_run(run_id: &str) -> String {
        format!(
            r#"subscription {{ commandsForRun(runId: "{}") {{ commandType runId timestamp agentToken }} }}"#,
            run_id
        )
    }
}

pub struct GraphQLClient {
    client: reqwest::Client,
    url: String,
}

impl GraphQLClient {
    pub fn new(server_url: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            url: graphql_url(server_url),
        }
    }

    pub async fn query<T: serde::de::DeserializeOwned>(&self, query: &str) -> anyhow::Result<T> {
        let response = self
            .client
            .post(&self.url)
            .json(&serde_json::json!({ "query": query }))
            .send()
            .await?;

        let result: GraphQLResponse<T> = response.json().await?;

        if let Some(errors) = result.errors {
            let messages: Vec<_> = errors.iter().map(|e| e.message.as_str()).collect();
            anyhow::bail!("GraphQL error: {}", messages.join(", "));
        }

        result
            .data
            .ok_or_else(|| anyhow::anyhow!("No data returned"))
    }

    pub async fn health(&self) -> anyhow::Result<String> {
        #[derive(serde::Deserialize)]
        struct Response {
            health: String,
        }
        let data: Response = self.query(&queries::health()).await?;
        Ok(data.health)
    }

    pub async fn get_runs(&self, limit: i32) -> anyhow::Result<Vec<Run>> {
        #[derive(serde::Deserialize)]
        struct Response {
            runs: Vec<Run>,
        }
        let data: Response = self.query(&queries::runs(limit)).await?;
        Ok(data.runs)
    }

    pub async fn get_run(&self, id: &str) -> anyhow::Result<Option<Run>> {
        #[derive(serde::Deserialize)]
        struct Response {
            run: Option<Run>,
        }
        let data: Response = self.query(&queries::run(id)).await?;
        Ok(data.run)
    }

    pub async fn get_run_events(&self, run_id: &str) -> anyhow::Result<Vec<RunEventResponse>> {
        #[derive(serde::Deserialize)]
        struct Response {
            #[serde(rename = "runEvents")]
            run_events: Vec<RunEventResponse>,
        }
        let data: Response = self.query(&queries::run_events(run_id)).await?;
        Ok(data.run_events)
    }

    pub async fn stop_run(&self, run_id: &str) -> anyhow::Result<bool> {
        #[derive(serde::Deserialize)]
        struct Response {
            #[serde(rename = "stopRun")]
            stop_run: bool,
        }
        let data: Response = self.query(&mutations::stop_run(run_id)).await?;
        Ok(data.stop_run)
    }

    pub async fn pause_run(&self, run_id: &str) -> anyhow::Result<bool> {
        #[derive(serde::Deserialize)]
        struct Response {
            #[serde(rename = "pauseRun")]
            pause_run: bool,
        }
        let data: Response = self.query(&mutations::pause_run(run_id)).await?;
        Ok(data.pause_run)
    }

    pub async fn resume_run(&self, run_id: &str) -> anyhow::Result<bool> {
        #[derive(serde::Deserialize)]
        struct Response {
            #[serde(rename = "resumeRun")]
            resume_run: bool,
        }
        let data: Response = self.query(&mutations::resume_run(run_id)).await?;
        Ok(data.resume_run)
    }

    pub async fn register_run(
        &self,
        run_id: &str,
        workflows_dir: &str,
        started_at: &str,
        agent_token: &str,
    ) -> anyhow::Result<()> {
        let query = mutations::register_run(run_id, workflows_dir, started_at, agent_token);
        let _: serde_json::Value = self.query(&query).await?;
        Ok(())
    }

    pub async fn complete_run(
        &self,
        run_id: &str,
        success: bool,
        completed_at: &str,
    ) -> anyhow::Result<()> {
        let query = mutations::complete_run(run_id, success, completed_at);
        let _: serde_json::Value = self.query(&query).await?;
        Ok(())
    }

    pub async fn report_events(&self, events: &[RunEvent]) -> anyhow::Result<i32> {
        let events_json = serde_json::to_string(events)?;
        let query = mutations::report_events(&events_json);
        #[derive(serde::Deserialize)]
        struct Response {
            #[serde(rename = "reportEvents")]
            report_events: i32,
        }
        let data: Response = self.query(&query).await?;
        Ok(data.report_events)
    }
}

pub fn status_icon(status: &str) -> &'static str {
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

pub fn format_timestamp(ts: &str) -> String {
    use chrono::{DateTime, Utc};
    if let Ok(dt) = ts.parse::<DateTime<Utc>>() {
        dt.format("%Y-%m-%d %H:%M:%S").to_string()
    } else {
        ts.to_string()
    }
}
