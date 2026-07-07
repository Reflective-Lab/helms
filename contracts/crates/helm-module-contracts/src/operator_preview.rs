//! Read-only operator preview views over the receipts vocabulary.
//!
//! # Overview
//!
//! This module provides thin serialization-ready view types for composing
//! operator-control live feed payloads and workbench dashboard responses.
//! All types depend only on [`crate::operator_receipts`] (serde + sha2,
//! no transport); they are safe to import in any pure consumer.
//!
//! # Consumers
//!
//! - **Operator-control live feed** — `helm-operator-control` composes
//!   [`OperatorControlPreview::live_app_feed`] from inbound packets and
//!   ledger entries, then serializes it for the SSE/HTTP feed.
//! - **Workbench dashboard** — `workbench-backend` renders
//!   [`OperatorReceiptFamilyView`] rows for the receipt-family explorer.
//!
//! # Transport-pure contract
//!
//! No axum import appears here. These types are constructed in the application
//! or workbench layer and serialized at the transport boundary, keeping this
//! module composable without pulling HTTP dependencies.

use serde::Serialize;

use crate::operator_receipts::{
    JobReadinessPacket, OperatorLedgerEntry, OperatorLedgerRecordKind, ReceiptFamily,
};

/// Operator-control preview payload for the live app feed.
///
/// Compose via [`OperatorControlPreview::live_app_feed`].
#[derive(Debug, Clone, Serialize)]
pub struct OperatorControlPreview {
    /// The readiness packet for the job under review.
    pub packet: JobReadinessPacket,
    /// Ledger entries associated with this job.
    pub ledger_entries: Vec<OperatorLedgerEntry>,
    /// All supported receipt families, each with their record kinds.
    pub receipt_families: Vec<OperatorReceiptFamilyView>,
    /// What data source backs this preview.
    pub backing: OperatorControlPreviewBacking,
    /// Human-readable label for the backing source.
    pub backing_label: &'static str,
}

impl OperatorControlPreview {
    /// Constructs a live app feed preview from an inbound readiness packet and
    /// ledger entries. Sets [`OperatorControlPreviewBacking::LiveAppFeed`] and
    /// populates all supported receipt family rows via
    /// [`operator_receipt_families`].
    pub fn live_app_feed(
        packet: JobReadinessPacket,
        ledger_entries: Vec<OperatorLedgerEntry>,
    ) -> Self {
        Self {
            packet,
            ledger_entries,
            receipt_families: operator_receipt_families(),
            backing: OperatorControlPreviewBacking::LiveAppFeed,
            backing_label: OperatorControlPreviewBacking::LiveAppFeed.label(),
        }
    }
}

/// The data source backing an [`OperatorControlPreview`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperatorControlPreviewBacking {
    /// Backed by a live app evidence feed.
    LiveAppFeed,
}

impl OperatorControlPreviewBacking {
    /// Returns the human-readable label for this backing variant.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::LiveAppFeed => "live",
        }
    }
}

/// A view of a receipt family and its associated record kinds.
///
/// Used in [`OperatorControlPreview::receipt_families`] to enumerate every
/// supported family for the operator dashboard.
#[derive(Debug, Clone, Serialize)]
pub struct OperatorReceiptFamilyView {
    /// The receipt family discriminant.
    pub family: ReceiptFamily,
    /// Human-readable description of what receipts in this family track.
    pub purpose: String,
    /// All record kinds that belong to this family.
    pub record_kinds: Vec<OperatorLedgerRecordKind>,
}

