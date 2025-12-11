//! Workflow DAG builder
//!
//! Builds a directed acyclic graph from multiple workflows based on their
//! `workflow_run` trigger dependencies, and computes execution levels for
//! parallel execution.

use std::collections::HashMap;

use crate::workflow::Workflow;

#[derive(Debug, thiserror::Error)]
pub enum DAGError {
    #[error("Workflow '{workflow}' depends on non-existent workflow '{dependency}'")]
    MissingDependency {
        workflow: String,
        dependency: String,
    },

    #[error("Cyclic dependency detected in workflows")]
    CyclicDependency,

    #[error("Duplicate workflow name: {0}")]
    DuplicateName(String),
}

#[derive(Debug)]
pub struct WorkflowNode {
    pub workflow: Workflow,
    pub dependencies: Vec<String>,
    pub always: bool,
}

#[derive(Debug)]
pub struct WorkflowDAG {
    nodes: HashMap<String, WorkflowNode>,
    execution_levels: Vec<Vec<String>>,
}

impl WorkflowDAG {
    pub fn build(workflows: Vec<Workflow>) -> Result<Self, DAGError> {
        let mut nodes = HashMap::new();

        for workflow in workflows {
            let name = workflow.name.clone();
            if nodes.contains_key(&name) {
                return Err(DAGError::DuplicateName(name));
            }

            let depends_on = &workflow.depends_on;
            let deps = depends_on.workflows.clone();
            let always = depends_on.always;
            nodes.insert(
                name,
                WorkflowNode {
                    workflow,
                    dependencies: deps,
                    always,
                },
            );
        }

        for (name, node) in &nodes {
            for dep in &node.dependencies {
                if !nodes.contains_key(dep) {
                    return Err(DAGError::MissingDependency {
                        workflow: name.clone(),
                        dependency: dep.clone(),
                    });
                }
            }
        }

        let execution_levels = Self::compute_execution_levels(&nodes)?;

        Ok(Self {
            nodes,
            execution_levels,
        })
    }

