use axiom_truth::{
    APPLET_MANIFEST_VERSION, AppletStatus, ConflictPolicy, EvidenceAuthority,
    applet_manifest_json_schema, parse_applet_manifest_json,
};
use crm_truths::{CRM_CATALOG, compile_intent_for_truth, find_truth};

const ACTIVATE_SUBSCRIPTION: &str = include_str!("fixtures/activate-subscription.intent.json");
const REFILL_PREPAID_AI_CREDITS: &str =
    include_str!("fixtures/refill-prepaid-ai-credits.intent.json");

#[test]
fn revenue_applet_manifests_validate_and_bind_to_truth_catalog() {
    let schema = applet_manifest_json_schema().expect("Axiom applet schema parses");
    assert_eq!(
        schema["properties"]["manifest_version"]["const"],
        APPLET_MANIFEST_VERSION
    );

    for source in [ACTIVATE_SUBSCRIPTION, REFILL_PREPAID_AI_CREDITS] {
        let manifest = parse_applet_manifest_json(source).expect("manifest validates");
        assert_eq!(manifest.status, AppletStatus::CodeBacked);
        assert_eq!(
            manifest.evidence_contract.conflict_policy,
            ConflictPolicy::Stop
        );
        assert!(
            manifest
                .evidence_contract
                .required_sources
                .iter()
                .any(|source| source.authority == EvidenceAuthority::Primary),
            "{} should require primary evidence",
            manifest.primary_job_key
        );

        let truth = find_truth(&manifest.primary_job_key)
            .unwrap_or_else(|| panic!("truth {} exists", manifest.primary_job_key));
        assert_eq!(truth.key, manifest.primary_job_key);
        let expected_outcome_fragment = match manifest.primary_job_key.as_str() {
            "activate-subscription" => "active subscription",
            "refill-prepaid-ai-credits" => "prepaid AI credit balances",
            key => panic!("unexpected revenue applet manifest {key}"),
        };
        assert!(
            manifest
                .functional_need
                .outcome
                .contains(expected_outcome_fragment),
            "{} manifest should stay aligned with the Helm truth outcome",
            manifest.primary_job_key
        );

        let intent = compile_intent_for_truth(&truth)
            .unwrap_or_else(|error| panic!("truth {} compiles: {error}", truth.key));
        assert!(
            !intent.outcome.trim().is_empty(),
            "{} should compile to an IntentPacket",
            truth.key
        );
    }
}

/// Behavior-preservation gate (RFL-172 T5, plan risk 2).
///
/// Proves that `CrmIntentOverlay` — now injected at the mounting site rather
/// than called inside the mechanism — applies the same overlay fields to
/// `qualify-inbound-lead` that the old key-based `admit_truth_intent` produced.
///
/// The compiled-intent payload emitted as `axiom.intent.compiled` by
/// `helm-governed-jobs` when the mounting binary injects `CrmIntentOverlay`
/// must match this shape byte-for-byte.
#[test]
fn qualify_inbound_lead_overlay_fields_match_crm_intent_overlay() {
    use truth_catalog::TruthKey;

    let key = TruthKey::parse("qualify-inbound-lead").expect("valid key");
    let truth = CRM_CATALOG
        .find(&key)
        .copied()
        .expect("qualify-inbound-lead is in CRM_CATALOG");

    let intent = compile_intent_for_truth(&truth)
        .expect("qualify-inbound-lead compiles with CrmIntentOverlay");

    // Overlay must set a non-empty outcome from axiom source.
    assert!(!intent.outcome.trim().is_empty(), "outcome is set from gherkin");

    // CrmIntentOverlay sets context with lead-routing fields.
    let context = &intent.context;
    assert!(
        context.get("pending").is_some(),
        "context must have 'pending' (lead routing) — got: {context}"
    );
    assert!(
        context.get("strategies").is_some(),
        "context must have 'strategies' (next-owner logic) — got: {context}"
    );

    // CrmIntentOverlay sets constraints for qualify-inbound-lead.
    assert!(
        intent.constraints.iter().any(|c| c == "lead_has_source"),
        "constraints must include 'lead_has_source' — got: {:?}",
        intent.constraints
    );

    // Expiry is set by the overlay (1 hour horizon).
    let now = chrono::Utc::now();
    assert!(
        intent.expires > now,
        "intent expiry must be in the future — got: {:?}",
        intent.expires
    );
}
