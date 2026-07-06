use std::sync::Arc;

use application_storage::{AppConfig, InMemoryKernelStore};
use helm_module_contracts::{HelmModule, ModuleState};
use helm_operator_control::{
    AdapterReceiptStatus, AuthorityEffect, EvidenceReadinessStatus, JobEvidenceStatus,
    JobReadinessPacket, JobReadinessPacketInput, JobVerdict, LiveOperatorControlSnapshot,
    LiveReadinessEvidence, OperatorControlModule, OperatorControlModuleState,
    OperatorControlPreview, OperatorControlPreviewBacking, OperatorControlReadinessFeed,
    OperatorControlState, OperatorLedgerRecordKind, ReceiptFamily,
    job_readiness_packet_ledger_entry, job_readiness_packet_payload_hash,
};
use serde_json::json;

fn test_config() -> AppConfig {
    AppConfig::default()
}

#[derive(Clone)]
struct StaticReadinessFeed {
    evidence: LiveReadinessEvidence,
    snapshots: Vec<LiveOperatorControlSnapshot>,
}

impl OperatorControlReadinessFeed for StaticReadinessFeed {
    fn live_evidence(&self) -> LiveReadinessEvidence {
        self.evidence
    }

    fn previews(
        &self,
    ) -> Result<Vec<LiveOperatorControlSnapshot>, helm_operator_control::OperatorControlError> {
        Ok(self.snapshots.clone())
    }
}

fn sample_packet() -> JobReadinessPacket {
    JobReadinessPacket::new(JobReadinessPacketInput {
        package_id: "pkg-operator-control-001".to_string(),
        truth_version: "truth-v1".to_string(),
        domain_hint: "operator-control.test".to_string(),
        job_key: "readiness-check".to_string(),
        subject_ref: "operator-control:test-subject:123".to_string(),
        adapter_receipt_id: "receipt:adapter:123".to_string(),
        adapter_status: AdapterReceiptStatus::Succeeded,
        verdict: Some(JobVerdict::Blocked),
        authorizes_domain_action: false,
        evidence_status: vec![JobEvidenceStatus {
            clause_id: "clause-1".to_string(),
            clause_key: "required-evidence".to_string(),
            label: "Required evidence is present".to_string(),
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
    .expect("packet builds")
}

fn sample_snapshot() -> LiveOperatorControlSnapshot {
    let packet = sample_packet();
    let ledger_entry = job_readiness_packet_ledger_entry(
        1,
        &packet,
        vec!["receipt:adapter:123".to_string()],
        "Live readiness packet recorded",
    )
    .expect("ledger entry builds");

    LiveOperatorControlSnapshot::new(packet, vec![ledger_entry])
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
    assert_eq!(
        <OperatorControlModule<InMemoryKernelStore> as HelmModule>::module_state(&m),
        ModuleState::Shell
    );
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
    assert_eq!(
        <OperatorControlModule<InMemoryKernelStore> as HelmModule>::module_state(&m),
        ModuleState::Live
    );
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
fn live_readiness_feed_reports_live_when_evidence_and_snapshot_exist() {
    let feed = Arc::new(StaticReadinessFeed {
        evidence: LiveReadinessEvidence::complete(),
        snapshots: vec![sample_snapshot()],
    });
    let m: OperatorControlModule<InMemoryKernelStore> =
        OperatorControlModule::new(test_config()).with_live_readiness_feed(feed);
    let status = m.readiness_status();

    assert_eq!(m.module_state(), OperatorControlModuleState::Live);
    assert_eq!(
        <OperatorControlModule<InMemoryKernelStore> as HelmModule>::module_state(&m),
        ModuleState::Live
    );
    assert_eq!(status.state, OperatorControlModuleState::Live);
    assert!(
        status
            .live_requirements
            .contains(&"readiness_feed".to_string())
    );
    assert!(status.missing_live_requirements.is_empty());
}

#[test]
fn live_readiness_feed_requires_at_least_one_snapshot() {
    let feed = Arc::new(StaticReadinessFeed {
        evidence: LiveReadinessEvidence::complete(),
        snapshots: Vec::new(),
    });
    let m: OperatorControlModule<InMemoryKernelStore> =
        OperatorControlModule::new(test_config()).with_live_readiness_feed(feed);
    let status = m.readiness_status();

    assert_eq!(m.module_state(), OperatorControlModuleState::ShellDefault);
    assert_eq!(
        <OperatorControlModule<InMemoryKernelStore> as HelmModule>::module_state(&m),
        ModuleState::Shell
    );
    assert_eq!(status.state, OperatorControlModuleState::ShellDefault);
    assert!(
        status
            .missing_live_requirements
            .contains(&"readiness_feed".to_string())
    );
}

#[test]
fn operator_control_state_uses_live_feed_previews_when_present() {
    let feed = Arc::new(StaticReadinessFeed {
        evidence: LiveReadinessEvidence::complete(),
        snapshots: vec![sample_snapshot()],
    });
    let store = InMemoryKernelStore::default_local();
    let state = OperatorControlState::new(store.config.clone(), store).with_readiness_feed(feed);
    let preview = state
        .operator_control_preview()
        .expect("live feed preview is available");

    assert_eq!(preview.backing, OperatorControlPreviewBacking::LiveAppFeed);
    assert_eq!(preview.backing_label, "live");
    assert_eq!(preview.packet.domain_hint, "operator-control.test");
}

#[test]
fn operator_control_state_does_not_fall_back_to_static_demo_when_live_feed_is_empty() {
    let feed = Arc::new(StaticReadinessFeed {
        evidence: LiveReadinessEvidence::complete(),
        snapshots: Vec::new(),
    });
    let store = InMemoryKernelStore::default_local();
    let state = OperatorControlState::new(store.config.clone(), store).with_readiness_feed(feed);
    let previews = state
        .operator_control_previews()
        .expect("empty live feed returns empty previews");
    let error = state
        .operator_control_preview()
        .expect_err("empty live feed must not synthesize a static preview");

    assert!(previews.is_empty());
    assert!(
        error
            .to_string()
            .contains("operator-control preview requires an injected live readiness feed")
    );
}

#[test]
fn operator_control_state_returns_empty_previews_without_live_feed() {
    let store = InMemoryKernelStore::default_local();
    let state = OperatorControlState::new(store.config.clone(), store);

    assert!(
        state
            .operator_control_previews()
            .expect("missing live feed returns empty previews")
            .is_empty()
    );
}

#[test]
fn live_snapshot_converts_to_live_app_feed_preview() {
    let preview: OperatorControlPreview = sample_snapshot().into();

    assert_eq!(preview.backing, OperatorControlPreviewBacking::LiveAppFeed);
    assert_eq!(preview.backing_label, "live");
    assert_eq!(preview.ledger_entries.len(), 1);
}

#[test]
fn helm_crate_exports_operator_control_contracts() {
    let packet = sample_packet();

    let hash = job_readiness_packet_payload_hash(&packet);
    assert!(hash.starts_with("sha256:"));

    let entry = job_readiness_packet_ledger_entry(
        1,
        &packet,
        vec!["receipt:adapter:123".to_string()],
        "Live readiness packet recorded",
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
