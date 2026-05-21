use capability_core::{ApiSurface, CapabilityModule, ModuleManifest, ModuleSuite};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fmt::{self, Write as _},
};

pub struct AgentOpsModule;

pub const MODULE: CapabilityModule = CapabilityModule {
    key: "agent-ops",
    display_name: "Agent Ops",
    suite: ModuleSuite::IntelligenceCore,
    crate_name: "prio-agent-ops",
    purpose: "Agent runs, operator control, validation contracts, and execution traceability.",
    dependencies: &["workflow", "facts", "audit", "approvals", "memory"],
    owned_objects: &[
        "agent",
        "agent_run",
        "tool_invocation",
        "job_readiness_packet",
        "operator_receipt",
        "operator_ledger_entry",
        "output_contract",
        "validation_result",
    ],
    api: ApiSurface {
        grpc_package: "prio.agentops.v1",
        grpc_service: "AgentOpsService",
        openapi_tag: "AgentOps",
        openapi_base_path: "/v1/agent-ops",
        graphql_query_root: "AgentOpsQuery",
        graphql_mutation_root: "AgentOpsMutation",
    },
};

impl ModuleManifest for AgentOpsModule {
    fn module() -> CapabilityModule {
        MODULE
    }
}

/// Helm-owned read model for "can this job be trusted enough for an operator
/// to continue reviewing it?"
///
/// This is intentionally not an Axiom type. Axiom supplies packages, reports,
/// clause ids, and adapter receipts. Helm composes those with app subject refs,
/// missing-evidence actions, and operator ledger links.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobReadinessPacket {
    pub packet_id: String,
    pub package_id: String,
    pub truth_version: String,
    pub domain_hint: String,
    pub job_key: String,
    pub subject_ref: String,
    pub adapter_receipt_id: String,
    pub adapter_status: AdapterReceiptStatus,
    pub verdict: Option<JobVerdict>,
    pub authorizes_domain_action: bool,
    pub evidence_status: Vec<JobEvidenceStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fuzzy_trace: Option<FuzzyReadinessTrace>,
    pub verifier_forbidden_actions: Vec<String>,
    pub operator_actions: Vec<String>,
}

