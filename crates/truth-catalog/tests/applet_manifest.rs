use axiom_truth::{
    APPLET_MANIFEST_VERSION, AppletStatus, ConflictPolicy, EvidenceAuthority,
    applet_manifest_json_schema, parse_applet_manifest_json,
};
use truth_catalog::{find_truth, intent_compile::compile_intent_for_truth};

const ACTIVATE_SUBSCRIPTION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../KB/02-product/applets/activate-subscription.intent.json"
));

const REFILL_PREPAID_AI_CREDITS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../KB/02-product/applets/refill-prepaid-ai-credits.intent.json"
));

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
