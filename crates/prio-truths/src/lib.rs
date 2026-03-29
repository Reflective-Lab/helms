mod converge;

use serde::Serialize;

pub use converge::{
    ActivateSubscriptionEvaluator, MatchRenewalContextEvaluator, PlanOutboundCampaignEvaluator,
    QualifyInboundLeadEvaluator, RefillPrepaidAiCreditsEvaluator, ScoreInboundFitEvaluator,
    StaticTruthCatalog, SuspendServiceOnPaymentFailureEvaluator, UpgradeSubscriptionPlanEvaluator,
    converge_truth_definition,
};
pub use converge::{TruthConvergeBinding, converge_binding_for_truth};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TruthKind {
    Job,
    Policy,
    ModuleLocal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TruthModuleTouch {
    pub module_key: &'static str,
    pub responsibility: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TruthDefinition {
    pub key: &'static str,
    pub display_name: &'static str,
    pub kind: TruthKind,
    pub summary: &'static str,
    pub feature_path: &'static str,
    pub actor_roles: &'static [&'static str],
    pub approval_points: &'static [&'static str],
    pub desired_outcomes: &'static [&'static str],
    pub guardrails: &'static [&'static str],
    pub modules: &'static [TruthModuleTouch],
    pub gherkin: &'static str,
}

