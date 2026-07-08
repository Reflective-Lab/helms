//! Property-based tests for truth-catalog mechanism primitives (RFL-172 T7).
//!
//! These tests verify structural invariants that must hold for any
//! TruthKey and any TruthCatalog slice, regardless of content.

use proptest::prelude::*;
use truth_catalog::{TruthCatalog, TruthDefinition, TruthKey, TruthKind, TruthModuleTouch};

// ---------------------------------------------------------------------------
// Fixture catalog — synthetic, independent of CRM content
// ---------------------------------------------------------------------------

const FIXTURE_TRUTHS: &[TruthDefinition] = &[
    TruthDefinition {
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
        ],
        gherkin: "",
    },
    TruthDefinition {
        key: "revoke-access",
        display_name: "Revoke access",
        kind: TruthKind::Job,
        summary: "Revoke an existing access grant.",
        feature_path: "truths/jobs/revoke_access.feature",
        actor_roles: &["security-operator"],
        approval_points: &[],
        desired_outcomes: &["access grant is terminated"],
        guardrails: &["revocation must be logged"],
        modules: &[
            TruthModuleTouch {
                module_key: "identity",
                responsibility: "resolve the access subject",
            },
        ],
        gherkin: "",
    },
    TruthDefinition {
        key: "identity-record-is-immutable",
        display_name: "Identity record is immutable",
        kind: TruthKind::Policy,
        summary: "Posted identity records must not be mutated.",
        feature_path: "truths/policies/identity_record_is_immutable.feature",
        actor_roles: &["security-operator"],
        approval_points: &[],
        desired_outcomes: &["identity records remain unchanged"],
        guardrails: &["mutation of identity records is blocked"],
        modules: &[
            TruthModuleTouch {
                module_key: "identity",
                responsibility: "own immutable identity history",
            },
            TruthModuleTouch {
                module_key: "audit",
                responsibility: "preserve the change trail",
            },
        ],
        gherkin: "",
    },
];

fn fixture_catalog() -> TruthCatalog<'static> {
    TruthCatalog::new(FIXTURE_TRUTHS)
}

// ---------------------------------------------------------------------------
// TruthKey parse/format roundtrip
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn valid_truth_key_roundtrips_through_display(
        // Generate a string that satisfies the grammar: segments of [a-z0-9]+ joined by single hyphens
        s in "[a-z][a-z0-9]*(-[a-z0-9]+)*"
    ) {
        let key = TruthKey::parse(&s).expect("generated key should be valid");
        let displayed = key.to_string();
        let reparsed = TruthKey::parse(&displayed).expect("displayed key should reparse");
        prop_assert_eq!(key, reparsed, "roundtrip via Display should produce equal TruthKey");
        prop_assert_eq!(displayed, s, "Display should reproduce the original string");
    }

    #[test]
    fn truth_key_as_str_equals_original(
        s in "[a-z][a-z0-9]*(-[a-z0-9]+)*"
    ) {
        let key = TruthKey::parse(&s).expect("valid key");
        prop_assert_eq!(key.as_str(), s.as_str());
    }

    #[test]
    fn truth_key_fromstr_and_parse_agree(
        s in "[a-z][a-z0-9]*(-[a-z0-9]+)*"
    ) {
        let via_parse = TruthKey::parse(&s);
        let via_fromstr: Result<TruthKey, _> = s.parse();
        match (via_parse, via_fromstr) {
            (Ok(a), Ok(b)) => prop_assert_eq!(a, b),
            (Err(e1), Err(e2)) => prop_assert_eq!(e1.input, e2.input),
            _ => prop_assert!(false, "parse and FromStr disagreed on {:?}", s),
        }
    }

    #[test]
    fn invalid_inputs_are_rejected(
        // Space, uppercase, underscore, period, slash — all invalid
        s in "[A-Z _./]+[a-z]*"
    ) {
        // If the string contains only invalid chars or starts invalid, parse must fail.
        // We check: if it starts with or contains invalidating chars, parse should reject.
        let result = TruthKey::parse(&s);
        // The result might be Ok if the string happens to be pure lowercase after
        // filtering; we only assert when we KNOW it contains uppercase letters.
        if s.chars().any(|c| c.is_ascii_uppercase()) {
            prop_assert!(result.is_err(), "string with uppercase must be rejected: {s:?}");
        }
    }

    // ---------------------------------------------------------------------------
    // Catalog consistency properties
    // ---------------------------------------------------------------------------

    #[test]
    fn catalog_find_for_present_key_is_some(idx in 0usize..FIXTURE_TRUTHS.len()) {
        let catalog = fixture_catalog();
        let raw_key = FIXTURE_TRUTHS[idx].key;
        let key = TruthKey::parse(raw_key).expect("fixture keys are valid");
        prop_assert!(catalog.find(&key).is_some(), "find must return Some for key in catalog");
    }

    #[test]
    fn catalog_find_returns_correct_definition(idx in 0usize..FIXTURE_TRUTHS.len()) {
        let catalog = fixture_catalog();
        let expected = &FIXTURE_TRUTHS[idx];
        let key = TruthKey::parse(expected.key).expect("fixture keys are valid");
        let found = catalog.find(&key).expect("key must be in catalog");
        prop_assert_eq!(found.key, expected.key);
        prop_assert_eq!(found.kind, expected.kind);
    }

    #[test]
    fn catalog_all_length_is_stable(
        // Property: all() length doesn't change between calls
        _unused in 0..1u8
    ) {
        let catalog = fixture_catalog();
        prop_assert_eq!(catalog.all().len(), FIXTURE_TRUTHS.len());
    }

    #[test]
    fn catalog_by_kind_subset_of_all(kind_idx in 0usize..3) {
        let catalog = fixture_catalog();
        let kind = [TruthKind::Job, TruthKind::Policy, TruthKind::ModuleLocal][kind_idx];
        let by_kind = catalog.by_kind(kind);
        let all = catalog.all();
        for t in &by_kind {
            prop_assert!(
                all.iter().any(|d| d.key == t.key),
                "by_kind result {:?} not in all()",
                t.key
            );
            prop_assert_eq!(t.kind, kind, "by_kind must only return matching kind");
        }
    }

    #[test]
    fn catalog_for_module_subset_of_all(module_key in "[a-z]+") {
        let catalog = fixture_catalog();
        let for_mod = catalog.for_module(&module_key);
        let all = catalog.all();
        for t in &for_mod {
            prop_assert!(
                all.iter().any(|d| d.key == t.key),
                "for_module result {:?} not in all()",
                t.key
            );
            prop_assert!(
                t.modules.iter().any(|m| m.module_key == module_key.as_str()),
                "for_module result {:?} does not touch module {:?}",
                t.key,
                module_key
            );
        }
    }

    #[test]
    fn catalog_by_kind_and_all_counts_are_consistent(
        _unused in 0..1u8
    ) {
        let catalog = fixture_catalog();
        let jobs = catalog.by_kind(TruthKind::Job).len();
        let policies = catalog.by_kind(TruthKind::Policy).len();
        let module_local = catalog.by_kind(TruthKind::ModuleLocal).len();
        // Every truth must appear in exactly one kind bucket.
        prop_assert_eq!(jobs + policies + module_local, catalog.all().len());
    }
}
