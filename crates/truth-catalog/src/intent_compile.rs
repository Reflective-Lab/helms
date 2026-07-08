//! axiom-driven IntentPacket construction (organism 1.8.0 migration step 2).
//!
//! Builds an `IntentPacket` for a given `TruthDefinition` by parsing the
//! truth's `.feature` source through axiom, then applying a content-side
//! overlay for fields the source schema doesn't yet capture (context JSON,
//! relative expiry, and bare-string constraints/authority).
//!
//! Content crates supply their own [`IntentOverlay`] (e.g. `CrmIntentOverlay`
//! in `crm-truths`); the mechanism crate carries zero per-truth knowledge.

use organism_pack::IntentPacket;

use crate::resolve::IntentOverlay;
use crate::TruthDefinition;

/// Errors produced by the axiom-driven compile path.
#[derive(Debug, thiserror::Error)]
pub enum CompileTruthError {
    #[error("truth source did not parse or compile: {0}")]
    Axiom(#[from] axiom_truth::CompileFromSourceError),
}

/// Compile a [`TruthDefinition`] into an [`IntentPacket`] via axiom, then
/// apply `overlay` to fill in content-specific fields.
///
/// # Errors
///
/// Returns [`CompileTruthError`] when axiom cannot parse or compile the
/// truth's `.feature` source.
pub fn compile_intent_with_overlay(
    def: &TruthDefinition,
    overlay: &dyn IntentOverlay,
) -> Result<IntentPacket, CompileTruthError> {
    let mut intent = axiom_truth::compile_intent_from_source(def.gherkin)?;
    overlay.apply(def, &mut intent);
    Ok(intent)
}
