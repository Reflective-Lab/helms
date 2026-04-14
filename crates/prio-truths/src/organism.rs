use chrono::{Duration, Utc};
use organism_domain::packs;
use organism_pack::{DeclarativeBinding, IntentBinding, IntentPacket, IntentResolver};
use organism_runtime::readiness::{self, BudgetProbe, CredentialProbe, PackProbe, ReadinessReport};
use organism_runtime::registry::{Registry, StructuralResolver};
use serde::Serialize;

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
pub fn organism_binding_for_truth(truth_key: &str) -> Option<TruthOrganismBinding> {
    find_truth(truth_key).and_then(build_binding)
}

#[must_use]
pub fn display_pack_names_for_truth(truth_key: &str) -> Option<Vec<String>> {
    organism_binding_for_truth(truth_key).map(|binding| binding.pack_names())
}

fn build_binding(truth: TruthDefinition) -> Option<TruthOrganismBinding> {
    let (blueprint, intent, baseline, readiness) = organism_recipe(truth)?;
    let registry = organism_registry();
    let resolver = StructuralResolver::new(&registry);
    let binding = resolver.resolve(&intent, &baseline);
    let pack_probe = PackProbe::new(&registry);
    let credential_probe = CredentialProbe::new().with_standard_checks();
    let probes: Vec<&dyn readiness::ReadinessProbe> =
        vec![&pack_probe, &credential_probe, &readiness];
    let readiness = readiness::check(&binding, &probes);

    Some(TruthOrganismBinding {
        truth_key: truth.key,
        blueprint,
        binding,
        readiness,
    })
}

fn organism_recipe(
    truth: TruthDefinition,
) -> Option<(
    Option<&'static str>,
    IntentPacket,
    IntentBinding,
    BudgetProbe,
)> {
    let expires = Utc::now() + Duration::hours(1);

    match truth.key {
        "submit-expense-report" => {
            let mut intent =
                IntentPacket::new("submit employee expense report for reimbursement", expires)
                    .with_context(serde_json::json!({
                        "expense": {
                            "receipt": "receipt:pending",
                            "category": "expense:travel",
                            "approval": "approval:route",
                            "budget": "budget_envelope:team-travel",
                        },
                        "documents": ["receipt:pending"],
                        "evaluations": "approval review required",
                    }));
            intent.constraints = vec![
                "approval_has_rationale".to_string(),
                "no_spend_beyond_envelope".to_string(),
            ];

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
                intent,
                binding,
                BudgetProbe::new()
                    .with_token_budget(5_000)
                    .with_spend_budget(5.0),
            ))
        }
        "qualify-inbound-lead" => {
            let mut intent = IntentPacket::new(
                "qualify inbound lead with external enrichment and governed routing",
                expires,
            )
            .with_context(serde_json::json!({
                "pending": ["lead:inbound"],
                "strategies": "next owner and route required",
            }));
            intent.constraints = vec!["lead_has_source".to_string()];

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
                intent,
                binding,
                BudgetProbe::new()
                    .with_token_budget(8_000)
                    .with_spend_budget(8.0),
            ))
        }
        "plan-outbound-campaign" => {
            let mut intent = IntentPacket::new(
                "plan outbound campaign with governed spend and downstream customer handoff",
                expires,
            )
            .with_context(serde_json::json!({
                "campaign": "campaign:q3-pipeline",
                "audience": "audience:target-accounts",
                "budget": "budget:quarterly-outbound",
                "evaluations": "attribution review",
            }));
            intent.constraints = vec!["budget_guardrails_enforced".to_string()];

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
                intent,
                binding,
                BudgetProbe::new()
                    .with_token_budget(6_000)
                    .with_spend_budget(6.0),
            ))
        }
        _ => None,
    }
}

fn organism_registry() -> Registry {
    let mut registry = Registry::new();
    registry.register_pack_with_profile(
        "customers",
        packs::customers::AGENTS,
        packs::customers::INVARIANTS,
        &packs::customers::PROFILE,
    );
    registry.register_pack_with_profile(
        "legal",
        packs::legal::AGENTS,
        packs::legal::INVARIANTS,
        &packs::legal::PROFILE,
    );
    registry.register_pack_with_profile(
        "autonomous_org",
        packs::autonomous_org::AGENTS,
        packs::autonomous_org::INVARIANTS,
        &packs::autonomous_org::PROFILE,
    );
    registry.register_pack_with_profile(
        "partnerships",
        packs::partnerships::AGENTS,
        packs::partnerships::INVARIANTS,
        &packs::partnerships::PROFILE,
    );
    registry.register_pack_with_profile(
        "people",
        packs::people::AGENTS,
        packs::people::INVARIANTS,
        &packs::people::PROFILE,
    );
    registry.register_pack_with_profile(
        "procurement",
        packs::procurement::AGENTS,
        packs::procurement::INVARIANTS,
        &packs::procurement::PROFILE,
    );
    registry.register_pack_with_profile(
        "linkedin_research",
        packs::linkedin_research::AGENTS,
        packs::linkedin_research::INVARIANTS,
        &packs::linkedin_research::PROFILE,
    );
    registry.register_pack_with_profile(
        "knowledge",
        packs::knowledge::AGENTS,
        packs::knowledge::INVARIANTS,
        &packs::knowledge::PROFILE,
    );
    registry.register_pack_with_profile(
        "growth_marketing",
        packs::growth_marketing::AGENTS,
        packs::growth_marketing::INVARIANTS,
        &packs::growth_marketing::PROFILE,
    );
    registry.register_pack_with_profile(
        "ops_support",
        packs::ops_support::AGENTS,
        packs::ops_support::INVARIANTS,
        &packs::ops_support::PROFILE,
    );
    registry.register_pack_with_profile(
        "performance",
        packs::performance::AGENTS,
        packs::performance::INVARIANTS,
        &packs::performance::PROFILE,
    );
    registry.register_pack_with_profile(
        "product_engineering",
        packs::product_engineering::AGENTS,
        packs::product_engineering::INVARIANTS,
        &packs::product_engineering::PROFILE,
    );
    registry.register_pack_with_profile(
        "virtual_teams",
        packs::virtual_teams::AGENTS,
        packs::virtual_teams::INVARIANTS,
        &packs::virtual_teams::PROFILE,
    );
    registry.register_pack_with_profile(
        "reskilling",
        packs::reskilling::AGENTS,
        packs::reskilling::INVARIANTS,
        &packs::reskilling::PROFILE,
    );

    registry.register_capability("web", "URL capture and metadata extraction");
    registry.register_capability("ocr", "Document understanding and receipt extraction");
    registry.register_capability("linkedin", "Professional network research");
    registry.register_capability("social", "Social profile extraction");
    registry.register_capability("patent", "Patent and IP search");

    registry
}
