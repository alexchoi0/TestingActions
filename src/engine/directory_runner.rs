//! Directory-based workflow runner
//!
//! Load and execute multiple workflows from a directory with parallel execution
//! based on the dependency DAG. Workflows start as soon as their dependencies
//! complete (eager scheduling).
//!
//! Supports multiple named configurations that run in parallel:
//!
//! ```yaml
//! # runner.yaml
//! parallel: 4
//! fail_fast: false
//!
//! configs:
//!   chrome:
//!     platforms:
//!       playwright:
//!         browser: chromium
//!   firefox:
//!     platforms:
//!       playwright:
//!         browser: firefox
//! ```

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures::future::join_all;
use tokio::sync::{mpsc, RwLock, Semaphore};

use super::executor::Executor;
use super::result::WorkflowResult;
use super::workflow_dag::{DAGError, WorkflowDAG};
use crate::workflow::loader::{LoadError, WorkflowLoader};
use crate::workflow::platform::PlatformsConfig;
use crate::workflow::RunnerConfig;

#[derive(Debug, thiserror::Error)]
pub enum DirectoryRunError {
    #[error("Load error: {0}")]
    Load(#[from] LoadError),

    #[error("DAG error: {0}")]
    Dag(#[from] DAGError),

    #[error("Executor error: {0}")]
    Executor(#[from] super::error::ExecutorError),
}

#[derive(Debug, Clone)]
pub struct DirectoryResult {
    pub success: bool,
    pub workflows: HashMap<String, WorkflowResult>,
    pub execution_order: Vec<Vec<String>>,
    pub skipped: Vec<String>,
}

/// Result from running multiple named profiles in parallel
#[derive(Debug)]
pub struct MultiProfileResult {
    pub success: bool,
    pub profiles: HashMap<String, DirectoryResult>,
}

pub async fn run_workflow_directory(
    dir: impl AsRef<Path>,
) -> Result<DirectoryResult, DirectoryRunError> {
    WorkflowDirectoryRunner::new(dir).run().await
}

pub struct WorkflowDirectoryRunner {
    directory: PathBuf,
    max_concurrent: usize,
    fail_fast: bool,
    platforms: PlatformsConfig,
    filter: Option<Box<dyn Fn(&str) -> bool + Send + Sync>>,
    config: Option<RunnerConfig>,
}

impl WorkflowDirectoryRunner {
    pub fn new(directory: impl AsRef<Path>) -> Self {
        Self {
            directory: directory.as_ref().to_path_buf(),
            max_concurrent: 4,
            fail_fast: false,
            platforms: PlatformsConfig::default(),
            filter: None,
            config: None,
        }
    }

    pub fn with_config(directory: impl AsRef<Path>, config: RunnerConfig) -> Self {
        Self {
            directory: directory.as_ref().to_path_buf(),
            max_concurrent: config.parallel,
            fail_fast: config.fail_fast,
            platforms: config.platforms.clone(),
            filter: None,
            config: Some(config),
        }
    }

    /// Check if this runner has multiple named profiles
    pub fn has_multiple_profiles(&self) -> bool {
        self.config.as_ref().map(|c| c.has_profiles()).unwrap_or(false)
    }

    pub fn parallel(mut self, max: usize) -> Self {
        self.max_concurrent = max;
        self
    }

    pub fn fail_fast(mut self, enabled: bool) -> Self {
        self.fail_fast = enabled;
        self
    }

    pub fn platforms(mut self, platforms: PlatformsConfig) -> Self {
        self.platforms = platforms;
        self
    }

    pub fn filter<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> bool + Send + Sync + 'static,
    {
        self.filter = Some(Box::new(f));
        self
    }

    pub async fn run(self) -> Result<DirectoryResult, DirectoryRunError> {
        let mut workflows = WorkflowLoader::load_directory(&self.directory)?;

        if let Some(filter) = &self.filter {
            workflows.retain(|w| filter(&w.name));
        }

        if workflows.is_empty() {
            return Ok(DirectoryResult {
                success: true,
                workflows: HashMap::new(),
                execution_order: vec![],
                skipped: vec![],
            });
        }

        let dag = WorkflowDAG::build(workflows)?;
        self.run_dag(dag).await
    }

    /// Run multiple named profiles in parallel
    ///
    /// Each profile runs the full workflow directory with its own platform settings.
    /// Returns a MultiProfileResult with results from all profiles.
    pub async fn run_multi(self) -> Result<MultiProfileResult, DirectoryRunError> {
        let config = match &self.config {
            Some(c) if c.has_profiles() => c.clone(),
            _ => {
                // No multi-profile, just run as single profile
                let result = self.run().await?;
                let mut profiles = HashMap::new();
                profiles.insert("default".to_string(), result);
                return Ok(MultiProfileResult {
                    success: profiles.values().all(|r| r.success),
                    profiles,
                });
            }
        };

        let profile_names = config.profile_names();
        let directory = self.directory.clone();
        let fail_fast = self.fail_fast;
        let max_concurrent = self.max_concurrent;

        // Run each profile in parallel
        let futures: Vec<_> = profile_names
            .into_iter()
            .map(|name| {
                let dir = directory.clone();
                let platforms = config.platforms_for(&name);

                async move {
                    let runner = WorkflowDirectoryRunner::new(&dir)
                        .parallel(max_concurrent)
                        .fail_fast(fail_fast)
                        .platforms(platforms);

                    let result = runner.run().await;
                    (name, result)
                }
            })
            .collect();

        let results = join_all(futures).await;

        let mut profile_results = HashMap::new();
        let mut all_success = true;

        for (name, result) in results {
            match result {
                Ok(dir_result) => {
                    if !dir_result.success {
                        all_success = false;
                    }
                    profile_results.insert(name, dir_result);
                }
                Err(e) => {
                    all_success = false;
                    profile_results.insert(
                        name,
                        DirectoryResult {
                            success: false,
                            workflows: HashMap::new(),
                            execution_order: vec![],
                            skipped: vec![format!("Error: {}", e)],
                        },
                    );
                }
            }
        }

        Ok(MultiProfileResult {
            success: all_success,
            profiles: profile_results,
        })
    }

    async fn run_dag(&self, dag: WorkflowDAG) -> Result<DirectoryResult, DirectoryRunError> {
        let semaphore = Arc::new(Semaphore::new(self.max_concurrent));
        let results: Arc<RwLock<HashMap<String, WorkflowResult>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let skipped: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(Vec::new()));
        let pending: Arc<RwLock<HashSet<String>>> = Arc::new(RwLock::new(
            dag.workflow_names().into_iter().map(|s| s.to_string()).collect(),
        ));
        let failed: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
        let fail_fast = self.fail_fast;
        let platforms = Arc::new(self.platforms.clone());

        let (complete_tx, mut complete_rx) = mpsc::channel::<String>(dag.len());

        let total_workflows = dag.len();
        let dag = Arc::new(dag);

        #[derive(Debug, PartialEq)]
        enum RunStatus {
            Ready,
            Waiting,
            Skip,
        }

        fn check_status(
            name: &str,
            dag: &WorkflowDAG,
            results: &HashMap<String, WorkflowResult>,
            skipped_set: &HashSet<String>,
        ) -> RunStatus {
            let node = match dag.get_node(name) {
                Some(n) => n,
                None => return RunStatus::Skip,
            };

            if node.dependencies.is_empty() {
                return RunStatus::Ready;
            }

            let mut all_deps_done = true;
            let mut any_dep_failed_or_skipped = false;

            for dep in &node.dependencies {
                if skipped_set.contains(dep) {
                    any_dep_failed_or_skipped = true;
                } else if let Some(result) = results.get(dep) {
                    if !result.success {
                        any_dep_failed_or_skipped = true;
                    }
                } else {
                    all_deps_done = false;
                }
            }

            if !all_deps_done {
                return RunStatus::Waiting;
            }

            if any_dep_failed_or_skipped {
                if node.always {
                    RunStatus::Ready
                } else {
                    RunStatus::Skip
                }
            } else {
                RunStatus::Ready
            }
        }

        fn spawn_ready_workflows(
            dag: Arc<WorkflowDAG>,
            pending: Arc<RwLock<HashSet<String>>>,
            results: Arc<RwLock<HashMap<String, WorkflowResult>>>,
            skipped: Arc<RwLock<Vec<String>>>,
            semaphore: Arc<Semaphore>,
            complete_tx: mpsc::Sender<String>,
            failed: Arc<AtomicBool>,
            fail_fast: bool,
            platforms: Arc<PlatformsConfig>,
        ) {
            tokio::spawn(async move {
                let results_read = results.read().await;
                let skipped_read = skipped.read().await;
                let skipped_set: HashSet<String> = skipped_read.iter().cloned().collect();
                drop(skipped_read);

                let mut pending_write = pending.write().await;
                let mut to_run = Vec::new();
                let mut to_skip = Vec::new();

                for name in pending_write.iter() {
                    let node = dag.get_node(name);
                    let is_always = node.map(|n| n.always).unwrap_or(false);

                    if fail_fast && failed.load(Ordering::SeqCst) && !is_always {
                        to_skip.push(name.clone());
                        continue;
                    }

                    match check_status(name, &dag, &results_read, &skipped_set) {
                        RunStatus::Ready => to_run.push(name.clone()),
                        RunStatus::Waiting => {}
                        RunStatus::Skip => to_skip.push(name.clone()),
                    }
                }
                drop(results_read);

                for name in &to_skip {
                    pending_write.remove(name);
                }
                for name in &to_run {
                    pending_write.remove(name);
                }
                drop(pending_write);

                if !to_skip.is_empty() {
                    skipped.write().await.extend(to_skip.clone());
                    for name in to_skip {
                        let _ = complete_tx.send(name).await;
                    }
                }

                for name in to_run {
                    let sem = semaphore.clone();
                    let workflow = dag.get_workflow(&name).unwrap().clone();
                    let results = results.clone();
                    let tx = complete_tx.clone();
                    let failed = failed.clone();
                    let platforms = platforms.clone();

                    tokio::spawn(async move {
                        let _permit = sem.acquire().await.unwrap();
                        let mut executor = Executor::new().with_platforms(&platforms);
                        let result = executor.run(workflow).await;

                        let workflow_result = match result {
                            Ok(wr) => wr,
                            Err(e) => WorkflowResult {
                                success: false,
                                jobs: HashMap::new(),
                                run_id: format!("error: {}", e),
                            },
                        };

                        if !workflow_result.success {
                            failed.store(true, Ordering::SeqCst);
                        }

                        results.write().await.insert(name.clone(), workflow_result);
                        let _ = tx.send(name).await;
                    });
                }
            });
        }

        spawn_ready_workflows(
            dag.clone(),
            pending.clone(),
            results.clone(),
            skipped.clone(),
            semaphore.clone(),
            complete_tx.clone(),
            failed.clone(),
            fail_fast,
            platforms.clone(),
        );

        let mut completed_count = 0;
        while completed_count < total_workflows {
            match complete_rx.recv().await {
                Some(_completed_name) => {
                    completed_count += 1;

                    if completed_count >= total_workflows {
                        break;
                    }

                    let has_pending = !pending.read().await.is_empty();
                    if has_pending {
                        spawn_ready_workflows(
                            dag.clone(),
                            pending.clone(),
                            results.clone(),
                            skipped.clone(),
                            semaphore.clone(),
                            complete_tx.clone(),
                            failed.clone(),
                            fail_fast,
                            platforms.clone(),
                        );
                    }
                }
                None => break,
            }
        }

        let final_results = match Arc::try_unwrap(results) {
            Ok(rw) => rw.into_inner(),
            Err(arc) => arc.read().await.clone(),
        };

        let final_skipped = match Arc::try_unwrap(skipped) {
            Ok(rw) => rw.into_inner(),
            Err(arc) => arc.read().await.clone(),
        };

        let all_success = final_results.values().all(|r| r.success);

        Ok(DirectoryResult {
            success: all_success,
            workflows: final_results,
            execution_order: dag.execution_levels().clone(),
            skipped: final_skipped,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_empty_directory() {
        let dir = tempdir().unwrap();

        let result = run_workflow_directory(dir.path()).await.unwrap();

        assert!(result.success);
        assert!(result.workflows.is_empty());
        assert!(result.execution_order.is_empty());
    }

    #[tokio::test]
    async fn test_filter() {
        let dir = tempdir().unwrap();

        fs::write(
            dir.path().join("test-one.yaml"),
            r#"
name: test-one
on:
  manual: true
jobs:
  test:
    steps:
      - uses: wait/ms
        with:
          duration: "1"
"#,
        )
        .unwrap();

        fs::write(
            dir.path().join("skip-this.yaml"),
            r#"
name: skip-this
on:
  manual: true
jobs:
  test:
    steps:
      - uses: wait/ms
        with:
          duration: "1"
"#,
        )
        .unwrap();

        let result = WorkflowDirectoryRunner::new(dir.path())
            .filter(|name| name.starts_with("test-"))
            .run()
            .await
            .unwrap();

        assert_eq!(result.workflows.len(), 1);
        assert!(result.workflows.contains_key("test-one"));
    }

    #[tokio::test]
    async fn test_multi_profile_run() {
        let dir = tempdir().unwrap();

        // Create a simple workflow
        fs::write(
            dir.path().join("test.yaml"),
            r#"
name: test
on:
  manual: true
jobs:
  test:
    steps:
      - uses: wait/ms
        with:
          duration: "1"
"#,
        )
        .unwrap();

        // Create runner.yaml with multiple profiles
        fs::write(
            dir.path().join("runner.yaml"),
            r#"
parallel: 2
fail_fast: false

profiles:
  chrome:
    platforms:
      web:
        base_url: "http://localhost:3000"
  firefox:
    platforms:
      web:
        base_url: "http://localhost:4000"
"#,
        )
        .unwrap();

        let config = RunnerConfig::load(dir.path().join("runner.yaml")).unwrap();
        assert!(config.has_profiles());
        assert_eq!(config.profile_names().len(), 2);

        let runner = WorkflowDirectoryRunner::with_config(dir.path(), config);
        let result = runner.run_multi().await.unwrap();

        assert!(result.success);
        assert_eq!(result.profiles.len(), 2);
        assert!(result.profiles.contains_key("chrome"));
        assert!(result.profiles.contains_key("firefox"));

        // Each profile should have run the workflow
        for (_, dir_result) in &result.profiles {
            assert!(dir_result.success);
            assert!(dir_result.workflows.contains_key("test"));
        }
    }

    #[tokio::test]
    async fn test_single_profile_run_multi() {
        let dir = tempdir().unwrap();

        // Create a simple workflow
        fs::write(
            dir.path().join("test.yaml"),
            r#"
name: test
on:
  manual: true
jobs:
  test:
    steps:
      - uses: wait/ms
        with:
          duration: "1"
"#,
        )
        .unwrap();

        // No runner.yaml, so use default profile
        let runner = WorkflowDirectoryRunner::new(dir.path());
        let result = runner.run_multi().await.unwrap();

        assert!(result.success);
        assert_eq!(result.profiles.len(), 1);
        assert!(result.profiles.contains_key("default"));
    }
}
