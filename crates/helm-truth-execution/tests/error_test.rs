//! Tests for TruthExecutionError variants and the grpc feature mapping.
//!
//! Negative tests assert exact error variants for the failure paths in
//! `common.rs` and the dispatcher.  The `grpc` module (behind
//! `#[cfg(feature = "grpc")]`) asserts that every variant maps to the exact
//! `tonic::Status` code that was produced before RFL-176 (oracle captured
//! from the pre-change source).

use std::collections::HashMap;

use helm_truth_execution::TruthExecutionError;
use helm_truth_execution::common::{
    optional_uuid, payload_from_result, required_datetime, required_input, required_uuid,
};

// ── common.rs negative tests ───────────────────────────────────────────────────

#[test]
fn required_input_missing_key_is_invalid_argument() {
    let inputs = HashMap::new();
    let err = required_input(&inputs, "org_id").unwrap_err();
    assert!(
        matches!(err, TruthExecutionError::InvalidArgument { .. }),
        "expected InvalidArgument, got {err:?}"
    );
    assert!(
        err.message().contains("org_id"),
        "message should name the missing key"
    );
}

#[test]
fn required_input_empty_value_is_invalid_argument() {
    let mut inputs = HashMap::new();
    inputs.insert("org_id".into(), "   ".into());
    let err = required_input(&inputs, "org_id").unwrap_err();
    assert!(matches!(err, TruthExecutionError::InvalidArgument { .. }));
}

#[test]
fn required_uuid_malformed_is_invalid_argument() {
    let mut inputs = HashMap::new();
    inputs.insert("id".into(), "not-a-uuid".into());
    let err = required_uuid(&inputs, "id").unwrap_err();
    assert!(
        matches!(err, TruthExecutionError::InvalidArgument { .. }),
        "expected InvalidArgument, got {err:?}"
    );
    assert!(err.message().contains("id"));
}

#[test]
fn required_datetime_malformed_is_invalid_argument() {
    let mut inputs = HashMap::new();
    inputs.insert("due_at".into(), "not-a-date".into());
    let err = required_datetime(&inputs, "due_at").unwrap_err();
    assert!(
        matches!(err, TruthExecutionError::InvalidArgument { .. }),
        "expected InvalidArgument, got {err:?}"
    );
    assert!(err.message().contains("due_at"));
}

#[test]
fn optional_uuid_malformed_is_invalid_argument() {
    let mut inputs = HashMap::new();
    inputs.insert("ref_id".into(), "bad-uuid".into());
    let err = optional_uuid(&inputs, "ref_id").unwrap_err();
    assert!(matches!(err, TruthExecutionError::InvalidArgument { .. }));
}

// ── message() helper ───────────────────────────────────────────────────────────

#[test]
fn message_returns_inner_string_without_prefix() {
    let e = TruthExecutionError::InvalidArgument {
        message: "missing required input: foo".into(),
    };
    // message() must return the bare string, not "invalid argument: ..."
    assert_eq!(e.message(), "missing required input: foo");
}

#[test]
fn display_includes_variant_prefix() {
    let e = TruthExecutionError::NotFound {
        message: "org not found: 123".into(),
    };
    let s = e.to_string();
    assert!(s.contains("not found"), "Display should include prefix: {s}");
    assert!(
        s.contains("org not found: 123"),
        "Display should include message: {s}"
    );
}

// ── payload_from_result negative tests ────────────────────────────────────────

#[test]
fn payload_from_result_missing_fact_is_failed_precondition() {
    use converge_core::integrity::{ContentHash, IntegrityProof, MerkleRoot};
    use converge_core::{ContextState, ConvergeResult, StopReason};
    use converge_kernel::ContextKey;

    let result = ConvergeResult {
        context: ContextState::default(),
        cycles: 0,
        converged: false,
        stop_reason: StopReason::Converged,
        criteria_outcomes: vec![],
        integrity: IntegrityProof {
            merkle_root: MerkleRoot(ContentHash([0u8; 32])),
            clock_time: 0,
            fact_count: 0,
        },
    };

    let err = payload_from_result::<serde_json::Value>(
        &result,
        ContextKey::Seeds,
        "fit-score-fact",
    )
    .unwrap_err();

    assert!(
        matches!(err, TruthExecutionError::FailedPrecondition { .. }),
        "missing fact should be FailedPrecondition, got {err:?}"
    );
    assert!(err.message().contains("fit-score-fact"));
}

