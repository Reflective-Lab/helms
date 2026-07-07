//! Negative constructor tests for `operator_receipts` — exact
//! `OperatorControlError` variant assertions (RFL-154 T7).
//!
//! Complements the in-module unit tests (which cover fuzzy-score domains and
//! domain-authority rejection) with the remaining rejection surface:
//! empty fields, empty backlinks, the basis-points boundary, and the
//! `sha256:` format gate.

use helm_module_contracts::operator_receipts::{
    AdapterReceiptStatus, FuzzyMembership, FuzzyReadinessTrace, JobReadinessPacket,
    JobReadinessPacketInput, OperatorControlError, OperatorLedgerEntry, OperatorLedgerEntryInput,
    OperatorLedgerRecordKind, ReceiptFamily,
};

fn valid_packet_input() -> JobReadinessPacketInput {
    JobReadinessPacketInput {
        package_id: "pkg.test.001".to_string(),
        truth_version: "truth.v1".to_string(),
        domain_hint: "test.domain".to_string(),
        job_key: "test-job".to_string(),
        subject_ref: "test.subject.abcdef".to_string(),
        adapter_receipt_id: "artifact.adapter.abcdef".to_string(),
        adapter_status: AdapterReceiptStatus::Succeeded,
        verdict: None,
        authorizes_domain_action: false,
        evidence_status: Vec::new(),
        fuzzy_trace: None,
        verifier_forbidden_actions: Vec::new(),
        operator_actions: Vec::new(),
    }
}

const VALID_SHA256: &str =
    "sha256:90b8fb64fdd6f926a4ef42d67a145215aa7e7e07480863217f8558c472da579f";

fn valid_ledger_input() -> OperatorLedgerEntryInput {
    OperatorLedgerEntryInput {
        sequence: 1,
        record_kind: OperatorLedgerRecordKind::JobReadinessPacket,
        receipt_family: ReceiptFamily::Common,
        source_ref: "helm.job_readiness.abcdef012345".to_string(),
        package_id: "pkg.test.001".to_string(),
        truth_version: "truth.v1".to_string(),
        domain_hint: "test.domain".to_string(),
        payload_hash: VALID_SHA256.to_string(),
        backlink_ids: vec!["helm.ledger.abcdef012345".to_string()],
        summary: "test ledger entry".to_string(),
    }
}

fn valid_fuzzy_trace(observed: u16, membership_score: u16) -> FuzzyReadinessTrace {
    FuzzyReadinessTrace {
        variable_key: "drift-severity".to_string(),
        observed_value_basis_points: observed,
        memberships: vec![FuzzyMembership {
            label: "high".to_string(),
            score_basis_points: membership_score,
        }],
        activated_rules: Vec::new(),
        defuzzified_score: None,
    }
}

// ── Empty-field rejections (packet) ───────────────────────────────────────────

#[test]
fn empty_package_id_is_rejected() {
    let mut input = valid_packet_input();
    input.package_id = String::new();
    let err = JobReadinessPacket::new(input).expect_err("empty package_id must fail");
    assert_eq!(err, OperatorControlError::EmptyField { field: "package_id" });
}

#[test]
fn whitespace_only_package_id_is_rejected() {
    let mut input = valid_packet_input();
    input.package_id = "   ".to_string();
    let err = JobReadinessPacket::new(input).expect_err("whitespace package_id must fail");
    assert_eq!(err, OperatorControlError::EmptyField { field: "package_id" });
}

#[test]
fn empty_truth_version_is_rejected() {
    let mut input = valid_packet_input();
    input.truth_version = String::new();
    let err = JobReadinessPacket::new(input).expect_err("empty truth_version must fail");
    assert_eq!(
        err,
        OperatorControlError::EmptyField {
            field: "truth_version"
        }
    );
}

#[test]
fn empty_domain_hint_is_rejected() {
    let mut input = valid_packet_input();
    input.domain_hint = String::new();
    let err = JobReadinessPacket::new(input).expect_err("empty domain_hint must fail");
    assert_eq!(err, OperatorControlError::EmptyField { field: "domain_hint" });
}

