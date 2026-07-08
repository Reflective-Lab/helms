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
/// This is the primary mechanism entry point introduced by Seam B T3
/// (RFL-172). Content crates supply their own [`IntentOverlay`] implementation
/// (e.g. `CrmIntentOverlay` in `crm-truths`).  The legacy shim below
/// (`compile_intent_for_truth`) uses [`LegacyOverlay`] so existing callers
/// compile unchanged until they migrate in T5/T6.
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

// Seam B T4: LegacyOverlay content moves to crm-truths as CrmIntentOverlay.
// No capability-* imports are needed here; the overlay is pure helms-side data.
struct LegacyOverlay;

impl IntentOverlay for LegacyOverlay {
    /// Per-truth helms-side overlay. Mirrors what the legacy `organism_recipe`
    /// inlined; will shrink as governance migrates into the `.feature` schema.
    /// Seam B T4: moves to crm-truths.
    fn apply(&self, truth: &TruthDefinition, intent: &mut IntentPacket) {
        // Default 1-hour expiry to match the legacy recipe.
        intent.expires = Utc::now() + Duration::hours(1);

        match truth.key {
            "qualify-inbound-lead" => {
                intent.context = serde_json::json!({
                    "pending": ["lead:inbound"],
                    "strategies": "next owner and route required",
                });
                intent.constraints = vec!["lead_has_source".to_string()];
            }
            "submit-expense-report" => {
                intent.context = serde_json::json!({
                    "expense": {
                        "receipt": "receipt:pending",
                        "category": "expense:travel",
                        "approval": "approval:route",
                        "budget": "budget_envelope:team-travel",
                    },
                    "documents": ["receipt:pending"],
                    "evaluations": "approval review required",
                });
                intent.constraints = vec![
                    "approval_has_rationale".to_string(),
                    "no_spend_beyond_envelope".to_string(),
                ];
            }
            "evaluate-acquisition-target" => {
                intent.context = serde_json::json!({
                    "target": "company:pending",
                    "research": ["market", "competition", "technology", "financials", "team"],
                    "evaluations": "investment committee review required",
                });
                intent.constraints = vec![
                    "contradictions_flagged".to_string(),
                    "synthesis_requires_coverage".to_string(),
                    "hypothesis_has_source".to_string(),
                ];
                intent.authority = vec!["investment-committee".to_string()];
            }
            "plan-outbound-campaign" => {
                intent.context = serde_json::json!({
                    "campaign": "campaign:q3-pipeline",
                    "audience": "audience:target-accounts",
                    "budget": "budget:quarterly-outbound",
                    "evaluations": "attribution review",
                });
                intent.constraints = vec!["budget_guardrails_enforced".to_string()];
            }
            _ => {
                // Other truths flow through axiom defaults; overlays land here as they migrate.
            }
        }
    }
}

/// Compile a [`TruthDefinition`] into an [`IntentPacket`] using the legacy
/// CRM overlay table.
///
/// # Seam B T5/T6: consumers migrate to `compile_intent_with_overlay`; this
/// shim then dies in T4-final when `LegacyOverlay` is removed.
pub fn compile_intent_for_truth(
    truth: &TruthDefinition,
) -> Result<IntentPacket, CompileTruthError> {
    compile_intent_with_overlay(truth, &LegacyOverlay)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::find_truth;

    /// Regression test: each truth's outcome string is round-trippable through
    /// axiom's parse + compile pipeline. The legacy `organism_recipe`
    /// equivalence gates lived here during the migration and were retired
    /// once the recipe path was deleted (handoff step 3).
    fn assert_compiles(key: &str) {
        let truth = find_truth(key).unwrap_or_else(|| panic!("truth {key} exists"));
        let intent =
            compile_intent_for_truth(&truth).unwrap_or_else(|e| panic!("compile {key}: {e}"));
        assert!(
            !intent.outcome.trim().is_empty(),
            "{key} compiled with empty outcome"
        );
    }

    #[test]
    fn qualify_inbound_lead_compiles() {
        assert_compiles("qualify-inbound-lead");
    }

    #[test]
    fn submit_expense_report_compiles() {
        assert_compiles("submit-expense-report");
    }

    #[test]
    fn evaluate_acquisition_target_compiles() {
        assert_compiles("evaluate-acquisition-target");
    }

    #[test]
    fn plan_outbound_campaign_compiles() {
        assert_compiles("plan-outbound-campaign");
    }
}