pub const TRUTHS: &[TruthDefinition] = &[
    TruthDefinition {
        key: "qualify-inbound-lead",
        display_name: "Qualify inbound lead",
        kind: TruthKind::Job,
        summary: "Capture inbound demand, verify fit, and assign an explicit next commercial step.",
        feature_path: "truths/jobs/qualify_inbound_lead.feature",
        actor_roles: &["commercial-operator", "sales-agent"],
        approval_points: &["manual handoff when fit or authority is ambiguous"],
        desired_outcomes: &[
            "lead is explicitly qualified or disqualified",
            "next owner and next step are recorded",
        ],
        guardrails: &[
            "qualification facts must cite attributable evidence",
            "disqualification reason must be explicit and queryable",
        ],
        modules: &[
            TruthModuleTouch {
                module_key: "parties",
                responsibility: "persist organization, contact, and stakeholder context",
            },
            TruthModuleTouch {
                module_key: "opportunities",
                responsibility: "create lead and opportunity state",
            },
            TruthModuleTouch {
                module_key: "conversations",
                responsibility: "capture the inbound thread and follow-up context",
            },
            TruthModuleTouch {
                module_key: "facts",
                responsibility: "promote verified qualification signals",
            },
            TruthModuleTouch {
                module_key: "intents",
                responsibility: "frame the JTBD and success criteria",
            },
        ],
        gherkin: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../truths/jobs/qualify_inbound_lead.feature"
        )),
    },
    TruthDefinition {
        key: "score-inbound-fit",
        display_name: "Score inbound fit",
        kind: TruthKind::Job,
        summary: "Use website behavior and inbound context to produce a governed fit score for a lead.",
        feature_path: "truths/jobs/score_inbound_fit.feature",
        actor_roles: &["growth-operator", "commercial-analyst", "runtime-agent"],
        approval_points: &["manual review when the behavioral signal quality is weak"],
        desired_outcomes: &[
            "a governed fit score is recorded for the inbound lead",
            "the score cites attributable behavioral evidence",
        ],
        guardrails: &[
            "fit scoring must retain traceable behavioral provenance",
            "weak or sparse signal quality must not be treated as high confidence",
        ],
        modules: &[
            TruthModuleTouch {
                module_key: "parties",
                responsibility: "anchor the score to an organization or contact context",
            },
            TruthModuleTouch {
                module_key: "metering",
                responsibility: "supply attributable website and usage event history",
            },
            TruthModuleTouch {
                module_key: "opportunities",
                responsibility: "make the commercial fit signal available to downstream lead handling",
            },
        ],
        gherkin: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../truths/jobs/score_inbound_fit.feature"
        )),
    },
    TruthDefinition {
        key: "plan-outbound-campaign",
        display_name: "Plan outbound campaign",
        kind: TruthKind::Job,
        summary: "Assign prospects to reps and schedule campaign work under capacity and budget guardrails.",
        feature_path: "truths/jobs/plan_outbound_campaign.feature",
        actor_roles: &["growth-operator", "sales-manager", "runtime-agent"],
        approval_points: &["manual approval when campaign spend exceeds the allocated budget"],
        desired_outcomes: &[
            "a governed outbound campaign plan exists",
            "campaign budget status is explicit and queryable",
        ],
        guardrails: &[
            "campaign plans must retain assignment rationale",
            "budget overruns require an explicit approval path",
        ],
        modules: &[
            TruthModuleTouch {
                module_key: "opportunities",
                responsibility: "provide the prospect pool and expected commercial value",
            },
            TruthModuleTouch {
                module_key: "tasks",
                responsibility: "translate campaign assignments into executable work",
            },
            TruthModuleTouch {
                module_key: "workflow",
                responsibility: "track the campaign plan and exception path",
            },
            TruthModuleTouch {
                module_key: "ledger",
                responsibility: "govern budget consumption and auditability",
            },
        ],
        gherkin: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../truths/jobs/plan_outbound_campaign.feature"
        )),
    },
    TruthDefinition {
        key: "match-renewal-context",
        display_name: "Match renewal context",
        kind: TruthKind::Job,
        summary: "Retrieve and converge the most relevant account history ahead of a contract renewal.",
        feature_path: "truths/jobs/match_renewal_context.feature",
        actor_roles: &["account-owner", "renewal-manager", "runtime-agent"],
        approval_points: &["manual review when renewal terms fall outside the standard path"],
        desired_outcomes: &[
            "a renewal brief is attached to the account or renewal motion",
            "retrieved renewal signals stay traceable to their source artifacts",
        ],
        guardrails: &[
            "renewal retrieval must preserve source attribution",
            "non-standard renewal terms require an explicit human gate",
        ],
        modules: &[
            TruthModuleTouch {
                module_key: "parties",
                responsibility: "anchor retrieval to the customer account and stakeholders",
            },
            TruthModuleTouch {
                module_key: "conversations",
                responsibility: "supply call, email, and timeline context",
            },
            TruthModuleTouch {
                module_key: "documents",
                responsibility: "store the resulting renewal brief and source artifacts",
            },
            TruthModuleTouch {
                module_key: "opportunities",
                responsibility: "tie retrieved context to the renewal commercial motion",
            },
            TruthModuleTouch {
                module_key: "memory",
                responsibility: "provide semantic retrieval and learned relevance",
            },
        ],
        gherkin: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../truths/jobs/match_renewal_context.feature"
        )),
    },
    TruthDefinition {
        key: "create-customer-workspace",
        display_name: "Create customer workspace",
        kind: TruthKind::Job,
        summary: "Provision a customer workspace with the right commercial and access context.",
        feature_path: "truths/jobs/create_customer_workspace.feature",
        actor_roles: &["customer-ops", "revops", "runtime-agent"],
        approval_points: &["exception approval before provisioning non-standard workspaces"],
        desired_outcomes: &[
            "workspace exists with the correct owner",
            "commercial plan and quotas are attached",
        ],
        guardrails: &[
            "provisioning cannot finish without a linked account",
            "workspace activation must reference a commercial commitment",
        ],
        modules: &[
            TruthModuleTouch {
                module_key: "parties",
                responsibility: "anchor the workspace to the customer account",
            },
            TruthModuleTouch {
                module_key: "subscriptions",
                responsibility: "bind the workspace to the purchased commitment",
            },
            TruthModuleTouch {
                module_key: "entitlements",
                responsibility: "apply quotas and feature access",
            },
            TruthModuleTouch {
                module_key: "workflow",
                responsibility: "track the provisioning case and exceptions",
            },
            TruthModuleTouch {
                module_key: "approvals",
                responsibility: "control exceptions or manual releases",
            },
            TruthModuleTouch {
                module_key: "intents",
                responsibility: "keep the operator-facing job context explicit",
            },
        ],
        gherkin: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../truths/jobs/create_customer_workspace.feature"
        )),
    },
    TruthDefinition {
        key: "activate-subscription",
        display_name: "Activate subscription",
        kind: TruthKind::Job,
        summary: "Turn an agreed commercial plan into an active subscription and entitlement state.",
        feature_path: "truths/jobs/activate_subscription.feature",
        actor_roles: &["revops", "billing-operator"],
        approval_points: &["manual review for non-standard plan terms"],
        desired_outcomes: &[
            "subscription becomes active with an explicit plan",
            "entitlements and financial opening state are aligned",
        ],
        guardrails: &[
            "an active subscription must resolve to a valid catalog plan",
            "activation events must remain auditable",
        ],
        modules: &[
            TruthModuleTouch {
                module_key: "catalog",
                responsibility: "resolve the plan and pricing definition",
            },
            TruthModuleTouch {
                module_key: "subscriptions",
                responsibility: "persist subscription lifecycle state",
            },
            TruthModuleTouch {
                module_key: "entitlements",
                responsibility: "derive usable access from the plan",
            },
            TruthModuleTouch {
                module_key: "ledger",
                responsibility: "open the auditable commercial balance context",
            },
            TruthModuleTouch {
                module_key: "workflow",
                responsibility: "coordinate activation checks and handoffs",
            },
        ],
        gherkin: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../truths/jobs/activate_subscription.feature"
        )),
    },
    TruthDefinition {
        key: "refill-prepaid-ai-credits",
        display_name: "Refill prepaid AI credits",
        kind: TruthKind::Job,
        summary: "Apply a top-up purchase to prepaid AI credit balances with financial traceability.",
        feature_path: "truths/jobs/refill_prepaid_ai_credits.feature",
        actor_roles: &["customer", "billing-operator", "runtime-agent"],
        approval_points: &["manual review for unusual top-up size or risk signal"],
        desired_outcomes: &[
            "confirmed top-up appears in the ledger",
            "entitlement balance increases for the correct account",
        ],
        guardrails: &[
            "payment must be confirmed before any credit grant",
            "top-up must remain linked to the customer account and commercial context",
        ],
        modules: &[
            TruthModuleTouch {
                module_key: "parties",
                responsibility: "link the purchase to the customer account",
            },
            TruthModuleTouch {
                module_key: "subscriptions",
                responsibility: "resolve the active commercial commitment",
            },
            TruthModuleTouch {
                module_key: "payments",
                responsibility: "confirm settlement state",
            },
            TruthModuleTouch {
                module_key: "ledger",
                responsibility: "record the auditable credit grant",
            },
            TruthModuleTouch {
                module_key: "entitlements",
                responsibility: "increase the usable prepaid balance",
            },
        ],
        gherkin: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../truths/jobs/refill_prepaid_ai_credits.feature"
        )),
    },
    TruthDefinition {
        key: "upgrade-subscription-plan",
        display_name: "Upgrade subscription plan",
        kind: TruthKind::Job,
        summary: "Migrate a customer to a better plan while keeping pricing, access, and approval history coherent.",
        feature_path: "truths/jobs/upgrade_subscription_plan.feature",
        actor_roles: &["account-owner", "customer", "revops"],
        approval_points: &["approval for price override or custom migration terms"],
        desired_outcomes: &[
            "subscription moves to the target plan on an explicit date",
            "entitlements and commercial delta stay aligned",
        ],
        guardrails: &[
            "target plan must exist in catalog",
            "non-standard commercial deltas require explicit approval",
        ],
        modules: &[
            TruthModuleTouch {
                module_key: "catalog",
                responsibility: "resolve target plan and pricing metadata",
            },
            TruthModuleTouch {
                module_key: "subscriptions",
                responsibility: "apply the lifecycle transition",
            },
            TruthModuleTouch {
                module_key: "entitlements",
                responsibility: "swap access and quota state",
            },
            TruthModuleTouch {
                module_key: "approvals",
                responsibility: "govern exceptional terms",
            },
            TruthModuleTouch {
                module_key: "ledger",
                responsibility: "record financial deltas and adjustments",
            },
        ],
        gherkin: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../truths/jobs/upgrade_subscription_plan.feature"
        )),
    },
    TruthDefinition {
        key: "suspend-service-on-payment-failure",
        display_name: "Suspend service on payment failure",
        kind: TruthKind::Job,
        summary: "Apply suspension policy when payment state fails while preserving controlled recovery paths.",
        feature_path: "truths/jobs/suspend_service_on_payment_failure.feature",
        actor_roles: &["billing-operator", "customer-success", "runtime-agent"],
        approval_points: &["override approval before suspending strategic accounts"],
        desired_outcomes: &[
            "service state matches payment policy",
            "customer receives a clear recovery path",
        ],
        guardrails: &[
            "grace rules must be evaluated before suspension",
            "reactivation path must remain explicit and auditable",
        ],
        modules: &[
            TruthModuleTouch {
                module_key: "payments",
                responsibility: "surface failed or overdue payment state",
            },
            TruthModuleTouch {
                module_key: "subscriptions",
                responsibility: "apply subscription lifecycle suspension",
            },
            TruthModuleTouch {
                module_key: "entitlements",
                responsibility: "reduce or pause access appropriately",
            },
            TruthModuleTouch {
                module_key: "workflow",
                responsibility: "run the suspension case and grace timers",
            },
            TruthModuleTouch {
                module_key: "parties",
                responsibility: "keep customer ownership and communication routing intact",
            },
        ],
        gherkin: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../truths/jobs/suspend_service_on_payment_failure.feature"
        )),
    },
    TruthDefinition {
        key: "resolve-support-incident",
        display_name: "Resolve support incident",
        kind: TruthKind::Job,
        summary: "Drive a customer issue from intake through diagnosis to a verified resolution or escalation.",
        feature_path: "truths/jobs/resolve_support_incident.feature",
        actor_roles: &["support-agent", "subject-matter-expert", "customer"],
        approval_points: &["escalation approval for risky or customer-impacting workaround"],
        desired_outcomes: &[
            "incident is resolved or deliberately escalated",
            "root cause and customer-facing resolution are documented",
        ],
        guardrails: &[
            "resolution claims require evidence",
            "customer-visible status must be updated before closure",
        ],
        modules: &[
            TruthModuleTouch {
                module_key: "conversations",
                responsibility: "hold the incident thread and external communications",
            },
            TruthModuleTouch {
                module_key: "tasks",
                responsibility: "coordinate follow-ups and handoffs",
            },
            TruthModuleTouch {
                module_key: "documents",
                responsibility: "store runbooks, notes, and attachments",
            },
            TruthModuleTouch {
                module_key: "workflow",
                responsibility: "track severity, SLA, and resolution state",
            },
            TruthModuleTouch {
                module_key: "facts",
                responsibility: "promote verified diagnosis and remediation facts",
            },
        ],
        gherkin: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../truths/jobs/resolve_support_incident.feature"
        )),
    },
    TruthDefinition {
        key: "reconcile-model-usage-against-customer-ledger",
        display_name: "Reconcile model usage against customer ledger",
        kind: TruthKind::Job,
        summary: "Align usage metering, financial balance, and entitlement burn-down without mutating history.",
        feature_path: "truths/jobs/reconcile_model_usage_against_customer_ledger.feature",
        actor_roles: &["finance-ops", "runtime-agent"],
        approval_points: &["human review for unreconciled delta above threshold"],
        desired_outcomes: &[
            "usage and financial state reconcile cleanly",
            "exceptions are recorded and routed",
        ],
        guardrails: &[
            "reconciliation must preserve immutable ledger history",
            "adjustments must remain traceable to evidence",
        ],
        modules: &[
            TruthModuleTouch {
                module_key: "metering",
                responsibility: "provide normalized usage events and consumption state",
            },
            TruthModuleTouch {
                module_key: "ledger",
                responsibility: "provide auditable financial balance movements",
            },
            TruthModuleTouch {
                module_key: "entitlements",
                responsibility: "compare usage against usable balance and quota state",
            },
            TruthModuleTouch {
                module_key: "subscriptions",
                responsibility: "resolve commercial terms and billing period context",
            },
            TruthModuleTouch {
                module_key: "audit",
                responsibility: "preserve reconciliation provenance and evidence",
            },
        ],
        gherkin: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../truths/jobs/reconcile_model_usage_against_customer_ledger.feature"
        )),
    },
    TruthDefinition {
        key: "detect-abnormal-token-burn",
        display_name: "Detect abnormal token burn",
        kind: TruthKind::Job,
        summary: "Detect unusual usage patterns early and route a controlled mitigation path.",
        feature_path: "truths/jobs/detect_abnormal_token_burn.feature",
        actor_roles: &["runtime-agent", "customer-success"],
        approval_points: &["operator approval before hard-limit intervention"],
        desired_outcomes: &[
            "anomaly is explained with telemetry",
            "a mitigation case is opened with recommended actions",
        ],
        guardrails: &[
            "automated intervention must respect policy thresholds",
            "anomaly assertions must cite observed telemetry",
        ],
        modules: &[
            TruthModuleTouch {
                module_key: "metering",
                responsibility: "surface the usage anomaly signals",
            },
            TruthModuleTouch {
                module_key: "entitlements",
                responsibility: "show quota and balance exposure",
            },
            TruthModuleTouch {
                module_key: "workflow",
                responsibility: "open and track the intervention case",
            },
            TruthModuleTouch {
                module_key: "memory",
                responsibility: "provide historical context and comparable patterns",
            },
            TruthModuleTouch {
                module_key: "agent-ops",
                responsibility: "track the detecting agents and validation chain",
            },
        ],
        gherkin: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../truths/jobs/detect_abnormal_token_burn.feature"
        )),
    },
    TruthDefinition {
        key: "renew-contract",
        display_name: "Renew contract",
        kind: TruthKind::Job,
        summary: "Move a renewal from account context to approved commercial terms and current documents.",
        feature_path: "truths/jobs/renew_contract.feature",
        actor_roles: &["account-owner", "legal-operator", "customer"],
        approval_points: &["approval for non-standard renewal terms"],
        desired_outcomes: &[
            "renewal ends in accepted terms or an explicit no-renew decision",
            "current commercial documents remain linked and versioned",
        ],
        guardrails: &[
            "renewal cannot close without explicit commercial terms",
            "the current proposal or contract version must remain traceable",
        ],
        modules: &[
            TruthModuleTouch {
                module_key: "parties",
                responsibility: "provide account and stakeholder ownership context",
            },
            TruthModuleTouch {
                module_key: "catalog",
                responsibility: "resolve current offerable plans and prices",
            },
            TruthModuleTouch {
                module_key: "opportunities",
                responsibility: "carry renewal pipeline and forecast state",
            },
            TruthModuleTouch {
                module_key: "subscriptions",
                responsibility: "link the renewal to the active commercial commitment",
            },
            TruthModuleTouch {
                module_key: "approvals",
                responsibility: "govern non-standard commercial decisions",
            },
            TruthModuleTouch {
                module_key: "documents",
                responsibility: "store proposal, quote, and contract artifacts",
            },
        ],
        gherkin: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../truths/jobs/renew_contract.feature"
        )),
    },
    TruthDefinition {
        key: "top-up-requires-confirmed-payment",
        display_name: "Top-up requires confirmed payment",
        kind: TruthKind::Policy,
        summary: "No prepaid balance increase may occur until settlement is confirmed.",
        feature_path: "truths/policies/top_up_requires_confirmed_payment.feature",
        actor_roles: &["billing-operator", "runtime-agent"],
        approval_points: &["override approval for manual corrective grant"],
        desired_outcomes: &[
            "credit grants only occur after confirmed settlement",
            "manual overrides remain explicit and auditable",
        ],
        guardrails: &[
            "unconfirmed payment blocks credit application",
            "override path must create provenance and rationale",
        ],
        modules: &[
            TruthModuleTouch {
                module_key: "payments",
                responsibility: "declare settlement state",
            },
            TruthModuleTouch {
                module_key: "ledger",
                responsibility: "block or record the credit movement",
            },
            TruthModuleTouch {
                module_key: "entitlements",
                responsibility: "avoid premature balance increase",
            },
            TruthModuleTouch {
                module_key: "policies",
                responsibility: "own the cross-module guardrail",
            },
            TruthModuleTouch {
                module_key: "audit",
                responsibility: "capture override evidence and decision trail",
            },
        ],
        gherkin: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../truths/policies/top_up_requires_confirmed_payment.feature"
        )),
    },
    TruthDefinition {
        key: "overdue-balance-blocks-entitlement-increase",
        display_name: "Overdue balance blocks entitlement increase",
        kind: TruthKind::Policy,
        summary: "Customers with overdue obligations should not receive expanded access without exception handling.",
        feature_path: "truths/policies/overdue_balance_blocks_entitlement_increase.feature",
        actor_roles: &["finance-ops", "customer-success"],
        approval_points: &["exception approval for temporary relief"],
        desired_outcomes: &[
            "overdue customers do not receive expanded entitlements by default",
            "temporary relief remains explicit and time-bound",
        ],
        guardrails: &[
            "overdue evaluation must use current payment state",
            "exceptions must expire or be revisited explicitly",
        ],
        modules: &[
            TruthModuleTouch {
                module_key: "payments",
                responsibility: "surface overdue state",
            },
            TruthModuleTouch {
                module_key: "entitlements",
                responsibility: "block entitlement expansion until resolved",
            },
            TruthModuleTouch {
                module_key: "policies",
                responsibility: "define the blocking rule and exception policy",
            },
            TruthModuleTouch {
                module_key: "workflow",
                responsibility: "run the exception path and follow-up timers",
            },
            TruthModuleTouch {
                module_key: "parties",
                responsibility: "bind the exception to the customer account",
            },
        ],
        gherkin: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../truths/policies/overdue_balance_blocks_entitlement_increase.feature"
        )),
    },
    TruthDefinition {
        key: "promoted-fact-requires-traceable-evidence",
        display_name: "Promoted fact requires traceable evidence",
        kind: TruthKind::Policy,
        summary: "Durable business truth must remain backed by evidence and provenance.",
        feature_path: "truths/policies/promoted_fact_requires_traceable_evidence.feature",
        actor_roles: &["analyst", "runtime-agent", "approver"],
        approval_points: &["approval for low-confidence promotion"],
        desired_outcomes: &[
            "every promoted fact links to evidence",
            "low-confidence facts stay proposed until reviewed",
        ],
        guardrails: &[
            "unverifiable statements shall not become durable truth",
            "provenance for promoted facts shall remain immutable",
        ],
        modules: &[
            TruthModuleTouch {
                module_key: "facts",
                responsibility: "hold proposed and promoted facts",
            },
            TruthModuleTouch {
                module_key: "documents",
                responsibility: "store or link the supporting evidence",
            },
            TruthModuleTouch {
                module_key: "audit",
                responsibility: "capture the promotion decision trail",
            },
            TruthModuleTouch {
                module_key: "policies",
                responsibility: "enforce the promotion guardrail",
            },
        ],
        gherkin: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../truths/policies/promoted_fact_requires_traceable_evidence.feature"
        )),
    },
    TruthDefinition {
        key: "ledger-entry-is-immutable",
        display_name: "Ledger entry is immutable",
        kind: TruthKind::ModuleLocal,
        summary: "Posted balance movements remain append-only, with corrections expressed as new entries.",
        feature_path: "truths/modules/ledger_entry_is_immutable.feature",
        actor_roles: &["finance-ops"],
        approval_points: &[],
        desired_outcomes: &[
            "original ledger entries remain unchanged",
            "corrections are expressed as adjusting entries",
        ],
        guardrails: &[
            "posted ledger entries are append-only",
            "correction chains must stay audit-linked",
        ],
        modules: &[
            TruthModuleTouch {
                module_key: "ledger",
                responsibility: "own immutable balance history",
            },
            TruthModuleTouch {
                module_key: "audit",
                responsibility: "preserve the correction provenance chain",
            },
        ],
        gherkin: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../truths/modules/ledger_entry_is_immutable.feature"
        )),
    },
    TruthDefinition {
        key: "active-subscription-requires-plan",
        display_name: "Active subscription requires plan",
        kind: TruthKind::ModuleLocal,
        summary: "A subscription cannot be active unless it resolves to a valid plan and entitlement source.",
        feature_path: "truths/modules/active_subscription_requires_plan.feature",
        actor_roles: &["revops", "runtime-agent"],
        approval_points: &[],
        desired_outcomes: &[
            "every active subscription maps to a valid plan",
            "the entitlement source for active access is explicit",
        ],
        guardrails: &[
            "activation is blocked without a valid plan",
            "entitlement template source must be explicit",
        ],
        modules: &[
            TruthModuleTouch {
                module_key: "catalog",
                responsibility: "provide the authoritative plan definition",
            },
            TruthModuleTouch {
                module_key: "subscriptions",
                responsibility: "own subscription lifecycle validity",
            },
            TruthModuleTouch {
                module_key: "entitlements",
                responsibility: "resolve access from the selected plan",
            },
        ],
        gherkin: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../truths/modules/active_subscription_requires_plan.feature"
        )),
    },
];