#[test]
fn empty_job_key_is_rejected() {
    let mut input = valid_packet_input();
    input.job_key = String::new();
    let err = JobReadinessPacket::new(input).expect_err("empty job_key must fail");
    assert_eq!(err, OperatorControlError::EmptyField { field: "job_key" });
}

#[test]
fn empty_subject_ref_is_rejected() {
    let mut input = valid_packet_input();
    input.subject_ref = String::new();
    let err = JobReadinessPacket::new(input).expect_err("empty subject_ref must fail");
    assert_eq!(err, OperatorControlError::EmptyField { field: "subject_ref" });
}

#[test]
fn empty_adapter_receipt_id_is_rejected() {
    let mut input = valid_packet_input();
    input.adapter_receipt_id = String::new();
    let err = JobReadinessPacket::new(input).expect_err("empty adapter_receipt_id must fail");
    assert_eq!(
        err,
        OperatorControlError::EmptyField {
            field: "adapter_receipt_id"
        }
    );
}

#[test]
fn empty_fuzzy_variable_key_is_rejected() {
    let mut input = valid_packet_input();
    let mut trace = valid_fuzzy_trace(5_000, 5_000);
    trace.variable_key = String::new();
    input.fuzzy_trace = Some(trace);
    let err = JobReadinessPacket::new(input).expect_err("empty variable_key must fail");
    assert_eq!(
        err,
        OperatorControlError::EmptyField {
            field: "fuzzy_trace.variable_key"
        }
    );
}

#[test]
fn empty_fuzzy_memberships_are_rejected() {
    let mut input = valid_packet_input();
    let mut trace = valid_fuzzy_trace(5_000, 5_000);
    trace.memberships = Vec::new();
    input.fuzzy_trace = Some(trace);
    let err = JobReadinessPacket::new(input).expect_err("empty memberships must fail");
    assert_eq!(
        err,
        OperatorControlError::EmptyField {
            field: "fuzzy_trace.memberships"
        }
    );
}

// ── Empty-field rejections (ledger) ───────────────────────────────────────────

#[test]
fn empty_source_ref_is_rejected() {
    let mut input = valid_ledger_input();
    input.source_ref = String::new();
    let err = OperatorLedgerEntry::new(input).expect_err("empty source_ref must fail");
    assert_eq!(err, OperatorControlError::EmptyField { field: "source_ref" });
}

#[test]
fn empty_summary_is_rejected() {
    let mut input = valid_ledger_input();
    input.summary = String::new();
    let err = OperatorLedgerEntry::new(input).expect_err("empty summary must fail");
    assert_eq!(err, OperatorControlError::EmptyField { field: "summary" });
}

// ── Empty-backlink rejection ──────────────────────────────────────────────────

#[test]
fn empty_backlink_id_in_list_is_rejected() {
    let mut input = valid_ledger_input();
    input.backlink_ids = vec!["valid.backlink.abc".to_string(), String::new()];
    let err = OperatorLedgerEntry::new(input).expect_err("empty backlink id must fail");
    assert_eq!(err, OperatorControlError::EmptyBacklink);
}

#[test]
fn whitespace_only_backlink_id_is_rejected() {
    let mut input = valid_ledger_input();
    input.backlink_ids = vec!["   ".to_string()];
    let err = OperatorLedgerEntry::new(input).expect_err("whitespace backlink id must fail");
    assert_eq!(err, OperatorControlError::EmptyBacklink);
}

#[test]
fn empty_backlink_list_is_accepted() {
    let mut input = valid_ledger_input();
    input.backlink_ids = Vec::new();
    OperatorLedgerEntry::new(input).expect("an empty backlink LIST is valid — only empty IDS fail");
}

// ── Basis-points boundary ─────────────────────────────────────────────────────

#[test]
fn basis_points_10000_is_accepted() {
    let mut input = valid_packet_input();
    input.fuzzy_trace = Some(valid_fuzzy_trace(10_000, 10_000));
    JobReadinessPacket::new(input).expect("10000 basis points is the inclusive maximum");
}

