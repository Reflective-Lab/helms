//! Truth IntentPacket → Converge typed admission boundary + formation selection.
//!
//! Wraps `axiom_truth::compile_intent` (via [`compile_intent_with_overlay`]) +
//! `organism_runtime::Runtime::admit_intent` so each truth executor stages
//! its IntentPacket once per run with one call. This is handoff step 4:
//! intents enter the Converge kernel through the typed admission gate
//! instead of being passed straight into the engine.
//!
//! [`select_formation_for_intent`] is the handoff §5 surface — given an
//! intent and the host's capability inventory, the FormationGuru picks a
//! primary template (and up to two alternates) from the standard organism
//! catalog. Helms keeps the existing Engine.run path for now; the
//! selection is observability + an input to step 6's tournament
//! orchestration.

use converge_kernel::formation::SuggestorCapability;
use converge_kernel::{
    AdmissionActor, AdmissionActorKind, AdmissionError, AdmissionSource, ContextState,
};
use organism_pack::IntentPacket;
use organism_runtime::guru::SelectionTrace;
use organism_runtime::templates::standard_formation_catalog;
use organism_runtime::{GuruError, IntentAdmissionError, Runtime};

use crate::intent_compile::{CompileTruthError, compile_intent_with_overlay};
use crate::resolve::IntentOverlay;
use crate::TruthDefinition;

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

/// Compile the truth's [`IntentPacket`] through axiom, apply `overlay`, then
/// stage through Converge's typed admission boundary.
///
/// Returns the compiled packet for downstream use (e.g. `Runtime::select_formation`).
///
/// # Errors
///
/// Returns [`AdmitTruthError`] on compile failure, invalid actor/source
/// identity, or admission rejection.
pub fn admit_truth_intent_for_def(
    def: TruthDefinition,
    overlay: &dyn IntentOverlay,
    actor_id: &str,
    source_label: &str,
    context: &mut ContextState,
) -> Result<IntentPacket, AdmitTruthError> {
    let intent = compile_intent_with_overlay(&def, overlay)?;
    let actor = AdmissionActor::new(actor_id, AdmissionActorKind::System)?;
    let source = AdmissionSource::new(source_label)?;
    Runtime::new().admit_intent(&intent, actor, source, context)?;
    Ok(intent)
}

/// The formation template the FormationGuru chose for an intent, plus the
/// auditable trace explaining why. Owned data — the catalog used for
/// selection is dropped by [`select_formation_for_intent`] before this is
/// returned.
#[derive(Debug, Clone)]
pub struct TruthFormationSelection {
    pub primary_template_id: String,
    pub alternate_template_ids: Vec<String>,
    pub trace: SelectionTrace,
}

/// Default capability inventory helms declares to the FormationGuru. helms
/// today supports the entire standard suggestor capability set; trimming
/// this list is how a deployment narrows what formations are eligible.
#[must_use]
pub fn default_helms_capabilities() -> Vec<SuggestorCapability> {
    vec![
        SuggestorCapability::LlmReasoning,
        SuggestorCapability::KnowledgeRetrieval,
        SuggestorCapability::Analytics,
        SuggestorCapability::Optimization,
        SuggestorCapability::PolicyEnforcement,
        SuggestorCapability::HumanInTheLoop,
        SuggestorCapability::ExperienceLearning,
    ]
}

/// Pick a formation template for `intent` from the standard organism
/// catalog given the host's `capabilities`. Returns the primary template
/// id, up to two alternates, and the SelectionTrace for audit/UI surfaces.
///
/// # Errors
///
/// Returns [`GuruError::NoMatch`] when no template in the standard catalog
/// satisfies the intent's classified problem under `capabilities`.
pub fn select_formation_for_intent(
    intent: &IntentPacket,
    capabilities: &[SuggestorCapability],
) -> Result<TruthFormationSelection, GuruError> {
    let catalog = standard_formation_catalog();
    let runtime = Runtime::new();
    let selection = runtime.select_formation(intent, &catalog, capabilities)?;
    Ok(TruthFormationSelection {
        primary_template_id: selection.primary.id().to_string(),
        alternate_template_ids: selection
            .alternates
            .iter()
            .map(|t| t.id().to_string())
            .collect(),
        trace: selection.trace,
    })
}
