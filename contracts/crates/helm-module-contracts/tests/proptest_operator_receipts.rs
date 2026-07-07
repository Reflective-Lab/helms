//! Property-based tests for the `operator_receipts` vocabulary (RFL-154 T7).
//!
//! Properties under test:
//! - `JobReadinessPacket::new` is deterministic: same input ⇒ identical `packet_id`.
//! - Any single-field mutation of the input changes `packet_id`.
//! - `job_readiness_packet_payload_hash` always yields a `sha256:`-prefixed
//!   64-hex digest.
//! - `OperatorLedgerEntry::new` always yields `AuthorityEffect::None`
//!   (the non-authority invariant).
//! - Serde round-trips are lossless for packets, ledger entries, and every
//!   vocabulary enum.
//!
//! Run: `cargo test -p helm-module-contracts`

use proptest::prelude::*;

use helm_module_contracts::operator_receipts::{
    AdapterReceiptStatus, AuthorityEffect, EvidenceReadinessStatus, FuzzyDefuzzifiedScore,
    FuzzyMembership, FuzzyReadinessTrace, FuzzyRuleActivation, JobEvidenceStatus,
    JobReadinessPacket, JobReadinessPacketInput, JobVerdict, OperatorLedgerEntry,
    OperatorLedgerEntryInput, OperatorLedgerRecordKind, ReceiptFamily,
    job_readiness_packet_ledger_entry, job_readiness_packet_payload_hash,
};

// ── Strategies ────────────────────────────────────────────────────────────────

fn arb_nonempty_str() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9\\-\\.]{0,19}"
}

fn arb_adapter_status() -> impl Strategy<Value = AdapterReceiptStatus> {
    prop_oneof![
        Just(AdapterReceiptStatus::Succeeded),
        Just(AdapterReceiptStatus::Rejected),
    ]
}

fn arb_verdict() -> impl Strategy<Value = Option<JobVerdict>> {
    prop_oneof![
        Just(None),
        Just(Some(JobVerdict::Satisfied)),
        Just(Some(JobVerdict::Blocked)),
        Just(Some(JobVerdict::Exhausted)),
        Just(Some(JobVerdict::Invalid)),
    ]
}

fn arb_evidence_readiness() -> impl Strategy<Value = EvidenceReadinessStatus> {
    prop_oneof![
        Just(EvidenceReadinessStatus::Present),
        Just(EvidenceReadinessStatus::Missing),
        Just(EvidenceReadinessStatus::Disputed),
        Just(EvidenceReadinessStatus::Blocked),
        Just(EvidenceReadinessStatus::Concern),
    ]
}

fn arb_evidence_item() -> impl Strategy<Value = JobEvidenceStatus> {
    (
        arb_nonempty_str(),
        arb_nonempty_str(),
        arb_nonempty_str(),
        arb_evidence_readiness(),
        prop::collection::vec(arb_nonempty_str(), 0..3),
    )
        .prop_map(
            |(clause_id, clause_key, label, status, fact_ids)| JobEvidenceStatus {
                clause_id,
                clause_key,
                label,
                status,
                fact_ids,
                evidence_refs: Vec::new(),
                trace_links: Vec::new(),
                concern_record_ids: Vec::new(),
            },
        )
}

fn arb_valid_basis_points() -> impl Strategy<Value = u16> {
    0u16..=10_000u16
}

fn arb_fuzzy_membership() -> impl Strategy<Value = FuzzyMembership> {
    (arb_nonempty_str(), arb_valid_basis_points()).prop_map(|(label, score_basis_points)| {
        FuzzyMembership {
            label,
            score_basis_points,
        }
    })
}

fn arb_fuzzy_rule() -> impl Strategy<Value = FuzzyRuleActivation> {
    (
        arb_nonempty_str(),
        arb_valid_basis_points(),
        arb_nonempty_str(),
    )
        .prop_map(
            |(rule_id, strength_basis_points, conclusion)| FuzzyRuleActivation {
                rule_id,
                strength_basis_points,
                conclusion,
            },
        )
}

fn arb_defuzzified_score() -> impl Strategy<Value = FuzzyDefuzzifiedScore> {
    (arb_nonempty_str(), arb_valid_basis_points(), 1u32..10_000u32).prop_map(
        |(method, score_basis_points, domain_steps)| FuzzyDefuzzifiedScore {
            method,
            score_basis_points,
            domain_min_basis_points: 0,
            domain_max_basis_points: 10_000,
            domain_steps,
        },
    )
}