// ── grpc feature — Status code mapping oracle ─────────────────────────────────

#[cfg(feature = "grpc")]
mod grpc_mapping {
    use helm_truth_execution::TruthExecutionError;
    use tonic::Code;

    fn assert_maps_to(e: TruthExecutionError, expected_code: Code, expected_message: &str) {
        let status = tonic::Status::from(e);
        assert_eq!(
            status.code(),
            expected_code,
            "wrong gRPC code for {expected_message}"
        );
        assert_eq!(
            status.message(),
            expected_message,
            "message must be preserved verbatim"
        );
    }

    #[test]
    fn invalid_argument_maps_to_invalid_argument() {
        assert_maps_to(
            TruthExecutionError::InvalidArgument {
                message: "missing required input: org_id".into(),
            },
            Code::InvalidArgument,
            "missing required input: org_id",
        );
    }

    #[test]
    fn not_found_maps_to_not_found() {
        assert_maps_to(
            TruthExecutionError::NotFound {
                message: "Organization not found: abc-123".into(),
            },
            Code::NotFound,
            "Organization not found: abc-123",
        );
    }

    #[test]
    fn failed_precondition_maps_to_failed_precondition() {
        assert_maps_to(
            TruthExecutionError::FailedPrecondition {
                message: "missing fact in converge context: fit-score-fact".into(),
            },
            Code::FailedPrecondition,
            "missing fact in converge context: fit-score-fact",
        );
    }

    #[test]
    fn internal_maps_to_internal() {
        assert_maps_to(
            TruthExecutionError::Internal {
                message: "storage lock poisoned".into(),
            },
            Code::Internal,
            "storage lock poisoned",
        );
    }

    #[test]
    fn already_exists_maps_to_already_exists() {
        assert_maps_to(
            TruthExecutionError::AlreadyExists {
                message: "conflict on entity xyz".into(),
            },
            Code::AlreadyExists,
            "conflict on entity xyz",
        );
    }

    #[test]
    fn resource_exhausted_maps_to_resource_exhausted() {
        assert_maps_to(
            TruthExecutionError::ResourceExhausted {
                message: "converge budget exhausted: cycles".into(),
            },
            Code::ResourceExhausted,
            "converge budget exhausted: cycles",
        );
    }

    #[test]
    fn aborted_maps_to_aborted() {
        assert_maps_to(
            TruthExecutionError::Aborted {
                message: "converge fact conflict: fact-id-xyz".into(),
            },
            Code::Aborted,
            "converge fact conflict: fact-id-xyz",
        );
    }

    #[test]
    fn data_loss_maps_to_data_loss() {
        assert_maps_to(
            TruthExecutionError::DataLoss {
                message: "converge invalid context snapshot: corrupt".into(),
            },
            Code::DataLoss,
            "converge invalid context snapshot: corrupt",
        );
    }

    #[test]
    fn unavailable_maps_to_unavailable() {
        assert_maps_to(
            TruthExecutionError::Unavailable {
                message: "surrealdb connection failed: refused".into(),
            },
            Code::Unavailable,
            "surrealdb connection failed: refused",
        );
    }

    #[test]
    fn deadline_exceeded_maps_to_deadline_exceeded() {
        assert_maps_to(
            TruthExecutionError::DeadlineExceeded {
                message: "query_records".into(),
            },
            Code::DeadlineExceeded,
            "query_records",
        );
    }

    #[test]
    fn unimplemented_maps_to_unimplemented() {
        assert_maps_to(
            TruthExecutionError::Unimplemented {
                message: "truth execution is not implemented yet for mystery-truth".into(),
            },
            Code::Unimplemented,
            "truth execution is not implemented yet for mystery-truth",
        );
    }
}