#[test]
fn observed_basis_points_10001_is_rejected() {
    let mut input = valid_packet_input();
    input.fuzzy_trace = Some(valid_fuzzy_trace(10_001, 5_000));
    let err = JobReadinessPacket::new(input).expect_err("10001 observed basis points must fail");
    assert_eq!(
        err,
        OperatorControlError::InvalidBasisPoints {
            field: "fuzzy_trace.observed_value_basis_points",
            value: 10_001,
        }
    );
}

#[test]
fn membership_basis_points_10001_is_rejected() {
    let mut input = valid_packet_input();
    input.fuzzy_trace = Some(valid_fuzzy_trace(5_000, 10_001));
    let err = JobReadinessPacket::new(input).expect_err("10001 membership score must fail");
    assert_eq!(
        err,
        OperatorControlError::InvalidBasisPoints {
            field: "fuzzy_trace.membership.score_basis_points",
            value: 10_001,
        }
    );
}

// ── sha256 format rejections ──────────────────────────────────────────────────

#[test]
fn raw_string_payload_hash_is_rejected() {
    let mut input = valid_ledger_input();
    input.payload_hash = "raw-payload-without-prefix".to_string();
    let err = OperatorLedgerEntry::new(input).expect_err("raw payload hash must fail");
    assert_eq!(
        err,
        OperatorControlError::InvalidSha256 {
            field: "payload_hash",
            value: "raw-payload-without-prefix".to_string(),
        }
    );
}

#[test]
fn sha256_prefix_with_short_digest_is_rejected() {
    let mut input = valid_ledger_input();
    // 63 hex chars — one short of 64.
    let short = format!("sha256:{}", &VALID_SHA256["sha256:".len()..VALID_SHA256.len() - 1]);
    input.payload_hash = short.clone();
    let err = OperatorLedgerEntry::new(input).expect_err("63-char digest must fail");
    assert_eq!(
        err,
        OperatorControlError::InvalidSha256 {
            field: "payload_hash",
            value: short,
        }
    );
}

#[test]
fn sha256_prefix_with_non_hex_digest_is_rejected() {
    let mut input = valid_ledger_input();
    // 64 chars but two are 'z' — not hex.
    let bad = format!(
        "sha256:{}zz",
        &VALID_SHA256["sha256:".len()..VALID_SHA256.len() - 2]
    );
    input.payload_hash = bad.clone();
    let err = OperatorLedgerEntry::new(input).expect_err("non-hex digest must fail");
    assert_eq!(
        err,
        OperatorControlError::InvalidSha256 {
            field: "payload_hash",
            value: bad,
        }
    );
}

#[test]
fn empty_payload_hash_is_rejected_as_invalid_sha256() {
    let mut input = valid_ledger_input();
    input.payload_hash = String::new();
    let err = OperatorLedgerEntry::new(input).expect_err("empty payload hash must fail");
    assert_eq!(
        err,
        OperatorControlError::InvalidSha256 {
            field: "payload_hash",
            value: String::new(),
        }
    );
}

#[test]
fn valid_sha256_payload_hash_is_accepted() {
    OperatorLedgerEntry::new(valid_ledger_input())
        .expect("well-formed sha256 payload hash must be accepted");
}

// ── Display contract ──────────────────────────────────────────────────────────

#[test]
fn error_display_names_the_offending_field() {
    let err = OperatorControlError::EmptyField { field: "package_id" };
    assert!(err.to_string().contains("package_id"));

    let err = OperatorControlError::InvalidBasisPoints {
        field: "fuzzy_trace.observed_value_basis_points",
        value: 10_001,
    };
    assert!(err.to_string().contains("10001"));

    let err = OperatorControlError::InvalidSha256 {
        field: "payload_hash",
        value: "nope".to_string(),
    };
    assert!(err.to_string().contains("payload_hash"));
    assert!(err.to_string().contains("nope"));
}
