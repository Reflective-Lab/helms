//! Truth IntentPacket → Converge typed admission boundary.
//!
//! Wraps `axiom_truth::compile_intent` (via [`compile_intent_for_truth`]) +
//! `organism_runtime::Runtime::admit_intent` so each truth executor stages
//! its IntentPacket once per run with one call. This is handoff step 4:
//! intents enter the Converge kernel through the typed admission gate
//! instead of being passed straight into the engine.

use converge_kernel::{AdmissionActor, AdmissionActorKind, AdmissionError, AdmissionSource, ContextState};
use organism_pack::IntentPacket;
use organism_runtime::{IntentAdmissionError, Runtime};

use crate::find_truth;
use crate::intent_compile::{CompileTruthError, compile_intent_for_truth};

/// Errors produced when staging a Truth's IntentPacket.
#[derive(Debug, thiserror::Error)]
pub enum AdmitTruthError {
    #[error("unknown truth: {0}")]
    UnknownTruth(String),
    #[error(transparent)]
    Compile(#[from] CompileTruthError),
    #[error("could not construct admission identity: {0}")]
    Identity(#[from] AdmissionError),
    #[error(transparent)]
    Admission(#[from] IntentAdmissionError),
}

/// Compile the Truth's IntentPacket through axiom and stage it through
/// Converge's typed admission boundary. Returns the compiled packet so the
/// caller can use it directly (e.g. as input to a future
/// `Runtime::select_formation`, handoff step 5).
///
/// `actor_id` identifies the principal staging the intent (e.g. an operator
/// id or `"helms"` for system-staged runs). `source_label` is recorded as
/// the admission source — pick something stable like `"truth:<key>"` or
/// `"helms-pipeline"`.
pub fn admit_truth_intent(
    truth_key: &str,
    actor_id: &str,
    source_label: &str,
    context: &mut ContextState,
) -> Result<IntentPacket, AdmitTruthError> {
    let truth = find_truth(truth_key)
        .ok_or_else(|| AdmitTruthError::UnknownTruth(truth_key.to_string()))?;
    let intent = compile_intent_for_truth(&truth)?;
    let actor = AdmissionActor::new(actor_id, AdmissionActorKind::System)?;
    let source = AdmissionSource::new(source_label)?;
    Runtime::new().admit_intent(&intent, actor, source, context)?;
    Ok(intent)
}
