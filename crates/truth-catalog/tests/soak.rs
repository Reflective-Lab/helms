//! Soak tests for truth-catalog mechanism primitives (RFL-172 T7).
//!
//! These tests are marked `#[ignore]` so they only run when explicitly
//! requested with `-- --include-ignored`. They exercise the mechanism
//! at volume to catch latent panics or regressions.

use truth_catalog::resolve::{PackResolver, UnknownModule};
use truth_catalog::{TruthConvergeBinding, TruthDefinition, TruthKind, TruthModuleTouch};

// ---------------------------------------------------------------------------
// Fixture resolver — maps module keys used in FIXTURE_TRUTH
// ---------------------------------------------------------------------------

struct FixtureResolver;

impl PackResolver for FixtureResolver {
    fn pack_ids_for(
        &self,
        modules: &[TruthModuleTouch],
    ) -> Result<Vec<&'static str>, UnknownModule> {
        let mut pack_ids = Vec::new();
        for touch in modules {
            let pack_id = match touch.module_key {
                "identity" => "trust",
                "policies" => "prio-foundation-pack",
                "audit" => "prio-foundation-pack",
                "conversations" => "prio-work-pack",
                "facts" => "prio-work-pack",
                "opportunities" => "prio-commercial-pack",
                "parties" => "prio-relationship-pack",
                "intents" => "knowledge",
                _ => {
                    return Err(UnknownModule {
                        truth_key: String::new(),
                        module_key: touch.module_key.to_owned(),
                    });
                }
            };
            if !pack_ids.contains(&pack_id) {
                pack_ids.push(pack_id);
            }
        }
        Ok(pack_ids)
    }
}

// ---------------------------------------------------------------------------
// Fixture truth definition
// ---------------------------------------------------------------------------

const FIXTURE_TRUTH: TruthDefinition = TruthDefinition {
    key: "approve-access-request",
    display_name: "Approve access request",
    kind: TruthKind::Job,
    summary: "Review and approve or deny an access request.",
    feature_path: "truths/jobs/approve_access_request.feature",
    actor_roles: &["security-operator"],
    approval_points: &["manual approval when risk is elevated"],
    desired_outcomes: &["access decision is recorded"],
    guardrails: &["decision must cite a policy"],
    modules: &[
        TruthModuleTouch {
            module_key: "identity",
            responsibility: "verify requestor identity",
        },
        TruthModuleTouch {
            module_key: "policies",
            responsibility: "apply access policy",
        },
    ],
    gherkin: "",
};

// ---------------------------------------------------------------------------
// Soak: 10 000 cycles
// ---------------------------------------------------------------------------

#[test]
#[ignore = "soak: run with -- --include-ignored"]
fn soak_10k_cycles_build_converge_binding() {
    let resolver = FixtureResolver;
    let start = std::time::Instant::now();
    for _ in 0..10_000 {
        let binding =
            TruthConvergeBinding::build(FIXTURE_TRUTH, &resolver).expect("build must not fail");
        assert_eq!(binding.truth_key, "approve-access-request");
        assert!(!binding.pack_ids.is_empty());
    }
    let elapsed = start.elapsed();
    println!("soak_10k: {:?}", elapsed);
    // Loose guard: 10k builds must complete in under 10 seconds on any dev machine.
    assert!(
        elapsed.as_secs() < 10,
        "10k builds took {:?} — unexpected regression in build() throughput",
        elapsed
    );
}

// ---------------------------------------------------------------------------
// Soak: 100 000 cycles
// ---------------------------------------------------------------------------

#[test]
#[ignore = "soak: run with -- --include-ignored"]
fn soak_100k_cycles_build_converge_binding() {
    let resolver = FixtureResolver;
    let start = std::time::Instant::now();
    for _ in 0..100_000 {
        let binding =
            TruthConvergeBinding::build(FIXTURE_TRUTH, &resolver).expect("build must not fail");
        assert_eq!(binding.truth_key, "approve-access-request");
        assert!(!binding.pack_ids.is_empty());
    }
    let elapsed = start.elapsed();
    println!("soak_100k: {:?}", elapsed);
    // Loose guard: 100k builds must complete in under 60 seconds.
    assert!(
        elapsed.as_secs() < 60,
        "100k builds took {:?} — unexpected regression in build() throughput",
        elapsed
    );
}

// ---------------------------------------------------------------------------
// Soak: TruthKey parse/format roundtrip
// ---------------------------------------------------------------------------

#[test]
#[ignore = "soak: run with -- --include-ignored"]
fn soak_10k_truth_key_parse_roundtrip() {
    use truth_catalog::TruthKey;
    let keys = &[
        "qualify-inbound-lead",
        "score-inbound-fit",
        "plan-outbound-campaign",
        "approve-access-request",
        "revoke-access",
        "identity-record-is-immutable",
        "ledger-entry-is-immutable",
        "active-subscription-requires-plan",
    ];
    let start = std::time::Instant::now();
    for _ in 0..10_000 {
        for &key in keys {
            let parsed = TruthKey::parse(key).expect("all fixture keys are valid");
            assert_eq!(parsed.as_str(), key);
            assert_eq!(parsed.to_string(), key);
        }
    }
    let elapsed = start.elapsed();
    println!("soak_10k_key_roundtrip: {:?}", elapsed);
}