fn arb_fuzzy_trace() -> impl Strategy<Value = Option<FuzzyReadinessTrace>> {
    prop_oneof![
        Just(None),
        (
            arb_nonempty_str(),
            arb_valid_basis_points(),
            prop::collection::vec(arb_fuzzy_membership(), 1..4),
            prop::collection::vec(arb_fuzzy_rule(), 0..3),
            prop::option::of(arb_defuzzified_score()),
        )
            .prop_map(
                |(variable_key, observed, memberships, activated_rules, defuzzified_score)| {
                    Some(FuzzyReadinessTrace {
                        variable_key,
                        observed_value_basis_points: observed,
                        memberships,
                        activated_rules,
                        defuzzified_score,
                    })
                }
            ),
    ]
}

fn arb_valid_packet_input() -> impl Strategy<Value = JobReadinessPacketInput> {
    (
        (
            arb_nonempty_str(), // package_id
            arb_nonempty_str(), // truth_version
            arb_nonempty_str(), // domain_hint
            arb_nonempty_str(), // job_key
            arb_nonempty_str(), // subject_ref
            arb_nonempty_str(), // adapter_receipt_id
        ),
        arb_adapter_status(),
        arb_verdict(),
        prop::collection::vec(arb_evidence_item(), 0..4),
        arb_fuzzy_trace(),
        prop::collection::vec(arb_nonempty_str(), 0..3), // verifier_forbidden_actions
        prop::collection::vec(arb_nonempty_str(), 0..3), // operator_actions
    )
        .prop_map(
            |(
                (package_id, truth_version, domain_hint, job_key, subject_ref, adapter_receipt_id),
                adapter_status,
                verdict,
                evidence_status,
                fuzzy_trace,
                verifier_forbidden_actions,
                operator_actions,
            )| {
                JobReadinessPacketInput {
                    package_id,
                    truth_version,
                    domain_hint,
                    job_key,
                    subject_ref,
                    adapter_receipt_id,
                    adapter_status,
                    verdict,
                    authorizes_domain_action: false,
                    evidence_status,
                    fuzzy_trace,
                    verifier_forbidden_actions,
                    operator_actions,
                }
            },
        )
}

fn arb_record_kind() -> impl Strategy<Value = OperatorLedgerRecordKind> {
    prop_oneof![
        Just(OperatorLedgerRecordKind::ObservationAdapterReceipt),
        Just(OperatorLedgerRecordKind::JobReadinessPacket),
        Just(OperatorLedgerRecordKind::OperatorDecisionReceipt),
        Just(OperatorLedgerRecordKind::ApprovalReceipt),
        Just(OperatorLedgerRecordKind::PlanReceipt),
        Just(OperatorLedgerRecordKind::ExecutionReceipt),
        Just(OperatorLedgerRecordKind::ActionReceipt),
        Just(OperatorLedgerRecordKind::OutcomeReceipt),
        Just(OperatorLedgerRecordKind::CorpusSnapshotReceipt),
        Just(OperatorLedgerRecordKind::EvidenceWindowReceipt),
        Just(OperatorLedgerRecordKind::DisagreementReceipt),
        Just(OperatorLedgerRecordKind::AnalystReviewReceipt),
        Just(OperatorLedgerRecordKind::NarrativeClaimReceipt),
        Just(OperatorLedgerRecordKind::CanonicalStoryReceipt),
        Just(OperatorLedgerRecordKind::ClaimReviewReceipt),
        Just(OperatorLedgerRecordKind::EditorialApprovalReceipt),
        Just(OperatorLedgerRecordKind::PublicationBoundaryReceipt),
        Just(OperatorLedgerRecordKind::AppLocalReceipt),
    ]
}

fn arb_receipt_family() -> impl Strategy<Value = ReceiptFamily> {
    prop_oneof![
        Just(ReceiptFamily::Common),
        Just(ReceiptFamily::LongRunningJob),
        Just(ReceiptFamily::TemporalEvidence),
        Just(ReceiptFamily::ContentPublication),
        Just(ReceiptFamily::AppLocal),
    ]
}

const VALID_SHA256: &str =
    "sha256:90b8fb64fdd6f926a4ef42d67a145215aa7e7e07480863217f8558c472da579f";

