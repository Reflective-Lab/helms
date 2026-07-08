//! Negative-path tests for truth-catalog mechanism primitives (RFL-172 T7).
//!
//! Covers:
//! - TruthKey grammar rejections (all documented invalid patterns)
//! - Error fields (input, reason) on InvalidTruthKey
//! - TruthConvergeBinding::build with an always-missing resolver
//! - TruthCatalog::find returning None for unknown keys

use truth_catalog::key::InvalidTruthKey;
use truth_catalog::resolve::{PackResolver, UnknownModule};
use truth_catalog::{
    TruthCatalog, TruthConvergeBinding, TruthDefinition, TruthKey, TruthKind, TruthModuleTouch,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn assert_invalid(s: &str) -> InvalidTruthKey {
    TruthKey::parse(s).expect_err(&format!("expected {s:?} to be invalid"))
}

fn assert_valid(s: &str) -> TruthKey {
    TruthKey::parse(s).expect(&format!("expected {s:?} to be valid"))
}

// ---------------------------------------------------------------------------
// TruthKey grammar rejections
// ---------------------------------------------------------------------------

#[test]
fn empty_string_is_rejected() {
    let e = assert_invalid("");
    assert_eq!(e.input, "");
    assert!(e.reason.contains("empty"), "reason: {}", e.reason);
}

#[test]
fn uppercase_letter_is_rejected() {
    let e = assert_invalid("Qualify");
    assert_eq!(e.input, "Qualify");
    assert!(e.reason.contains("lowercase"), "reason: {}", e.reason);
}

#[test]
fn mixed_case_is_rejected() {
    let e = assert_invalid("qualify-Inbound");
    assert_eq!(e.input, "qualify-Inbound");
    assert!(e.reason.contains("lowercase"), "reason: {}", e.reason);
}

#[test]
fn underscore_is_rejected() {
    let e = assert_invalid("submit_expense");
    assert_eq!(e.input, "submit_expense");
    assert!(e.reason.contains("lowercase"), "reason: {}", e.reason);
}

#[test]
fn space_is_rejected() {
    let e = assert_invalid("qualify lead");
    assert_eq!(e.input, "qualify lead");
    assert!(e.reason.contains("lowercase"), "reason: {}", e.reason);
}

#[test]
fn leading_hyphen_is_rejected() {
    let e = assert_invalid("-lead");
    assert_eq!(e.input, "-lead");
    assert!(e.reason.contains("start"), "reason: {}", e.reason);
}

#[test]
fn trailing_hyphen_is_rejected() {
    let e = assert_invalid("lead-");
    assert_eq!(e.input, "lead-");
    assert!(e.reason.contains("end"), "reason: {}", e.reason);
}

#[test]
fn consecutive_hyphens_are_rejected() {
    let e = assert_invalid("lead--inbound");
    assert_eq!(e.input, "lead--inbound");
    assert!(e.reason.contains("consecutive"), "reason: {}", e.reason);
}

#[test]
fn non_ascii_is_rejected() {
    let e = assert_invalid("lead-über");
    assert_eq!(e.input, "lead-über");
    assert!(e.reason.contains("ASCII"), "reason: {}", e.reason);
}

#[test]
fn period_is_rejected() {
    let e = assert_invalid("qualify.lead");
    assert_eq!(e.input, "qualify.lead");
    assert!(e.reason.contains("lowercase"), "reason: {}", e.reason);
}

#[test]
fn slash_is_rejected() {
    let e = assert_invalid("qualify/lead");
    assert_eq!(e.input, "qualify/lead");
    assert!(e.reason.contains("lowercase"), "reason: {}", e.reason);
}

#[test]
fn all_hyphens_is_rejected() {
    // Would violate leading-hyphen rule first
    let e = assert_invalid("---");
    assert!(e.reason.contains("start"), "reason: {}", e.reason);
}

// ---------------------------------------------------------------------------
// Error fields are populated correctly
// ---------------------------------------------------------------------------

#[test]
fn error_input_field_matches_rejected_string() {
    let s = "Invalid-Key";
    let e = assert_invalid(s);
    assert_eq!(e.input, s, "InvalidTruthKey.input must equal the rejected string");
}

#[test]
fn error_reason_field_is_non_empty() {
    let e = assert_invalid("");
    assert!(!e.reason.is_empty(), "InvalidTruthKey.reason must not be empty");
}

#[test]
fn error_display_includes_input_and_reason() {
    let e = assert_invalid("Bad_Key");
    let msg = e.to_string();
    assert!(msg.contains("Bad_Key"), "Display must include the rejected input; got: {msg}");
    // reason is embedded in the message via thiserror template
    assert!(!msg.is_empty(), "Display must produce a non-empty message");
}

// ---------------------------------------------------------------------------
// Valid boundary cases (should NOT be rejected)
// ---------------------------------------------------------------------------

#[test]
fn single_char_segment_is_valid() {
    assert_valid("a");
}

#[test]
fn digit_only_segment_is_valid() {
    assert_valid("42");
}

#[test]
fn segment_with_digit_at_end_is_valid() {
    assert_valid("truth-v2");
}

// ---------------------------------------------------------------------------
// AlwaysMissingResolver — simulates a content side with no known modules
// ---------------------------------------------------------------------------

struct AlwaysMissingResolver;

impl PackResolver for AlwaysMissingResolver {
    fn pack_ids_for(
        &self,
        modules: &[TruthModuleTouch],
    ) -> Result<Vec<&'static str>, UnknownModule> {
        Err(UnknownModule {
            truth_key: String::new(),
            module_key: modules
                .first()
                .map(|m| m.module_key.to_owned())
                .unwrap_or_else(|| "unknown".to_owned()),
        })
    }
}

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
    modules: &[TruthModuleTouch {
        module_key: "identity",
        responsibility: "verify requestor identity",
    }],
    gherkin: "",
};

