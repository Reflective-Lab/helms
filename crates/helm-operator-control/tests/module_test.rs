use std::sync::Arc;

use application_storage::{AppConfig, InMemoryKernelStore};
use helm_operator_control::{
    AdapterReceiptStatus, AuthorityEffect, EvidenceReadinessStatus, JobEvidenceStatus,
    JobReadinessPacket, JobReadinessPacketInput, JobVerdict, LiveReadinessEvidence,
    OperatorControlModule, OperatorControlModuleState, OperatorLedgerRecordKind, ReceiptFamily,
    job_readiness_packet_ledger_entry, job_readiness_packet_payload_hash,
};
use runway_app_host::HelmModule;
use serde_json::json;

fn test_config() -> AppConfig {
    AppConfig::default()
}

#[test]
fn module_id_is_stable() {
    let m: Arc<OperatorControlModule<InMemoryKernelStore>> =
        Arc::new(OperatorControlModule::new(test_config()));
    assert_eq!(m.module_id(), "helm.operator-control");
}

#[test]
fn module_exposes_router() {
    let m: Arc<OperatorControlModule<InMemoryKernelStore>> =
        Arc::new(OperatorControlModule::new(test_config()));
    // Calling router() consumes the Arc — just verify it doesn't panic.
    let _router = m.router();
}

#[test]
fn default_module_reports_shell_default() {
    let m: OperatorControlModule<InMemoryKernelStore> = OperatorControlModule::new(test_config());
    let status = m.readiness_status();

    assert_eq!(m.module_state(), OperatorControlModuleState::ShellDefault);
    assert_eq!(status.state, OperatorControlModuleState::ShellDefault);
    assert_eq!(status.registered_truths, Some(0));
    assert!(
        status
            .missing_live_requirements
            .contains(&"process_receipt".to_string())
    );
    assert!(
        status
            .missing_live_requirements
            .contains(&"integrity_proof".to_string())
    );
    assert!(
        status
            .missing_live_requirements
            .contains(&"adapter_receipt".to_string())
    );
    assert!(
        status
            .missing_live_requirements
            .contains(&"axiom_report".to_string())
    );
}

#[test]
fn complete_live_evidence_reports_live() {
    let m: OperatorControlModule<InMemoryKernelStore> = OperatorControlModule::new(test_config())
        .with_live_readiness_evidence(LiveReadinessEvidence::complete());
    let status = m.readiness_status();

    assert_eq!(m.module_state(), OperatorControlModuleState::Live);
    assert_eq!(status.state, OperatorControlModuleState::Live);
    assert!(status.missing_live_requirements.is_empty());
}

#[test]
fn readiness_status_serializes_shell_default_for_rr_verifier() {
    let m: OperatorControlModule<InMemoryKernelStore> = OperatorControlModule::new(test_config());
    let value = serde_json::to_value(m.readiness_status()).expect("status serializes");

    assert_eq!(value["module_id"], "helm.operator-control");
    assert_eq!(value["state"], "shell-default");
    assert_eq!(value["registered_truths"], 0);
    assert_eq!(
        value["live_requirements"],
        json!([
            "process_receipt",
            "integrity_proof",
            "adapter_receipt",
            "axiom_report"
        ])
    );
    assert_eq!(
        value["missing_live_requirements"],
        json!([
            "process_receipt",
            "integrity_proof",
            "adapter_receipt",
            "axiom_report"
        ])
    );
}

#[test]
fn readiness_status_serializes_live_without_missing_requirements() {
    let m: OperatorControlModule<InMemoryKernelStore> = OperatorControlModule::new(test_config())
        .with_live_readiness_evidence(LiveReadinessEvidence::complete());
    let value = serde_json::to_value(m.readiness_status()).expect("status serializes");

    assert_eq!(value["module_id"], "helm.operator-control");
    assert_eq!(value["state"], "live");
    assert_eq!(value["registered_truths"], 0);
    assert!(value.get("missing_live_requirements").is_none());
}

#[test]
fn helm_crate_exports_operator_control_contracts() {
    let packet = JobReadinessPacket::new(JobReadinessPacketInput {
        package_id: "pkg-quorum-001".to_string(),
        truth_version: "truth-v1".to_string(),
        domain_hint: "quorum-sense".to_string(),
        job_key: "adaptive-inquiry".to_string(),
        subject_ref: "quorum:inquiry:123".to_string(),
        adapter_receipt_id: "receipt:adapter:123".to_string(),
        adapter_status: AdapterReceiptStatus::Succeeded,
        verdict: Some(JobVerdict::Blocked),
        authorizes_domain_action: false,
        evidence_status: vec![JobEvidenceStatus {
            clause_id: "clause-1".to_string(),
            clause_key: "participant-consent".to_string(),
            label: "Participant consent is present".to_string(),
            status: EvidenceReadinessStatus::Present,
            fact_ids: vec!["fact-1".to_string()],
            evidence_refs: vec!["evidence-1".to_string()],
            trace_links: Vec::new(),
            concern_record_ids: Vec::new(),
        }],
        fuzzy_trace: None,
        verifier_forbidden_actions: vec!["authorize_domain_action".to_string()],
        operator_actions: vec!["review_packet".to_string()],
    })
    .expect("packet builds from Helm export");

    let hash = job_readiness_packet_payload_hash(&packet);
    assert!(hash.starts_with("sha256:"));

    let entry = job_readiness_packet_ledger_entry(
        1,
        &packet,
        vec!["receipt:adapter:123".to_string()],
        "Quorum readiness packet recorded",
    )
    .expect("ledger entry builds from Helm export");

    assert_eq!(
        entry.record_kind,
        OperatorLedgerRecordKind::JobReadinessPacket
    );
    assert_eq!(entry.receipt_family, ReceiptFamily::Common);
    assert_eq!(entry.authority_effect, AuthorityEffect::None);
    assert_eq!(entry.source_ref, packet.packet_id);
}