fn arb_ledger_entry_input() -> impl Strategy<Value = OperatorLedgerEntryInput> {
    (
        0u64..1_000_000u64,
        arb_record_kind(),
        arb_receipt_family(),
        arb_nonempty_str(), // source_ref
        arb_nonempty_str(), // package_id
        arb_nonempty_str(), // truth_version
        arb_nonempty_str(), // domain_hint
        prop::collection::vec(arb_nonempty_str(), 0..3), // backlink_ids
        arb_nonempty_str(), // summary
    )
        .prop_map(
            |(
                sequence,
                record_kind,
                receipt_family,
                source_ref,
                package_id,
                truth_version,
                domain_hint,
                backlink_ids,
                summary,
            )| {
                OperatorLedgerEntryInput {
                    sequence,
                    record_kind,
                    receipt_family,
                    source_ref,
                    package_id,
                    truth_version,
                    domain_hint,
                    payload_hash: VALID_SHA256.to_string(),
                    backlink_ids,
                    summary,
                }
            },
        )
}

// ── Determinism ───────────────────────────────────────────────────────────────

proptest! {
    /// Same input ⇒ identical packet (including `packet_id`).
    #[test]
    fn packet_id_is_deterministic(input in arb_valid_packet_input()) {
        let first = JobReadinessPacket::new(input.clone()).expect("packet builds");
        let second = JobReadinessPacket::new(input).expect("packet builds again");
        prop_assert_eq!(&first.packet_id, &second.packet_id);
        prop_assert_eq!(first, second);
    }

    /// Same input ⇒ identical ledger entry (including `entry_id`).
    #[test]
    fn ledger_entry_id_is_deterministic(input in arb_ledger_entry_input()) {
        let first = OperatorLedgerEntry::new(input.clone()).expect("entry builds");
        let second = OperatorLedgerEntry::new(input).expect("entry builds again");
        prop_assert_eq!(&first.entry_id, &second.entry_id);
        prop_assert_eq!(first, second);
    }
}

// ── Single-field mutation sensitivity ─────────────────────────────────────────

