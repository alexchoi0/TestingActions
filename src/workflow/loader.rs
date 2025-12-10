//! Workflow directory loader
//!
//! Load multiple workflow YAML files from a directory.

use std::path::Path;

use super::Workflow;

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parse error in {file}: {error}")]
    Yaml {
        file: String,
        error: serde_yaml::Error,
    },
}

pub struct WorkflowLoader;

impl WorkflowLoader {
    pub fn load_directory(dir: &Path) -> Result<Vec<Workflow>, LoadError> {
        let mut workflows = Vec::new();

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                let ext = path.extension().and_then(|e| e.to_str());
                let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                // Skip runner.yaml - it's a config file, not a workflow
                if filename == "runner.yaml" || filename == "runner.yml" {
                    continue;
                }

                if ext == Some("yaml") || ext == Some("yml") {
                    let content = std::fs::read_to_string(&path)?;
                    let workflow: Workflow =
                        serde_yaml::from_str(&content).map_err(|e| LoadError::Yaml {
                            file: path.display().to_string(),
                            error: e,
                        })?;
                    workflows.push(workflow);
                }
            }
        }

        Ok(workflows)
    }

    pub fn load_file(path: &Path) -> Result<Workflow, LoadError> {
        let content = std::fs::read_to_string(path)?;
        serde_yaml::from_str(&content).map_err(|e| LoadError::Yaml {
            file: path.display().to_string(),
            error: e,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_load_directory() {
        let dir = tempdir().unwrap();

        fs::write(
            dir.path().join("workflow1.yaml"),
            r#"
name: workflow-1
jobs:
  test:
    steps:
      - uses: page/goto
        with:
          url: https://example.com
"#,
        )
        .unwrap();

        fs::write(
            dir.path().join("workflow2.yml"),
            r#"
name: workflow-2
depends_on: [workflow-1]
jobs:
  test:
    steps:
      - uses: page/goto
        with:
          url: https://example.com
"#,
        )
        .unwrap();

        fs::write(dir.path().join("not-a-workflow.txt"), "ignored").unwrap();

        let workflows = WorkflowLoader::load_directory(dir.path()).unwrap();
        assert_eq!(workflows.len(), 2);

        let names: Vec<_> = workflows.iter().map(|w| w.name.as_str()).collect();
        assert!(names.contains(&"workflow-1"));
        assert!(names.contains(&"workflow-2"));
    }

    #[test]
    fn test_load_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.yaml");

        fs::write(
            &path,
            r#"
name: single-workflow
jobs:
  test:
    steps:
      - uses: page/goto
        with:
          url: https://example.com
"#,
        )
        .unwrap();

        let workflow = WorkflowLoader::load_file(&path).unwrap();
        assert_eq!(workflow.name, "single-workflow");
    }
}
