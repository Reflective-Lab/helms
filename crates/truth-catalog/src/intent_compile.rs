//! axiom-driven IntentPacket construction (organism 1.8.0 migration step 2).
//!
//! Builds an `IntentPacket` for a given `TruthDefinition` by parsing the
//! truth's `.feature` source through axiom, then applying a small helms-side
//! overlay for fields the source schema doesn't yet capture (context JSON,
//! relative expiry, and bare-string constraints/authority).
//!
//! As truths are progressively migrated, the overlay shrinks. When all fields
//! land in source, `truth_overlay` becomes a no-op and `organism_recipe` /
//! `TruthDefinition` can be deleted (handoff step 3).

use chrono::{Duration, Utc};
use organism_pack::IntentPacket;

use crate::TruthDefinition;

/// Errors produced by the axiom-driven compile path.
#[derive(Debug, thiserror::Error)]
pub enum CompileTruthError {
    #[error("truth source did not parse or compile: {0}")]
    Axiom(#[from] axiom_truth::CompileFromSourceError),
}



/// Compile a `TruthDefinition` into an `IntentPacket` via axiom + helms overlay.
///
/// The overlay (per-truth `with_context`, `expires`, supplementary constraints
/// or authority) lives in [`truth_overlay`] until the corresponding governance
/// gets pushed into the source schema.
pub fn compile_intent_for_truth(truth: &TruthDefinition) -> Result<IntentPacket, CompileTruthError> {
    let mut intent = axiom_truth::compile_intent_from_source(truth.gherkin)?;
    truth_overlay(truth, &mut intent);
    Ok(intent)
}

/// Per-truth helms-side overlay. Mirrors what the legacy `organism_recipe`
/// inlined; will shrink as governance migrates into the source schema.
fn truth_overlay(truth: &TruthDefinition, intent: &mut IntentPacket) {
    // Default 1-hour expiry to match the legacy recipe; per-truth overrides
    // can land here once axiom expresses an absolute Authority.expires.
    intent.expires = Utc::now() + Duration::hours(1);

    match truth.key {
        "qualify-inbound-lead" => {
            intent.context = serde_json::json!({
                "pending": ["lead:inbound"],
                "strategies": "next owner and route required",
            });
            intent.constraints = vec!["lead_has_source".to_string()];
        }
        _ => {
            // Other truths still flow through the legacy `organism_recipe`
            // path; their overlays land here as they migrate.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{equiv::intent_packet_equiv, find_truth};

    /// Equivalence gate (handoff step 2): the axiom-compiled IntentPacket plus
    /// helms overlay must match the legacy `organism_recipe` output before the
    /// recipe path can be deleted for this truth.
    #[test]
    fn qualify_inbound_lead_axiom_matches_legacy_recipe() {
        let truth = find_truth("qualify-inbound-lead").expect("truth exists");

        let mut from_axiom = compile_intent_for_truth(&truth).expect("axiom compiles");

        let legacy = crate::organism::organism_recipe_for_test(truth)
            .expect("legacy recipe still produces an intent");

        // Pin expires to the same instant — both sides default to "now + 1h"
        // and the test runs across two `Utc::now()` calls.
        from_axiom.expires = legacy.expires;

        if let Err(diff) = intent_packet_equiv(&from_axiom, &legacy) {
            panic!(
                "axiom-compiled intent diverges from organism_recipe for qualify-inbound-lead:\n{diff}"
            );
        }
    }
}
