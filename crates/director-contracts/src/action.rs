// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

use crate::context::ContextLevel;
use helm_session_contracts::gate::GateId;
use serde::{Deserialize, Serialize};

/// A gate verdict the human can return. **Intentionally `Approve` / `Reject`
/// only** — it mirrors `helm_session_contracts::GatedDecision` today. A
/// "defer / later" verdict must be added to the Helms gate contract FIRST; it
/// must never exist as a UI-only choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateVerdict {
    Approve,
    Reject,
}

/// Stance on a focused evidence review. Maps to a temperature signal server-side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStance {
    Agree,
    Disagree,
    NeedMoreContext,
}

/// The typed intent the native UI sends back. Every interactive director surface
/// maps a user choice to exactly one of these — the UI never invents action
/// strings, and there is no verdict here the Helms contracts cannot honor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DirectorIntent {
    OpenTask { frame_id: String },
    SubmitJudgment { frame_id: String, choice_id: String },
    RespondGate { gate_id: GateId, verdict: GateVerdict },
    SubmitReview { frame_id: String, stance: ReviewStance },
    RequestContext { level: ContextLevel },
}

/// The single privileged action the UI should offer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimaryAction {
    pub label: String,
    pub intent: DirectorIntent,
}

/// An escape hatch / secondary affordance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecondaryAction {
    pub label: String,
    pub intent: DirectorIntent,
}