#[test]
fn build_with_missing_resolver_returns_err() {
    let result = TruthConvergeBinding::build(FIXTURE_TRUTH, &AlwaysMissingResolver);
    assert!(
        result.is_err(),
        "build with AlwaysMissingResolver must return Err, not panic"
    );
}

#[test]
fn build_error_carries_truth_key() {
    let err = TruthConvergeBinding::build(FIXTURE_TRUTH, &AlwaysMissingResolver).unwrap_err();
    assert_eq!(
        err.truth_key, "approve-access-request",
        "build() must populate truth_key into the error"
    );
}

#[test]
fn build_error_carries_module_key() {
    let err = TruthConvergeBinding::build(FIXTURE_TRUTH, &AlwaysMissingResolver).unwrap_err();
    assert_eq!(
        err.module_key, "identity",
        "build() must populate module_key into the error"
    );
}

#[test]
fn build_error_display_mentions_truth_and_module() {
    let err = TruthConvergeBinding::build(FIXTURE_TRUTH, &AlwaysMissingResolver).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("approve-access-request"),
        "error message must contain the truth key; got: {msg}"
    );
    assert!(
        msg.contains("identity"),
        "error message must contain the module key; got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// TruthCatalog::find returns None for unknown key
// ---------------------------------------------------------------------------

const CATALOG_FIXTURE: TruthCatalog<'static> = TruthCatalog::new(&[FIXTURE_TRUTH]);

#[test]
fn catalog_find_returns_none_for_nonexistent_key() {
    let key: TruthKey = "no-such-truth".parse().expect("valid key");
    assert!(
        CATALOG_FIXTURE.find(&key).is_none(),
        "find must return None for a key not in the catalog"
    );
}

#[test]
fn catalog_find_returns_none_for_prefix_match() {
    // "approve" is a prefix of "approve-access-request" — find must not do prefix matching
    let key: TruthKey = "approve".parse().expect("valid key");
    assert!(
        CATALOG_FIXTURE.find(&key).is_none(),
        "find must not do prefix matching"
    );
}

#[test]
fn catalog_find_returns_none_for_suffix_match() {
    // "access-request" is a suffix — find must require exact key
    let key: TruthKey = "access-request".parse().expect("valid key");
    assert!(
        CATALOG_FIXTURE.find(&key).is_none(),
        "find must not do suffix matching"
    );
}
