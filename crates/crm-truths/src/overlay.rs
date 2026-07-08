use chrono::{Duration, Utc};
use organism_pack::IntentPacket;
use truth_catalog::{TruthDefinition, resolve::IntentOverlay, intent_compile::{compile_intent_with_overlay, CompileTruthError}};

pub struct CrmIntentOverlay;

impl IntentOverlay for CrmIntentOverlay {
    fn apply(&self, truth: &TruthDefinition, intent: &mut IntentPacket) {
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
            _ => {}
        }
    }
}

/// Compile a [`TruthDefinition`] into an [`IntentPacket`] using the CRM overlay.
///
/// # Errors
/// Returns [`CompileTruthError`] when axiom cannot parse or compile the truth source.
pub fn compile_intent_for_truth(truth: &TruthDefinition) -> Result<IntentPacket, CompileTruthError> {
    compile_intent_with_overlay(truth, &CrmIntentOverlay)
}
