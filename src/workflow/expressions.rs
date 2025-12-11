//! Expression evaluation for GitHub Actions-style ${{ }} syntax
//!
//! Supports:
//! - ${{ env.VAR_NAME }}
//! - ${{ secrets.SECRET_NAME }}
//! - ${{ steps.step_id.outputs.output_name }}
//! - ${{ jobs.job_name.outputs.output_name }}

use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

use super::context::ExecutionContext;

static EXPRESSION_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$\{\{\s*([^}]+)\s*\}\}").unwrap());

/// Errors that can occur during expression evaluation
#[derive(Debug, thiserror::Error)]
pub enum ExpressionError {
    #[error("Unknown variable: {0}")]
    UnknownVariable(String),

    #[error("Invalid expression syntax: {0}")]
    InvalidSyntax(String),

    #[error("Missing context: {0}")]
    MissingContext(String),
}

/// Evaluate all expressions in a string
pub fn evaluate(input: &str, ctx: &ExecutionContext) -> Result<String, ExpressionError> {
    let mut result = input.to_string();

    // Find all expressions and evaluate them
    for cap in EXPRESSION_REGEX.captures_iter(input) {
        let full_match = cap.get(0).unwrap().as_str();
        let expr = cap.get(1).unwrap().as_str().trim();

        let value = evaluate_single(expr, ctx)?;
        result = result.replace(full_match, &value);
    }

    Ok(result)
}

/// Evaluate a single expression (without the ${{ }} wrapper)
fn evaluate_single(expr: &str, ctx: &ExecutionContext) -> Result<String, ExpressionError> {
    let parts: Vec<&str> = expr.split('.').collect();

    if parts.is_empty() {
        return Err(ExpressionError::InvalidSyntax(expr.to_string()));
    }

    match parts[0] {
        "env" => {
            if parts.len() != 2 {
                return Err(ExpressionError::InvalidSyntax(format!(
                    "env expressions must be env.VAR_NAME, got: {}",
                    expr
                )));
            }
            ctx.env
                .get(parts[1])
                .cloned()
                .ok_or_else(|| ExpressionError::UnknownVariable(format!("env.{}", parts[1])))
        }

        "secrets" => {
            if parts.len() != 2 {
                return Err(ExpressionError::InvalidSyntax(format!(
                    "secrets expressions must be secrets.SECRET_NAME, got: {}",
                    expr
                )));
            }
            ctx.secrets
                .get(parts[1])
                .cloned()
                .ok_or_else(|| ExpressionError::UnknownVariable(format!("secrets.{}", parts[1])))
        }

        "steps" => {
            // steps.step_id.outputs.output_name
            if parts.len() != 4 || parts[2] != "outputs" {
                return Err(ExpressionError::InvalidSyntax(format!(
                    "steps expressions must be steps.STEP_ID.outputs.OUTPUT_NAME, got: {}",
                    expr
                )));
            }
            ctx.get_output(parts[1], parts[3]).cloned().ok_or_else(|| {
                ExpressionError::UnknownVariable(format!("steps.{}.outputs.{}", parts[1], parts[3]))
            })
        }

        "jobs" => {
            // jobs.job_name.outputs.output_name
            if parts.len() != 4 || parts[2] != "outputs" {
                return Err(ExpressionError::InvalidSyntax(format!(
                    "jobs expressions must be jobs.JOB_NAME.outputs.OUTPUT_NAME, got: {}",
                    expr
                )));
            }
            ctx.jobs
                .get(parts[1])
                .and_then(|outputs| outputs.get(parts[3]))
                .cloned()
                .ok_or_else(|| {
                    ExpressionError::UnknownVariable(format!(
                        "jobs.{}.outputs.{}",
                        parts[1], parts[3]
                    ))
                })
        }

        "github" => {
            // For compatibility, we support some github.* expressions
            // In our context, these map to run metadata
            match parts.get(1).copied() {
                Some("run_id") => Ok(ctx.run_id.clone()),
                Some("job") => ctx
                    .current_job
                    .clone()
                    .ok_or_else(|| ExpressionError::MissingContext("current job".to_string())),
                _ => Err(ExpressionError::UnknownVariable(expr.to_string())),
            }
        }

        _ => Err(ExpressionError::UnknownVariable(expr.to_string())),
    }
}

/// Check if a condition expression evaluates to true
pub fn evaluate_condition(
    condition: &str,
    ctx: &ExecutionContext,
) -> Result<bool, ExpressionError> {
    let condition = condition.trim();

    // Simple truthiness check for now
    // TODO: Implement proper expression evaluation with operators

    // Check for simple comparisons first (before checking ${{ }} expressions)
    // This handles cases like "${{ env.VAR }} == 'value'"
    if condition.contains("==") {
        let parts: Vec<&str> = condition.split("==").collect();
        if parts.len() == 2 {
            let left =
                evaluate(parts[0].trim(), ctx).unwrap_or_else(|_| parts[0].trim().to_string());
            let right =
                evaluate(parts[1].trim(), ctx).unwrap_or_else(|_| parts[1].trim().to_string());
            return Ok(left.trim_matches('"').trim_matches('\'')
                == right.trim_matches('"').trim_matches('\''));
        }
    }

    if condition.contains("!=") {
        let parts: Vec<&str> = condition.split("!=").collect();
        if parts.len() == 2 {
            let left =
                evaluate(parts[0].trim(), ctx).unwrap_or_else(|_| parts[0].trim().to_string());
            let right =
                evaluate(parts[1].trim(), ctx).unwrap_or_else(|_| parts[1].trim().to_string());
            return Ok(left.trim_matches('"').trim_matches('\'')
                != right.trim_matches('"').trim_matches('\''));
        }
    }

    // Check for success()/failure()/always() functions
    match condition {
        "success()" => Ok(true),  // TODO: Track actual success state
        "failure()" => Ok(false), // TODO: Track actual failure state
        "always()" => Ok(true),
        _ => {
            // Try to evaluate as expression
            let value = evaluate(condition, ctx)?;
            Ok(is_truthy(&value))
        }
    }
}

