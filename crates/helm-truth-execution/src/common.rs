use std::collections::HashMap;
use std::future::Future;

use chrono::{DateTime, Utc};
use converge_kernel::{ContextKey, ConvergeResult};
use converge_pack::{Context as ContextView, ProposalId, ProposedFact, Provenance, TextPayload};
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::TruthExecutionError;

pub fn has_fact_id(ctx: &dyn ContextView, key: ContextKey, fact_id: &str) -> bool {
    ctx.get(key).iter().any(|fact| fact.id() == fact_id)
}

pub fn proposed_text_fact(
    key: ContextKey,
    id: impl Into<ProposalId>,
    text: impl Into<String>,
    provenance: Provenance,
) -> ProposedFact {
    ProposedFact::new(key, id, TextPayload::new(text), provenance)
}

pub fn payload_from_result<T: DeserializeOwned>(
    result: &ConvergeResult,
    key: ContextKey,
    fact_id: &str,
) -> Result<T, TruthExecutionError> {
    let fact = result
        .context
        .get(key)
        .iter()
        .find(|fact| fact.id() == fact_id)
        .ok_or_else(|| TruthExecutionError::FailedPrecondition {
            message: format!("missing fact in converge context: {fact_id}"),
        })?;
    serde_json::from_str(fact.text().unwrap_or_default()).map_err(|error| {
        TruthExecutionError::Internal {
            message: format!("invalid {fact_id} payload: {error}"),
        }
    })
}

pub fn required_input<'a>(
    inputs: &'a HashMap<String, String>,
    key: &str,
) -> Result<&'a str, TruthExecutionError> {
    inputs
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| TruthExecutionError::InvalidArgument {
            message: format!("missing required input: {key}"),
        })
}

pub fn optional_input(inputs: &HashMap<String, String>, key: &str) -> Option<String> {
    inputs
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub fn required_uuid(
    inputs: &HashMap<String, String>,
    key: &str,
) -> Result<Uuid, TruthExecutionError> {
    required_input(inputs, key).and_then(|value| {
        Uuid::parse_str(value).map_err(|error| TruthExecutionError::InvalidArgument {
            message: format!("invalid uuid for {key}: {error}"),
        })
    })
}

pub fn required_datetime(
    inputs: &HashMap<String, String>,
    key: &str,
) -> Result<DateTime<Utc>, TruthExecutionError> {
    required_input(inputs, key).and_then(|value| {
        chrono::DateTime::parse_from_rfc3339(value)
            .map(|value| value.with_timezone(&Utc))
            .map_err(|error| TruthExecutionError::InvalidArgument {
                message: format!("invalid RFC3339 datetime for {key}: {error}"),
            })
    })
}

pub fn optional_uuid(
    inputs: &HashMap<String, String>,
    key: &str,
) -> Result<Option<Uuid>, TruthExecutionError> {
    optional_input(inputs, key)
        .map(|value| {
            Uuid::parse_str(&value).map_err(|error| TruthExecutionError::InvalidArgument {
                message: format!("invalid uuid for {key}: {error}"),
            })
        })
        .transpose()
}

pub fn optional_i64(inputs: &HashMap<String, String>, key: &str) -> Option<i64> {
    optional_input(inputs, key).and_then(|value| value.parse::<i64>().ok())
}

pub fn optional_bool(inputs: &HashMap<String, String>, key: &str) -> Option<bool> {
    optional_input(inputs, key).and_then(|value| match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    })
}

pub fn converge_confidence_to_bps(confidence: f64) -> u16 {
    (confidence.clamp(0.0, 1.0) * 10_000.0).round() as u16
}

pub fn block_on_async<F>(future: F) -> F::Output
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("temporary tokio runtime should build")
            .block_on(future)
    })
    .join()
    .expect("knowledge async thread should join")
}