proptest! {
    #[test]
    fn packet_id_changes_on_package_id_mutation(input in arb_valid_packet_input()) {
        let original = JobReadinessPacket::new(input.clone()).expect("packet builds");
        let mut mutated = input;
        mutated.package_id.push_str("-x");
        let changed = JobReadinessPacket::new(mutated).expect("mutated packet builds");
        prop_assert_ne!(original.packet_id, changed.packet_id);
    }

    #[test]
    fn packet_id_changes_on_truth_version_mutation(input in arb_valid_packet_input()) {
        let original = JobReadinessPacket::new(input.clone()).expect("packet builds");
        let mut mutated = input;
        mutated.truth_version.push_str("-x");
        let changed = JobReadinessPacket::new(mutated).expect("mutated packet builds");
        prop_assert_ne!(original.packet_id, changed.packet_id);
    }

    #[test]
    fn packet_id_changes_on_domain_hint_mutation(input in arb_valid_packet_input()) {
        let original = JobReadinessPacket::new(input.clone()).expect("packet builds");
        let mut mutated = input;
        mutated.domain_hint.push_str("-x");
        let changed = JobReadinessPacket::new(mutated).expect("mutated packet builds");
        prop_assert_ne!(original.packet_id, changed.packet_id);
    }

    #[test]
    fn packet_id_changes_on_job_key_mutation(input in arb_valid_packet_input()) {
        let original = JobReadinessPacket::new(input.clone()).expect("packet builds");
        let mut mutated = input;
        mutated.job_key.push_str("-x");
        let changed = JobReadinessPacket::new(mutated).expect("mutated packet builds");
        prop_assert_ne!(original.packet_id, changed.packet_id);
    }

    #[test]
    fn packet_id_changes_on_subject_ref_mutation(input in arb_valid_packet_input()) {
        let original = JobReadinessPacket::new(input.clone()).expect("packet builds");
        let mut mutated = input;
        mutated.subject_ref.push_str("-x");
        let changed = JobReadinessPacket::new(mutated).expect("mutated packet builds");
        prop_assert_ne!(original.packet_id, changed.packet_id);
    }

    #[test]
    fn packet_id_changes_on_adapter_receipt_id_mutation(input in arb_valid_packet_input()) {
        let original = JobReadinessPacket::new(input.clone()).expect("packet builds");
        let mut mutated = input;
        mutated.adapter_receipt_id.push_str("-x");
        let changed = JobReadinessPacket::new(mutated).expect("mutated packet builds");
        prop_assert_ne!(original.packet_id, changed.packet_id);
    }

    #[test]
    fn packet_id_changes_on_adapter_status_mutation(input in arb_valid_packet_input()) {
        let original = JobReadinessPacket::new(input.clone()).expect("packet builds");
        let mut mutated = input;
        mutated.adapter_status = match mutated.adapter_status {
            AdapterReceiptStatus::Succeeded => AdapterReceiptStatus::Rejected,
            AdapterReceiptStatus::Rejected => AdapterReceiptStatus::Succeeded,
        };
        let changed = JobReadinessPacket::new(mutated).expect("mutated packet builds");
        prop_assert_ne!(original.packet_id, changed.packet_id);
    }

    #[test]
    fn packet_id_changes_on_verdict_mutation(input in arb_valid_packet_input()) {
        let original = JobReadinessPacket::new(input.clone()).expect("packet builds");
        let mut mutated = input;
        mutated.verdict = match mutated.verdict {
            None => Some(JobVerdict::Satisfied),
            Some(JobVerdict::Satisfied) => Some(JobVerdict::Blocked),
            Some(_) => None,
        };
        let changed = JobReadinessPacket::new(mutated).expect("mutated packet builds");
        prop_assert_ne!(original.packet_id, changed.packet_id);
    }

    #[test]
    fn packet_id_changes_on_evidence_mutation(input in arb_valid_packet_input()) {
        let original = JobReadinessPacket::new(input.clone()).expect("packet builds");
        let mut mutated = input;
        mutated.evidence_status.push(JobEvidenceStatus {
            clause_id: "mutation.clause".to_string(),
            clause_key: "mutation-added".to_string(),
            label: "mutation sentinel".to_string(),
            status: EvidenceReadinessStatus::Concern,
            fact_ids: Vec::new(),
            evidence_refs: Vec::new(),
            trace_links: Vec::new(),
            concern_record_ids: Vec::new(),
        });
        let changed = JobReadinessPacket::new(mutated).expect("mutated packet builds");
        prop_assert_ne!(original.packet_id, changed.packet_id);
    }

    #[test]
    fn packet_id_changes_on_operator_actions_mutation(input in arb_valid_packet_input()) {
        let original = JobReadinessPacket::new(input.clone()).expect("packet builds");
        let mut mutated = input;
        mutated.operator_actions.push("mutation-action".to_string());
        let changed = JobReadinessPacket::new(mutated).expect("mutated packet builds");
        prop_assert_ne!(original.packet_id, changed.packet_id);
    }

    #[test]
    fn packet_id_changes_on_forbidden_actions_mutation(input in arb_valid_packet_input()) {
        let original = JobReadinessPacket::new(input.clone()).expect("packet builds");
        let mut mutated = input;
        mutated
            .verifier_forbidden_actions
            .push("mutation-forbidden".to_string());
        let changed = JobReadinessPacket::new(mutated).expect("mutated packet builds");
        prop_assert_ne!(original.packet_id, changed.packet_id);
    }

    #[test]
    fn ledger_entry_id_changes_on_sequence_mutation(input in arb_ledger_entry_input()) {
        let original = OperatorLedgerEntry::new(input.clone()).expect("entry builds");
        let mut mutated = input;
        mutated.sequence = mutated.sequence.wrapping_add(1);
        let changed = OperatorLedgerEntry::new(mutated).expect("mutated entry builds");
        prop_assert_ne!(original.entry_id, changed.entry_id);
    }

    #[test]
    fn ledger_entry_id_changes_on_backlink_mutation(input in arb_ledger_entry_input()) {
        let original = OperatorLedgerEntry::new(input.clone()).expect("entry builds");
        let mut mutated = input;
        mutated.backlink_ids.push("mutation.backlink".to_string());
        let changed = OperatorLedgerEntry::new(mutated).expect("mutated entry builds");
        prop_assert_ne!(original.entry_id, changed.entry_id);
    }
}

// ── Hash format ───────────────────────────────────────────────────────────────

proptest! {
    /// `job_readiness_packet_payload_hash` always yields `sha256:` + 64 hex chars.
    #[test]
    fn payload_hash_is_always_sha256_prefixed_64_hex(input in arb_valid_packet_input()) {
        let packet = JobReadinessPacket::new(input).expect("packet builds");
        let hash = job_readiness_packet_payload_hash(&packet);
        let digest = hash
            .strip_prefix("sha256:")
            .expect("hash must start with sha256:");
        prop_assert_eq!(digest.len(), 64);
        prop_assert!(digest.bytes().all(|b| b.is_ascii_hexdigit()));
    }
}

// ── Non-authority invariant ───────────────────────────────────────────────────

