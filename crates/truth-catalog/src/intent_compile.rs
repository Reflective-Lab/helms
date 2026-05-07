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
            // Other truths still flow through the legacy `organism_recipe`
            // path; their overlays land here as they migrate.
        }
    }
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
