// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

use crate::urgency::UrgencyIntent;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Opaque identifier for a coordinator finding.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FindingId(String);

impl FindingId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    #[must_use]
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for FindingId {
    fn default() -> Self {
        Self::new()
    }
}

/// The class of finding the coordinator detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingType {
    ContradictionDetected,
    ConsensusEmerging,
    HighConvictionDissent,
    EvidenceGap,
    HypothesisReady,
    UncertaintyCluster,
}

/// A finding from the CoordinatorSuggestor, after promotion to Fact.
///
/// `P` is the domain payload — opaque to Helms, rendered by the client app.
/// Helms routes this; it never inspects `payload`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinatorFinding<P> {
    pub finding_id: FindingId,
    pub finding_type: FindingType,
    /// Domain-specific payload — opaque to Helms and Client Helm.
    pub payload: P,
    /// Derived from evidence topology by CoordinatorSuggestor. Never set by Helms.
    pub urgency_intent: UrgencyIntent,
    /// When true, this finding paused the main formation (requires_human = true).
    pub requires_human: bool,
    /// ActorIds of participants who should receive this finding.
    pub target_participants: Vec<String>,
}