/// Returns the canonical ordered list of all operator receipt families with
/// their associated [`OperatorLedgerRecordKind`] members.
///
/// Every call returns the same four-element slice in this order:
/// [`ReceiptFamily::Common`], [`ReceiptFamily::LongRunningJob`],
/// [`ReceiptFamily::TemporalEvidence`], [`ReceiptFamily::ContentPublication`].
///
/// This function is used by [`OperatorControlPreview::live_app_feed`] to
/// populate the `receipt_families` field on every preview response.
pub fn operator_receipt_families() -> Vec<OperatorReceiptFamilyView> {
    vec![
        OperatorReceiptFamilyView {
            family: ReceiptFamily::Common,
            purpose: "shared adapter and readiness receipts used by every app probe".to_string(),
            record_kinds: vec![
                OperatorLedgerRecordKind::ObservationAdapterReceipt,
                OperatorLedgerRecordKind::JobReadinessPacket,
            ],
        },
        OperatorReceiptFamilyView {
            family: ReceiptFamily::LongRunningJob,
            purpose: "approval, decision, plan, execution, action, and outcome milestones"
                .to_string(),
            record_kinds: vec![
                OperatorLedgerRecordKind::OperatorDecisionReceipt,
                OperatorLedgerRecordKind::ApprovalReceipt,
                OperatorLedgerRecordKind::PlanReceipt,
                OperatorLedgerRecordKind::ExecutionReceipt,
                OperatorLedgerRecordKind::ActionReceipt,
                OperatorLedgerRecordKind::OutcomeReceipt,
            ],
        },
        OperatorReceiptFamilyView {
            family: ReceiptFamily::TemporalEvidence,
            purpose: "corpus snapshots, evidence windows, preserved disagreements, analyst review, and cited narrative claims".to_string(),
            record_kinds: vec![
                OperatorLedgerRecordKind::CorpusSnapshotReceipt,
                OperatorLedgerRecordKind::EvidenceWindowReceipt,
                OperatorLedgerRecordKind::DisagreementReceipt,
                OperatorLedgerRecordKind::AnalystReviewReceipt,
                OperatorLedgerRecordKind::NarrativeClaimReceipt,
            ],
        },
        OperatorReceiptFamilyView {
            family: ReceiptFamily::ContentPublication,
            purpose: "canonical story, claim review, editorial approval, and publication boundary receipts".to_string(),
            record_kinds: vec![
                OperatorLedgerRecordKind::CanonicalStoryReceipt,
                OperatorLedgerRecordKind::ClaimReviewReceipt,
                OperatorLedgerRecordKind::EditorialApprovalReceipt,
                OperatorLedgerRecordKind::PublicationBoundaryReceipt,
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::{
        operator_receipt_families, OperatorControlPreview, OperatorControlPreviewBacking,
    };
    use crate::operator_receipts::{
        AdapterReceiptStatus, JobReadinessPacket, JobReadinessPacketInput,
        OperatorLedgerRecordKind, ReceiptFamily,
    };

    fn minimal_packet() -> JobReadinessPacket {
        let input = JobReadinessPacketInput {
            package_id: "truth_package.test.1".to_string(),
            truth_version: "truth.v1".to_string(),
            domain_hint: "test.domain".to_string(),
            job_key: "test-job".to_string(),
            subject_ref: "test.subject.abc123".to_string(),
            adapter_receipt_id: "artifact.adapter.abc123".to_string(),
            adapter_status: AdapterReceiptStatus::Succeeded,
            verdict: None,
            authorizes_domain_action: false,
            evidence_status: vec![],
            fuzzy_trace: None,
            verifier_forbidden_actions: vec![],
            operator_actions: vec![],
        };
        JobReadinessPacket::new(input).expect("minimal packet builds")
    }

    #[test]
    fn live_app_feed_sets_live_app_feed_backing() {
        let preview = OperatorControlPreview::live_app_feed(minimal_packet(), vec![]);
        assert_eq!(preview.backing, OperatorControlPreviewBacking::LiveAppFeed);
    }

    #[test]
    fn live_app_feed_label_is_live() {
        let preview = OperatorControlPreview::live_app_feed(minimal_packet(), vec![]);
        assert_eq!(preview.backing_label, "live");
    }

    #[test]
    fn live_app_feed_backing_label_matches_variant_label() {
        let preview = OperatorControlPreview::live_app_feed(minimal_packet(), vec![]);
        assert_eq!(preview.backing_label, preview.backing.label());
    }

    #[test]
    fn live_app_feed_populates_four_receipt_families() {
        let preview = OperatorControlPreview::live_app_feed(minimal_packet(), vec![]);
        assert_eq!(preview.receipt_families.len(), 4);
    }

    #[test]
    fn live_app_feed_receipt_families_order_is_stable() {
        let preview = OperatorControlPreview::live_app_feed(minimal_packet(), vec![]);
        assert_eq!(preview.receipt_families[0].family, ReceiptFamily::Common);
        assert_eq!(
            preview.receipt_families[1].family,
            ReceiptFamily::LongRunningJob
        );
        assert_eq!(
            preview.receipt_families[2].family,
            ReceiptFamily::TemporalEvidence
        );
        assert_eq!(
            preview.receipt_families[3].family,
            ReceiptFamily::ContentPublication
        );
    }

    #[test]
    fn operator_receipt_families_common_record_kinds() {
        let families = operator_receipt_families();
        let common = families
            .iter()
            .find(|f| f.family == ReceiptFamily::Common)
            .expect("common family present");

        assert!(
            common
                .record_kinds
                .contains(&OperatorLedgerRecordKind::ObservationAdapterReceipt),
            "common must include ObservationAdapterReceipt"
        );
        assert!(
            common
                .record_kinds
                .contains(&OperatorLedgerRecordKind::JobReadinessPacket),
            "common must include JobReadinessPacket"
        );
    }

    #[test]
    fn operator_receipt_families_long_running_job_record_kinds() {
        let families = operator_receipt_families();
        let lrj = families
            .iter()
            .find(|f| f.family == ReceiptFamily::LongRunningJob)
            .expect("long running job family present");

        for kind in [
            OperatorLedgerRecordKind::OperatorDecisionReceipt,
            OperatorLedgerRecordKind::ApprovalReceipt,
            OperatorLedgerRecordKind::PlanReceipt,
            OperatorLedgerRecordKind::ExecutionReceipt,
            OperatorLedgerRecordKind::ActionReceipt,
            OperatorLedgerRecordKind::OutcomeReceipt,
        ] {
            assert!(
                lrj.record_kinds.contains(&kind),
                "LongRunningJob must include {kind:?}"
            );
        }
    }

    #[test]
    fn operator_receipt_families_temporal_evidence_record_kinds() {
        let families = operator_receipt_families();
        let te = families
            .iter()
            .find(|f| f.family == ReceiptFamily::TemporalEvidence)
            .expect("temporal evidence family present");

        for kind in [
            OperatorLedgerRecordKind::CorpusSnapshotReceipt,
            OperatorLedgerRecordKind::EvidenceWindowReceipt,
            OperatorLedgerRecordKind::DisagreementReceipt,
            OperatorLedgerRecordKind::AnalystReviewReceipt,
            OperatorLedgerRecordKind::NarrativeClaimReceipt,
        ] {
            assert!(
                te.record_kinds.contains(&kind),
                "TemporalEvidence must include {kind:?}"
            );
        }
    }

    #[test]
    fn operator_receipt_families_content_publication_record_kinds() {
        let families = operator_receipt_families();
        let cp = families
            .iter()
            .find(|f| f.family == ReceiptFamily::ContentPublication)
            .expect("content publication family present");

        for kind in [
            OperatorLedgerRecordKind::CanonicalStoryReceipt,
            OperatorLedgerRecordKind::ClaimReviewReceipt,
            OperatorLedgerRecordKind::EditorialApprovalReceipt,
            OperatorLedgerRecordKind::PublicationBoundaryReceipt,
        ] {
            assert!(
                cp.record_kinds.contains(&kind),
                "ContentPublication must include {kind:?}"
            );
        }
    }
}