impl JobReadinessPacket {
    /// Builds a deterministic packet and enforces the Helm boundary that a
    /// readiness view never authorizes the underlying app action.
    pub fn new(input: JobReadinessPacketInput) -> Result<Self, OperatorControlError> {
        validate_nonempty("package_id", &input.package_id)?;
        validate_nonempty("truth_version", &input.truth_version)?;
        validate_nonempty("domain_hint", &input.domain_hint)?;
        validate_nonempty("job_key", &input.job_key)?;
        validate_nonempty("subject_ref", &input.subject_ref)?;
        validate_nonempty("adapter_receipt_id", &input.adapter_receipt_id)?;
        if let Some(trace) = &input.fuzzy_trace {
            validate_fuzzy_trace(trace)?;
        }
        if input.authorizes_domain_action {
            return Err(OperatorControlError::DomainActionAuthorityRequested);
        }

        let packet_id = job_readiness_packet_id(&input);
        Ok(Self {
            packet_id,
            package_id: input.package_id,
            truth_version: input.truth_version,
            domain_hint: input.domain_hint,
            job_key: input.job_key,
            subject_ref: input.subject_ref,
            adapter_receipt_id: input.adapter_receipt_id,
            adapter_status: input.adapter_status,
            verdict: input.verdict,
            authorizes_domain_action: false,
            evidence_status: input.evidence_status,
            fuzzy_trace: input.fuzzy_trace,
            verifier_forbidden_actions: input.verifier_forbidden_actions,
            operator_actions: input.operator_actions,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobReadinessPacketInput {
    pub package_id: String,
    pub truth_version: String,
    pub domain_hint: String,
    pub job_key: String,
    pub subject_ref: String,
    pub adapter_receipt_id: String,
    pub adapter_status: AdapterReceiptStatus,
    pub verdict: Option<JobVerdict>,
    pub authorizes_domain_action: bool,
    pub evidence_status: Vec<JobEvidenceStatus>,
    pub fuzzy_trace: Option<FuzzyReadinessTrace>,
    pub verifier_forbidden_actions: Vec<String>,
    pub operator_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobEvidenceStatus {
    pub clause_id: String,
    pub clause_key: String,
    pub label: String,
    pub status: EvidenceReadinessStatus,
    pub fact_ids: Vec<String>,
    pub evidence_refs: Vec<String>,
    pub trace_links: Vec<String>,
    pub concern_record_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FuzzyReadinessTrace {
    pub variable_key: String,
    pub observed_value_basis_points: u16,
    pub memberships: Vec<FuzzyMembership>,
    pub activated_rules: Vec<FuzzyRuleActivation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defuzzified_score: Option<FuzzyDefuzzifiedScore>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FuzzyMembership {
    pub label: String,
    pub score_basis_points: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FuzzyRuleActivation {
    pub rule_id: String,
    pub strength_basis_points: u16,
    pub conclusion: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FuzzyDefuzzifiedScore {
    pub method: String,
    pub score_basis_points: u16,
    pub domain_min_basis_points: u16,
    pub domain_max_basis_points: u16,
    pub domain_steps: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterReceiptStatus {
    Succeeded,
    Rejected,
}

impl AdapterReceiptStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobVerdict {
    Satisfied,
    Blocked,
    Exhausted,
    Invalid,
}

impl JobVerdict {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Satisfied => "satisfied",
            Self::Blocked => "blocked",
            Self::Exhausted => "exhausted",
            Self::Invalid => "invalid",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceReadinessStatus {
    Present,
    Missing,
    Disputed,
    Blocked,
    Concern,
}

impl EvidenceReadinessStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Missing => "missing",
            Self::Disputed => "disputed",
            Self::Blocked => "blocked",
            Self::Concern => "concern",
        }
    }
}

/// Deterministic append-only ledger entry for Helm operator-control receipts.
///
/// This is a control-plane journal entry. It stores ids, refs, hashes, and
/// backlinks; it does not store raw app transcripts and it never grants domain
/// authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorLedgerEntry {
    pub entry_id: String,
    pub sequence: u64,
    pub record_kind: OperatorLedgerRecordKind,
    pub receipt_family: ReceiptFamily,
    pub source_ref: String,
    pub package_id: String,
    pub truth_version: String,
    pub domain_hint: String,
    pub payload_hash: String,
    pub backlink_ids: Vec<String>,
    pub authority_effect: AuthorityEffect,
    pub summary: String,
}

impl OperatorLedgerEntry {
    pub fn new(input: OperatorLedgerEntryInput) -> Result<Self, OperatorControlError> {
        validate_nonempty("source_ref", &input.source_ref)?;
        validate_nonempty("package_id", &input.package_id)?;
        validate_nonempty("truth_version", &input.truth_version)?;
        validate_nonempty("domain_hint", &input.domain_hint)?;
        validate_nonempty("summary", &input.summary)?;
        validate_sha256("payload_hash", &input.payload_hash)?;
        if input.backlink_ids.iter().any(|id| id.trim().is_empty()) {
            return Err(OperatorControlError::EmptyBacklink);
        }

        let entry_id = operator_ledger_entry_id(&input);
        Ok(Self {
            entry_id,
            sequence: input.sequence,
            record_kind: input.record_kind,
            receipt_family: input.receipt_family,
            source_ref: input.source_ref,
            package_id: input.package_id,
            truth_version: input.truth_version,
            domain_hint: input.domain_hint,
            payload_hash: input.payload_hash,
            backlink_ids: input.backlink_ids,
            authority_effect: AuthorityEffect::None,
            summary: input.summary,
        })
    }
}

pub fn job_readiness_packet_payload_hash(packet: &JobReadinessPacket) -> String {
    let evidence_hash = evidence_status_hash(&packet.evidence_status);
    let fuzzy_hash = packet
        .fuzzy_trace
        .as_ref()
        .map_or_else(|| "none".to_string(), fuzzy_trace_hash);
    let forbidden_hash = string_list_hash(&packet.verifier_forbidden_actions);
    let action_hash = string_list_hash(&packet.operator_actions);
    let verdict = packet.verdict.map_or("none", JobVerdict::as_str);

    sha256_lines(&[
        "job_readiness_packet_payload",
        packet.packet_id.as_str(),
        packet.package_id.as_str(),
        packet.truth_version.as_str(),
        packet.domain_hint.as_str(),
        packet.job_key.as_str(),
        packet.subject_ref.as_str(),
        packet.adapter_receipt_id.as_str(),
        packet.adapter_status.as_str(),
        verdict,
        evidence_hash.as_str(),
        fuzzy_hash.as_str(),
        forbidden_hash.as_str(),
        action_hash.as_str(),
    ])
}

pub fn job_readiness_packet_ledger_entry(
    sequence: u64,
    packet: &JobReadinessPacket,
    backlink_ids: Vec<String>,
    summary: impl Into<String>,
) -> Result<OperatorLedgerEntry, OperatorControlError> {
    OperatorLedgerEntry::new(OperatorLedgerEntryInput {
        sequence,
        record_kind: OperatorLedgerRecordKind::JobReadinessPacket,
        receipt_family: ReceiptFamily::Common,
        source_ref: packet.packet_id.clone(),
        package_id: packet.package_id.clone(),
        truth_version: packet.truth_version.clone(),
        domain_hint: packet.domain_hint.clone(),
        payload_hash: job_readiness_packet_payload_hash(packet),
        backlink_ids,
        summary: summary.into(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorLedgerEntryInput {
    pub sequence: u64,
    pub record_kind: OperatorLedgerRecordKind,
    pub receipt_family: ReceiptFamily,
    pub source_ref: String,
    pub package_id: String,
    pub truth_version: String,
    pub domain_hint: String,
    pub payload_hash: String,
    pub backlink_ids: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorLedgerRecordKind {
    ObservationAdapterReceipt,
    JobReadinessPacket,
    OperatorDecisionReceipt,
    ApprovalReceipt,
    PlanReceipt,
    ExecutionReceipt,
    ActionReceipt,
    OutcomeReceipt,
    CorpusSnapshotReceipt,
    EvidenceWindowReceipt,
    DisagreementReceipt,
    AnalystReviewReceipt,
    NarrativeClaimReceipt,
    CanonicalStoryReceipt,
    ClaimReviewReceipt,
    EditorialApprovalReceipt,
    PublicationBoundaryReceipt,
    AppLocalReceipt,
}

impl OperatorLedgerRecordKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ObservationAdapterReceipt => "observation_adapter_receipt",
            Self::JobReadinessPacket => "job_readiness_packet",
            Self::OperatorDecisionReceipt => "operator_decision_receipt",
            Self::ApprovalReceipt => "approval_receipt",
            Self::PlanReceipt => "plan_receipt",
            Self::ExecutionReceipt => "execution_receipt",
            Self::ActionReceipt => "action_receipt",
            Self::OutcomeReceipt => "outcome_receipt",
            Self::CorpusSnapshotReceipt => "corpus_snapshot_receipt",
            Self::EvidenceWindowReceipt => "evidence_window_receipt",
            Self::DisagreementReceipt => "disagreement_receipt",
            Self::AnalystReviewReceipt => "analyst_review_receipt",
            Self::NarrativeClaimReceipt => "narrative_claim_receipt",
            Self::CanonicalStoryReceipt => "canonical_story_receipt",
            Self::ClaimReviewReceipt => "claim_review_receipt",
            Self::EditorialApprovalReceipt => "editorial_approval_receipt",
            Self::PublicationBoundaryReceipt => "publication_boundary_receipt",
            Self::AppLocalReceipt => "app_local_receipt",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptFamily {
    Common,
    LongRunningJob,
    TemporalEvidence,
    ContentPublication,
    AppLocal,
}

impl ReceiptFamily {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Common => "common",
            Self::LongRunningJob => "long_running_job",
            Self::TemporalEvidence => "temporal_evidence",
            Self::ContentPublication => "content_publication",
            Self::AppLocal => "app_local",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityEffect {
    None,
}

impl AuthorityEffect {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorControlError {
    EmptyField {
        field: &'static str,
    },
    EmptyBacklink,
    InvalidBasisPoints {
        field: &'static str,
        value: u16,
    },
    InvalidRange {
        field: &'static str,
        min: u16,
        max: u16,
    },
    InvalidCount {
        field: &'static str,
        value: u32,
    },
    InvalidSha256 {
        field: &'static str,
        value: String,
    },
    DomainActionAuthorityRequested,
}

impl fmt::Display for OperatorControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` must not be empty"),
            Self::EmptyBacklink => write!(f, "backlink ids must not contain empty values"),
            Self::InvalidBasisPoints { field, value } => {
                write!(f, "`{field}` must be between 0 and 10000, got `{value}`")
            }
            Self::InvalidRange { field, min, max } => {
                write!(f, "`{field}` must have min < max, got `{min}`..`{max}`")
            }
            Self::InvalidCount { field, value } => {
                write!(f, "`{field}` must be greater than zero, got `{value}`")
            }
            Self::InvalidSha256 { field, value } => {
                write!(f, "`{field}` must be a sha256 hash, got `{value}`")
            }
            Self::DomainActionAuthorityRequested => {
                write!(f, "job readiness packets must not authorize domain action")
            }
        }
    }
}

impl Error for OperatorControlError {}

fn job_readiness_packet_id(input: &JobReadinessPacketInput) -> String {
    let evidence_hash = evidence_status_hash(&input.evidence_status);
    let fuzzy_hash = input
        .fuzzy_trace
        .as_ref()
        .map_or_else(|| "none".to_string(), fuzzy_trace_hash);
    let forbidden_hash = string_list_hash(&input.verifier_forbidden_actions);
    let action_hash = string_list_hash(&input.operator_actions);
    let verdict = input.verdict.map_or("none", JobVerdict::as_str);

    short_id(
        "helm.job_readiness",
        &sha256_lines(&[
            "job_readiness_packet",
            input.package_id.as_str(),
            input.truth_version.as_str(),
            input.domain_hint.as_str(),
            input.job_key.as_str(),
            input.subject_ref.as_str(),
            input.adapter_receipt_id.as_str(),
            input.adapter_status.as_str(),
            verdict,
            evidence_hash.as_str(),
            fuzzy_hash.as_str(),
            forbidden_hash.as_str(),
            action_hash.as_str(),
        ]),
    )
}

fn operator_ledger_entry_id(input: &OperatorLedgerEntryInput) -> String {
    let backlinks_hash = string_list_hash(&input.backlink_ids);
    let sequence = input.sequence.to_string();
    short_id(
        "helm.ledger_entry",
        &sha256_lines(&[
            "operator_ledger_entry",
            sequence.as_str(),
            input.record_kind.as_str(),
            input.receipt_family.as_str(),
            input.source_ref.as_str(),
            input.package_id.as_str(),
            input.truth_version.as_str(),
            input.domain_hint.as_str(),
            input.payload_hash.as_str(),
            backlinks_hash.as_str(),
        ]),
    )
}

fn evidence_status_hash(statuses: &[JobEvidenceStatus]) -> String {
    let mut parts = Vec::new();
    for status in statuses {
        parts.push(status.clause_id.clone());
        parts.push(status.clause_key.clone());
        parts.push(status.label.clone());
        parts.push(status.status.as_str().to_string());
        parts.push(string_list_hash(&status.fact_ids));
        parts.push(string_list_hash(&status.evidence_refs));
        parts.push(string_list_hash(&status.trace_links));
        parts.push(string_list_hash(&status.concern_record_ids));
    }
    string_list_hash(&parts)
}

fn fuzzy_trace_hash(trace: &FuzzyReadinessTrace) -> String {
    let observed = trace.observed_value_basis_points.to_string();
    let membership_hash = fuzzy_membership_hash(&trace.memberships);
    let rule_hash = fuzzy_rule_hash(&trace.activated_rules);
    let defuzzified_hash = trace
        .defuzzified_score
        .as_ref()
        .map_or_else(|| "none".to_string(), fuzzy_defuzzified_score_hash);
    sha256_lines(&[
        "fuzzy_readiness_trace",
        trace.variable_key.as_str(),
        observed.as_str(),
        membership_hash.as_str(),
        rule_hash.as_str(),
        defuzzified_hash.as_str(),
    ])
}

fn fuzzy_membership_hash(memberships: &[FuzzyMembership]) -> String {
    let mut parts = Vec::new();
    for membership in memberships {
        parts.push(membership.label.clone());
        parts.push(membership.score_basis_points.to_string());
    }
    string_list_hash(&parts)
}

fn fuzzy_rule_hash(rules: &[FuzzyRuleActivation]) -> String {
    let mut parts = Vec::new();
    for rule in rules {
        parts.push(rule.rule_id.clone());
        parts.push(rule.strength_basis_points.to_string());
        parts.push(rule.conclusion.clone());
    }
    string_list_hash(&parts)
}

fn fuzzy_defuzzified_score_hash(score: &FuzzyDefuzzifiedScore) -> String {
    let score_basis_points = score.score_basis_points.to_string();
    let domain_min_basis_points = score.domain_min_basis_points.to_string();
    let domain_max_basis_points = score.domain_max_basis_points.to_string();
    let domain_steps = score.domain_steps.to_string();
    sha256_lines(&[
        "fuzzy_defuzzified_score",
        score.method.as_str(),
        score_basis_points.as_str(),
        domain_min_basis_points.as_str(),
        domain_max_basis_points.as_str(),
        domain_steps.as_str(),
    ])
}

fn string_list_hash(values: &[String]) -> String {
    let refs = values.iter().map(String::as_str).collect::<Vec<_>>();
    sha256_lines(&refs)
}

fn validate_fuzzy_trace(trace: &FuzzyReadinessTrace) -> Result<(), OperatorControlError> {
    validate_nonempty("fuzzy_trace.variable_key", &trace.variable_key)?;
    validate_basis_points(
        "fuzzy_trace.observed_value_basis_points",
        trace.observed_value_basis_points,
    )?;
    if trace.memberships.is_empty() {
        return Err(OperatorControlError::EmptyField {
            field: "fuzzy_trace.memberships",
        });
    }
    for membership in &trace.memberships {
        validate_nonempty("fuzzy_trace.membership.label", &membership.label)?;
        validate_basis_points(
            "fuzzy_trace.membership.score_basis_points",
            membership.score_basis_points,
        )?;
    }
    for rule in &trace.activated_rules {
        validate_nonempty("fuzzy_trace.rule.rule_id", &rule.rule_id)?;
        validate_basis_points(
            "fuzzy_trace.rule.strength_basis_points",
            rule.strength_basis_points,
        )?;
        validate_nonempty("fuzzy_trace.rule.conclusion", &rule.conclusion)?;
    }
    if let Some(score) = &trace.defuzzified_score {
        validate_nonempty("fuzzy_trace.defuzzified_score.method", &score.method)?;
        validate_basis_points(
            "fuzzy_trace.defuzzified_score.score_basis_points",
            score.score_basis_points,
        )?;
        validate_basis_points(
            "fuzzy_trace.defuzzified_score.domain_min_basis_points",
            score.domain_min_basis_points,
        )?;
        validate_basis_points(
            "fuzzy_trace.defuzzified_score.domain_max_basis_points",
            score.domain_max_basis_points,
        )?;
        if score.domain_min_basis_points >= score.domain_max_basis_points {
            return Err(OperatorControlError::InvalidRange {
                field: "fuzzy_trace.defuzzified_score.domain",
                min: score.domain_min_basis_points,
                max: score.domain_max_basis_points,
            });
        }
        if score.domain_steps == 0 {
            return Err(OperatorControlError::InvalidCount {
                field: "fuzzy_trace.defuzzified_score.domain_steps",
                value: score.domain_steps,
            });
        }
    }
    Ok(())
}

fn validate_basis_points(field: &'static str, value: u16) -> Result<(), OperatorControlError> {
    if value <= 10_000 {
        Ok(())
    } else {
        Err(OperatorControlError::InvalidBasisPoints { field, value })
    }
}

fn validate_nonempty(field: &'static str, value: &str) -> Result<(), OperatorControlError> {
    if value.trim().is_empty() {
        Err(OperatorControlError::EmptyField { field })
    } else {
        Ok(())
    }
}

fn validate_sha256(field: &'static str, value: &str) -> Result<(), OperatorControlError> {
    if value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    }) {
        Ok(())
    } else {
        Err(OperatorControlError::InvalidSha256 {
            field,
            value: value.to_string(),
        })
    }
}

fn short_id(prefix: &str, digest: &str) -> String {
    let short_digest = &digest
        .strip_prefix("sha256:")
        .expect("local digest has sha256 prefix")[..12];
    format!("{prefix}.{short_digest}")
}

fn sha256_lines(parts: &[&str]) -> String {
    sha256_bytes(parts.join("\n").as_bytes())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut output = String::with_capacity("sha256:".len() + digest.len() * 2);
    output.push_str("sha256:");
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_readiness_packet_id_is_deterministic() {
        let first = JobReadinessPacket::new(sample_packet_input()).expect("packet builds");
        let second = JobReadinessPacket::new(sample_packet_input()).expect("packet builds");

        assert_eq!(first, second);
        assert!(first.packet_id.starts_with("helm.job_readiness."));
        assert!(!first.authorizes_domain_action);
        assert_eq!(first.evidence_status.len(), 2);
    }

    #[test]
    fn job_readiness_packet_id_changes_when_evidence_changes() {
        let first = JobReadinessPacket::new(sample_packet_input()).expect("packet builds");
        let mut input = sample_packet_input();
        input.evidence_status[1].status = EvidenceReadinessStatus::Present;
        input.evidence_status[1]
            .fact_ids
            .push("folio.editorial.claim-citations".to_string());
        let second = JobReadinessPacket::new(input).expect("packet builds");

        assert_ne!(first.packet_id, second.packet_id);
    }

    #[test]
    fn job_readiness_packet_id_changes_when_fuzzy_trace_changes() {
        let mut input = sample_packet_input();
        input.fuzzy_trace = Some(sample_fuzzy_trace(6_200, 3_500));
        let first = JobReadinessPacket::new(input).expect("packet builds");
        let mut input = sample_packet_input();
        input.fuzzy_trace = Some(sample_fuzzy_trace(7_000, 5_000));
        let second = JobReadinessPacket::new(input).expect("packet builds");

        assert_ne!(first.packet_id, second.packet_id);
        assert_eq!(
            first
                .fuzzy_trace
                .as_ref()
                .expect("fuzzy trace")
                .memberships
                .len(),
            2
        );
    }

    #[test]
    fn job_readiness_packet_id_changes_when_defuzzified_score_changes() {
        let mut first_input = sample_packet_input();
        let mut first_trace = sample_fuzzy_trace(6_200, 3_500);
        first_trace.defuzzified_score = Some(sample_defuzzified_score(3_750));
        first_input.fuzzy_trace = Some(first_trace);
        let first = JobReadinessPacket::new(first_input).expect("packet builds");

        let mut second_input = sample_packet_input();
        let mut second_trace = sample_fuzzy_trace(6_200, 3_500);
        second_trace.defuzzified_score = Some(sample_defuzzified_score(4_250));
        second_input.fuzzy_trace = Some(second_trace);
        let second = JobReadinessPacket::new(second_input).expect("packet builds");

        assert_ne!(first.packet_id, second.packet_id);
        assert_eq!(
            first
                .fuzzy_trace
                .as_ref()
                .and_then(|trace| trace.defuzzified_score.as_ref())
                .expect("defuzzified score")
                .method
                .as_str(),
            "centroid"
        );
    }

    #[test]
    fn job_readiness_packet_rejects_invalid_fuzzy_scores() {
        let mut input = sample_packet_input();
        input.fuzzy_trace = Some(sample_fuzzy_trace(10_001, 3_500));

        let error = JobReadinessPacket::new(input).expect_err("invalid score");
        assert_eq!(
            error,
            OperatorControlError::InvalidBasisPoints {
                field: "fuzzy_trace.observed_value_basis_points",
                value: 10_001
            }
        );
    }

    #[test]
    fn job_readiness_packet_rejects_invalid_defuzzified_score_domain() {
        let mut input = sample_packet_input();
        let mut trace = sample_fuzzy_trace(6_200, 3_500);
        trace.defuzzified_score = Some(FuzzyDefuzzifiedScore {
            method: "centroid".to_string(),
            score_basis_points: 3_750,
            domain_min_basis_points: 10_000,
            domain_max_basis_points: 10_000,
            domain_steps: 1_000,
        });
        input.fuzzy_trace = Some(trace);

        let error = JobReadinessPacket::new(input).expect_err("invalid domain");
        assert_eq!(
            error,
            OperatorControlError::InvalidRange {
                field: "fuzzy_trace.defuzzified_score.domain",
                min: 10_000,
                max: 10_000,
            }
        );
    }

    #[test]
    fn job_readiness_packet_rejects_domain_authority() {
        let mut input = sample_packet_input();
        input.authorizes_domain_action = true;

        let error = JobReadinessPacket::new(input).expect_err("authority is rejected");
        assert_eq!(error, OperatorControlError::DomainActionAuthorityRequested);
    }

    #[test]
    fn operator_ledger_entry_is_deterministic_and_non_authoritative() {
        let first = OperatorLedgerEntry::new(sample_ledger_input()).expect("entry builds");
        let second = OperatorLedgerEntry::new(sample_ledger_input()).expect("entry builds");

        assert_eq!(first, second);
        assert!(first.entry_id.starts_with("helm.ledger_entry."));
        assert_eq!(first.authority_effect, AuthorityEffect::None);
        assert_eq!(
            first.record_kind,
            OperatorLedgerRecordKind::JobReadinessPacket
        );
        assert_eq!(first.receipt_family, ReceiptFamily::Common);
    }

    #[test]
    fn operator_ledger_entry_rejects_non_hash_payloads() {
        let mut input = sample_ledger_input();
        input.payload_hash = "raw-json-payload".to_string();

        let error = OperatorLedgerEntry::new(input).expect_err("raw payload hash is rejected");
        assert_eq!(
            error,
            OperatorControlError::InvalidSha256 {
                field: "payload_hash",
                value: "raw-json-payload".to_string()
            }
        );
    }

    #[test]
    fn operator_ledger_entry_id_changes_when_backlinks_change() {
        let first = OperatorLedgerEntry::new(sample_ledger_input()).expect("entry builds");
        let mut input = sample_ledger_input();
        input
            .backlink_ids
            .push("helm.claim_review.9b8f00ab1111".to_string());
        let second = OperatorLedgerEntry::new(input).expect("entry builds");

        assert_ne!(first.entry_id, second.entry_id);
    }

    #[test]
    fn job_readiness_packet_ledger_entry_uses_packet_payload_hash() {
        let packet = JobReadinessPacket::new(sample_packet_input()).expect("packet builds");
        let entry = job_readiness_packet_ledger_entry(
            7,
            &packet,
            vec!["artifact.adapter.abcdef012345".to_string()],
            "job readiness preview",
        )
        .expect("entry builds");

        assert_eq!(
            entry.record_kind,
            OperatorLedgerRecordKind::JobReadinessPacket
        );
        assert_eq!(entry.receipt_family, ReceiptFamily::Common);
        assert_eq!(entry.source_ref, packet.packet_id);
        assert_eq!(
            entry.payload_hash,
            job_readiness_packet_payload_hash(&packet)
        );
        assert_eq!(entry.authority_effect, AuthorityEffect::None);
    }

    fn sample_packet_input() -> JobReadinessPacketInput {
        JobReadinessPacketInput {
            package_id: "truth_package.folio.1234".to_string(),
            truth_version: "truth.v1".to_string(),
            domain_hint: "folio-editor.publication-boundary".to_string(),
            job_key: "folio-publication-package".to_string(),
            subject_ref: "folio.subject.abcdef012345".to_string(),
            adapter_receipt_id: "artifact.adapter.abcdef012345".to_string(),
            adapter_status: AdapterReceiptStatus::Succeeded,
            verdict: Some(JobVerdict::Invalid),
            authorizes_domain_action: false,
            evidence_status: vec![
                JobEvidenceStatus {
                    clause_id: "clause.evidence.1".to_string(),
                    clause_key: "canonical_story_snapshot_bound".to_string(),
                    label: "canonical story snapshot is bound".to_string(),
                    status: EvidenceReadinessStatus::Present,
                    fact_ids: vec!["folio.editorial.canonical-story".to_string()],
                    evidence_refs: vec!["evidence:folio.editorial.canonical-story".to_string()],
                    trace_links: vec!["trace:folio.editorial.canonical-story".to_string()],
                    concern_record_ids: Vec::new(),
                },
                JobEvidenceStatus {
                    clause_id: "clause.evidence.2".to_string(),
                    clause_key: "claim_citations_attached".to_string(),
                    label: "public claims carry resolving citations".to_string(),
                    status: EvidenceReadinessStatus::Missing,
                    fact_ids: Vec::new(),
                    evidence_refs: Vec::new(),
                    trace_links: Vec::new(),
                    concern_record_ids: vec!["calibration.concern.123".to_string()],
                },
            ],
            fuzzy_trace: None,
            verifier_forbidden_actions: vec![
                "public package is published without editorial approval".to_string(),
            ],
            operator_actions: vec![
                "inspect axiom report".to_string(),
                "request missing evidence for claim_citations_attached".to_string(),
            ],
        }
    }

    fn sample_fuzzy_trace(
        observed_value_basis_points: u16,
        material_score_basis_points: u16,
    ) -> FuzzyReadinessTrace {
        FuzzyReadinessTrace {
            variable_key: "drift_severity".to_string(),
            observed_value_basis_points,
            memberships: vec![
                FuzzyMembership {
                    label: "moderate".to_string(),
                    score_basis_points: 4_000,
                },
                FuzzyMembership {
                    label: "material".to_string(),
                    score_basis_points: material_score_basis_points,
                },
            ],
            activated_rules: vec![FuzzyRuleActivation {
                rule_id: "revision-trigger-on-materializing-drift".to_string(),
                strength_basis_points: material_score_basis_points,
                conclusion: "revision_urgency:advisable".to_string(),
            }],
            defuzzified_score: None,
        }
    }

    fn sample_defuzzified_score(score_basis_points: u16) -> FuzzyDefuzzifiedScore {
        FuzzyDefuzzifiedScore {
            method: "centroid".to_string(),
            score_basis_points,
            domain_min_basis_points: 0,
            domain_max_basis_points: 10_000,
            domain_steps: 1_000,
        }
    }

    fn sample_ledger_input() -> OperatorLedgerEntryInput {
        OperatorLedgerEntryInput {
            sequence: 1,
            record_kind: OperatorLedgerRecordKind::JobReadinessPacket,
            receipt_family: ReceiptFamily::Common,
            source_ref: "helm.job_readiness.abcdef012345".to_string(),
            package_id: "truth_package.folio.1234".to_string(),
            truth_version: "truth.v1".to_string(),
            domain_hint: "folio-editor.publication-boundary".to_string(),
            payload_hash: "sha256:90b8fb64fdd6f926a4ef42d67a145215aa7e7e07480863217f8558c472da579f"
                .to_string(),
            backlink_ids: vec!["artifact.adapter.abcdef012345".to_string()],
            summary: "job readiness Invalid for folio-publication-package".to_string(),
        }
    }
}
