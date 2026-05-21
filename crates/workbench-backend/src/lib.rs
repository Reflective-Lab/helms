mod views;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use application_kernel::{
    ActivityOutcome, Actor, ActorKind, Approval, ApprovalStatus, BillingPeriod, CatalogPlanKind,
    CommunicationChannel, CommunicationDirection, CommunicationRecord, CreditGrantApply,
    DocumentAttach, DocumentStatus, FactRecord, Money, NoteAppend, OpportunityCreate,
    OpportunityStage, Organization, OrganizationLifecycle, OrganizationUpsert, Person,
    PersonUpsert, RecordKind, RecordRef, SubscriptionActivate, TimelineEntry, WorkflowCase,
    WorkflowCaseCreate, WorkflowPriority, WorkflowState,
};
use application_storage::{
    AppConfig, InMemoryKernelStore, KernelStore, StorageError, StoreWriteResult,
};
use capability_registry::all_modules;
use chrono::Utc;
use organism_domain::packs;
use organism_runtime::Registry;
use prio_agent_ops::{
    AdapterReceiptStatus, EvidenceReadinessStatus, FuzzyDefuzzifiedScore, FuzzyMembership,
    FuzzyReadinessTrace, FuzzyRuleActivation, JobEvidenceStatus, JobReadinessPacket,
    JobReadinessPacketInput, JobVerdict, OperatorControlError, OperatorLedgerRecordKind,
    ReceiptFamily, job_readiness_packet_ledger_entry,
};
use thiserror::Error;
use truth_catalog::{
    TruthDefinition,
    admission::{TruthFormationSelection, default_helms_capabilities, select_formation_for_intent},
    all_truths, converge_binding_for_truth, display_pack_names_for_truth, find_truth,
    intent_compile::compile_intent_for_truth,
};
use uuid::Uuid;

pub use views::{
    AccountWorkspaceSummary, ApprovalFilter, ApprovalListItem, AxiomIntentView,
    CatalogItemListItem, ConvergeTruthResolutionView, CriteriaOutcomeItem, CriterionStatus,
    EntitlementListItem, ExecutionState, FeatureToggles, FormationSelectionView,
    OperatorControlPreview, OperatorDashboard, OperatorReceiptFamilyView, OpportunityListItem,
    OrganismCapabilityRequirementView, OrganismPackRequirementView, OrganismTruthResolutionView,
    OrganizationListItem, OrganizationWorkspaceItem, PersonWorkspaceItem, RecordReferenceItem,
    SubscriptionListItem, SystemProfile, TimelineEventItem, TruthDetailItem,
    TruthExecutionProjection, TruthExecutionResult, TruthExecutionSession, TruthListItem,
    TruthModuleTouchItem, TruthReadinessConfirmationView, TruthReadinessGapView,
    TruthReadinessView, WorkbenchAppKind, WorkbenchAppManifest, WorkbenchAppStatus,
    WorkflowCaseFilter, WorkflowCaseListItem,
};

const QUALIFY_INBOUND_LEAD: &str = "qualify-inbound-lead";
const SUBMIT_EXPENSE_REPORT: &str = "submit-expense-report";
const ACTIVATE_SUBSCRIPTION: &str = "activate-subscription";
const REFILL_PREPAID_AI_CREDITS: &str = "refill-prepaid-ai-credits";

