// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Opaque identifier for a HITL gate.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GateId(String);

impl GateId {
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

impl Default for GateId {
    fn default() -> Self {
        Self::new()
    }
}

/// Condition that must be satisfied before the main formation resumes.
///
/// Compiled from Axiom Truth / Gherkin definitions. These variants are
/// the runtime representation of Axiom-generated validators.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GateCondition {
    QuorumOfRoles { roles: Vec<String> },
    SpecificAuthority { actor_id: String },
    AnyParticipant,
    Unanimous,
}

/// A HITL gate event sent from Server Session Helm when requires_human = true.
///
/// The main formation is already paused at a Converge `RunResult::HitlPause`.
/// The user's response is sent to the server, which delivers it as a
/// `GateDecision` to `Engine::resume` (approve promotes the held proposal,
/// reject discards it). This is a verdict on the paused proposal — Client Helm
/// does not resume the formation directly, and this is NOT the admission path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatedDecision {
    pub gate_id: GateId,
    pub condition: GateCondition,
    /// Domain payload — opaque. The client app renders it.
    pub payload: serde_json::Value,
    /// Unix timestamp ms — None means no deadline.
    pub deadline: Option<u64>,
}