fn is_truthy(value: &str) -> bool {
    !value.is_empty()
        && value != "false"
        && value != "0"
        && value.to_lowercase() != "null"
        && value.to_lowercase() != "none"
}

/// Evaluate all expressions in a HashMap of parameters
pub fn evaluate_params(
    params: &HashMap<String, serde_yaml::Value>,
    ctx: &ExecutionContext,
) -> Result<HashMap<String, String>, ExpressionError> {
    let mut result = HashMap::new();

    for (key, value) in params {
        let string_value = match value {
            serde_yaml::Value::String(s) => evaluate(s, ctx)?,
            serde_yaml::Value::Number(n) => n.to_string(),
            serde_yaml::Value::Bool(b) => b.to_string(),
            serde_yaml::Value::Null => String::new(),
            _ => serde_yaml::to_string(value).unwrap_or_default(),
        };
        result.insert(key.clone(), string_value);
    }

    Ok(result)
}

/// Evaluate expressions and convert YAML values to JSON, preserving structure
pub fn evaluate_params_json(
    params: &HashMap<String, serde_yaml::Value>,
    ctx: &ExecutionContext,
) -> Result<HashMap<String, serde_json::Value>, ExpressionError> {
    let mut result = HashMap::new();

    for (key, value) in params {
        let json_value = yaml_to_json_with_expressions(value, ctx)?;
        result.insert(key.clone(), json_value);
    }

    Ok(result)
}

/// Convert a YAML value to JSON, evaluating any string expressions
fn yaml_to_json_with_expressions(
    value: &serde_yaml::Value,
    ctx: &ExecutionContext,
) -> Result<serde_json::Value, ExpressionError> {
    match value {
        serde_yaml::Value::Null => Ok(serde_json::Value::Null),
        serde_yaml::Value::Bool(b) => Ok(serde_json::Value::Bool(*b)),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(serde_json::Value::Number(i.into()))
            } else if let Some(u) = n.as_u64() {
                Ok(serde_json::Value::Number(u.into()))
            } else if let Some(f) = n.as_f64() {
                Ok(serde_json::Number::from_f64(f)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null))
            } else {
                Ok(serde_json::Value::Null)
            }
        }
        serde_yaml::Value::String(s) => {
            let evaluated = evaluate(s, ctx)?;
            Ok(serde_json::Value::String(evaluated))
        }
        serde_yaml::Value::Sequence(seq) => {
            let json_seq: Result<Vec<_>, _> = seq
                .iter()
                .map(|v| yaml_to_json_with_expressions(v, ctx))
                .collect();
            Ok(serde_json::Value::Array(json_seq?))
        }
        serde_yaml::Value::Mapping(map) => {
            let mut json_map = serde_json::Map::new();
            for (k, v) in map {
                let key = match k {
                    serde_yaml::Value::String(s) => s.clone(),
                    _ => serde_yaml::to_string(k).unwrap_or_default(),
                };
                json_map.insert(key, yaml_to_json_with_expressions(v, ctx)?);
            }
            Ok(serde_json::Value::Object(json_map))
        }
        serde_yaml::Value::Tagged(tagged) => yaml_to_json_with_expressions(&tagged.value, ctx),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context() -> ExecutionContext {
        let mut ctx = ExecutionContext::new();
        ctx.env
            .insert("BASE_URL".to_string(), "https://example.com".to_string());
        ctx.secrets
            .insert("API_KEY".to_string(), "secret123".to_string());
        ctx.set_output("login", "token", "abc123".to_string());
        ctx
    }

    #[test]
    fn test_evaluate_env() {
        let ctx = test_context();
        let result = evaluate("${{ env.BASE_URL }}/api", &ctx).unwrap();
        assert_eq!(result, "https://example.com/api");
    }

    #[test]
    fn test_evaluate_secrets() {
        let ctx = test_context();
        let result = evaluate("Bearer ${{ secrets.API_KEY }}", &ctx).unwrap();
        assert_eq!(result, "Bearer secret123");
    }

    #[test]
    fn test_evaluate_step_output() {
        let ctx = test_context();
        let result = evaluate("${{ steps.login.outputs.token }}", &ctx).unwrap();
        assert_eq!(result, "abc123");
    }

    #[test]
    fn test_evaluate_multiple() {
        let ctx = test_context();
        let result = evaluate(
            "${{ env.BASE_URL }}?token=${{ steps.login.outputs.token }}",
            &ctx,
        )
        .unwrap();
        assert_eq!(result, "https://example.com?token=abc123");
    }

    #[test]
    fn test_condition_equality() {
        let ctx = test_context();
        assert!(evaluate_condition("${{ env.BASE_URL }} == 'https://example.com'", &ctx).unwrap());
        assert!(!evaluate_condition("${{ env.BASE_URL }} == 'other'", &ctx).unwrap());
    }
}
