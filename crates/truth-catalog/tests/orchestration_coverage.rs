//! Fixture-catalog coverage for `prepare_candidates` orchestration invariants.
//!
//! These tests verify the tournament-policy behavior of `prepare_candidates`
//! without depending on real CRM content or a specific formation outcome.

use chrono::{Duration, Utc};
use organism_pack::{IntentPacket, Reversibility};
use truth_catalog::admission::default_helms_capabilities;
use truth_catalog::orchestration::{AutoRunError, AutoRunOptions, ExclusionReason, prepare_candidates};

// ---------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------

fn fixture_intent_reversible() -> IntentPacket {
    let expires = Utc::now() + Duration::hours(1);
    IntentPacket::new("qualify inbound lead", expires)
}

fn fixture_intent_irreversible() -> IntentPacket {
    let expires = Utc::now() + Duration::hours(1);
    IntentPacket::new("permanently delete all records", expires)
        .with_reversibility(Reversibility::Irreversible)
}

// ---------------------------------------------------------------------------
// AutoRunOptions defaults
// ---------------------------------------------------------------------------

#[test]
fn auto_run_options_default_is_single_shot() {
    let opts = AutoRunOptions::default();
    assert!(!opts.race_alternates, "default must be single-shot (race_alternates=false)");
    assert!(opts.max_candidates >= 1, "max_candidates must be at least 1");
    assert!(
        (0.0..=1.0).contains(&opts.relative_cutoff),
        "relative_cutoff must be in [0.0, 1.0]"
    );
}

// ---------------------------------------------------------------------------
// IrreversibleCannotRace guard
// ---------------------------------------------------------------------------

#[test]
fn irreversible_intent_with_race_returns_cannot_race_error() {
    let intent = fixture_intent_irreversible();
    let caps = default_helms_capabilities();
    let opts = AutoRunOptions {
        race_alternates: true,
        ..Default::default()
    };
    let result = prepare_candidates(&intent, &caps, &opts);
    assert!(
        matches!(result, Err(AutoRunError::IrreversibleCannotRace)),
        "expected IrreversibleCannotRace, got: {result:?}"
    );
}

#[test]
fn irreversible_intent_single_shot_does_not_fire_cannot_race() {
    // Single-shot on an irreversible intent is allowed — the guard only
    // applies when racing is requested.
    let intent = fixture_intent_irreversible();
    let caps = default_helms_capabilities();
    let opts = AutoRunOptions {
        race_alternates: false,
        ..Default::default()
    };
    let result = prepare_candidates(&intent, &caps, &opts);
    // Must NOT be IrreversibleCannotRace — any other outcome is acceptable.
    assert!(
        !matches!(result, Err(AutoRunError::IrreversibleCannotRace)),
        "single-shot irreversible must not fire IrreversibleCannotRace"
    );
}

// ---------------------------------------------------------------------------
// Single-shot invariant: all alternates go to excluded with SingleShotRequested
// ---------------------------------------------------------------------------

#[test]
fn single_shot_excludes_all_alternates_with_correct_reason() {
    let intent = fixture_intent_reversible();
    let caps = default_helms_capabilities();
    let opts = AutoRunOptions {
        race_alternates: false,
        ..Default::default()
    };
    match prepare_candidates(&intent, &caps, &opts) {
        Ok(slate) => {
            // Single-shot: alternate_template_ids must be empty.
            assert!(
                slate.alternate_template_ids.is_empty(),
                "single-shot: alternate_template_ids must be empty, got {:?}",
                slate.alternate_template_ids
            );
            // Every exclusion must cite SingleShotRequested.
            for exc in &slate.excluded {
                assert_eq!(
                    exc.reason,
                    ExclusionReason::SingleShotRequested,
                    "all excluded in single-shot must have SingleShotRequested reason"
                );
            }
            // Primary must be non-empty.
            assert!(
                !slate.primary_template_id.is_empty(),
                "primary_template_id must not be empty"
            );
        }
        Err(AutoRunError::Selection(_)) => {
            // No formation matched the fixture intent — acceptable; the
            // IrreversibleCannotRace guard test already proves the
            // happy-path guard behavior.
        }
        Err(e) => panic!("unexpected error from prepare_candidates: {e}"),
    }
}

// ---------------------------------------------------------------------------
// Racing invariant: max_candidates=1 keeps only the primary
// ---------------------------------------------------------------------------

#[test]
fn racing_with_max_candidates_1_keeps_only_primary() {
    let intent = fixture_intent_reversible();
    let caps = default_helms_capabilities();
    let opts = AutoRunOptions {
        race_alternates: true,
        max_candidates: 1,
        relative_cutoff: 0.0, // accept any alternate that gets past the cap
    };
    match prepare_candidates(&intent, &caps, &opts) {
        Ok(slate) => {
            // With max_candidates=1, only the primary fits.
            assert!(
                slate.alternate_template_ids.is_empty(),
                "max_candidates=1: no alternates allowed, got {:?}",
                slate.alternate_template_ids
            );
            // Any alternates the guru returned must be BeyondMaxCandidates.
            for exc in &slate.excluded {
                assert!(
                    matches!(exc.reason, ExclusionReason::BeyondMaxCandidates | ExclusionReason::BelowRelativeCutoff),
                    "unexpected exclusion reason with max_candidates=1: {:?}",
                    exc.reason
                );
            }
        }
        Err(AutoRunError::IrreversibleCannotRace) => {
            panic!("reversible intent must not fire IrreversibleCannotRace");
        }
        Err(AutoRunError::Selection(_)) => {
            // No match in the formation catalog — acceptable.
        }
    }
}

// ---------------------------------------------------------------------------
// ExclusionReason structural completeness
// ---------------------------------------------------------------------------

#[test]
fn exclusion_reason_all_variants_are_copy() {
    // Verify Copy (and PartialEq) work as expected — these are used in
    // assertions throughout orchestration consumers.
    let a = ExclusionReason::SingleShotRequested;
    let b = a;
    assert_eq!(a, b);

    let c = ExclusionReason::BelowRelativeCutoff;
    let d = c;
    assert_eq!(c, d);

    let e = ExclusionReason::BeyondMaxCandidates;
    let f = e;
    assert_eq!(e, f);
}
