use std::collections::HashMap;
use std::future::Future;

use chrono::{DateTime, Utc};
use converge_kernel::{ContextKey, ConvergeResult};
use converge_pack::{Context as ContextView, ProposalId, ProposedFact, Provenance, TextPayload};
use serde::de::DeserializeOwned;
use tonic::Status;
use uuid::Uuid;

pub(super) fn has_fact_id(ctx: &dyn ContextView, key: ContextKey, fact_id: &str) -> bool {
    ctx.get(key).iter().any(|fact| fact.id() == fact_id)
}

pub(crate) fn proposed_text_fact(
    key: ContextKey,
    id: impl Into<ProposalId>,
    text: impl Into<String>,
    provenance: impl Into<Provenance>,
) -> ProposedFact {
    ProposedFact::new(key, id, TextPayload::new(text), provenance)
}

pub(super) fn payload_from_result<T: DeserializeOwned>(
    result: &ConvergeResult,
    key: ContextKey,
    fact_id: &str,
) -> Result<T, Status> {
    let fact = result
        .context
        .get(key)
        .iter()
        .find(|fact| fact.id() == fact_id)
        .ok_or_else(|| {
            Status::failed_precondition(format!("missing fact in converge context: {fact_id}"))
        })?;
    serde_json::from_str(fact.text().unwrap_or_default())
        .map_err(|error| Status::internal(format!("invalid {fact_id} payload: {error}")))
}

pub(super) fn required_input<'a>(
    inputs: &'a HashMap<String, String>,
    key: &str,
) -> Result<&'a str, Status> {
    inputs
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| Status::invalid_argument(format!("missing required input: {key}")))
}

pub(super) fn optional_input(inputs: &HashMap<String, String>, key: &str) -> Option<String> {
    inputs
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub(super) fn required_uuid(inputs: &HashMap<String, String>, key: &str) -> Result<Uuid, Status> {
    required_input(inputs, key).and_then(|value| {
        Uuid::parse_str(value)
            .map_err(|error| Status::invalid_argument(format!("invalid uuid for {key}: {error}")))
    })
}

pub(super) fn required_datetime(
    inputs: &HashMap<String, String>,
    key: &str,
) -> Result<DateTime<Utc>, Status> {
    required_input(inputs, key).and_then(|value| {
        chrono::DateTime::parse_from_rfc3339(value)
            .map(|value| value.with_timezone(&Utc))
            .map_err(|error| {
                Status::invalid_argument(format!("invalid RFC3339 datetime for {key}: {error}"))
            })
    })
}

pub(super) fn optional_uuid(
    inputs: &HashMap<String, String>,
    key: &str,
) -> Result<Option<Uuid>, Status> {
    optional_input(inputs, key)
        .map(|value| {
            Uuid::parse_str(&value).map_err(|error| {
                Status::invalid_argument(format!("invalid uuid for {key}: {error}"))
            })
        })
        .transpose()
}

pub(super) fn optional_i64(inputs: &HashMap<String, String>, key: &str) -> Option<i64> {
    optional_input(inputs, key).and_then(|value| value.parse::<i64>().ok())
}

pub(super) fn optional_bool(inputs: &HashMap<String, String>, key: &str) -> Option<bool> {
    optional_input(inputs, key).and_then(|value| match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    })
}

pub(super) fn converge_confidence_to_bps(confidence: f64) -> u16 {
    (confidence.clamp(0.0, 1.0) * 10_000.0).round() as u16
}

pub(super) fn block_on_async<F>(future: F) -> F::Output
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