proptest! {
    /// `OperatorLedgerEntry::new` always yields `AuthorityEffect::None`,
    /// whatever the input.
    #[test]
    fn ledger_entry_authority_effect_is_always_none(input in arb_ledger_entry_input()) {
        let entry = OperatorLedgerEntry::new(input).expect("entry builds");
        prop_assert_eq!(entry.authority_effect, AuthorityEffect::None);
    }

    /// The convenience constructor inherits the invariant.
    #[test]
    fn packet_ledger_entry_helper_is_always_non_authoritative(
        input in arb_valid_packet_input(),
        sequence in 0u64..1_000u64,
    ) {
        let packet = JobReadinessPacket::new(input).expect("packet builds");
        let entry = job_readiness_packet_ledger_entry(
            sequence,
            &packet,
            vec!["proptest.backlink".to_string()],
            "proptest summary",
        )
        .expect("entry builds");
        prop_assert_eq!(entry.authority_effect, AuthorityEffect::None);
        prop_assert_eq!(&entry.payload_hash, &job_readiness_packet_payload_hash(&packet));
        prop_assert_eq!(&entry.source_ref, &packet.packet_id);
    }
}

// ── Serde round-trips ─────────────────────────────────────────────────────────

proptest! {
    #[test]
    fn job_readiness_packet_serde_roundtrip(input in arb_valid_packet_input()) {
        let packet = JobReadinessPacket::new(input).expect("packet builds");
        let json = serde_json::to_string(&packet).expect("serialize");
        let roundtripped: JobReadinessPacket = serde_json::from_str(&json).expect("deserialize");
        prop_assert_eq!(packet, roundtripped);
    }

    #[test]
    fn operator_ledger_entry_serde_roundtrip(input in arb_ledger_entry_input()) {
        let entry = OperatorLedgerEntry::new(input).expect("entry builds");
        let json = serde_json::to_string(&entry).expect("serialize");
        let roundtripped: OperatorLedgerEntry = serde_json::from_str(&json).expect("deserialize");
        prop_assert_eq!(entry, roundtripped);
    }

    #[test]
    fn packet_input_serde_roundtrip(input in arb_valid_packet_input()) {
        let json = serde_json::to_string(&input).expect("serialize");
        let roundtripped: JobReadinessPacketInput =
            serde_json::from_str(&json).expect("deserialize");
        prop_assert_eq!(input, roundtripped);
    }

    #[test]
    fn ledger_entry_input_serde_roundtrip(input in arb_ledger_entry_input()) {
        let json = serde_json::to_string(&input).expect("serialize");
        let roundtripped: OperatorLedgerEntryInput =
            serde_json::from_str(&json).expect("deserialize");
        prop_assert_eq!(input, roundtripped);
    }

    #[test]
    fn record_kind_serde_roundtrip(kind in arb_record_kind()) {
        let json = serde_json::to_string(&kind).expect("serialize");
        let roundtripped: OperatorLedgerRecordKind =
            serde_json::from_str(&json).expect("deserialize");
        prop_assert_eq!(kind, roundtripped);
        // Wire string equals the canonical `as_str` value.
        prop_assert_eq!(json, format!("\"{}\"", kind.as_str()));
    }

    #[test]
    fn receipt_family_serde_roundtrip(family in arb_receipt_family()) {
        let json = serde_json::to_string(&family).expect("serialize");
        let roundtripped: ReceiptFamily = serde_json::from_str(&json).expect("deserialize");
        prop_assert_eq!(family, roundtripped);
        prop_assert_eq!(json, format!("\"{}\"", family.as_str()));
    }
}

/// Serde round-trip for the remaining vocabulary enums (small closed sets;
/// exhaustive loops instead of proptest).
#[test]
fn remaining_vocab_enums_serde_roundtrip_exhaustively() {
    for v in [AdapterReceiptStatus::Succeeded, AdapterReceiptStatus::Rejected] {
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, format!("\"{}\"", v.as_str()));
        let rt: AdapterReceiptStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(v, rt);
    }
    for v in [
        JobVerdict::Satisfied,
        JobVerdict::Blocked,
        JobVerdict::Exhausted,
        JobVerdict::Invalid,
    ] {
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, format!("\"{}\"", v.as_str()));
        let rt: JobVerdict = serde_json::from_str(&json).unwrap();
        assert_eq!(v, rt);
    }
    for v in [
        EvidenceReadinessStatus::Present,
        EvidenceReadinessStatus::Missing,
        EvidenceReadinessStatus::Disputed,
        EvidenceReadinessStatus::Blocked,
        EvidenceReadinessStatus::Concern,
    ] {
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, format!("\"{}\"", v.as_str()));
        let rt: EvidenceReadinessStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(v, rt);
    }
    {
        let v = AuthorityEffect::None;
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, format!("\"{}\"", v.as_str()));
        let rt: AuthorityEffect = serde_json::from_str(&json).unwrap();
        assert_eq!(v, rt);
    }
}