#[must_use]
pub fn all_truths() -> Vec<TruthDefinition> {
    TRUTHS.to_vec()
}

#[must_use]
pub fn truths_by_kind(kind: TruthKind) -> Vec<TruthDefinition> {
    TRUTHS
        .iter()
        .copied()
        .filter(|truth| truth.kind == kind)
        .collect()
}

#[must_use]
pub fn truths_for_module(module_key: &str) -> Vec<TruthDefinition> {
    TRUTHS
        .iter()
        .copied()
        .filter(|truth| {
            truth
                .modules
                .iter()
                .any(|touch| touch.module_key == module_key)
        })
        .collect()
}

#[must_use]
pub fn find_truth(key: &str) -> Option<TruthDefinition> {
    TRUTHS.iter().copied().find(|truth| truth.key == key)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use prio_modules::MODULES;

    use super::{TRUTHS, TruthKind, converge_binding_for_truth, truths_by_kind};

    #[test]
    fn starter_catalog_has_thirteen_job_truths() {
        assert_eq!(truths_by_kind(TruthKind::Job).len(), 13);
    }

    #[test]
    fn starter_catalog_spans_all_truth_classes() {
        assert!(!truths_by_kind(TruthKind::Job).is_empty());
        assert!(!truths_by_kind(TruthKind::Policy).is_empty());
        assert!(!truths_by_kind(TruthKind::ModuleLocal).is_empty());
    }

    #[test]
    fn every_referenced_module_exists_in_registry() {
        let known_modules = MODULES
            .iter()
            .map(|module| module.key)
            .collect::<BTreeSet<_>>();
        for truth in TRUTHS {
            for touch in truth.modules {
                assert!(
                    known_modules.contains(touch.module_key),
                    "unknown module '{}' in truth '{}'",
                    touch.module_key,
                    truth.key
                );
            }
        }
    }

    #[test]
    fn qualify_inbound_lead_maps_to_converge_binding() {
        let binding = converge_binding_for_truth("qualify-inbound-lead")
            .expect("binding should exist for starter truth");
        assert_eq!(binding.runtime, "converge");
        assert_eq!(
            binding.pack_ids,
            vec![
                "prio-relationship-pack",
                "prio-commercial-pack",
                "prio-work-pack",
                "trust",
                "knowledge",
            ]
        );
        assert_eq!(binding.intent.id.as_str(), "truth:qualify-inbound-lead");
        assert_eq!(
            binding.intent.request,
            "Qualify inbound lead: Capture inbound demand, verify fit, and assign an explicit next commercial step."
        );
        assert_eq!(
            binding.intent.active_packs,
            vec![
                "prio-relationship-pack".to_string(),
                "prio-commercial-pack".to_string(),
                "prio-work-pack".to_string(),
                "trust".to_string(),
                "knowledge".to_string(),
            ]
        );
        assert_eq!(binding.intent.success_criteria.len(), 2);
        assert_eq!(binding.intent.constraints.len(), 3);
    }
}
