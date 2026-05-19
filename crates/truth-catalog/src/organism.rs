use organism_pack::{DeclarativeBinding, IntentBinding, IntentResolver};
use organism_runtime::{
    BudgetProbe, CredentialProbe, PackProbe, ReadinessProbe, ReadinessReport, Registry,
    StructuralResolver, check_readiness,
};
use serde::Serialize;

use crate::intent_compile::compile_intent_for_truth;
use crate::{TruthDefinition, find_truth};

#[derive(Debug, Clone, Serialize)]
pub struct TruthOrganismBinding {
    pub truth_key: &'static str,
    pub blueprint: Option<&'static str>,
    pub binding: IntentBinding,
    pub readiness: ReadinessReport,
}

impl TruthOrganismBinding {
    #[must_use]
    pub fn pack_names(&self) -> Vec<String> {
        self.binding
            .packs
            .iter()
            .map(|pack| pack.pack_name.clone())
            .collect()
    }
}

#[must_use]
pub fn organism_binding_for_truth(
    truth_key: &str,
    registry: &Registry,
) -> Option<TruthOrganismBinding> {
    find_truth(truth_key).and_then(|truth| build_binding(truth, registry))
}

#[must_use]
pub fn display_pack_names_for_truth(truth_key: &str, registry: &Registry) -> Option<Vec<String>> {
    organism_binding_for_truth(truth_key, registry).map(|binding| binding.pack_names())
}

fn build_binding(truth: TruthDefinition, registry: &Registry) -> Option<TruthOrganismBinding> {
    let (blueprint, baseline, readiness) = binding_recipe(truth)?;
    let intent = compile_intent_for_truth(&truth)
        .expect("truth has axiom-compilable governance and a known overlay");
    let resolver = StructuralResolver::new(registry);
    let binding = resolver.resolve(&intent, &baseline);
    let pack_probe = PackProbe::new(registry);
    let credential_probe = CredentialProbe::new().with_standard_checks();
    let probes: Vec<&dyn ReadinessProbe> = vec![&pack_probe, &credential_probe, &readiness];
    let readiness = check_readiness(&binding, &probes);

    Some(TruthOrganismBinding {
        truth_key: truth.key,
        blueprint,
        binding,
        readiness,
    })
}

/// Per-truth helms-static binding metadata: blueprint label, the declarative
/// pack/capability/invariant baseline that `StructuralResolver` resolves
/// against, and the budget probe used by readiness checks.
///
/// The IntentPacket part of the legacy `organism_recipe` lives in
/// `intent_compile::compile_intent_for_truth` now (axiom-compiled +
/// helms overlay). Whatever remains here is the "smart selection" surface
/// the handoff explicitly tells helms to keep until `select_formation`
/// replaces it (handoff step 5).
fn binding_recipe(
    truth: TruthDefinition,
) -> Option<(Option<&'static str>, IntentBinding, BudgetProbe)> {
    match truth.key {
        "submit-expense-report" => {
            let binding = DeclarativeBinding::new()
                .pack(
                    "procurement",
                    "expense intake, reimbursement routing, and export readiness",
                )
                .pack(
                    "autonomous_org",
                    "approval policy, spend governance, and exception handling",
                )
                .capability("ocr", "extract receipt fields from uploaded evidence")
                .invariant("approval_has_rationale")
                .invariant("no_spend_beyond_envelope")
                .build();
            Some((
                Some("procure_to_pay"),
                binding,
                BudgetProbe::new()
                    .with_token_budget(5_000)
                    .with_spend_budget(5.0),
            ))
        }
        "qualify-inbound-lead" => {
            let binding = DeclarativeBinding::new()
                .pack("customers", "lead qualification workflow")
                .pack(
                    "linkedin_research",
                    "external company and stakeholder enrichment",
                )
                .invariant("lead_has_source")
                .build();
            Some((
                Some("lead_to_cash"),
                binding,
                BudgetProbe::new()
                    .with_token_budget(8_000)
                    .with_spend_budget(8.0),
            ))
        }
        "evaluate-acquisition-target" => {
            let binding = DeclarativeBinding::new()
                .pack(
                    "due_diligence",
                    "convergent research, fact extraction, gap detection, contradiction finding, synthesis",
                )
                .pack("legal", "legal review of findings and contractual implications")
                .pack(
                    "knowledge",
                    "persist confirmed findings to the knowledge base",
                )
                .capability("web", "broad and deep web research for company intelligence")
                .capability("llm", "fact extraction, gap detection, and synthesis")
                .invariant("hypothesis_has_source")
                .invariant("contradictions_flagged")
                .invariant("synthesis_requires_coverage")
                .build();
            Some((
                Some("diligence_to_decision"),
                binding,
                BudgetProbe::new()
                    .with_token_budget(20_000)
                    .with_spend_budget(20.0),
            ))
        }
        "plan-outbound-campaign" => {
            let binding = DeclarativeBinding::new()
                .pack(
                    "growth_marketing",
                    "campaign planning, allocation, and channel execution",
                )
                .pack("customers", "downstream lead handling and handoff")
                .invariant("budget_guardrails_enforced")
                .build();
            Some((
                Some("campaign_to_revenue"),
                binding,
                BudgetProbe::new()
                    .with_token_budget(6_000)
                    .with_spend_budget(6.0),
            ))
        }
        _ => None,
    }
}
