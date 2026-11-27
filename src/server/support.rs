//! Helpers shared by the tool handlers: result conversion, number rounding, blocking jobs.

use anyhow::{Context, Result};
use rmcp::model::{CallToolResult, ContentBlock};
use serde::Serialize;
use serde_json::{json, Value};

pub(crate) fn tool_result(result: Result<Value>) -> CallToolResult {
    match result {
        Ok(value) => CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()),
        )]),
        Err(error) => CallToolResult::error(vec![ContentBlock::text(error.to_string())]),
    }
}

pub(crate) fn round_json(value: Value, places: i32) -> Value {
    match value {
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.is_finite() && !n.is_i64() && !n.is_u64() => {
                let scale = 10f64.powi(places);
                json!((f * scale).round() / scale)
            }
            Some(f) if !f.is_finite() => Value::Null,
            _ => Value::Number(n),
        },
        Value::Array(items) => {
            Value::Array(items.into_iter().map(|v| round_json(v, places)).collect())
        }
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| (k, round_json(v, places)))
                .collect(),
        ),
        other => other,
    }
}

pub(crate) async fn run_blocking<T: Serialize + Send + 'static>(
    job: impl FnOnce() -> Result<T> + Send + 'static,
) -> CallToolResult {
    let result = tokio::task::spawn_blocking(job)
        .await
        .context("analysis worker stopped unexpectedly")
        .and_then(|result| result)
        .and_then(|value| Ok(round_json(serde_json::to_value(value)?, 1)));
    tool_result(result)
}
