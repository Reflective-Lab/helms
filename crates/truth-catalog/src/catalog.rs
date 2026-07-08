//! `TruthCatalog<'a>` — a borrowing view over a slice of [`TruthDefinition`]s.
//!
//! The catalog wraps any `&[TruthDefinition]` and provides typed query methods.
//! The mechanism crate ships with the global [`TRUTHS`] const, but content
//! crates (e.g. `crm-truths`) and test harnesses can construct their own
//! catalog over any slice — including synthetic fixtures — by calling
//! [`TruthCatalog::new`].
//!
//! Temporary free-function wrappers in `lib.rs` delegate to
//! `TruthCatalog::new(&TRUTHS)` until T4 migrates their callers directly to
//! injected catalogs.

use crate::{TruthDefinition, TruthKey, TruthKind};

/// A borrowing view over a slice of [`TruthDefinition`]s.
///
/// Construct with [`TruthCatalog::new`] and inject wherever a truth lookup is
/// needed. The standard global catalog is available as
/// `TruthCatalog::new(truth_catalog::TRUTHS)`.
#[derive(Debug, Clone, Copy)]
pub struct TruthCatalog<'a>(&'a [TruthDefinition]);

impl<'a> TruthCatalog<'a> {
    /// Wrap a truth-definition slice.
    ///
    /// Pass [`crate::TRUTHS`] for the global CRM catalog, or a synthetic slice
    /// in tests.
    pub const fn new(truths: &'a [TruthDefinition]) -> Self {
        Self(truths)
    }

    /// Find a truth by its typed key.
    ///
    /// Returns `None` when no definition in the catalog has a `key` field that
    /// matches `key.as_str()`.
    #[must_use]
    pub fn find(&self, key: &TruthKey) -> Option<&'a TruthDefinition> {
        self.0.iter().find(|t| t.key == key.as_str())
    }

    /// Return every truth definition in the catalog.
    #[must_use]
    pub fn all(&self) -> &'a [TruthDefinition] {
        self.0
    }

    /// Return truths whose `kind` matches the given [`TruthKind`].
    #[must_use]
    pub fn by_kind(&self, kind: TruthKind) -> Vec<&'a TruthDefinition> {
        self.0.iter().filter(|t| t.kind == kind).collect()
    }

    /// Return truths that touch the given module (matched by `module_key`).
    #[must_use]
    pub fn for_module(&self, module_key: &str) -> Vec<&'a TruthDefinition> {
        self.0
            .iter()
            .filter(|t| t.modules.iter().any(|touch| touch.module_key == module_key))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::{TruthDefinition, TruthKey, TruthKind, TruthModuleTouch};

    use super::TruthCatalog;

    // Minimal synthetic fixture — intentionally NOT using the global TRUTHS
    // const so that tests remain independent of CRM content changes.
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
                TruthModuleTouch {
                    module_key: "policies",
                    responsibility: "apply access policy",
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

    fn catalog() -> TruthCatalog<'static> {
        TruthCatalog::new(FIXTURE_TRUTHS)
    }

    #[test]
    fn all_returns_all_definitions() {
        assert_eq!(catalog().all().len(), 3);
    }

    #[test]
    fn find_returns_matching_definition() {
        let key: TruthKey = "revoke-access".parse().unwrap();
        let truth = catalog().find(&key).expect("revoke-access should exist");
        assert_eq!(truth.key, "revoke-access");
        assert_eq!(truth.kind, TruthKind::Job);
    }

    #[test]
    fn find_returns_none_for_unknown_key() {
        let key: TruthKey = "no-such-truth".parse().unwrap();
        assert!(catalog().find(&key).is_none());
    }

    #[test]
    fn by_kind_filters_correctly() {
        let jobs = catalog().by_kind(TruthKind::Job);
        assert_eq!(jobs.len(), 2);
        assert!(jobs.iter().all(|t| t.kind == TruthKind::Job));

        let policies = catalog().by_kind(TruthKind::Policy);
        assert_eq!(policies.len(), 1);
        assert_eq!(policies[0].key, "identity-record-is-immutable");

        let module_local = catalog().by_kind(TruthKind::ModuleLocal);
        assert!(module_local.is_empty());
    }

    #[test]
    fn for_module_returns_touching_truths() {
        let identity_truths = catalog().for_module("identity");
        // All three fixture truths touch the "identity" module.
        assert_eq!(identity_truths.len(), 3);

        let audit_truths = catalog().for_module("audit");
        assert_eq!(audit_truths.len(), 1);
        assert_eq!(audit_truths[0].key, "identity-record-is-immutable");

        let unknown_truths = catalog().for_module("nonexistent-module");
        assert!(unknown_truths.is_empty());
    }

    #[test]
    fn new_is_const_usable() {
        // Verify that const fn new can be used in a const context.
        const CAT: TruthCatalog<'_> = TruthCatalog::new(FIXTURE_TRUTHS);
        assert_eq!(CAT.all().len(), 3);
    }
}