#[derive(Debug, Error)]
pub enum OperatorAppError {
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error("truth not found: {0}")]
    TruthNotFound(String),
    #[error("missing required input: {0}")]
    MissingInput(&'static str),
    #[error("invalid uuid for {field}: {value}")]
    InvalidUuid { field: &'static str, value: String },
    #[error("invalid integer for {field}: {value}")]
    InvalidInteger { field: &'static str, value: String },
    #[error("validation error: {0}")]
    Validation(String),
    #[error("operator control error: {0}")]
    OperatorControl(String),
    #[error("unsupported truth execution: {0}")]
    UnsupportedTruth(String),
}

pub type OperatorAppResult<T> = Result<T, OperatorAppError>;

#[derive(Debug, Clone)]
pub struct OperatorApp<S = InMemoryKernelStore> {
    store: S,
    config: AppConfig,
    default_actor: Actor,
    metadata: Arc<RwLock<RuntimeMetadata>>,
    organism_registry: Arc<Registry>,
}

#[derive(Debug, Default)]
struct RuntimeMetadata {
    approvals: HashMap<Uuid, ApprovalMetadata>,
    workflows: HashMap<Uuid, WorkflowMetadata>,
}

#[derive(Debug, Clone)]
struct ApprovalMetadata {
    truth_key: String,
    reason: String,
}

#[derive(Debug, Clone)]
struct WorkflowMetadata {
    definition_key: String,
}

#[derive(Debug)]
struct QualifyLeadWriteResult {
    organization: Organization,
    person: Option<Person>,
    opportunity_id: Option<Uuid>,
    workflow_case: Option<WorkflowCase>,
    approval_id: Option<Uuid>,
    fact_ids: Vec<Uuid>,
}

#[derive(Debug)]
struct ActivationWriteResult {
    organization_id: Uuid,
    subscription_id: Uuid,
    workflow_case: Option<WorkflowCase>,
    approval_id: Option<Uuid>,
    entitlement_ids: Vec<Uuid>,
}

#[derive(Debug)]
struct CreditRefillWriteResult {
    organization_id: Uuid,
    subscription_id: Uuid,
    workflow_case: Option<WorkflowCase>,
    approval_id: Option<Uuid>,
    entitlement_ids: Vec<Uuid>,
}

#[derive(Debug)]
struct ExpenseReportWriteResult {
    organization: Organization,
    workflow_case: WorkflowCase,
    approval_id: Option<Uuid>,
    fact_ids: Vec<Uuid>,
    document_ids: Vec<Uuid>,
}

fn default_organism_registry() -> Registry {
    let mut registry = Registry::new();
    registry.register_pack_with_profile(
        "customers",
        packs::customers::AGENTS,
        packs::customers::INVARIANTS,
        &packs::customers::PROFILE,
    );
    registry.register_pack_with_profile(
        "legal",
        packs::legal::AGENTS,
        packs::legal::INVARIANTS,
        &packs::legal::PROFILE,
    );
    registry.register_pack_with_profile(
        "autonomous_org",
        packs::autonomous_org::AGENTS,
        packs::autonomous_org::INVARIANTS,
        &packs::autonomous_org::PROFILE,
    );
    registry.register_pack_with_profile(
        "partnerships",
        packs::partnerships::AGENTS,
        packs::partnerships::INVARIANTS,
        &packs::partnerships::PROFILE,
    );
    registry.register_pack_with_profile(
        "people",
        packs::people::AGENTS,
        packs::people::INVARIANTS,
        &packs::people::PROFILE,
    );
    registry.register_pack_with_profile(
        "procurement",
        packs::procurement::AGENTS,
        packs::procurement::INVARIANTS,
        &packs::procurement::PROFILE,
    );
    registry.register_pack_with_profile(
        "linkedin_research",
        packs::linkedin_research::AGENTS,
        packs::linkedin_research::INVARIANTS,
        &packs::linkedin_research::PROFILE,
    );
    registry.register_pack_with_profile(
        "knowledge",
        packs::knowledge::AGENTS,
        packs::knowledge::INVARIANTS,
        &packs::knowledge::PROFILE,
    );
    registry.register_pack_with_profile(
        "due_diligence",
        packs::due_diligence::AGENTS,
        packs::due_diligence::INVARIANTS,
        &packs::due_diligence::PROFILE,
    );
    registry.register_pack_with_profile(
        "growth_marketing",
        packs::growth_marketing::AGENTS,
        packs::growth_marketing::INVARIANTS,
        &packs::growth_marketing::PROFILE,
    );
    registry.register_pack_with_profile(
        "ops_support",
        packs::ops_support::AGENTS,
        packs::ops_support::INVARIANTS,
        &packs::ops_support::PROFILE,
    );
    registry.register_pack_with_profile(
        "performance",
        packs::performance::AGENTS,
        packs::performance::INVARIANTS,
        &packs::performance::PROFILE,
    );
    registry.register_pack_with_profile(
        "product_engineering",
        packs::product_engineering::AGENTS,
        packs::product_engineering::INVARIANTS,
        &packs::product_engineering::PROFILE,
    );
    registry.register_pack_with_profile(
        "virtual_teams",
        packs::virtual_teams::AGENTS,
        packs::virtual_teams::INVARIANTS,
        &packs::virtual_teams::PROFILE,
    );
    registry.register_pack_with_profile(
        "reskilling",
        packs::reskilling::AGENTS,
        packs::reskilling::INVARIANTS,
        &packs::reskilling::PROFILE,
    );

    registry.register_capability("web", "URL capture and metadata extraction");
    registry.register_capability("ocr", "Document understanding and receipt extraction");
    registry.register_capability("linkedin", "Professional network research");
    registry.register_capability("social", "Social profile extraction");
    registry.register_capability("patent", "Patent and IP search");

    registry
}

fn formation_selection_view(selection: &TruthFormationSelection) -> FormationSelectionView {
    FormationSelectionView {
        primary_template_id: selection.primary_template_id.clone(),
        alternate_template_ids: selection.alternate_template_ids.clone(),
        problem_class: format!("{:?}", selection.trace.problem_class).to_ascii_lowercase(),
        matched_keywords: selection.trace.matched_keywords.clone(),
        defaulted: selection.trace.defaulted,
        query_keywords: selection.trace.query_keywords.clone(),
        query_capabilities: selection
            .trace
            .query_capabilities
            .iter()
            .map(|capability| format!("{capability:?}").to_ascii_lowercase())
            .collect(),
        considered_template_ids: selection.trace.considered.clone(),
        primary_reason: selection.trace.primary_reason.clone(),
    }
}

fn operator_control_error(error: OperatorControlError) -> OperatorAppError {
    OperatorAppError::OperatorControl(error.to_string())
}

fn tally_escrow_release_packet() -> Result<JobReadinessPacket, OperatorControlError> {
    JobReadinessPacket::new(JobReadinessPacketInput {
        package_id: "axiom.truth-package.escrow-release.v0.12".to_string(),
        truth_version: "escrow-release.truths.v0.12".to_string(),
        domain_hint: "tally-escrow.release".to_string(),
        job_key: "escrow-release".to_string(),
        subject_ref: "tally://agreement/agreement-7/release/2026-05-19".to_string(),
        adapter_receipt_id: "artifact.observation_adapter.tally-release-2026-05-19".to_string(),
        adapter_status: AdapterReceiptStatus::Succeeded,
        verdict: Some(JobVerdict::Satisfied),
        authorizes_domain_action: false,
        evidence_status: vec![
            JobEvidenceStatus {
                clause_id: "jtbd.escrow-release.evidence.buyer_approval".to_string(),
                clause_key: "buyer_approval".to_string(),
                label: "buyer authorization is signed, on file, and non-revoked".to_string(),
                status: EvidenceReadinessStatus::Present,
                fact_ids: vec!["fact.tally.release.buyer-authorization".to_string()],
                evidence_refs: vec!["evidence:tally.signing-witness.acquirer".to_string()],
                trace_links: vec![
                    "trace:tally.release.truth.transition-requires-signature".to_string(),
                ],
                concern_record_ids: Vec::new(),
            },
            JobEvidenceStatus {
                clause_id: "jtbd.escrow-release.evidence.delivery_confirmed".to_string(),
                clause_key: "delivery_confirmed".to_string(),
                label: "vendor delivery is confirmed by release-condition evidence".to_string(),
                status: EvidenceReadinessStatus::Present,
                fact_ids: vec!["fact.tally.release.conditions-met".to_string()],
                evidence_refs: vec![
                    "evidence:tally.truth.release-requires-conditions-met".to_string(),
                ],
                trace_links: vec!["trace:tally.release.conditions".to_string()],
                concern_record_ids: Vec::new(),
            },
            JobEvidenceStatus {
                clause_id: "jtbd.escrow-release.evidence.compliance_cleared".to_string(),
                clause_key: "compliance_cleared".to_string(),
                label: "promotion authority observed the current release policy gate".to_string(),
                status: EvidenceReadinessStatus::Present,
                fact_ids: vec!["fact.tally.release.policy-gate-cleared".to_string()],
                evidence_refs: vec![
                    "evidence:converge.policy.sha256.tally-release-policy".to_string(),
                ],
                trace_links: vec!["trace:converge.gate.tally-release".to_string()],
                concern_record_ids: Vec::new(),
            },
            JobEvidenceStatus {
                clause_id: "jtbd.escrow-release.evidence.idempotency_key".to_string(),
                clause_key: "idempotency_key".to_string(),
                label: "release transition carries a unique idempotency record".to_string(),
                status: EvidenceReadinessStatus::Present,
                fact_ids: vec!["fact.tally.release.idempotency-agreement-7".to_string()],
                evidence_refs: vec![
                    "evidence:tally.transition.agreement-7.verified-released".to_string(),
                ],
                trace_links: vec!["trace:tally.transition.record".to_string()],
                concern_record_ids: Vec::new(),
            },
            JobEvidenceStatus {
                clause_id: "jtbd.escrow-release.evidence.disbursement_recorded".to_string(),
                clause_key: "disbursement_recorded".to_string(),
                label: "custody release receipt is recorded for the payment handoff".to_string(),
                status: EvidenceReadinessStatus::Present,
                fact_ids: vec!["fact.tally.release.custody-receipt".to_string()],
                evidence_refs: vec!["evidence:tally.release-receipt.attestation".to_string()],
                trace_links: vec!["trace:tally.custody.release-receipt".to_string()],
                concern_record_ids: Vec::new(),
            },
            JobEvidenceStatus {
                clause_id: "jtbd.escrow-release.failure.double_release".to_string(),
                clause_key: "double_release_guard".to_string(),
                label: "double-release failure guard is covered by the idempotency check"
                    .to_string(),
                status: EvidenceReadinessStatus::Present,
                fact_ids: vec!["fact.tally.release.double-release-guard".to_string()],
                evidence_refs: vec!["evidence:tally.idempotency.no-prior-promotion".to_string()],
                trace_links: vec!["trace:axiom.failure.double-release".to_string()],
                concern_record_ids: Vec::new(),
            },
        ],
        fuzzy_trace: None,
        verifier_forbidden_actions: vec![
            "do not release funds without active buyer authorization".to_string(),
            "do not release funds for a sanctioned recipient".to_string(),
            "do not release funds while the agreement has an open dispute".to_string(),
            "do not promote a second release for the same idempotency key".to_string(),
            "do not treat Helm readiness as payment-rail authority".to_string(),
        ],
        operator_actions: vec![
            "inspect the Axiom escrow-release report".to_string(),
            "review the Tally custody receipt and transition record".to_string(),
            "confirm Tally's transition guard remains the release authority".to_string(),
        ],
    })
}

fn quorum_adaptive_inquiry_packet() -> Result<JobReadinessPacket, OperatorControlError> {
    JobReadinessPacket::new(JobReadinessPacketInput {
        package_id: "axiom.truth-package.quorum-sense.v0.1".to_string(),
        truth_version: "quorum-sense.truths.v0.1".to_string(),
        domain_hint: "quorum-sense.inquiry".to_string(),
        job_key: "adaptive-inquiry".to_string(),
        subject_ref: "quorum://inquiry/org-transition-001/session/2026-05-20".to_string(),
        adapter_receipt_id: "artifact.observation_adapter.quorum-sense-2026-05-20".to_string(),
        adapter_status: AdapterReceiptStatus::Succeeded,
        verdict: Some(JobVerdict::Blocked),
        authorizes_domain_action: false,
        evidence_status: vec![
            JobEvidenceStatus {
                clause_id: "jtbd.adaptive-inquiry.evidence.core_question".to_string(),
                clause_key: "core_question_declared".to_string(),
                label: "operator-defined inquiry question anchors the sensemaking run".to_string(),
                status: EvidenceReadinessStatus::Present,
                fact_ids: vec!["fact.quorum.inquiry.core-question".to_string()],
                evidence_refs: vec!["evidence:quorum.inquiry-thread.created".to_string()],
                trace_links: vec!["trace:quorum.open-inquiry".to_string()],
                concern_record_ids: Vec::new(),
            },
            JobEvidenceStatus {
                clause_id: "jtbd.adaptive-inquiry.evidence.participant_consent".to_string(),
                clause_key: "participant_consent".to_string(),
                label: "participant consent scope is recorded before signals are used".to_string(),
                status: EvidenceReadinessStatus::Present,
                fact_ids: vec!["fact.quorum.participant.consent-scope".to_string()],
                evidence_refs: vec!["evidence:quorum.consent.workspace-review".to_string()],
                trace_links: vec!["trace:quorum.consent-gate".to_string()],
                concern_record_ids: Vec::new(),
            },
            JobEvidenceStatus {
                clause_id: "jtbd.adaptive-inquiry.evidence.signal_mass".to_string(),
                clause_key: "signal_mass".to_string(),
                label: "participant signals have enough mass to form initial hypotheses"
                    .to_string(),
                status: EvidenceReadinessStatus::Present,
                fact_ids: vec!["fact.quorum.signals.initial-mass".to_string()],
                evidence_refs: vec!["evidence:quorum.signal-batch.round-2".to_string()],
                trace_links: vec!["trace:quorum.signal-extraction".to_string()],
                concern_record_ids: Vec::new(),
            },
            JobEvidenceStatus {
                clause_id: "jtbd.adaptive-inquiry.evidence.adaptive_probe".to_string(),
                clause_key: "adaptive_probe_generated".to_string(),
                label: "next probe cites the hypothesis it is meant to test".to_string(),
                status: EvidenceReadinessStatus::Present,
                fact_ids: vec!["fact.quorum.probe.generated-from-hypothesis".to_string()],
                evidence_refs: vec!["evidence:quorum.probe.hypothesis-link".to_string()],
                trace_links: vec!["trace:quorum.probe-generation".to_string()],
                concern_record_ids: Vec::new(),
            },
            JobEvidenceStatus {
                clause_id: "jtbd.adaptive-inquiry.evidence.competing_hypotheses".to_string(),
                clause_key: "competing_hypotheses_preserved".to_string(),
                label: "minority hypothesis remains preserved instead of collapsed into consensus"
                    .to_string(),
                status: EvidenceReadinessStatus::Disputed,
                fact_ids: vec![
                    "fact.quorum.hypothesis.operations-risk".to_string(),
                    "fact.quorum.hypothesis.strategy-confusion".to_string(),
                ],
                evidence_refs: vec!["evidence:quorum.disagreement.round-2".to_string()],
                trace_links: vec!["trace:quorum.branch-synthesis.disagreement".to_string()],
                concern_record_ids: vec![
                    "operator.concern.quorum.competing-hypotheses".to_string(),
                ],
            },
            JobEvidenceStatus {
                clause_id: "jtbd.adaptive-inquiry.evidence.role_coverage".to_string(),
                clause_key: "role_coverage".to_string(),
                label: "role coverage is skewed; frontline and executive voices are not balanced"
                    .to_string(),
                status: EvidenceReadinessStatus::Concern,
                fact_ids: vec!["fact.quorum.coverage.middle-management-heavy".to_string()],
                evidence_refs: vec!["evidence:quorum.coverage.role-topology".to_string()],
                trace_links: vec!["trace:quorum.quorum-topology".to_string()],
                concern_record_ids: vec!["operator.concern.quorum.role-coverage".to_string()],
            },
            JobEvidenceStatus {
                clause_id: "jtbd.adaptive-inquiry.failure.false_consensus".to_string(),
                clause_key: "false_consensus_guard".to_string(),
                label: "false-consensus guard blocks quorum declaration before topology is sound"
                    .to_string(),
                status: EvidenceReadinessStatus::Present,
                fact_ids: vec!["fact.quorum.guard.false-consensus".to_string()],
                evidence_refs: vec!["evidence:quorum.truth.no-false-consensus".to_string()],
                trace_links: vec!["trace:axiom.failure.false-consensus".to_string()],
                concern_record_ids: Vec::new(),
            },
        ],
        fuzzy_trace: None,
        verifier_forbidden_actions: vec![
            "do not steer participants toward a predetermined conclusion".to_string(),
            "do not suppress minority hypotheses below quorum threshold".to_string(),
            "do not declare quorum without role coverage and dissent review".to_string(),
            "do not convert synthesis into organizational action without operator approval"
                .to_string(),
            "do not treat Helm readiness as inquiry or synthesis authority".to_string(),
        ],
        operator_actions: vec![
            "inspect the Quorum inquiry thread".to_string(),
            "review preserved disagreement and competing hypothesis citations".to_string(),
            "collect missing frontline and executive role coverage".to_string(),
            "choose whether to continue probing or hold synthesis for approval".to_string(),
        ],
    })
}

fn fathom_temporal_evidence_packet() -> Result<JobReadinessPacket, OperatorControlError> {
    JobReadinessPacket::new(JobReadinessPacketInput {
        package_id: "axiom.truth-package.fathom-narrative.v0.1".to_string(),
        truth_version: "fathom-narrative.truths.v0.1".to_string(),
        domain_hint: "fathom-narrative.temporal-evidence".to_string(),
        job_key: "temporal-filing-window".to_string(),
        subject_ref: "fathom://cik/0000320193/risk-factors/fy2025".to_string(),
        adapter_receipt_id: "artifact.observation_adapter.fathom-window-2026-05-20".to_string(),
        adapter_status: AdapterReceiptStatus::Succeeded,
        verdict: Some(JobVerdict::Blocked),
        authorizes_domain_action: false,
        evidence_status: vec![
            JobEvidenceStatus {
                clause_id: "jtbd.temporal-filing-window.evidence.corpus_snapshot".to_string(),
                clause_key: "corpus_snapshot".to_string(),
                label: "EDGAR corpus snapshot is recorded for the filing comparison".to_string(),
                status: EvidenceReadinessStatus::Present,
                fact_ids: vec!["fact.fathom.corpus.edgar-snapshot".to_string()],
                evidence_refs: vec!["evidence:fathom.edgar.snapshot.2026-05-20".to_string()],
                trace_links: vec!["trace:fathom.ingest.edgar".to_string()],
                concern_record_ids: Vec::new(),
            },
            JobEvidenceStatus {
                clause_id: "jtbd.temporal-filing-window.evidence.evidence_window".to_string(),
                clause_key: "evidence_window".to_string(),
                label: "prior and current risk-factor windows are bound to filing dates"
                    .to_string(),
                status: EvidenceReadinessStatus::Present,
                fact_ids: vec!["fact.fathom.window.2024-to-2025".to_string()],
                evidence_refs: vec!["evidence:fathom.window.10k-2024-2025".to_string()],
                trace_links: vec!["trace:fathom.temporal-window".to_string()],
                concern_record_ids: Vec::new(),
            },
            JobEvidenceStatus {
                clause_id: "jtbd.temporal-filing-window.evidence.risk_factor_drift".to_string(),
                clause_key: "risk_factor_drift".to_string(),
                label: "risk-factor language drift is material enough for analyst review"
                    .to_string(),
                status: EvidenceReadinessStatus::Concern,
                fact_ids: vec!["fact.fathom.risk-factor.language-drift".to_string()],
                evidence_refs: vec!["evidence:fathom.language-drift.jaccard".to_string()],
                trace_links: vec!["trace:fathom.suggestor.risk-factor-language".to_string()],
                concern_record_ids: vec!["operator.concern.fathom.material-drift".to_string()],
            },
            JobEvidenceStatus {
                clause_id: "jtbd.temporal-filing-window.evidence.segment_consistency".to_string(),
                clause_key: "segment_consistency".to_string(),
                label: "segment narrative and reported numbers have not been reconciled"
                    .to_string(),
                status: EvidenceReadinessStatus::Disputed,
                fact_ids: vec![
                    "fact.fathom.segment.narrative-growth".to_string(),
                    "fact.fathom.segment.reported-decline".to_string(),
                ],
                evidence_refs: vec!["evidence:fathom.segment-disagreement".to_string()],
                trace_links: vec!["trace:fathom.disagreement-map".to_string()],
                concern_record_ids: vec!["operator.concern.fathom.segment-mismatch".to_string()],
            },
            JobEvidenceStatus {
                clause_id: "jtbd.temporal-filing-window.failure.early_collapse".to_string(),
                clause_key: "no_early_collapse_guard".to_string(),
                label: "conflicting filing perspectives remain separate before promotion"
                    .to_string(),
                status: EvidenceReadinessStatus::Present,
                fact_ids: vec!["fact.fathom.guard.no-early-collapse".to_string()],
                evidence_refs: vec!["evidence:fathom.truth.perspective-separation".to_string()],
                trace_links: vec!["trace:axiom.failure.early-collapse".to_string()],
                concern_record_ids: Vec::new(),
            },
        ],
        fuzzy_trace: None,
        verifier_forbidden_actions: vec![
            "do not promote a narrative claim without source filing windows".to_string(),
            "do not collapse conflicting segment evidence into one summary".to_string(),
            "do not treat stale filings as current evidence".to_string(),
            "do not publish analyst conclusions before review".to_string(),
            "do not treat Helm readiness as narrative authority".to_string(),
        ],
        operator_actions: vec![
            "inspect the Fathom corpus snapshot and evidence window".to_string(),
            "review risk-factor language drift before promotion".to_string(),
            "resolve segment narrative disagreement with analyst notes".to_string(),
        ],
    })
}

fn warden_compliance_packet() -> Result<JobReadinessPacket, OperatorControlError> {
    JobReadinessPacket::new(JobReadinessPacketInput {
        package_id: "axiom.truth-package.warden-compliance.v0.1".to_string(),
        truth_version: "warden-compliance.truths.v0.1".to_string(),
        domain_hint: "warden-compliance.audit".to_string(),
        job_key: "compliance-gate-review".to_string(),
        subject_ref: "warden://audit-pack/cross-framework/week-20".to_string(),
        adapter_receipt_id: "artifact.observation_adapter.warden-audit-2026-05-20".to_string(),
        adapter_status: AdapterReceiptStatus::Succeeded,
        verdict: Some(JobVerdict::Blocked),
        authorizes_domain_action: false,
        evidence_status: vec![
            JobEvidenceStatus {
                clause_id: "jtbd.compliance-gate-review.evidence.rule_registry".to_string(),
                clause_key: "rule_registry".to_string(),
                label: "GDPR, SOC2, HIPAA, and PCI-DSS rule registry is loaded".to_string(),
                status: EvidenceReadinessStatus::Present,
                fact_ids: vec!["fact.warden.rules.loaded".to_string()],
                evidence_refs: vec!["evidence:warden.rule-registry.week-20".to_string()],
                trace_links: vec!["trace:warden.rule-load".to_string()],
                concern_record_ids: Vec::new(),
            },
            JobEvidenceStatus {
                clause_id: "jtbd.compliance-gate-review.evidence.document_stream".to_string(),
                clause_key: "document_stream".to_string(),
                label: "cross-app document stream is inspected by the compliance gate".to_string(),
                status: EvidenceReadinessStatus::Present,
                fact_ids: vec!["fact.warden.documents.inspected-6".to_string()],
                evidence_refs: vec!["evidence:warden.document-stream.fixture".to_string()],
                trace_links: vec!["trace:warden.document-inspection".to_string()],
                concern_record_ids: Vec::new(),
            },
            JobEvidenceStatus {
                clause_id: "jtbd.compliance-gate-review.evidence.verdict_log".to_string(),
                clause_key: "verdict_log".to_string(),
                label: "seven compliance verdicts were emitted across regulated apps".to_string(),
                status: EvidenceReadinessStatus::Present,
                fact_ids: vec!["fact.warden.verdicts.emitted-7".to_string()],
                evidence_refs: vec!["evidence:warden.verdict-log.2026-05-19".to_string()],
                trace_links: vec!["trace:warden.compliance-gate".to_string()],
                concern_record_ids: Vec::new(),
            },
            JobEvidenceStatus {
                clause_id: "jtbd.compliance-gate-review.evidence.shadow_rule".to_string(),
                clause_key: "shadow_rule_diff".to_string(),
                label: "PCI-DSS shadow rule would newly block legacy card records".to_string(),
                status: EvidenceReadinessStatus::Concern,
                fact_ids: vec!["fact.warden.shadow-rule.newly-blocked-2".to_string()],
                evidence_refs: vec!["evidence:warden.shadow-diff.pci-card-data".to_string()],
                trace_links: vec!["trace:warden.shadow-runner".to_string()],
                concern_record_ids: vec!["operator.concern.warden.pci-remediation".to_string()],
            },
            JobEvidenceStatus {
                clause_id: "jtbd.compliance-gate-review.failure.clean_attestation".to_string(),
                clause_key: "clean_attestation_guard".to_string(),
                label: "clean attestation is blocked while remediation is still required"
                    .to_string(),
                status: EvidenceReadinessStatus::Blocked,
                fact_ids: vec!["fact.warden.remediation.required".to_string()],
                evidence_refs: vec!["evidence:warden.audit-pack.cross-framework".to_string()],
                trace_links: vec!["trace:axiom.failure.clean-attestation".to_string()],
                concern_record_ids: vec!["operator.concern.warden.clean-attestation".to_string()],
            },
        ],
        fuzzy_trace: None,
        verifier_forbidden_actions: vec![
            "do not mark compliance clean while block verdicts remain open".to_string(),
            "do not roll out a candidate rule without shadow-run review".to_string(),
            "do not hide framework citations from the audit pack".to_string(),
            "do not let Helm override compliance remediation authority".to_string(),
        ],
        operator_actions: vec![
            "inspect the Warden verdict log and framework citations".to_string(),
            "review the PCI-DSS shadow diff before rule rollout".to_string(),
            "assign remediation for newly blocked records".to_string(),
        ],
    })
}

fn plumb_execution_drift_packet() -> Result<JobReadinessPacket, OperatorControlError> {
    JobReadinessPacket::new(JobReadinessPacketInput {
        package_id: "axiom.truth-package.plumb-execution.v0.1".to_string(),
        truth_version: "plumb-execution.truths.v0.1".to_string(),
        domain_hint: "plumb-execution.drift".to_string(),
        job_key: "strategy-drift-review".to_string(),
        subject_ref: "plumb://strategy/fy2026-growth/version/3".to_string(),
        adapter_receipt_id: "artifact.observation_adapter.plumb-drift-2026-05-20".to_string(),
        adapter_status: AdapterReceiptStatus::Succeeded,
        verdict: Some(JobVerdict::Blocked),
        authorizes_domain_action: false,
        evidence_status: vec![
            JobEvidenceStatus {
                clause_id: "jtbd.strategy-drift-review.evidence.strategy_anchor".to_string(),
                clause_key: "strategy_anchor".to_string(),
                label: "versioned strategy anchor is recorded before drift is measured".to_string(),
                status: EvidenceReadinessStatus::Present,
                fact_ids: vec!["fact.plumb.strategy.anchor-v3".to_string()],
                evidence_refs: vec!["evidence:plumb.strategy.version-history".to_string()],
                trace_links: vec!["trace:plumb.anchor".to_string()],
                concern_record_ids: Vec::new(),
            },
            JobEvidenceStatus {
                clause_id: "jtbd.strategy-drift-review.evidence.execution_signals".to_string(),
                clause_key: "execution_signals".to_string(),
                label: "operating telemetry is attached to the current strategy review".to_string(),
                status: EvidenceReadinessStatus::Present,
                fact_ids: vec!["fact.plumb.execution.telemetry-week-20".to_string()],
                evidence_refs: vec!["evidence:plumb.execution-signal.batch".to_string()],
                trace_links: vec!["trace:plumb.signal-ingest".to_string()],
                concern_record_ids: Vec::new(),
            },
            JobEvidenceStatus {
                clause_id: "jtbd.strategy-drift-review.evidence.drift_verdict".to_string(),
                clause_key: "drift_verdict".to_string(),
                label: "material drift verdict cites source signals and fuzzy severity".to_string(),
                status: EvidenceReadinessStatus::Concern,
                fact_ids: vec!["fact.plumb.drift.materializing".to_string()],
                evidence_refs: vec!["evidence:plumb.drift-verdict.fuzzy".to_string()],
                trace_links: vec!["trace:plumb.drift-detector".to_string()],
                concern_record_ids: vec!["operator.concern.plumb.material-drift".to_string()],
            },
            JobEvidenceStatus {
                clause_id: "jtbd.strategy-drift-review.evidence.adversarial_review".to_string(),
                clause_key: "adversarial_review".to_string(),
                label: "adversarial review has not yet challenged the proposed correction"
                    .to_string(),
                status: EvidenceReadinessStatus::Missing,
                fact_ids: Vec::new(),
                evidence_refs: Vec::new(),
                trace_links: vec!["trace:plumb.adversarial-review.pending".to_string()],
                concern_record_ids: vec![
                    "operator.concern.plumb.adversarial-review-missing".to_string(),
                ],
            },
            JobEvidenceStatus {
                clause_id: "jtbd.strategy-drift-review.failure.ungated_revision".to_string(),
                clause_key: "ungated_revision_guard".to_string(),
                label: "strategy revision cannot commit before promotion gates pass".to_string(),
                status: EvidenceReadinessStatus::Blocked,
                fact_ids: vec!["fact.plumb.guard.no-ungated-revision".to_string()],
                evidence_refs: vec!["evidence:plumb.truth.revision-gate".to_string()],
                trace_links: vec!["trace:axiom.failure.ungated-revision".to_string()],
                concern_record_ids: Vec::new(),
            },
        ],
        fuzzy_trace: Some(FuzzyReadinessTrace {
            variable_key: "drift_severity".to_string(),
            observed_value_basis_points: 6_200,
            memberships: vec![
                FuzzyMembership {
                    label: "negligible".to_string(),
                    score_basis_points: 0,
                },
                FuzzyMembership {
                    label: "minor".to_string(),
                    score_basis_points: 0,
                },
                FuzzyMembership {
                    label: "moderate".to_string(),
                    score_basis_points: 4_000,
                },
                FuzzyMembership {
                    label: "material".to_string(),
                    score_basis_points: 3_500,
                },
                FuzzyMembership {
                    label: "critical".to_string(),
                    score_basis_points: 0,
                },
            ],
            activated_rules: vec![FuzzyRuleActivation {
                rule_id: "revision-trigger-on-materializing-drift".to_string(),
                strength_basis_points: 3_500,
                conclusion: "revision_urgency:advisable".to_string(),
            }],
            defuzzified_score: Some(FuzzyDefuzzifiedScore {
                method: "centroid".to_string(),
                score_basis_points: 3_750,
                domain_min_basis_points: 0,
                domain_max_basis_points: 10_000,
                domain_steps: 1_000,
            }),
        }),
        verifier_forbidden_actions: vec![
            "do not overwrite the strategy anchor".to_string(),
            "do not commit a revision without adversarial review".to_string(),
            "do not treat drift detection as revision authority".to_string(),
            "do not let Helm close the execution loop for the app".to_string(),
        ],
        operator_actions: vec![
            "inspect the Plumb drift verdict and cited source signals".to_string(),
            "request adversarial review for the proposed correction".to_string(),
            "hold strategy revision until promotion gates pass".to_string(),
        ],
    })
}

fn atlas_integration_packet() -> Result<JobReadinessPacket, OperatorControlError> {
    JobReadinessPacket::new(JobReadinessPacketInput {
        package_id: "axiom.truth-package.atlas-integration.v0.1".to_string(),
        truth_version: "atlas-integration.truths.v0.1".to_string(),
        domain_hint: "atlas-integration.integration-room".to_string(),
        job_key: "integration-candidate-review".to_string(),
        subject_ref: "atlas://integration-room/omnistack-ai/candidate/shared-identity-core"
            .to_string(),
        adapter_receipt_id: "artifact.observation_adapter.atlas-candidate-2026-05-20".to_string(),
        adapter_status: AdapterReceiptStatus::Succeeded,
        verdict: Some(JobVerdict::Blocked),
        authorizes_domain_action: false,
        evidence_status: vec![
            JobEvidenceStatus {
                clause_id: "jtbd.integration-candidate-review.evidence.repository_profiles"
                    .to_string(),
                clause_key: "repository_profiles".to_string(),
                label: "two acquired repositories have owner and capability profiles".to_string(),
                status: EvidenceReadinessStatus::Present,
                fact_ids: vec!["fact.atlas.repositories.profiled".to_string()],
                evidence_refs: vec!["evidence:atlas.repo-profile.seed-room".to_string()],
                trace_links: vec!["trace:atlas.map-repositories".to_string()],
                concern_record_ids: Vec::new(),
            },
            JobEvidenceStatus {
                clause_id: "jtbd.integration-candidate-review.evidence.reviewable_evidence"
                    .to_string(),
                clause_key: "reviewable_evidence".to_string(),
                label: "identity overlap claim cites AST shape and contract-test evidence"
                    .to_string(),
                status: EvidenceReadinessStatus::Present,
                fact_ids: vec!["fact.atlas.identity-overlap.candidate".to_string()],
                evidence_refs: vec![
                    "evidence:atlas.ast.jwt-shape".to_string(),
                    "evidence:atlas.contract.oidc-test".to_string(),
                ],
                trace_links: vec!["trace:atlas.evidence-graph".to_string()],
                concern_record_ids: Vec::new(),
            },
            JobEvidenceStatus {
                clause_id: "jtbd.integration-candidate-review.evidence.integration_strength"
                    .to_string(),
                clause_key: "integration_strength".to_string(),
                label: "similarity is high but remains a candidate until bounded proof lands"
                    .to_string(),
                status: EvidenceReadinessStatus::Concern,
                fact_ids: vec!["fact.atlas.similarity.845bps".to_string()],
                evidence_refs: vec!["evidence:atlas.similarity.prism-planned".to_string()],
                trace_links: vec!["trace:atlas.candidate-scoring".to_string()],
                concern_record_ids: vec![
                    "operator.concern.atlas.bounded-proof-missing".to_string(),
                ],
            },
            JobEvidenceStatus {
                clause_id: "jtbd.integration-candidate-review.evidence.owner_approval".to_string(),
                clause_key: "owner_approval".to_string(),
                label: "named repository owners have not approved writeback".to_string(),
                status: EvidenceReadinessStatus::Missing,
                fact_ids: Vec::new(),
                evidence_refs: Vec::new(),
                trace_links: vec!["trace:atlas.owner-gate.pending".to_string()],
                concern_record_ids: vec!["operator.concern.atlas.owner-approval".to_string()],
            },
            JobEvidenceStatus {
                clause_id: "jtbd.integration-candidate-review.failure.writeback".to_string(),
                clause_key: "writeback_guard".to_string(),
                label: "customer repository writeback is blocked until owner and policy gates pass"
                    .to_string(),
                status: EvidenceReadinessStatus::Blocked,
                fact_ids: vec!["fact.atlas.guard.no-writeback".to_string()],
                evidence_refs: vec![
                    "evidence:atlas.truth.owner-approval-before-writeback".to_string(),
                ],
                trace_links: vec!["trace:axiom.failure.writeback".to_string()],
                concern_record_ids: Vec::new(),
            },
        ],
        fuzzy_trace: None,
        verifier_forbidden_actions: vec![
            "do not treat similarity score as integration authority".to_string(),
            "do not write to a customer repository without named owner approval".to_string(),
            "do not merge bounded-proof and reviewer evidence into one score".to_string(),
            "do not let Helm bypass Atlas integration gates".to_string(),
        ],
        operator_actions: vec![
            "inspect Atlas evidence graph for the shared identity candidate".to_string(),
            "collect owner approval before any writeback".to_string(),
            "request bounded proof for the integration-strength claim".to_string(),
        ],
    })
}

fn operator_control_preview_from_packet(
    sequence: u64,
    packet: JobReadinessPacket,
    summary: &str,
) -> Result<OperatorControlPreview, OperatorControlError> {
    let ledger_entry = job_readiness_packet_ledger_entry(
        sequence,
        &packet,
        vec![packet.adapter_receipt_id.clone()],
        summary,
    )?;

    Ok(OperatorControlPreview {
        packet,
        ledger_entries: vec![ledger_entry],
        receipt_families: operator_receipt_families(),
    })
}

fn operator_receipt_families() -> Vec<OperatorReceiptFamilyView> {
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

impl<S> OperatorApp<S>
where
    S: KernelStore,
{
    #[must_use]
    pub fn new(config: AppConfig, store: S) -> Self {
        Self {
            store,
            config,
            default_actor: Actor {
                actor_id: "operator-ui".to_string(),
                display_name: "Operator UI".to_string(),
                kind: ActorKind::Human,
            },
            metadata: Arc::new(RwLock::new(RuntimeMetadata::default())),
            organism_registry: Arc::new(default_organism_registry()),
        }
    }

    #[must_use]
    pub fn with_default_actor(mut self, actor: Actor) -> Self {
        self.default_actor = actor;
        self
    }

    pub fn system_profile(&self) -> SystemProfile {
        SystemProfile {
            config: self.config.clone(),
            modules: all_modules(),
            feature_toggles: FeatureToggles {
                analytics_enabled: self.config.converge.analytics_enabled,
                optimization_enabled: self.config.converge.optimization_enabled,
                llm_enabled: self.config.converge.llm_enabled,
                runtime_modules: self.config.converge.runtime_modules.clone(),
                supported_truth_keys: supported_truth_keys(),
            },
        }
    }

    pub fn operator_dashboard(&self) -> OperatorAppResult<OperatorDashboard> {
        Ok(OperatorDashboard {
            jobs: self.list_truths(),
            approvals: self.list_approvals(ApprovalFilter::default())?,
            exceptions: self.list_workflow_cases(WorkflowCaseFilter {
                states: vec![
                    WorkflowState::AwaitingApproval,
                    WorkflowState::WaitingExternal,
                    WorkflowState::Blocked,
                ],
                ..WorkflowCaseFilter::default()
            })?,
            recent_timeline: self.list_timeline(None, 12)?,
        })
    }

    pub fn operator_control_preview(&self) -> OperatorAppResult<OperatorControlPreview> {
        let packet = tally_escrow_release_packet().map_err(operator_control_error)?;
        operator_control_preview_from_packet(
            0,
            packet,
            "Tally escrow-release readiness is satisfied; Helm records no release authority",
        )
        .map_err(operator_control_error)
    }

    pub fn operator_control_previews(&self) -> OperatorAppResult<Vec<OperatorControlPreview>> {
        let quorum = quorum_adaptive_inquiry_packet().map_err(operator_control_error)?;
        let quorum = operator_control_preview_from_packet(
            1,
            quorum,
            "Quorum inquiry is blocked on role coverage and unresolved dissent; Helm records no synthesis authority",
        )
        .map_err(operator_control_error)?;
        let fathom = fathom_temporal_evidence_packet().map_err(operator_control_error)?;
        let fathom = operator_control_preview_from_packet(
            2,
            fathom,
            "Fathom temporal evidence is blocked on analyst review; Helm records no narrative authority",
        )
        .map_err(operator_control_error)?;
        let warden = warden_compliance_packet().map_err(operator_control_error)?;
        let warden = operator_control_preview_from_packet(
            3,
            warden,
            "Warden audit is blocked on remediation; Helm records no compliance override authority",
        )
        .map_err(operator_control_error)?;
        let plumb = plumb_execution_drift_packet().map_err(operator_control_error)?;
        let plumb = operator_control_preview_from_packet(
            4,
            plumb,
            "Plumb drift review is blocked before adversarial review; Helm records no revision authority",
        )
        .map_err(operator_control_error)?;
        let atlas = atlas_integration_packet().map_err(operator_control_error)?;
        let atlas = operator_control_preview_from_packet(
            5,
            atlas,
            "Atlas integration candidate is blocked before owner approval; Helm records no writeback authority",
        )
        .map_err(operator_control_error)?;

        Ok(vec![
            self.operator_control_preview()?,
            quorum,
            fathom,
            warden,
            plumb,
            atlas,
        ])
    }

    #[must_use]
    pub fn list_truths(&self) -> Vec<TruthListItem> {
        let mut items = all_truths()
            .into_iter()
            .map(|truth| TruthListItem {
                key: truth.key.to_string(),
                display_name: truth.display_name.to_string(),
                kind: truth.kind,
                summary: truth.summary.to_string(),
                packs: display_pack_names_for_truth(truth.key, &self.organism_registry)
                    .unwrap_or_else(|| {
                        converge_binding_for_truth(truth.key)
                            .map(|binding| {
                                binding
                                    .pack_ids
                                    .into_iter()
                                    .map(ToString::to_string)
                                    .collect()
                            })
                            .unwrap_or_default()
                    }),
                executable: is_truth_supported(truth.key),
            })
            .collect::<Vec<_>>();
        items.sort_by(|left, right| {
            right
                .executable
                .cmp(&left.executable)
                .then_with(|| left.display_name.cmp(&right.display_name))
        });
        items
    }

    #[must_use]
    pub fn truth_detail(&self, key: &str) -> Option<TruthDetailItem> {
        let truth = find_truth(key)?;
        let compiled_intent = compile_intent_for_truth(&truth).ok();
        let axiom_intent = compiled_intent.as_ref().map(|intent| AxiomIntentView {
            intent_id: intent.id.to_string(),
            outcome: intent.outcome.clone(),
            context: intent.context.clone(),
            constraints: intent.constraints.clone(),
            authority: intent.authority.clone(),
            forbidden_count: intent.forbidden.len(),
            reversibility: format!("{:?}", intent.reversibility).to_ascii_lowercase(),
            expires_at: intent.expires.to_rfc3339(),
            expiry_action: format!("{:?}", intent.expiry_action).to_ascii_lowercase(),
        });
        let formation_selection = compiled_intent.as_ref().and_then(|intent| {
            select_formation_for_intent(intent, &default_helms_capabilities())
                .ok()
                .map(|selection| formation_selection_view(&selection))
        });
        let organism_resolution = truth_catalog::organism_binding_for_truth(
            truth.key,
            &self.organism_registry,
        )
        .map(|binding| {
            let truth_catalog::TruthOrganismBinding {
                truth_key,
                blueprint,
                binding,
                readiness,
            } = binding;
            let levels_attempted = binding
                .resolution
                .levels_attempted
                .iter()
                .map(|level| format!("{level:?}").to_ascii_lowercase())
                .collect();
            let levels_contributed = binding
                .resolution
                .levels_contributed
                .iter()
                .map(|level| format!("{level:?}").to_ascii_lowercase())
                .collect();
            let completeness_confidence_bps =
                (binding.resolution.completeness_confidence.as_f64() * 10_000.0).round() as u16;

            OrganismTruthResolutionView {
                truth_key: truth_key.to_string(),
                blueprint: blueprint.map(str::to_string),
                packs: binding
                    .packs
                    .into_iter()
                    .map(|pack| OrganismPackRequirementView {
                        pack_name: pack.pack_name,
                        reason: pack.reason,
                        confidence_bps: (pack.confidence.as_f64() * 10_000.0).round() as u16,
                        source: format!("{:?}", pack.source).to_ascii_lowercase(),
                    })
                    .collect(),
                capabilities: binding
                    .capabilities
                    .into_iter()
                    .map(|capability| OrganismCapabilityRequirementView {
                        capability: capability.capability.into_inner(),
                        reason: capability.reason,
                        confidence_bps: (capability.confidence.as_f64() * 10_000.0).round() as u16,
                        source: format!("{:?}", capability.source).to_ascii_lowercase(),
                    })
                    .collect(),
                invariants: binding.invariants.into_iter().map(Into::into).collect(),
                levels_attempted,
                levels_contributed,
                completeness_confidence_bps,
                readiness: TruthReadinessView {
                    ready: readiness.ready,
                    confirmed: readiness
                        .confirmed
                        .into_iter()
                        .map(|item| TruthReadinessConfirmationView {
                            resource: item.resource,
                            kind: format!("{:?}", item.kind).to_ascii_lowercase(),
                            detail: item.detail,
                        })
                        .collect(),
                    gaps: readiness
                        .gaps
                        .into_iter()
                        .map(|gap| TruthReadinessGapView {
                            resource: gap.resource,
                            kind: format!("{:?}", gap.kind).to_ascii_lowercase(),
                            severity: format!("{:?}", gap.severity).to_ascii_lowercase(),
                            reason: gap.reason,
                            suggestion: gap.suggestion,
                        })
                        .collect(),
                },
            }
        });
        let converge_resolution =
            truth_catalog::converge_binding_for_truth(truth.key).map(|binding| {
                let intent_kind = binding.intent_kind_name().to_string();
                let required_success_criteria = binding.required_success_criteria();
                let hard_constraints = binding.hard_constraints();
                ConvergeTruthResolutionView {
                    truth_key: binding.truth_key.to_string(),
                    runtime: binding.runtime.to_string(),
                    pack_ids: binding.pack_ids.into_iter().map(str::to_string).collect(),
                    approval_points: binding
                        .approval_points
                        .into_iter()
                        .map(str::to_string)
                        .collect(),
                    intent_kind,
                    request: binding.intent.request,
                    required_success_criteria,
                    hard_constraints,
                }
            });

        Some(TruthDetailItem {
            key: truth.key.to_string(),
            display_name: truth.display_name.to_string(),
            kind: truth.kind,
            summary: truth.summary.to_string(),
            feature_path: truth.feature_path.to_string(),
            actor_roles: truth
                .actor_roles
                .iter()
                .map(|value| value.to_string())
                .collect(),
            approval_points: truth
                .approval_points
                .iter()
                .map(|value| value.to_string())
                .collect(),
            desired_outcomes: truth
                .desired_outcomes
                .iter()
                .map(|value| value.to_string())
                .collect(),
            guardrails: truth
                .guardrails
                .iter()
                .map(|value| value.to_string())
                .collect(),
            modules: truth
                .modules
                .iter()
                .map(|touch| TruthModuleTouchItem {
                    module_key: touch.module_key.to_string(),
                    responsibility: touch.responsibility.to_string(),
                })
                .collect(),
            gherkin: truth.gherkin.to_string(),
            packs: display_pack_names_for_truth(truth.key, &self.organism_registry).unwrap_or_else(
                || {
                    converge_binding_for_truth(truth.key)
                        .map(|binding| binding.pack_ids.into_iter().map(str::to_string).collect())
                        .unwrap_or_default()
                },
            ),
            executable: is_truth_supported(truth.key),
            axiom_intent,
            formation_selection,
            organism_resolution,
            converge_resolution,
        })
    }

    #[must_use]
    pub fn workbench_apps(&self) -> Vec<WorkbenchAppManifest> {
        built_in_workbench_apps()
    }

    pub fn execute_truth(
        &self,
        key: &str,
        inputs: HashMap<String, String>,
    ) -> OperatorAppResult<TruthExecutionSession> {
        let truth =
            find_truth(key).ok_or_else(|| OperatorAppError::TruthNotFound(key.to_string()))?;
        if !is_truth_supported(truth.key) {
            return Ok(unsupported_truth_session(truth));
        }

        match key {
            QUALIFY_INBOUND_LEAD => self.execute_qualify_inbound_lead(truth, inputs),
            SUBMIT_EXPENSE_REPORT => self.execute_submit_expense_report(truth, inputs),
            ACTIVATE_SUBSCRIPTION => self.execute_activate_subscription(truth, inputs),
            REFILL_PREPAID_AI_CREDITS => self.execute_refill_prepaid_ai_credits(truth, inputs),
            _ => Ok(unsupported_truth_session(truth)),
        }
    }

    pub fn list_organizations(&self) -> OperatorAppResult<Vec<OrganizationListItem>> {
        self.store
            .read(|kernel| {
                kernel
                    .list_organizations()
                    .into_iter()
                    .map(|organization| {
                        let people_count = kernel
                            .people
                            .values()
                            .filter(|person| person.organization_id == Some(organization.id))
                            .count();
                        let open_opportunity_count = kernel
                            .opportunities
                            .values()
                            .filter(|opportunity| {
                                opportunity.organization_id == organization.id
                                    && !matches!(
                                        opportunity.stage,
                                        OpportunityStage::ClosedWon | OpportunityStage::ClosedLost
                                    )
                            })
                            .count();
                        OrganizationListItem {
                            id: organization.id.to_string(),
                            name: organization.name,
                            lifecycle: organization.lifecycle,
                            website: organization.website,
                            owner_user_id: organization.owner_user_id,
                            tags: organization.tags,
                            people_count,
                            open_opportunity_count,
                            updated_at: organization.updated_at,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .map_err(Into::into)
    }

    pub fn list_opportunities(&self) -> OperatorAppResult<Vec<OpportunityListItem>> {
        self.store
            .read(|kernel| {
                kernel
                    .list_opportunities(None)
                    .into_iter()
                    .map(|opportunity| OpportunityListItem {
                        id: opportunity.id.to_string(),
                        organization_id: opportunity.organization_id.to_string(),
                        organization_name: kernel
                            .organizations
                            .get(&opportunity.organization_id)
                            .map(|organization| organization.name.clone())
                            .unwrap_or_else(|| "Unknown organization".to_string()),
                        name: opportunity.name,
                        stage: opportunity.stage,
                        value_minor: opportunity.value.amount_minor,
                        currency_code: opportunity.value.currency_code,
                        confidence_bps: opportunity.confidence_bps,
                        next_step: opportunity.next_step,
                        updated_at: opportunity.updated_at,
                    })
                    .collect::<Vec<_>>()
            })
            .map_err(Into::into)
    }

    pub fn list_subscriptions(
        &self,
        organization_id: Option<&str>,
    ) -> OperatorAppResult<Vec<SubscriptionListItem>> {
        let organization_id = organization_id
            .map(|value| parse_uuid("organization_id", value))
            .transpose()?;

        self.store
            .read(|kernel| {
                kernel
                    .list_subscriptions(organization_id)
                    .into_iter()
                    .map(|subscription| {
                        let organization_name = kernel
                            .organizations
                            .get(&subscription.organization_id)
                            .map(|organization| organization.name.clone())
                            .unwrap_or_else(|| "Unknown organization".to_string());
                        let catalog_item_name =
                            subscription.catalog_item_id.and_then(|catalog_item_id| {
                                kernel
                                    .catalog_items
                                    .get(&catalog_item_id)
                                    .map(|catalog_item| catalog_item.name.clone())
                            });

                        SubscriptionListItem {
                            id: subscription.id.to_string(),
                            organization_id: subscription.organization_id.to_string(),
                            organization_name,
                            status: subscription.status,
                            catalog_item_id: subscription.catalog_item_id.map(|id| id.to_string()),
                            catalog_item_name,
                            value_minor: subscription.value.amount_minor,
                            currency_code: subscription.value.currency_code,
                            started_at: subscription.started_at,
                            activated_at: subscription.activated_at,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .map_err(Into::into)
    }

    pub fn list_catalog_items(
        &self,
        active_only: bool,
    ) -> OperatorAppResult<Vec<CatalogItemListItem>> {
        self.store
            .read(|kernel| {
                kernel
                    .list_catalog_items(active_only)
                    .into_iter()
                    .map(|catalog_item| CatalogItemListItem {
                        id: catalog_item.id.to_string(),
                        sku: catalog_item.sku,
                        name: catalog_item.name,
                        description: catalog_item.description,
                        plan_kind: catalog_plan_kind_name(catalog_item.plan_kind).to_string(),
                        active: catalog_item.active,
                        billing_period: catalog_item
                            .pricing
                            .as_ref()
                            .map(|pricing| billing_period_name(pricing.billing_period).to_string()),
                        price_minor: catalog_item
                            .pricing
                            .as_ref()
                            .map(|pricing| pricing.list_price.amount_minor),
                        currency_code: catalog_item
                            .pricing
                            .as_ref()
                            .map(|pricing| pricing.list_price.currency_code.clone()),
                        entitlements_summary: entitlement_template_summary(
                            &catalog_item.entitlement_template,
                        ),
                    })
                    .collect::<Vec<_>>()
            })
            .map_err(Into::into)
    }

    pub fn account_summary(&self, org_id: &str) -> OperatorAppResult<AccountWorkspaceSummary> {
        let organization_id = parse_uuid("org_id", org_id)?;
        let summary = self
            .store
            .read(|kernel| {
                let summary = kernel.get_account_summary(organization_id, 12)?;
                let subscriptions = kernel
                    .orders
                    .values()
                    .filter(|subscription| subscription.organization_id == organization_id)
                    .map(|subscription| SubscriptionListItem {
                        id: subscription.id.to_string(),
                        organization_id: subscription.organization_id.to_string(),
                        organization_name: summary.organization.name.clone(),
                        status: subscription.status,
                        catalog_item_id: subscription.catalog_item_id.map(|id| id.to_string()),
                        catalog_item_name: subscription.catalog_item_id.and_then(
                            |catalog_item_id| {
                                kernel
                                    .catalog_items
                                    .get(&catalog_item_id)
                                    .map(|catalog_item| catalog_item.name.clone())
                            },
                        ),
                        value_minor: subscription.value.amount_minor,
                        currency_code: subscription.value.currency_code.clone(),
                        started_at: subscription.started_at,
                        activated_at: subscription.activated_at,
                    })
                    .collect::<Vec<_>>();
                let entitlements = kernel
                    .entitlements
                    .values()
                    .filter(|entitlement| entitlement.organization_id == organization_id)
                    .map(|entitlement| EntitlementListItem {
                        id: entitlement.id.to_string(),
                        subscription_id: entitlement.subscription_id.to_string(),
                        key: entitlement.key.clone(),
                        value_summary: entitlement_value_summary(&entitlement.value),
                        created_at: entitlement.created_at,
                    })
                    .collect::<Vec<_>>();
                Ok::<_, application_kernel::KernelError>((summary, subscriptions, entitlements))
            })?
            .map_err(StorageError::from)?;
        let (summary, subscriptions, entitlements) = summary;
        let organization = summary.organization;
        let organization_name = organization.name.clone();

        Ok(AccountWorkspaceSummary {
            organization: OrganizationWorkspaceItem {
                id: organization.id.to_string(),
                name: organization.name,
                lifecycle: organization.lifecycle,
                website: organization.website,
                industry: organization.industry,
                owner_user_id: organization.owner_user_id,
                tags: organization.tags,
            },
            people: summary
                .contacts
                .into_iter()
                .map(|person| PersonWorkspaceItem {
                    id: person.id.to_string(),
                    full_name: person.full_name,
                    title: person.title,
                    email: person.email,
                    phone: person.phone,
                })
                .collect(),
            opportunities: summary
                .opportunities
                .into_iter()
                .map(|opportunity| OpportunityListItem {
                    id: opportunity.id.to_string(),
                    organization_id: opportunity.organization_id.to_string(),
                    organization_name: organization_name.clone(),
                    name: opportunity.name,
                    stage: opportunity.stage,
                    value_minor: opportunity.value.amount_minor,
                    currency_code: opportunity.value.currency_code,
                    confidence_bps: opportunity.confidence_bps,
                    next_step: opportunity.next_step,
                    updated_at: opportunity.updated_at,
                })
                .collect(),
            subscriptions,
            entitlements,
            recent_timeline: summary
                .recent_timeline
                .into_iter()
                .map(timeline_event_item)
                .collect(),
        })
    }

    pub fn list_timeline(
        &self,
        anchor: Option<RecordReferenceItem>,
        limit: usize,
    ) -> OperatorAppResult<Vec<TimelineEventItem>> {
        self.store
            .read(|kernel| {
                let anchors = anchor
                    .into_iter()
                    .filter_map(record_ref_from_item)
                    .collect::<Vec<_>>();
                kernel
                    .list_timeline(&anchors, limit)
                    .into_iter()
                    .map(timeline_event_item)
                    .collect::<Vec<_>>()
            })
            .map_err(Into::into)
    }

    pub fn list_workflow_cases(
        &self,
        filter: WorkflowCaseFilter,
    ) -> OperatorAppResult<Vec<WorkflowCaseListItem>> {
        let metadata = self
            .metadata
            .read()
            .expect("operator metadata read lock poisoned")
            .workflows
            .clone();
        self.store
            .read(|kernel| {
                let mut items = kernel
                    .workflow_cases
                    .values()
                    .filter(|case| filter.states.is_empty() || filter.states.contains(&case.state))
                    .filter(|case| {
                        filter.related_record_id.as_ref().is_none_or(|record_id| {
                            case.related_to
                                .iter()
                                .any(|reference| reference.id.to_string() == *record_id)
                        })
                    })
                    .map(|case| WorkflowCaseListItem {
                        id: case.id.to_string(),
                        definition_key: metadata
                            .get(&case.id)
                            .map(|value| value.definition_key.clone())
                            .or_else(|| Some(slugify(&case.title)))
                            .unwrap_or_else(|| "workflow-case".to_string()),
                        title: case.title.clone(),
                        state: case.state,
                        related_to: case
                            .related_to
                            .iter()
                            .copied()
                            .map(record_reference_item)
                            .collect(),
                        created_at: case.opened_at,
                        priority: case.priority,
                    })
                    .collect::<Vec<_>>();
                if let Some(definition_key) = &filter.definition_key {
                    items.retain(|item| item.definition_key == *definition_key);
                }
                items.sort_by(|left, right| right.created_at.cmp(&left.created_at));
                items
            })
            .map_err(Into::into)
    }

    pub fn list_approvals(
        &self,
        filter: ApprovalFilter,
    ) -> OperatorAppResult<Vec<ApprovalListItem>> {
        let metadata = self
            .metadata
            .read()
            .expect("operator metadata read lock poisoned")
            .approvals
            .clone();
        self.store
            .read(|kernel| {
                let mut items = kernel
                    .approvals
                    .values()
                    .map(|approval| {
                        let approval_meta = metadata.get(&approval.id);
                        ApprovalListItem {
                            id: approval.id.to_string(),
                            truth_key: approval_meta
                                .map(|value| value.truth_key.clone())
                                .unwrap_or_else(|| "manual".to_string()),
                            reason: approval_meta
                                .map(|value| value.reason.clone())
                                .unwrap_or_else(|| {
                                    format!(
                                        "Approval required for {}",
                                        record_kind_name(approval.record.kind)
                                    )
                                }),
                            created_at: approval.created_at,
                            status: approval.status,
                        }
                    })
                    .collect::<Vec<_>>();

                if let Some(status) = &filter.status {
                    items.retain(|item| item.status == *status);
                }
                if let Some(truth_key) = &filter.truth_key {
                    items.retain(|item| item.truth_key == *truth_key);
                }

                items.sort_by(|left, right| right.created_at.cmp(&left.created_at));
                items
            })
            .map_err(Into::into)
    }

    fn execute_qualify_inbound_lead(
        &self,
        truth: TruthDefinition,
        inputs: HashMap<String, String>,
    ) -> OperatorAppResult<TruthExecutionSession> {
        let organization_name = required_input(&inputs, "organization_name")?;
        let inbound_summary = required_input(&inputs, "inbound_summary")?;
        let organization_id = optional_uuid(&inputs, "organization_id")?;
        let person_id = optional_uuid(&inputs, "person_id")?;
        let require_manual_review = optional_bool(&inputs, "require_manual_review");
        let reason = inputs
            .get("manual_review_reason")
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("Operator requested manual review before qualification");
        let actor = self.default_actor.clone();

        let StoreWriteResult { value, events } = self.store.write_with_events(|kernel| {
            let organization = kernel.upsert_organization(
                OrganizationUpsert {
                    organization_id,
                    name: organization_name.to_string(),
                    external_key: inputs.get("organization_external_key").cloned(),
                    website: inputs.get("website").cloned(),
                    industry: inputs.get("industry").cloned(),
                    lifecycle: OrganizationLifecycle::Prospect,
                    owner_user_id: inputs.get("owner_user_id").cloned(),
                    tags: vec!["inbound-lead".to_string()],
                },
                actor.clone(),
            )?;

            let person = inputs
                .get("contact_name")
                .map(String::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|full_name| {
                    kernel.upsert_person(
                        PersonUpsert {
                            person_id,
                            organization_id: Some(organization.id),
                            full_name: full_name.to_string(),
                            title: inputs.get("contact_title").cloned(),
                            email: inputs.get("contact_email").cloned(),
                            phone: inputs.get("contact_phone").cloned(),
                            linkedin_url: inputs.get("contact_linkedin_url").cloned(),
                        },
                        actor.clone(),
                    )
                })
                .transpose()?;

            let mut related_to = vec![RecordRef {
                kind: RecordKind::Organization,
                id: organization.id,
            }];
            if let Some(person) = &person {
                related_to.push(RecordRef {
                    kind: RecordKind::Person,
                    id: person.id,
                });
            }

            let _ = kernel.record_communication(
                CommunicationRecord {
                    channel: CommunicationChannel::Email,
                    direction: CommunicationDirection::Inbound,
                    subject: inputs.get("subject").cloned(),
                    summary: inbound_summary.to_string(),
                    counterpart: person
                        .as_ref()
                        .map(|contact| contact.full_name.clone())
                        .unwrap_or_else(|| organization.name.clone()),
                    related_to: related_to.clone(),
                    occurred_at: None,
                },
                actor.clone(),
            )?;

            let qualification_fact = kernel.record_fact(
                FactRecord {
                    statement: format!(
                        "{} shows qualified inbound intent based on the submitted summary.",
                        organization.name
                    ),
                    confidence_bps: 8_900,
                    related_to: related_to.clone(),
                    source_note_id: None,
                },
                actor.clone(),
            )?;

            let owner_fact = kernel.record_fact(
                FactRecord {
                    statement: format!(
                        "Suggested owner for {} is {}.",
                        organization.name,
                        inputs
                            .get("owner_user_id")
                            .filter(|value| !value.trim().is_empty())
                            .cloned()
                            .unwrap_or_else(|| "revops-queue".to_string())
                    ),
                    confidence_bps: 7_800,
                    related_to: related_to.clone(),
                    source_note_id: None,
                },
                actor.clone(),
            )?;

            let next_step_fact = kernel.record_fact(
                FactRecord {
                    statement: format!(
                        "Next step for {} is {}.",
                        organization.name,
                        inputs
                            .get("next_step")
                            .filter(|value| !value.trim().is_empty())
                            .cloned()
                            .unwrap_or_else(|| "Schedule qualification review".to_string())
                    ),
                    confidence_bps: 7_600,
                    related_to: related_to.clone(),
                    source_note_id: None,
                },
                actor.clone(),
            )?;

            if require_manual_review {
                let workflow_case = kernel.create_workflow_case(
                    WorkflowCaseCreate {
                        title: format!("Manual qualification review for {}", organization.name),
                        priority: WorkflowPriority::High,
                        owner_user_id: inputs.get("owner_user_id").cloned(),
                        related_to: related_to.clone(),
                    },
                    actor.clone(),
                )?;

                let workflow_case = kernel.advance_workflow_case(
                    application_kernel::WorkflowCaseAdvance {
                        workflow_case_id: workflow_case.id,
                        state: WorkflowState::AwaitingApproval,
                    },
                    actor.clone(),
                )?;

                let _ = kernel.append_note(
                    NoteAppend {
                        subject: "Manual review required".to_string(),
                        body: reason.to_string(),
                        related_to: {
                            let mut references = related_to.clone();
                            references.push(RecordRef {
                                kind: RecordKind::WorkflowCase,
                                id: workflow_case.id,
                            });
                            references
                        },
                    },
                    actor.clone(),
                )?;

                let approval = Approval {
                    id: Uuid::new_v4(),
                    record: RecordRef {
                        kind: RecordKind::WorkflowCase,
                        id: workflow_case.id,
                    },
                    status: ApprovalStatus::Pending,
                    requested_by: actor.clone(),
                    approver_user_id: None,
                    created_at: Utc::now(),
                    decided_at: None,
                };
                kernel.approvals.insert(approval.id, approval.clone());

                Ok(QualifyLeadWriteResult {
                    organization,
                    person,
                    opportunity_id: None,
                    workflow_case: Some(workflow_case),
                    approval_id: Some(approval.id),
                    fact_ids: vec![qualification_fact.id, owner_fact.id, next_step_fact.id],
                })
            } else {
                let opportunity = kernel.create_opportunity(
                    OpportunityCreate {
                        organization_id: organization.id,
                        primary_contact_id: person.as_ref().map(|value| value.id),
                        name: inputs
                            .get("opportunity_name")
                            .filter(|value| !value.trim().is_empty())
                            .cloned()
                            .unwrap_or_else(|| format!("Inbound lead: {}", organization.name)),
                        value: Money {
                            currency_code: inputs
                                .get("currency_code")
                                .filter(|value| !value.trim().is_empty())
                                .cloned()
                                .unwrap_or_else(|| "USD".to_string()),
                            amount_minor: inputs
                                .get("opportunity_value_minor")
                                .and_then(|value| value.parse::<i64>().ok())
                                .unwrap_or(0),
                        },
                        confidence_bps: 7_400,
                        next_step: inputs
                            .get("next_step")
                            .filter(|value| !value.trim().is_empty())
                            .cloned(),
                        expected_close_at: None,
                    },
                    actor.clone(),
                )?;

                let _ = kernel.append_activity(
                    application_kernel::ActivityAppend {
                        subject: "Inbound lead qualified".to_string(),
                        details: inbound_summary.to_string(),
                        related_to: {
                            let mut references = related_to.clone();
                            references.push(RecordRef {
                                kind: RecordKind::Opportunity,
                                id: opportunity.id,
                            });
                            references
                        },
                        outcome: ActivityOutcome::Completed,
                        occurred_at: None,
                        next_action_due_at: None,
                    },
                    actor.clone(),
                )?;

                Ok(QualifyLeadWriteResult {
                    organization,
                    person,
                    opportunity_id: Some(opportunity.id),
                    workflow_case: None,
                    approval_id: None,
                    fact_ids: vec![qualification_fact.id, owner_fact.id, next_step_fact.id],
                })
            }
        })?;

        if let Some(workflow_case) = &value.workflow_case {
            self.remember_workflow(workflow_case.id, "lead-manual-review");
        }
        if let Some(approval_id) = value.approval_id {
            self.remember_approval(approval_id, QUALIFY_INBOUND_LEAD, reason);
        }

        let blocked = value.workflow_case.is_some();
        Ok(TruthExecutionSession {
            truth_key: truth.key.to_string(),
            state: if blocked {
                ExecutionState::Blocked
            } else {
                ExecutionState::Completed
            },
            result: Some(TruthExecutionResult {
                converged: !blocked,
                cycles: 1,
                stop_reason: if blocked {
                    "manual review required".to_string()
                } else {
                    "projection persisted".to_string()
                },
                experience_event_kinds: events.iter().map(domain_event_kind_name).collect(),
            }),
            criteria_outcomes: qualify_inbound_lead_criteria(blocked, value.approval_id),
            projection: Some(TruthExecutionProjection {
                organization_id: Some(value.organization.id.to_string()),
                person_id: value.person.as_ref().map(|person| person.id.to_string()),
                opportunity_id: value.opportunity_id.map(|id| id.to_string()),
                subscription_id: None,
                workflow_case_ids: value
                    .workflow_case
                    .iter()
                    .map(|case| case.id.to_string())
                    .collect(),
                approval_ids: value
                    .approval_id
                    .into_iter()
                    .map(|id| id.to_string())
                    .collect(),
                fact_ids: value
                    .fact_ids
                    .into_iter()
                    .map(|id| id.to_string())
                    .collect(),
                document_ids: Vec::new(),
                entitlement_ids: Vec::new(),
                projected_event_kinds: events.iter().map(domain_event_kind_name).collect(),
            }),
            error: None,
        })
    }

    fn execute_activate_subscription(
        &self,
        truth: TruthDefinition,
        inputs: HashMap<String, String>,
    ) -> OperatorAppResult<TruthExecutionSession> {
        let organization_id = parse_uuid(
            "organization_id",
            required_input(&inputs, "organization_id")?,
        )?;
        let subscription_id = parse_uuid(
            "subscription_id",
            required_input(&inputs, "subscription_id")?,
        )?;
        let catalog_item_id = optional_uuid(&inputs, "catalog_item_id")?;
        let payment_confirmed = optional_bool(&inputs, "payment_confirmed");
        let actor = self.default_actor.clone();

        if !payment_confirmed {
            let reason =
                "Payment confirmation is required before subscription activation.".to_string();
            let StoreWriteResult { value, events } = self.store.write_with_events(|kernel| {
                let subscription = kernel.get_subscription(subscription_id)?;
                validate_subscription_organization(
                        subscription.organization_id,
                        organization_id,
                    )
                    .map_err(|error| {
                        application_kernel::KernelError::Validation(error.to_string())
                    })?;
                let related_to = subscription_related_to(
                    subscription.organization_id,
                    subscription.id,
                    catalog_item_id.or(subscription.catalog_item_id),
                );
                let workflow_case = kernel.create_workflow_case(
                    WorkflowCaseCreate {
                        title: format!("Manual review: activate subscription {}", subscription.id),
                        priority: WorkflowPriority::High,
                        owner_user_id: None,
                        related_to,
                    },
                    actor.clone(),
                )?;
                let workflow_case = kernel.advance_workflow_case(
                    application_kernel::WorkflowCaseAdvance {
                        workflow_case_id: workflow_case.id,
                        state: WorkflowState::AwaitingApproval,
                    },
                    actor.clone(),
                )?;
                let approval = Approval {
                    id: Uuid::new_v4(),
                    record: RecordRef {
                        kind: RecordKind::WorkflowCase,
                        id: workflow_case.id,
                    },
                    status: ApprovalStatus::Pending,
                    requested_by: actor.clone(),
                    approver_user_id: None,
                    created_at: Utc::now(),
                    decided_at: None,
                };
                kernel.approvals.insert(approval.id, approval.clone());

                Ok(ActivationWriteResult {
                    organization_id,
                    subscription_id,
                    workflow_case: Some(workflow_case),
                    approval_id: Some(approval.id),
                    entitlement_ids: Vec::new(),
                })
            })?;

            let workflow_case = value
                .workflow_case
                .expect("blocked activation should create workflow");
            let approval_id = value
                .approval_id
                .expect("blocked activation should create approval");
            self.remember_workflow(workflow_case.id, "subscription-activation-review");
            self.remember_approval(approval_id, ACTIVATE_SUBSCRIPTION, &reason);

            return Ok(TruthExecutionSession {
                truth_key: truth.key.to_string(),
                state: ExecutionState::Blocked,
                result: Some(TruthExecutionResult {
                    converged: false,
                    cycles: 1,
                    stop_reason: "payment confirmation required".to_string(),
                    experience_event_kinds: events.iter().map(domain_event_kind_name).collect(),
                }),
                criteria_outcomes: activate_subscription_criteria(true, Some(approval_id)),
                projection: Some(TruthExecutionProjection {
                    organization_id: Some(value.organization_id.to_string()),
                    person_id: None,
                    opportunity_id: None,
                    subscription_id: Some(value.subscription_id.to_string()),
                    workflow_case_ids: vec![workflow_case.id.to_string()],
                    approval_ids: vec![approval_id.to_string()],
                    fact_ids: Vec::new(),
                    document_ids: Vec::new(),
                    entitlement_ids: Vec::new(),
                    projected_event_kinds: events.iter().map(domain_event_kind_name).collect(),
                }),
                error: None,
            });
        }

        let StoreWriteResult { value, events } = self.store.write_with_events(|kernel| {
            let subscription = kernel.get_subscription(subscription_id)?;
            validate_subscription_organization(subscription.organization_id, organization_id)
                .map_err(|error| application_kernel::KernelError::Validation(error.to_string()))?;
            let activation = kernel.activate_subscription(
                SubscriptionActivate {
                    subscription_id,
                    catalog_item_id,
                    opening_balance: None,
                },
                actor.clone(),
            )?;

            Ok(ActivationWriteResult {
                organization_id: activation.subscription.organization_id,
                subscription_id: activation.subscription.id,
                workflow_case: None,
                approval_id: None,
                entitlement_ids: activation
                    .entitlements
                    .iter()
                    .map(|entitlement| entitlement.id)
                    .collect(),
            })
        })?;

        Ok(TruthExecutionSession {
            truth_key: truth.key.to_string(),
            state: ExecutionState::Completed,
            result: Some(TruthExecutionResult {
                converged: true,
                cycles: 1,
                stop_reason: "subscription activated".to_string(),
                experience_event_kinds: events.iter().map(domain_event_kind_name).collect(),
            }),
            criteria_outcomes: activate_subscription_criteria(false, None),
            projection: Some(TruthExecutionProjection {
                organization_id: Some(value.organization_id.to_string()),
                person_id: None,
                opportunity_id: None,
                subscription_id: Some(value.subscription_id.to_string()),
                workflow_case_ids: Vec::new(),
                approval_ids: Vec::new(),
                fact_ids: Vec::new(),
                document_ids: Vec::new(),
                entitlement_ids: value
                    .entitlement_ids
                    .into_iter()
                    .map(|id| id.to_string())
                    .collect(),
                projected_event_kinds: events.iter().map(domain_event_kind_name).collect(),
            }),
            error: None,
        })
    }

    fn execute_submit_expense_report(
        &self,
        truth: TruthDefinition,
        inputs: HashMap<String, String>,
    ) -> OperatorAppResult<TruthExecutionSession> {
        let organization_name = required_input(&inputs, "organization_name")?;
        let organization_id = optional_uuid(&inputs, "organization_id")?;
        let amount_minor = required_i64(&inputs, "amount_minor")?;
        let receipt_uri = required_input(&inputs, "receipt_uri")?.to_string();
        let currency_code = inputs
            .get("currency_code")
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("EUR")
            .to_string();
        let merchant_name = inputs
            .get("merchant_name")
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("Unknown merchant")
            .to_string();
        let category = inputs
            .get("category")
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("uncategorized")
            .to_string();
        let expense_date = inputs
            .get("expense_date")
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("unspecified date")
            .to_string();
        let report_title = inputs
            .get("report_title")
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("Expense report: {merchant_name} {expense_date}"));
        let receipt_title = inputs
            .get("receipt_title")
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("Receipt: {merchant_name}"));
        let receipt_media_type = inputs
            .get("receipt_media_type")
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("application/pdf")
            .to_string();
        let ocr_confidence_bps = optional_u16(&inputs, "ocr_confidence_bps")?.unwrap_or(8_500);
        let out_of_policy = optional_bool(&inputs, "out_of_policy");
        let require_manual_review = optional_bool(&inputs, "require_manual_review");
        let actor = self.default_actor.clone();

        if amount_minor <= 0 {
            return Err(OperatorAppError::Validation(
                "amount_minor must be positive".to_string(),
            ));
        }

        let review_reason = inputs
            .get("manual_review_reason")
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| {
                if out_of_policy {
                    "Expense falls outside policy and needs approval.".to_string()
                } else if ocr_confidence_bps < 7_000 {
                    "OCR confidence is below the automatic submission threshold.".to_string()
                } else {
                    "Operator requested manual review before submission.".to_string()
                }
            });
        let blocked = require_manual_review || out_of_policy || ocr_confidence_bps < 7_000;

        let StoreWriteResult { value, events } = self.store.write_with_events(|kernel| {
            let organization = kernel.upsert_organization(
                OrganizationUpsert {
                    organization_id,
                    name: organization_name.to_string(),
                    external_key: inputs.get("organization_external_key").cloned(),
                    website: None,
                    industry: Some("Internal operations".to_string()),
                    lifecycle: OrganizationLifecycle::Active,
                    owner_user_id: inputs.get("owner_user_id").cloned(),
                    tags: vec!["expense-report".to_string()],
                },
                actor.clone(),
            )?;

            let organization_ref = RecordRef {
                kind: RecordKind::Organization,
                id: organization.id,
            };
            let receipt = kernel.attach_document(
                DocumentAttach {
                    title: receipt_title.clone(),
                    media_type: receipt_media_type.clone(),
                    uri: receipt_uri.clone(),
                    status: if blocked {
                        DocumentStatus::Draft
                    } else {
                        DocumentStatus::Verified
                    },
                    related_to: vec![organization_ref],
                },
                actor.clone(),
            )?;

            let receipt_ref = RecordRef {
                kind: RecordKind::Document,
                id: receipt.id,
            };
            let workflow_case = kernel.create_workflow_case(
                WorkflowCaseCreate {
                    title: report_title.clone(),
                    priority: if blocked {
                        WorkflowPriority::High
                    } else {
                        WorkflowPriority::Medium
                    },
                    owner_user_id: inputs.get("owner_user_id").cloned(),
                    related_to: vec![organization_ref, receipt_ref],
                },
                actor.clone(),
            )?;

            let workflow_case = kernel.advance_workflow_case(
                application_kernel::WorkflowCaseAdvance {
                    workflow_case_id: workflow_case.id,
                    state: if blocked {
                        WorkflowState::AwaitingApproval
                    } else {
                        WorkflowState::Done
                    },
                },
                actor.clone(),
            )?;

            let workflow_ref = RecordRef {
                kind: RecordKind::WorkflowCase,
                id: workflow_case.id,
            };
            let related_to = vec![organization_ref, receipt_ref, workflow_ref];

            let _ = kernel.append_note(
                NoteAppend {
                    subject: if blocked {
                        "Expense report requires review".to_string()
                    } else {
                        "Expense report ready for export".to_string()
                    },
                    body: if blocked {
                        format!(
                            "{report_title} for {merchant_name} is blocked: {review_reason}"
                        )
                    } else {
                        format!(
                            "{report_title} for {merchant_name} is staged and ready for bookkeeping export."
                        )
                    },
                    related_to: related_to.clone(),
                },
                actor.clone(),
            )?;

            let evidence_fact = kernel.record_fact(
                FactRecord {
                    statement: format!(
                        "Expense report {report_title} claims {amount_minor} {currency_code} for {merchant_name} on {expense_date} in category {category}."
                    ),
                    confidence_bps: ocr_confidence_bps,
                    related_to: related_to.clone(),
                    source_note_id: None,
                },
                actor.clone(),
            )?;
            let approval_route_fact = kernel.record_fact(
                FactRecord {
                    statement: if blocked {
                        format!(
                            "Approval route for {report_title} requires manual finance review: {review_reason}"
                        )
                    } else {
                        format!(
                            "Approval route for {report_title} is the standard reimbursement path with no extra approver required."
                        )
                    },
                    confidence_bps: 8_000,
                    related_to: related_to.clone(),
                    source_note_id: None,
                },
                actor.clone(),
            )?;
            let export_status_fact = kernel.record_fact(
                FactRecord {
                    statement: if blocked {
                        format!(
                            "Export status for {report_title} is blocked pending approval."
                        )
                    } else {
                        format!(
                            "Export status for {report_title} is ready for bookkeeping handoff."
                        )
                    },
                    confidence_bps: 8_400,
                    related_to: related_to.clone(),
                    source_note_id: None,
                },
                actor.clone(),
            )?;

            let approval_id = if blocked {
                let approval = Approval {
                    id: Uuid::new_v4(),
                    record: workflow_ref,
                    status: ApprovalStatus::Pending,
                    requested_by: actor.clone(),
                    approver_user_id: None,
                    created_at: Utc::now(),
                    decided_at: None,
                };
                kernel.approvals.insert(approval.id, approval.clone());
                Some(approval.id)
            } else {
                None
            };

            Ok(ExpenseReportWriteResult {
                organization,
                workflow_case,
                approval_id,
                fact_ids: vec![evidence_fact.id, approval_route_fact.id, export_status_fact.id],
                document_ids: vec![receipt.id],
            })
        })?;

        self.remember_workflow(
            value.workflow_case.id,
            if blocked {
                "expense-report-review"
            } else {
                "expense-report-export"
            },
        );
        if let Some(approval_id) = value.approval_id {
            self.remember_approval(approval_id, SUBMIT_EXPENSE_REPORT, &review_reason);
        }

        Ok(TruthExecutionSession {
            truth_key: truth.key.to_string(),
            state: if blocked {
                ExecutionState::Blocked
            } else {
                ExecutionState::Completed
            },
            result: Some(TruthExecutionResult {
                converged: !blocked,
                cycles: 1,
                stop_reason: if blocked {
                    "manual review required".to_string()
                } else {
                    "expense report staged for export".to_string()
                },
                experience_event_kinds: events.iter().map(domain_event_kind_name).collect(),
            }),
            criteria_outcomes: submit_expense_report_criteria(blocked, value.approval_id),
            projection: Some(TruthExecutionProjection {
                organization_id: Some(value.organization.id.to_string()),
                person_id: None,
                opportunity_id: None,
                subscription_id: None,
                workflow_case_ids: vec![value.workflow_case.id.to_string()],
                approval_ids: value
                    .approval_id
                    .into_iter()
                    .map(|id| id.to_string())
                    .collect(),
                fact_ids: value
                    .fact_ids
                    .into_iter()
                    .map(|id| id.to_string())
                    .collect(),
                document_ids: value
                    .document_ids
                    .into_iter()
                    .map(|id| id.to_string())
                    .collect(),
                entitlement_ids: Vec::new(),
                projected_event_kinds: events.iter().map(domain_event_kind_name).collect(),
            }),
            error: None,
        })
    }

    fn execute_refill_prepaid_ai_credits(
        &self,
        truth: TruthDefinition,
        inputs: HashMap<String, String>,
    ) -> OperatorAppResult<TruthExecutionSession> {
        let organization_id = parse_uuid(
            "organization_id",
            required_input(&inputs, "organization_id")?,
        )?;
        let subscription_id = parse_uuid(
            "subscription_id",
            required_input(&inputs, "subscription_id")?,
        )?;
        let amount_minor = required_i64(&inputs, "amount_minor")?;
        let currency_code = required_input(&inputs, "currency_code")?.to_string();
        let payment_reference = required_input(&inputs, "payment_reference")?.to_string();
        let payment_status = parse_payment_status(required_input(&inputs, "payment_status")?)?;
        let actor = self.default_actor.clone();

        if amount_minor <= 0 {
            return Err(OperatorAppError::Validation(
                "amount_minor must be positive".to_string(),
            ));
        }

        if matches!(payment_status, PaymentStatus::Failed) {
            return Ok(TruthExecutionSession {
                truth_key: truth.key.to_string(),
                state: ExecutionState::Failed,
                result: Some(TruthExecutionResult {
                    converged: false,
                    cycles: 1,
                    stop_reason: "payment failed".to_string(),
                    experience_event_kinds: Vec::new(),
                }),
                criteria_outcomes: refill_prepaid_ai_credits_criteria(false, None),
                projection: Some(TruthExecutionProjection {
                    organization_id: Some(organization_id.to_string()),
                    person_id: None,
                    opportunity_id: None,
                    subscription_id: Some(subscription_id.to_string()),
                    workflow_case_ids: Vec::new(),
                    approval_ids: Vec::new(),
                    fact_ids: Vec::new(),
                    document_ids: Vec::new(),
                    entitlement_ids: Vec::new(),
                    projected_event_kinds: Vec::new(),
                }),
                error: Some(
                    "Payment status must be confirmed before credits can be applied.".to_string(),
                ),
            });
        }

        if !matches!(payment_status, PaymentStatus::Confirmed) {
            let reason = format!(
                "Payment status is {} and requires manual review before credits can be granted.",
                payment_status_name(payment_status)
            );
            let StoreWriteResult { value, events } = self.store.write_with_events(|kernel| {
                let subscription = kernel.get_subscription(subscription_id)?;
                validate_subscription_organization(
                        subscription.organization_id,
                        organization_id,
                    )
                    .map_err(|error| {
                        application_kernel::KernelError::Validation(error.to_string())
                    })?;
                validate_subscription_currency(
                        &subscription.value.currency_code,
                        &currency_code,
                    )
                    .map_err(|error| {
                        application_kernel::KernelError::Validation(error.to_string())
                    })?;
                let related_to = subscription_related_to(
                    subscription.organization_id,
                    subscription.id,
                    subscription.catalog_item_id,
                );
                let workflow_case = kernel.create_workflow_case(
                    WorkflowCaseCreate {
                        title: format!("Manual review: prepaid refill {}", payment_reference),
                        priority: WorkflowPriority::High,
                        owner_user_id: None,
                        related_to,
                    },
                    actor.clone(),
                )?;
                let workflow_case = kernel.advance_workflow_case(
                    application_kernel::WorkflowCaseAdvance {
                        workflow_case_id: workflow_case.id,
                        state: WorkflowState::AwaitingApproval,
                    },
                    actor.clone(),
                )?;
                let approval = Approval {
                    id: Uuid::new_v4(),
                    record: RecordRef {
                        kind: RecordKind::WorkflowCase,
                        id: workflow_case.id,
                    },
                    status: ApprovalStatus::Pending,
                    requested_by: actor.clone(),
                    approver_user_id: None,
                    created_at: Utc::now(),
                    decided_at: None,
                };
                kernel.approvals.insert(approval.id, approval.clone());

                Ok(CreditRefillWriteResult {
                    organization_id,
                    subscription_id,
                    workflow_case: Some(workflow_case),
                    approval_id: Some(approval.id),
                    entitlement_ids: Vec::new(),
                })
            })?;

            let workflow_case = value
                .workflow_case
                .expect("blocked refill should create workflow");
            let approval_id = value
                .approval_id
                .expect("blocked refill should create approval");
            self.remember_workflow(workflow_case.id, "prepaid-credit-refill-review");
            self.remember_approval(approval_id, REFILL_PREPAID_AI_CREDITS, &reason);

            return Ok(TruthExecutionSession {
                truth_key: truth.key.to_string(),
                state: ExecutionState::Blocked,
                result: Some(TruthExecutionResult {
                    converged: false,
                    cycles: 1,
                    stop_reason: "manual review required".to_string(),
                    experience_event_kinds: events.iter().map(domain_event_kind_name).collect(),
                }),
                criteria_outcomes: refill_prepaid_ai_credits_criteria(true, Some(approval_id)),
                projection: Some(TruthExecutionProjection {
                    organization_id: Some(value.organization_id.to_string()),
                    person_id: None,
                    opportunity_id: None,
                    subscription_id: Some(value.subscription_id.to_string()),
                    workflow_case_ids: vec![workflow_case.id.to_string()],
                    approval_ids: vec![approval_id.to_string()],
                    fact_ids: Vec::new(),
                    document_ids: Vec::new(),
                    entitlement_ids: Vec::new(),
                    projected_event_kinds: events.iter().map(domain_event_kind_name).collect(),
                }),
                error: None,
            });
        }

        let StoreWriteResult { value, events } = self.store.write_with_events(|kernel| {
            let subscription = kernel.get_subscription(subscription_id)?;
            validate_subscription_organization(subscription.organization_id, organization_id)
                .map_err(|error| application_kernel::KernelError::Validation(error.to_string()))?;
            validate_subscription_currency(&subscription.value.currency_code, &currency_code)
                .map_err(|error| application_kernel::KernelError::Validation(error.to_string()))?;
            let grant = kernel.apply_credit_grant(
                CreditGrantApply {
                    subscription_id,
                    amount: Money {
                        currency_code: currency_code.clone(),
                        amount_minor,
                    },
                    payment_reference: payment_reference.clone(),
                    reason: Some("Prepaid AI credit refill".to_string()),
                },
                actor.clone(),
            )?;

            Ok(CreditRefillWriteResult {
                organization_id: grant.subscription.organization_id,
                subscription_id: grant.subscription.id,
                workflow_case: None,
                approval_id: None,
                entitlement_ids: vec![grant.entitlement.id],
            })
        })?;

        Ok(TruthExecutionSession {
            truth_key: truth.key.to_string(),
            state: ExecutionState::Completed,
            result: Some(TruthExecutionResult {
                converged: true,
                cycles: 1,
                stop_reason: "credit grant applied".to_string(),
                experience_event_kinds: events.iter().map(domain_event_kind_name).collect(),
            }),
            criteria_outcomes: refill_prepaid_ai_credits_criteria(false, None),
            projection: Some(TruthExecutionProjection {
                organization_id: Some(value.organization_id.to_string()),
                person_id: None,
                opportunity_id: None,
                subscription_id: Some(value.subscription_id.to_string()),
                workflow_case_ids: Vec::new(),
                approval_ids: Vec::new(),
                fact_ids: Vec::new(),
                document_ids: Vec::new(),
                entitlement_ids: value
                    .entitlement_ids
                    .into_iter()
                    .map(|id| id.to_string())
                    .collect(),
                projected_event_kinds: events.iter().map(domain_event_kind_name).collect(),
            }),
            error: None,
        })
    }

    fn remember_approval(&self, id: Uuid, truth_key: &str, reason: &str) {
        let mut metadata = self
            .metadata
            .write()
            .expect("operator metadata write lock poisoned");
        metadata.approvals.insert(
            id,
            ApprovalMetadata {
                truth_key: truth_key.to_string(),
                reason: reason.to_string(),
            },
        );
    }

    fn remember_workflow(&self, id: Uuid, definition_key: &str) {
        let mut metadata = self
            .metadata
            .write()
            .expect("operator metadata write lock poisoned");
        metadata.workflows.insert(
            id,
            WorkflowMetadata {
                definition_key: definition_key.to_string(),
            },
        );
    }
}

fn required_input<'a>(
    inputs: &'a HashMap<String, String>,
    key: &'static str,
) -> OperatorAppResult<&'a str> {
    inputs
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(OperatorAppError::MissingInput(key))
}

fn required_i64(inputs: &HashMap<String, String>, key: &'static str) -> OperatorAppResult<i64> {
    let value = required_input(inputs, key)?;
    value
        .parse::<i64>()
        .map_err(|_| OperatorAppError::InvalidInteger {
            field: key,
            value: value.to_string(),
        })
}

fn optional_bool(inputs: &HashMap<String, String>, key: &str) -> bool {
    inputs
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "true" | "1" | "yes"))
        .unwrap_or(false)
}

fn optional_uuid(
    inputs: &HashMap<String, String>,
    key: &'static str,
) -> OperatorAppResult<Option<Uuid>> {
    inputs
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| parse_uuid(key, value))
        .transpose()
}

fn optional_u16(
    inputs: &HashMap<String, String>,
    key: &'static str,
) -> OperatorAppResult<Option<u16>> {
    inputs
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            value
                .parse::<u16>()
                .map_err(|_| OperatorAppError::InvalidInteger {
                    field: key,
                    value: value.to_string(),
                })
        })
        .transpose()
}

fn parse_uuid(field: &'static str, value: &str) -> OperatorAppResult<Uuid> {
    Uuid::parse_str(value).map_err(|_| OperatorAppError::InvalidUuid {
        field,
        value: value.to_string(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaymentStatus {
    Confirmed,
    Pending,
    Failed,
}

fn parse_payment_status(value: &str) -> OperatorAppResult<PaymentStatus> {
    match value.trim().to_ascii_lowercase().as_str() {
        "confirmed" | "paid" | "settled" => Ok(PaymentStatus::Confirmed),
        "pending" | "authorized" | "review" | "manual-review" => Ok(PaymentStatus::Pending),
        "failed" | "declined" | "voided" => Ok(PaymentStatus::Failed),
        other => Err(OperatorAppError::Validation(format!(
            "unsupported payment_status: {other}"
        ))),
    }
}

fn payment_status_name(value: PaymentStatus) -> &'static str {
    match value {
        PaymentStatus::Confirmed => "confirmed",
        PaymentStatus::Pending => "pending",
        PaymentStatus::Failed => "failed",
    }
}

fn validate_subscription_organization(actual: Uuid, expected: Uuid) -> OperatorAppResult<()> {
    if actual == expected {
        return Ok(());
    }
    Err(OperatorAppError::Validation(format!(
        "organization_id does not match subscription organization ({expected} != {actual})"
    )))
}

fn validate_subscription_currency(actual: &str, expected: &str) -> OperatorAppResult<()> {
    if actual.eq_ignore_ascii_case(expected) {
        return Ok(());
    }
    Err(OperatorAppError::Validation(format!(
        "currency_code does not match subscription currency ({expected} != {actual})"
    )))
}

fn subscription_related_to(
    organization_id: Uuid,
    subscription_id: Uuid,
    catalog_item_id: Option<Uuid>,
) -> Vec<RecordRef> {
    let mut related_to = vec![
        RecordRef {
            kind: RecordKind::Organization,
            id: organization_id,
        },
        RecordRef {
            kind: RecordKind::OrderSubscription,
            id: subscription_id,
        },
    ];
    if let Some(catalog_item_id) = catalog_item_id {
        related_to.push(RecordRef {
            kind: RecordKind::CatalogItem,
            id: catalog_item_id,
        });
    }
    related_to
}

fn record_reference_item(reference: RecordRef) -> RecordReferenceItem {
    RecordReferenceItem {
        kind: reference.kind,
        record_id: reference.id.to_string(),
    }
}

fn record_ref_from_item(item: RecordReferenceItem) -> Option<RecordRef> {
    Some(RecordRef {
        kind: item.kind,
        id: Uuid::parse_str(&item.record_id).ok()?,
    })
}

fn timeline_event_item(entry: TimelineEntry) -> TimelineEventItem {
    TimelineEventItem {
        id: entry.id.to_string(),
        kind: entry.kind,
        summary: format!("{} — {}", entry.headline, entry.body),
        actor: entry.actor.display_name,
        timestamp: entry.occurred_at,
        related_to: entry
            .related_to
            .into_iter()
            .map(record_reference_item)
            .collect(),
    }
}

fn unsupported_truth_session(truth: TruthDefinition) -> TruthExecutionSession {
    TruthExecutionSession {
        truth_key: truth.key.to_string(),
        state: ExecutionState::Failed,
        result: Some(TruthExecutionResult {
            converged: false,
            cycles: 0,
            stop_reason: "truth execution not implemented in the workbench backend yet".to_string(),
            experience_event_kinds: Vec::new(),
        }),
        criteria_outcomes: Vec::new(),
        projection: None,
        error: Some(
            "This truth is visible in the catalog but not executable in the workbench backend yet."
                .to_string(),
        ),
    }
}

fn qualify_inbound_lead_criteria(
    blocked: bool,
    approval_id: Option<Uuid>,
) -> Vec<CriteriaOutcomeItem> {
    vec![
        CriteriaOutcomeItem {
            criterion_id: "outcome.lead-is-explicitly-qualified-or-disqualified".to_string(),
            description: "Lead is explicitly qualified or disqualified.".to_string(),
            required: true,
            status: if blocked {
                CriterionStatus::Blocked
            } else {
                CriterionStatus::Met
            },
            detail: if blocked {
                Some("Manual review is required before qualification can complete.".to_string())
            } else {
                Some("Qualification fact and follow-up ownership were projected.".to_string())
            },
            approval_ref: approval_id.map(|id| id.to_string()),
            evidence_fact_ids: Vec::new(),
        },
        CriteriaOutcomeItem {
            criterion_id: "outcome.next-owner-and-next-step-are-recorded".to_string(),
            description: "Next owner and next step are recorded.".to_string(),
            required: true,
            status: if blocked {
                CriterionStatus::Blocked
            } else {
                CriterionStatus::Met
            },
            detail: Some(
                "The workbench backend stores ownership and next-step guidance as projected operator facts."
                    .to_string(),
            ),
            approval_ref: approval_id.map(|id| id.to_string()),
            evidence_fact_ids: Vec::new(),
        },
    ]
}

fn activate_subscription_criteria(
    blocked: bool,
    approval_id: Option<Uuid>,
) -> Vec<CriteriaOutcomeItem> {
    vec![
        CriteriaOutcomeItem {
            criterion_id: "outcome.subscription-becomes-active-with-an-explicit-plan".to_string(),
            description: "Subscription becomes active with an explicit plan.".to_string(),
            required: true,
            status: if blocked {
                CriterionStatus::Blocked
            } else {
                CriterionStatus::Met
            },
            detail: if blocked {
                Some("Activation is paused until payment confirmation is reviewed.".to_string())
            } else {
                Some(
                    "The workbench backend activated the subscription against the selected catalog plan."
                        .to_string(),
                )
            },
            approval_ref: approval_id.map(|id| id.to_string()),
            evidence_fact_ids: Vec::new(),
        },
        CriteriaOutcomeItem {
            criterion_id: "outcome.entitlements-and-financial-opening-state-are-aligned"
                .to_string(),
            description: "Entitlements and financial opening state are aligned.".to_string(),
            required: true,
            status: if blocked {
                CriterionStatus::Blocked
            } else {
                CriterionStatus::Met
            },
            detail: if blocked {
                Some(
                    "Entitlements and opening balance are withheld until approval clears."
                        .to_string(),
                )
            } else {
                Some(
                    "Activation projected entitlements and opening balance through the kernel."
                        .to_string(),
                )
            },
            approval_ref: approval_id.map(|id| id.to_string()),
            evidence_fact_ids: Vec::new(),
        },
    ]
}

fn refill_prepaid_ai_credits_criteria(
    blocked: bool,
    approval_id: Option<Uuid>,
) -> Vec<CriteriaOutcomeItem> {
    vec![
        CriteriaOutcomeItem {
            criterion_id: "outcome.confirmed-top-up-appears-in-the-ledger".to_string(),
            description: "Confirmed top-up appears in the ledger.".to_string(),
            required: true,
            status: if blocked {
                CriterionStatus::Blocked
            } else {
                CriterionStatus::Met
            },
            detail: if blocked {
                Some(
                    "The payment status still needs review before a credit grant can be posted."
                        .to_string(),
                )
            } else {
                Some(
                    "The workbench backend applied a ledger-backed credit grant for the prepaid refill."
                        .to_string(),
                )
            },
            approval_ref: approval_id.map(|id| id.to_string()),
            evidence_fact_ids: Vec::new(),
        },
        CriteriaOutcomeItem {
            criterion_id: "outcome.entitlement-balance-increases-for-the-correct-account"
                .to_string(),
            description: "Entitlement balance increases for the correct account.".to_string(),
            required: true,
            status: if blocked {
                CriterionStatus::Blocked
            } else {
                CriterionStatus::Met
            },
            detail: if blocked {
                Some(
                    "No entitlement balance changes are applied until payment is cleared."
                        .to_string(),
                )
            } else {
                Some(
                    "The credit balance entitlement was incremented on the target subscription."
                        .to_string(),
                )
            },
            approval_ref: approval_id.map(|id| id.to_string()),
            evidence_fact_ids: Vec::new(),
        },
    ]
}

fn submit_expense_report_criteria(
    blocked: bool,
    approval_id: Option<Uuid>,
) -> Vec<CriteriaOutcomeItem> {
    vec![
        CriteriaOutcomeItem {
            criterion_id: "outcome.expense-report-is-submitted-with-attributable-receipt-evidence"
                .to_string(),
            description: "Expense report is submitted with attributable receipt evidence."
                .to_string(),
            required: true,
            status: if blocked {
                CriterionStatus::Blocked
            } else {
                CriterionStatus::Met
            },
            detail: Some(
                "The workbench backend attached a receipt document and promoted evidence facts tied to the report workflow."
                    .to_string(),
            ),
            approval_ref: approval_id.map(|id| id.to_string()),
            evidence_fact_ids: Vec::new(),
        },
        CriteriaOutcomeItem {
            criterion_id: "outcome.approval-route-and-policy-state-are-explicit".to_string(),
            description: "Approval route and policy state are explicit.".to_string(),
            required: true,
            status: if blocked {
                CriterionStatus::Blocked
            } else {
                CriterionStatus::Met
            },
            detail: if blocked {
                Some("A pending approval now governs the manual review path.".to_string())
            } else {
                Some("The standard reimbursement path was recorded without extra approvals."
                    .to_string())
            },
            approval_ref: approval_id.map(|id| id.to_string()),
            evidence_fact_ids: Vec::new(),
        },
        CriteriaOutcomeItem {
            criterion_id: "outcome.export-status-is-queryable".to_string(),
            description: "Export status is queryable.".to_string(),
            required: true,
            status: if blocked {
                CriterionStatus::Blocked
            } else {
                CriterionStatus::Met
            },
            detail: if blocked {
                Some("Export remains blocked until manual approval clears.".to_string())
            } else {
                Some("The report workflow was closed as ready for bookkeeping export."
                    .to_string())
            },
            approval_ref: approval_id.map(|id| id.to_string()),
            evidence_fact_ids: Vec::new(),
        },
    ]
}

fn entitlement_template_summary(template: &application_kernel::EntitlementTemplate) -> Vec<String> {
    let mut items = template
        .feature_flags
        .iter()
        .map(|flag| format!("feature {flag}"))
        .collect::<Vec<_>>();
    items.extend(
        template
            .quotas
            .iter()
            .map(|(key, value)| format!("quota {key}={value}")),
    );
    if let Some(value) = template.credit_balance_minor {
        items.push(format!("credits {value}"));
    }
    items
}

fn entitlement_value_summary(value: &application_kernel::EntitlementValue) -> String {
    match value {
        application_kernel::EntitlementValue::FeatureFlag(flag) => {
            if *flag {
                "enabled".to_string()
            } else {
                "disabled".to_string()
            }
        }
        application_kernel::EntitlementValue::Quota(value) => format!("quota {value}"),
        application_kernel::EntitlementValue::Credits(value) => format!("credits {value}"),
        application_kernel::EntitlementValue::Text(value) => value.clone(),
    }
}

fn supported_truth_keys() -> Vec<String> {
    vec![
        QUALIFY_INBOUND_LEAD.to_string(),
        SUBMIT_EXPENSE_REPORT.to_string(),
        ACTIVATE_SUBSCRIPTION.to_string(),
        REFILL_PREPAID_AI_CREDITS.to_string(),
    ]
}

fn is_truth_supported(key: &str) -> bool {
    matches!(
        key,
        QUALIFY_INBOUND_LEAD
            | SUBMIT_EXPENSE_REPORT
            | ACTIVATE_SUBSCRIPTION
            | REFILL_PREPAID_AI_CREDITS
    )
}

fn catalog_plan_kind_name(kind: CatalogPlanKind) -> &'static str {
    match kind {
        CatalogPlanKind::Subscription => "subscription",
        CatalogPlanKind::PrepaidCredits => "prepaid-credits",
        CatalogPlanKind::EnterpriseCustom => "enterprise-custom",
    }
}

fn billing_period_name(period: BillingPeriod) -> &'static str {
    match period {
        BillingPeriod::Monthly => "monthly",
        BillingPeriod::Quarterly => "quarterly",
        BillingPeriod::Annual => "annual",
        BillingPeriod::OneTime => "one-time",
        BillingPeriod::Custom => "custom",
    }
}

fn record_kind_name(value: RecordKind) -> &'static str {
    match value {
        RecordKind::Organization => "organization",
        RecordKind::Person => "person",
        RecordKind::Relationship => "relationship",
        RecordKind::Lead => "lead",
        RecordKind::Opportunity => "opportunity",
        RecordKind::Conversation => "conversation",
        RecordKind::Activity => "activity",
        RecordKind::Task => "task",
        RecordKind::OfferQuote => "offer-quote",
        RecordKind::OrderSubscription => "order-subscription",
        RecordKind::Document => "document",
        RecordKind::Fact => "fact",
        RecordKind::Intent => "intent",
        RecordKind::WorkflowCase => "workflow-case",
        RecordKind::CommunicationEvent => "communication-event",
        RecordKind::PermissionGrant => "permission-grant",
        RecordKind::AuditEntry => "audit-entry",
        RecordKind::Note => "note",
        RecordKind::CatalogItem => "catalog-item",
    }
}

fn slugify(input: &str) -> String {
    input
        .chars()
        .map(|character| match character {
            'a'..='z' | '0'..='9' => character,
            'A'..='Z' => character.to_ascii_lowercase(),
            _ => '-',
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn domain_event_kind_name(event: &application_kernel::DomainEvent) -> String {
    match event {
        application_kernel::DomainEvent::OrganizationUpserted { .. } => "organization-upserted",
        application_kernel::DomainEvent::PersonUpserted { .. } => "person-upserted",
        application_kernel::DomainEvent::RelationshipLinked { .. } => "relationship-linked",
        application_kernel::DomainEvent::OpportunityCreated { .. } => "opportunity-created",
        application_kernel::DomainEvent::OpportunityStageChanged { .. } => {
            "opportunity-stage-changed"
        }
        application_kernel::DomainEvent::ActivityAppended { .. } => "activity-appended",
        application_kernel::DomainEvent::NoteAppended { .. } => "note-appended",
        application_kernel::DomainEvent::DocumentAttached { .. } => "document-attached",
        application_kernel::DomainEvent::CommunicationRecorded { .. } => "communication-recorded",
        application_kernel::DomainEvent::WorkflowCaseCreated { .. } => "workflow-case-created",
        application_kernel::DomainEvent::WorkflowCaseStateChanged { .. } => {
            "workflow-case-state-changed"
        }
        application_kernel::DomainEvent::PermissionGranted { .. } => "permission-granted",
        application_kernel::DomainEvent::CatalogItemUpserted { .. } => "catalog-item-upserted",
        application_kernel::DomainEvent::OrderSubscriptionCreated { .. } => "subscription-created",
        application_kernel::DomainEvent::OrderSubscriptionStateChanged { .. } => {
            "subscription-state-changed"
        }
        application_kernel::DomainEvent::OrderSubscriptionPlanChanged { .. } => {
            "subscription-plan-changed"
        }
        application_kernel::DomainEvent::EntitlementsGranted { .. } => "entitlements-granted",
        application_kernel::DomainEvent::EntitlementsReplaced { .. } => "entitlements-replaced",
        application_kernel::DomainEvent::EntitlementAdjusted { .. } => "entitlement-adjusted",
        application_kernel::DomainEvent::LedgerEntryAppended { .. } => "ledger-entry-appended",
        application_kernel::DomainEvent::FactRecorded { .. } => "fact-recorded",
        application_kernel::DomainEvent::ObjectDefinitionUpserted { .. } => {
            "object-definition-upserted"
        }
        application_kernel::DomainEvent::ViewDefinitionUpserted { .. } => {
            "view-definition-upserted"
        }
        application_kernel::DomainEvent::AuditRecorded { .. } => "audit-recorded",
        application_kernel::DomainEvent::TimelineEntryRecorded { .. } => "timeline-entry-recorded",
    }
    .to_string()
}

fn built_in_workbench_apps() -> Vec<WorkbenchAppManifest> {
    vec![
        WorkbenchAppManifest {
            id: "home".to_string(),
            display_name: "Home".to_string(),
            route: "/".to_string(),
            summary: "Operator home surface for active jobs, approvals, and exceptions."
                .to_string(),
            kind: WorkbenchAppKind::Workspace,
            status: WorkbenchAppStatus::Ready,
            capability_keys: vec![
                "truth-execution".to_string(),
                "workflow".to_string(),
                "approvals".to_string(),
                "timeline".to_string(),
            ],
            truth_keys: supported_truth_keys(),
        },
        WorkbenchAppManifest {
            id: "notes".to_string(),
            display_name: "Notes".to_string(),
            route: "/notes".to_string(),
            summary: "Capture notes and keep them close to the Outcome Workbench.".to_string(),
            kind: WorkbenchAppKind::Workspace,
            status: WorkbenchAppStatus::Preview,
            capability_keys: vec!["documents".to_string(), "notes".to_string()],
            truth_keys: Vec::new(),
        },
        WorkbenchAppManifest {
            id: "receipt-management".to_string(),
            display_name: "Receipt Management".to_string(),
            route: "/expenses".to_string(),
            summary: "Collect receipts, review extracted fields, and stage clean expense reports."
                .to_string(),
            kind: WorkbenchAppKind::Workspace,
            status: WorkbenchAppStatus::Preview,
            capability_keys: vec![
                "expenses".to_string(),
                "documents".to_string(),
                "ocr".to_string(),
            ],
            truth_keys: vec!["submit-expense-report".to_string()],
        },
        WorkbenchAppManifest {
            id: "revenue-review".to_string(),
            display_name: "Revenue Review".to_string(),
            route: "/revenue".to_string(),
            summary: "Review organizations, subscriptions, plans, and prepaid balances."
                .to_string(),
            kind: WorkbenchAppKind::Review,
            status: WorkbenchAppStatus::Ready,
            capability_keys: vec![
                "organizations".to_string(),
                "catalog".to_string(),
                "subscriptions".to_string(),
            ],
            truth_keys: vec![
                ACTIVATE_SUBSCRIPTION.to_string(),
                REFILL_PREPAID_AI_CREDITS.to_string(),
            ],
        },
        WorkbenchAppManifest {
            id: "workflow-review".to_string(),
            display_name: "Workflow Review".to_string(),
            route: "/workflow".to_string(),
            summary: "Review cases, approvals, and exception paths across the workbench."
                .to_string(),
            kind: WorkbenchAppKind::Review,
            status: WorkbenchAppStatus::Ready,
            capability_keys: vec!["workflow".to_string(), "approvals".to_string()],
            truth_keys: Vec::new(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use application_kernel::{
        Actor, BillingPeriod, CatalogItemUpsert, CatalogPlanKind, EntitlementTemplate, Money,
        OrganizationLifecycle, OrganizationUpsert, SubscriptionActivate, SubscriptionCreate,
        SubscriptionStatus,
    };
    use application_storage::InMemoryKernelStore;
    use prio_agent_ops::{
        AuthorityEffect, EvidenceReadinessStatus, JobVerdict, OperatorLedgerRecordKind,
        ReceiptFamily,
    };
    use uuid::Uuid;

    use super::{ApprovalFilter, ExecutionState, OperatorApp, WorkflowCaseFilter};

    struct RevenueSeed {
        organization_id: String,
        subscription_catalog_id: String,
        activation_subscription_id: String,
        refill_subscription_id: String,
    }

    fn app() -> OperatorApp<InMemoryKernelStore> {
        let store = InMemoryKernelStore::default_local();
        OperatorApp::new(store.config.clone(), store)
    }

    fn revenue_ready_app() -> (OperatorApp<InMemoryKernelStore>, RevenueSeed) {
        let app = app();
        let actor = Actor::system();
        let organization_id = seed_uuid("11111111-1111-4111-8111-111111111111");
        let subscription_catalog_id = seed_uuid("22222222-2222-4222-8222-222222222222");
        let activation_subscription_id = seed_uuid("33333333-3333-4333-8333-333333333333");
        let credits_catalog_id = seed_uuid("44444444-4444-4444-8444-444444444444");
        let refill_subscription_id = seed_uuid("55555555-5555-4555-8555-555555555555");

        app.store
            .write(|kernel| {
                kernel.upsert_organization(
                    OrganizationUpsert {
                        organization_id: Some(organization_id),
                        name: "Revenue Test".to_string(),
                        external_key: None,
                        website: None,
                        industry: Some("Software".to_string()),
                        lifecycle: OrganizationLifecycle::Active,
                        owner_user_id: Some("revops".to_string()),
                        tags: vec!["test".to_string()],
                    },
                    actor.clone(),
                )?;

                kernel.upsert_catalog_item(
                    CatalogItemUpsert {
                        catalog_item_id: Some(subscription_catalog_id),
                        sku: "prio-platform-annual".to_string(),
                        name: "Operator Workspace Annual".to_string(),
                        description: Some("Annual governed operator workspace".to_string()),
                        plan_kind: CatalogPlanKind::Subscription,
                        pricing: Some(application_kernel::PricingMetadata {
                            billing_period: BillingPeriod::Annual,
                            list_price: Money {
                                currency_code: "USD".to_string(),
                                amount_minor: 12_000_00,
                            },
                            meter_name: Some("workspace-annual".to_string()),
                        }),
                        entitlement_template: EntitlementTemplate {
                            feature_flags: vec!["workspace_access".to_string()],
                            quotas: BTreeMap::from([("seats".to_string(), 25)]),
                            credit_balance_minor: None,
                        },
                        active: true,
                    },
                    actor.clone(),
                )?;

                kernel.upsert_catalog_item(
                    CatalogItemUpsert {
                        catalog_item_id: Some(credits_catalog_id),
                        sku: "prio-ai-credits-100k".to_string(),
                        name: "AI Credits 100k".to_string(),
                        description: Some("Prepaid AI credits pack".to_string()),
                        plan_kind: CatalogPlanKind::PrepaidCredits,
                        pricing: Some(application_kernel::PricingMetadata {
                            billing_period: BillingPeriod::OneTime,
                            list_price: Money {
                                currency_code: "USD".to_string(),
                                amount_minor: 5_000_00,
                            },
                            meter_name: Some("ai-credits".to_string()),
                        }),
                        entitlement_template: EntitlementTemplate {
                            feature_flags: vec![],
                            quotas: BTreeMap::new(),
                            credit_balance_minor: Some(0),
                        },
                        active: true,
                    },
                    actor.clone(),
                )?;

                kernel.create_order_subscription(
                    SubscriptionCreate {
                        subscription_id: Some(activation_subscription_id),
                        organization_id,
                        quote_id: None,
                        catalog_item_id: Some(subscription_catalog_id),
                        status: SubscriptionStatus::PendingActivation,
                        value: Money {
                            currency_code: "USD".to_string(),
                            amount_minor: 12_000_00,
                        },
                        started_at: None,
                    },
                    actor.clone(),
                )?;

                let refill = kernel.create_order_subscription(
                    SubscriptionCreate {
                        subscription_id: Some(refill_subscription_id),
                        organization_id,
                        quote_id: None,
                        catalog_item_id: Some(credits_catalog_id),
                        status: SubscriptionStatus::PendingActivation,
                        value: Money {
                            currency_code: "USD".to_string(),
                            amount_minor: 5_000_00,
                        },
                        started_at: None,
                    },
                    actor.clone(),
                )?;

                kernel.activate_subscription(
                    SubscriptionActivate {
                        subscription_id: refill.id,
                        catalog_item_id: None,
                        opening_balance: None,
                    },
                    actor.clone(),
                )?;

                Ok(())
            })
            .expect("revenue seed should persist");

        (
            app,
            RevenueSeed {
                organization_id: organization_id.to_string(),
                subscription_catalog_id: subscription_catalog_id.to_string(),
                activation_subscription_id: activation_subscription_id.to_string(),
                refill_subscription_id: refill_subscription_id.to_string(),
            },
        )
    }

    fn seed_uuid(value: &str) -> Uuid {
        Uuid::parse_str(value).expect("valid seed uuid")
    }

    #[test]
    fn list_truths_marks_supported_truths() {
        let truths = app().list_truths();
        assert!(
            truths
                .iter()
                .filter(|truth| truth.executable)
                .map(|truth| truth.key.as_str())
                .eq([
                    "activate-subscription",
                    "qualify-inbound-lead",
                    "refill-prepaid-ai-credits",
                    "submit-expense-report"
                ]
                .into_iter())
        );
    }

    #[test]
    fn workbench_apps_expose_builtin_surfaces() {
        let apps = app().workbench_apps();
        assert_eq!(apps.first().map(|app| app.id.as_str()), Some("home"));
        assert!(
            apps.iter()
                .any(|app| app.id == "receipt-management" && app.route == "/expenses")
        );
        assert!(
            apps.iter()
                .any(|app| app.id == "notes"
                    && app.capability_keys.contains(&"documents".to_string()))
        );
    }

    #[test]
    fn operator_control_preview_exposes_non_authoritative_readiness_chain() {
        let preview = app()
            .operator_control_preview()
            .expect("operator control preview");

        assert!(preview.packet.packet_id.starts_with("helm.job_readiness."));
        assert_eq!(preview.packet.domain_hint, "tally-escrow.release");
        assert_eq!(preview.packet.job_key, "escrow-release");
        assert_eq!(preview.packet.verdict, Some(JobVerdict::Satisfied));
        assert!(!preview.packet.authorizes_domain_action);
        assert!(
            preview
                .packet
                .evidence_status
                .iter()
                .any(|status| status.clause_key == "buyer_approval")
        );
        assert!(
            preview
                .packet
                .evidence_status
                .iter()
                .any(|status| status.clause_key == "double_release_guard")
        );
        assert!(
            preview
                .packet
                .evidence_status
                .iter()
                .all(|status| status.status == EvidenceReadinessStatus::Present)
        );
        assert!(
            preview
                .packet
                .verifier_forbidden_actions
                .iter()
                .any(|action| action.contains("payment-rail authority"))
        );
        assert_eq!(preview.ledger_entries.len(), 1);
        assert_eq!(
            preview.ledger_entries[0].record_kind,
            OperatorLedgerRecordKind::JobReadinessPacket
        );
        assert_eq!(
            preview.ledger_entries[0].authority_effect,
            AuthorityEffect::None
        );
        assert!(
            preview
                .receipt_families
                .iter()
                .any(|family| family.family == ReceiptFamily::TemporalEvidence)
        );
        assert!(
            preview
                .receipt_families
                .iter()
                .any(|family| family.family == ReceiptFamily::ContentPublication)
        );
    }

    #[test]
    fn operator_control_previews_list_tally_first() {
        let previews = app()
            .operator_control_previews()
            .expect("operator control previews");

        assert_eq!(
            previews
                .iter()
                .map(|preview| preview.packet.domain_hint.as_str())
                .collect::<Vec<_>>(),
            vec![
                "tally-escrow.release",
                "quorum-sense.inquiry",
                "fathom-narrative.temporal-evidence",
                "warden-compliance.audit",
                "plumb-execution.drift",
                "atlas-integration.integration-room",
            ]
        );
        assert_eq!(previews[0].packet.domain_hint, "tally-escrow.release");
        assert_eq!(previews[0].packet.job_key, "escrow-release");
        assert_eq!(previews[0].packet.verdict, Some(JobVerdict::Satisfied));
        assert!(
            previews[0]
                .packet
                .evidence_status
                .iter()
                .all(|status| status.status == EvidenceReadinessStatus::Present)
        );

        assert_eq!(previews[1].packet.domain_hint, "quorum-sense.inquiry");
        assert_eq!(previews[1].packet.job_key, "adaptive-inquiry");
        assert_eq!(previews[1].packet.verdict, Some(JobVerdict::Blocked));
        assert!(!previews[1].packet.authorizes_domain_action);
        assert_eq!(previews[1].ledger_entries[0].sequence, 1);
        assert!(
            previews[1]
                .packet
                .evidence_status
                .iter()
                .any(
                    |status| status.clause_key == "competing_hypotheses_preserved"
                        && status.status == EvidenceReadinessStatus::Disputed
                )
        );
        assert!(
            previews[1]
                .packet
                .evidence_status
                .iter()
                .any(|status| status.clause_key == "role_coverage"
                    && status.status == EvidenceReadinessStatus::Concern)
        );
        assert!(
            previews[1]
                .packet
                .verifier_forbidden_actions
                .iter()
                .any(|action| action.contains("synthesis authority"))
        );

        assert_eq!(previews[2].packet.job_key, "temporal-filing-window");
        assert_eq!(previews[2].packet.verdict, Some(JobVerdict::Blocked));
        assert_eq!(previews[2].ledger_entries[0].sequence, 2);
        assert!(
            previews[2]
                .packet
                .evidence_status
                .iter()
                .any(|status| status.clause_key == "evidence_window"
                    && status.status == EvidenceReadinessStatus::Present)
        );
        assert!(
            previews[2]
                .packet
                .evidence_status
                .iter()
                .any(|status| status.clause_key == "segment_consistency"
                    && status.status == EvidenceReadinessStatus::Disputed)
        );

        assert_eq!(previews[3].packet.job_key, "compliance-gate-review");
        assert_eq!(previews[3].packet.verdict, Some(JobVerdict::Blocked));
        assert_eq!(previews[3].ledger_entries[0].sequence, 3);
        assert!(
            previews[3]
                .packet
                .evidence_status
                .iter()
                .any(|status| status.clause_key == "clean_attestation_guard"
                    && status.status == EvidenceReadinessStatus::Blocked)
        );

        assert_eq!(previews[4].packet.job_key, "strategy-drift-review");
        assert_eq!(previews[4].packet.verdict, Some(JobVerdict::Blocked));
        assert_eq!(previews[4].ledger_entries[0].sequence, 4);
        let fuzzy_trace = previews[4]
            .packet
            .fuzzy_trace
            .as_ref()
            .expect("plumb preview carries fuzzy drift trace");
        assert_eq!(fuzzy_trace.variable_key, "drift_severity");
        assert_eq!(fuzzy_trace.observed_value_basis_points, 6_200);
        assert!(
            fuzzy_trace
                .memberships
                .iter()
                .any(|membership| membership.label == "material"
                    && membership.score_basis_points == 3_500)
        );
        assert_eq!(
            fuzzy_trace
                .defuzzified_score
                .as_ref()
                .expect("plumb preview carries defuzzified score")
                .score_basis_points,
            3_750
        );
        assert!(
            previews[4]
                .packet
                .evidence_status
                .iter()
                .any(|status| status.clause_key == "adversarial_review"
                    && status.status == EvidenceReadinessStatus::Missing)
        );

        assert_eq!(previews[5].packet.job_key, "integration-candidate-review");
        assert_eq!(previews[5].packet.verdict, Some(JobVerdict::Blocked));
        assert_eq!(previews[5].ledger_entries[0].sequence, 5);
        assert!(
            previews[5]
                .packet
                .evidence_status
                .iter()
                .any(|status| status.clause_key == "writeback_guard"
                    && status.status == EvidenceReadinessStatus::Blocked)
        );
        assert!(
            previews
                .iter()
                .all(|preview| !preview.packet.authorizes_domain_action)
        );
    }

    #[test]
    fn truth_detail_includes_runtime_resolution_views() {
        let app = app();
        let expense_truth = app
            .truth_detail("submit-expense-report")
            .expect("expense truth detail");
        assert!(expense_truth.executable);
        assert_eq!(
            expense_truth
                .organism_resolution
                .as_ref()
                .map(|view| view.blueprint.as_deref()),
            Some(Some("procure_to_pay"))
        );
        assert!(
            expense_truth
                .organism_resolution
                .as_ref()
                .expect("organism resolution")
                .packs
                .iter()
                .any(|pack| pack.pack_name == "procurement")
        );

        let lead_truth = app
            .truth_detail("qualify-inbound-lead")
            .expect("lead truth detail");
        assert!(
            lead_truth
                .axiom_intent
                .as_ref()
                .is_some_and(|intent| !intent.outcome.trim().is_empty())
        );
        assert!(
            lead_truth
                .formation_selection
                .as_ref()
                .is_some_and(|selection| !selection.primary_template_id.trim().is_empty())
        );
        assert!(
            lead_truth
                .converge_resolution
                .as_ref()
                .expect("converge resolution")
                .pack_ids
                .contains(&"prio-commercial-pack".to_string())
        );
    }

    #[test]
    fn execute_truth_projects_happy_path() {
        let app = app();
        let session = app
            .execute_truth(
                "qualify-inbound-lead",
                HashMap::from([
                    ("organization_name".to_string(), "Northwind".to_string()),
                    (
                        "inbound_summary".to_string(),
                        "Asked for a governed CRM and audit trail.".to_string(),
                    ),
                    ("contact_name".to_string(), "Alice Doe".to_string()),
                    (
                        "opportunity_value_minor".to_string(),
                        "12000000".to_string(),
                    ),
                ]),
            )
            .expect("session should execute");

        assert_eq!(session.state, ExecutionState::Completed);
        assert!(
            session
                .projection
                .expect("projection exists")
                .opportunity_id
                .is_some()
        );
        assert_eq!(app.list_organizations().expect("organizations").len(), 1);
        assert_eq!(app.list_opportunities().expect("opportunities").len(), 1);
    }

    #[test]
    fn execute_truth_creates_blocked_workflow_and_approval() {
        let app = app();
        let session = app
            .execute_truth(
                "qualify-inbound-lead",
                HashMap::from([
                    ("organization_name".to_string(), "Apex".to_string()),
                    (
                        "inbound_summary".to_string(),
                        "Enterprise buyer needs non-standard approval.".to_string(),
                    ),
                    ("require_manual_review".to_string(), "true".to_string()),
                    (
                        "manual_review_reason".to_string(),
                        "Commercial terms exceed the standard path.".to_string(),
                    ),
                ]),
            )
            .expect("session should execute");

        assert_eq!(session.state, ExecutionState::Blocked);
        assert_eq!(
            app.list_approvals(ApprovalFilter::default())
                .expect("approvals")
                .len(),
            1
        );
        assert_eq!(
            app.list_workflow_cases(WorkflowCaseFilter::default())
                .expect("workflow cases")
                .len(),
            1
        );
    }

    #[test]
    fn execute_submit_expense_report_projects_export_ready_state() {
        let app = app();
        let session = app
            .execute_truth(
                "submit-expense-report",
                HashMap::from([
                    (
                        "organization_name".to_string(),
                        "Outcome Workbench".to_string(),
                    ),
                    (
                        "report_title".to_string(),
                        "April travel reimbursement".to_string(),
                    ),
                    ("merchant_name".to_string(), "SJ Rail".to_string()),
                    ("category".to_string(), "travel".to_string()),
                    ("amount_minor".to_string(), "12850".to_string()),
                    ("currency_code".to_string(), "SEK".to_string()),
                    ("expense_date".to_string(), "2026-04-12".to_string()),
                    (
                        "receipt_uri".to_string(),
                        "file:///receipts/sj-rail-april-12.pdf".to_string(),
                    ),
                    ("ocr_confidence_bps".to_string(), "9200".to_string()),
                ]),
            )
            .expect("expense report should execute");

        assert_eq!(session.state, ExecutionState::Completed);
        let projection = session.projection.expect("projection exists");
        assert_eq!(projection.document_ids.len(), 1);
        assert_eq!(projection.workflow_case_ids.len(), 1);
        assert!(projection.approval_ids.is_empty());
        assert_eq!(
            app.list_workflow_cases(WorkflowCaseFilter::default())
                .expect("workflow cases")
                .len(),
            1
        );
    }

    #[test]
    fn execute_submit_expense_report_opens_manual_review_when_policy_is_ambiguous() {
        let app = app();
        let session = app
            .execute_truth(
                "submit-expense-report",
                HashMap::from([
                    (
                        "organization_name".to_string(),
                        "Outcome Workbench".to_string(),
                    ),
                    (
                        "report_title".to_string(),
                        "Client dinner reimbursement".to_string(),
                    ),
                    ("merchant_name".to_string(), "Maison du Port".to_string()),
                    ("category".to_string(), "entertainment".to_string()),
                    ("amount_minor".to_string(), "98000".to_string()),
                    ("currency_code".to_string(), "EUR".to_string()),
                    ("expense_date".to_string(), "2026-04-11".to_string()),
                    (
                        "receipt_uri".to_string(),
                        "file:///receipts/maison-du-port.jpeg".to_string(),
                    ),
                    ("ocr_confidence_bps".to_string(), "6200".to_string()),
                    ("out_of_policy".to_string(), "true".to_string()),
                ]),
            )
            .expect("blocked expense report should execute");

        assert_eq!(session.state, ExecutionState::Blocked);
        assert_eq!(
            app.list_approvals(ApprovalFilter::default())
                .expect("approvals")
                .len(),
            1
        );
        assert_eq!(
            app.list_workflow_cases(WorkflowCaseFilter::default())
                .expect("workflow cases")
                .len(),
            1
        );
    }

    #[test]
    fn execute_activate_subscription_projects_revenue_state() {
        let (app, seed) = revenue_ready_app();
        let session = app
            .execute_truth(
                "activate-subscription",
                HashMap::from([
                    ("organization_id".to_string(), seed.organization_id.clone()),
                    (
                        "subscription_id".to_string(),
                        seed.activation_subscription_id.clone(),
                    ),
                    (
                        "catalog_item_id".to_string(),
                        seed.subscription_catalog_id.clone(),
                    ),
                    ("payment_confirmed".to_string(), "true".to_string()),
                ]),
            )
            .expect("activation should execute");

        assert_eq!(session.state, ExecutionState::Completed);
        assert_eq!(
            session
                .projection
                .expect("projection exists")
                .subscription_id
                .as_deref(),
            Some(seed.activation_subscription_id.as_str())
        );
    }

    #[test]
    fn execute_activate_subscription_blocks_without_payment_confirmation() {
        let (app, seed) = revenue_ready_app();
        let session = app
            .execute_truth(
                "activate-subscription",
                HashMap::from([
                    ("organization_id".to_string(), seed.organization_id.clone()),
                    (
                        "subscription_id".to_string(),
                        seed.activation_subscription_id.clone(),
                    ),
                    (
                        "catalog_item_id".to_string(),
                        seed.subscription_catalog_id.clone(),
                    ),
                    ("payment_confirmed".to_string(), "false".to_string()),
                ]),
            )
            .expect("activation should return a blocked session");

        assert_eq!(session.state, ExecutionState::Blocked);
        assert_eq!(
            app.list_approvals(ApprovalFilter::default())
                .expect("approvals")
                .len(),
            1
        );
    }

    #[test]
    fn execute_refill_prepaid_ai_credits_updates_entitlement_balance() {
        let (app, seed) = revenue_ready_app();
        let session = app
            .execute_truth(
                "refill-prepaid-ai-credits",
                HashMap::from([
                    ("organization_id".to_string(), seed.organization_id.clone()),
                    (
                        "subscription_id".to_string(),
                        seed.refill_subscription_id.clone(),
                    ),
                    ("amount_minor".to_string(), "150000".to_string()),
                    ("currency_code".to_string(), "USD".to_string()),
                    (
                        "payment_reference".to_string(),
                        "pay_test_refill".to_string(),
                    ),
                    ("payment_status".to_string(), "confirmed".to_string()),
                ]),
            )
            .expect("refill should execute");

        assert_eq!(session.state, ExecutionState::Completed);
        let account = app
            .account_summary(&seed.organization_id)
            .expect("account summary");
        assert!(
            account
                .entitlements
                .iter()
                .any(|entitlement| entitlement.key == "credit_balance_minor")
        );
    }

    #[test]
    fn execute_refill_prepaid_ai_credits_blocks_pending_payment() {
        let (app, seed) = revenue_ready_app();
        let session = app
            .execute_truth(
                "refill-prepaid-ai-credits",
                HashMap::from([
                    ("organization_id".to_string(), seed.organization_id.clone()),
                    (
                        "subscription_id".to_string(),
                        seed.refill_subscription_id.clone(),
                    ),
                    ("amount_minor".to_string(), "150000".to_string()),
                    ("currency_code".to_string(), "USD".to_string()),
                    (
                        "payment_reference".to_string(),
                        "pay_test_pending".to_string(),
                    ),
                    ("payment_status".to_string(), "pending".to_string()),
                ]),
            )
            .expect("pending refill should return a blocked session");

        assert_eq!(session.state, ExecutionState::Blocked);
        assert_eq!(
            app.list_workflow_cases(WorkflowCaseFilter::default())
                .expect("workflow cases")
                .len(),
            1
        );
    }
}