    fn compute_execution_levels(
        nodes: &HashMap<String, WorkflowNode>,
    ) -> Result<Vec<Vec<String>>, DAGError> {
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();

        for (name, node) in nodes {
            in_degree.entry(name.as_str()).or_insert(0);
            for dep in &node.dependencies {
                *in_degree.entry(name.as_str()).or_insert(0) += 1;
                dependents
                    .entry(dep.as_str())
                    .or_default()
                    .push(name.as_str());
            }
        }

        let mut levels: Vec<Vec<String>> = Vec::new();
        let mut current_level: Vec<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&name, _)| name.to_string())
            .collect();

        current_level.sort();

        while !current_level.is_empty() {
            levels.push(current_level.clone());

            let mut next_level = Vec::new();
            for name in &current_level {
                if let Some(deps) = dependents.get(name.as_str()) {
                    for &dep in deps {
                        let degree = in_degree.get_mut(dep).unwrap();
                        *degree -= 1;
                        if *degree == 0 {
                            next_level.push(dep.to_string());
                        }
                    }
                }
            }
            next_level.sort();
            current_level = next_level;
        }

        let total_processed: usize = levels.iter().map(|l| l.len()).sum();
        if total_processed != nodes.len() {
            return Err(DAGError::CyclicDependency);
        }

        Ok(levels)
    }

    pub fn execution_levels(&self) -> &Vec<Vec<String>> {
        &self.execution_levels
    }

    pub fn get_workflow(&self, name: &str) -> Option<&Workflow> {
        self.nodes.get(name).map(|n| &n.workflow)
    }

    pub fn get_node(&self, name: &str) -> Option<&WorkflowNode> {
        self.nodes.get(name)
    }

    pub fn workflow_names(&self) -> Vec<&str> {
        self.nodes.keys().map(|s| s.as_str()).collect()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::{DependsOn, Job, Step};

    fn make_workflow(name: &str, deps: Vec<&str>) -> Workflow {
        Workflow {
            name: name.to_string(),
            depends_on: DependsOn {
                workflows: deps.into_iter().map(String::from).collect(),
                always: false,
            },
            platform: None,
            platforms: Default::default(),
            env: Default::default(),
            defaults: None,
            before: vec![],
            after: vec![],
            jobs: [(
                "test".to_string(),
                Job {
                    name: None,
                    platform: None,
                    browser: Default::default(),
                    headless: true,
                    viewport: None,
                    needs: vec![],
                    condition: None,
                    env: Default::default(),
                    before: vec![],
                    after: vec![],
                    steps: vec![Step {
                        name: Some("Test step".to_string()),
                        platform: None,
                        uses: "page/goto".to_string(),
                        with: [(
                            "url".to_string(),
                            serde_yaml::Value::String("https://example.com".to_string()),
                        )]
                        .into_iter()
                        .collect(),
                        env: Default::default(),
                        condition: None,
                        id: None,
                        timeout: None,
                        continue_on_error: false,
                        retry: None,
                    }],
                    continue_on_error: false,
                    timeout: None,
                },
            )]
            .into_iter()
            .collect(),
        }
    }

    #[test]
    fn test_simple_dag() {
        let workflows = vec![
            make_workflow("setup", vec![]),
            make_workflow("tests", vec!["setup"]),
            make_workflow("cleanup", vec!["tests"]),
        ];

        let dag = WorkflowDAG::build(workflows).unwrap();

        assert_eq!(dag.len(), 3);
        assert_eq!(dag.execution_levels().len(), 3);
        assert_eq!(dag.execution_levels()[0], vec!["setup"]);
        assert_eq!(dag.execution_levels()[1], vec!["tests"]);
        assert_eq!(dag.execution_levels()[2], vec!["cleanup"]);
    }

    #[test]
    fn test_parallel_dag() {
        let workflows = vec![
            make_workflow("setup", vec![]),
            make_workflow("api-tests", vec!["setup"]),
            make_workflow("e2e-tests", vec!["setup"]),
            make_workflow("cleanup", vec!["api-tests", "e2e-tests"]),
        ];

        let dag = WorkflowDAG::build(workflows).unwrap();

        assert_eq!(dag.execution_levels().len(), 3);
        assert_eq!(dag.execution_levels()[0], vec!["setup"]);
        assert_eq!(dag.execution_levels()[1], vec!["api-tests", "e2e-tests"]);
        assert_eq!(dag.execution_levels()[2], vec!["cleanup"]);
    }

    #[test]
    fn test_independent_workflows() {
        let workflows = vec![
            make_workflow("a", vec![]),
            make_workflow("b", vec![]),
            make_workflow("c", vec![]),
        ];

        let dag = WorkflowDAG::build(workflows).unwrap();

        assert_eq!(dag.execution_levels().len(), 1);
        assert_eq!(dag.execution_levels()[0], vec!["a", "b", "c"]);
    }

    #[test]
    fn test_missing_dependency() {
        let workflows = vec![make_workflow("tests", vec!["setup"])];

        let result = WorkflowDAG::build(workflows);
        assert!(matches!(result, Err(DAGError::MissingDependency { .. })));
    }

    #[test]
    fn test_cyclic_dependency() {
        let workflows = vec![
            make_workflow("a", vec!["c"]),
            make_workflow("b", vec!["a"]),
            make_workflow("c", vec!["b"]),
        ];

        let result = WorkflowDAG::build(workflows);
        assert!(matches!(result, Err(DAGError::CyclicDependency)));
    }

    #[test]
    fn test_duplicate_name() {
        let workflows = vec![
            make_workflow("same-name", vec![]),
            make_workflow("same-name", vec![]),
        ];

        let result = WorkflowDAG::build(workflows);
        assert!(matches!(result, Err(DAGError::DuplicateName(_))));
    }
}
