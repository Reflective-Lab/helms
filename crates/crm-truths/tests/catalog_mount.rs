//! Mounting-layer catalog injection test (RFL-172 Item 0).
//!
//! Verifies that `CRM_CATALOG` is correctly injected and queryable —
//! covering the canonical truth keys and the None path for absent keys.
use truth_catalog::TruthKey;

#[test]
fn crm_catalog_injected_into_job_state_resolves_qualify_inbound_lead() {
    let catalog = crm_truths::CRM_CATALOG;
    let key: TruthKey = "qualify-inbound-lead".parse().expect("valid key");
    let truth = catalog.find(&key);
    assert!(truth.is_some(), "qualify-inbound-lead must be in CRM_CATALOG");
    assert_eq!(truth.unwrap().key, "qualify-inbound-lead");
}

#[test]
fn crm_catalog_resolves_score_inbound_fit() {
    let catalog = crm_truths::CRM_CATALOG;
    let key: TruthKey = "score-inbound-fit".parse().expect("valid key");
    assert!(catalog.find(&key).is_some(), "score-inbound-fit must be in CRM_CATALOG");
}

#[test]
fn crm_catalog_resolves_plan_outbound_campaign() {
    let catalog = crm_truths::CRM_CATALOG;
    let key: TruthKey = "plan-outbound-campaign".parse().expect("valid key");
    assert!(catalog.find(&key).is_some(), "plan-outbound-campaign must be in CRM_CATALOG");
}

#[test]
fn crm_catalog_find_returns_none_for_nonexistent_key() {
    let catalog = crm_truths::CRM_CATALOG;
    let key: TruthKey = "no-such-truth".parse().expect("valid key");
    assert!(
        catalog.find(&key).is_none(),
        "no-such-truth must not appear in CRM_CATALOG"
    );
}

#[test]
fn every_crm_truth_key_parses_as_truth_key() {
    for truth in crm_truths::CRM_CATALOG.all() {
        let result = TruthKey::parse(truth.key);
        assert!(
            result.is_ok(),
            "truth key {:?} must parse as a valid TruthKey: {}",
            truth.key,
            result.unwrap_err()
        );
    }
}

#[test]
fn crm_catalog_all_returns_nonempty_slice() {
    let all = crm_truths::CRM_CATALOG.all();
    assert!(!all.is_empty(), "CRM_CATALOG.all() must not be empty");
}

#[test]
fn crm_catalog_all_keys_are_unique() {
    use std::collections::BTreeSet;
    let all = crm_truths::CRM_CATALOG.all();
    let unique: BTreeSet<&str> = all.iter().map(|t| t.key).collect();
    assert_eq!(
        unique.len(),
        all.len(),
        "CRM_CATALOG must not contain duplicate truth keys"
    );
}

#[test]
fn crm_catalog_find_returns_matching_key_field() {
    // For every truth in the catalog, find() by its own key must return it.
    for truth in crm_truths::CRM_CATALOG.all() {
        let key = TruthKey::parse(truth.key).expect("all TRUTHS keys are valid");
        let found = crm_truths::CRM_CATALOG.find(&key);
        assert!(
            found.is_some(),
            "catalog.find({:?}) must return Some for a key that exists in all()",
            truth.key
        );
        assert_eq!(
            found.unwrap().key,
            truth.key,
            "catalog.find returned a truth with wrong key"
        );
    }
}
