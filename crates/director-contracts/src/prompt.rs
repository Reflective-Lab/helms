// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

use helm_session_contracts::gate::{GateCondition, GateId};
use serde::{Deserialize, Serialize};

/// One bounded choice in a prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub choice_id: String,
    pub label: String,
}

/// A focused human judgment with bounded choices (≤3 on mobile-first surfaces).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgmentPrompt {
    pub question: String,
    pub body: String,
    pub choices: Vec<Choice>,
}

/// Render projection of `helm_session_contracts::GatedDecision` / `GateCondition`
/// — **not** a second gate model. `gate_id` correlates the user's
/// `DirectorIntent::RespondGate` back to the originating gate. Renderable
/// choices are limited to contract-backed verdicts (`GateVerdict`); there is no
/// "later" until the Helms gate contract gains it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatePrompt {
    pub gate_id: GateId,
    pub reason: String,
    pub consequence: String,
    pub deadline_ms: Option<u64>,
    pub condition: GateCondition,
}

/// A focused evidence review. Resolves to `DirectorIntent::SubmitReview`, which
/// the projector maps to a temperature signal. Every stance it offers is
/// contract-backed (`ReviewStance`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewPrompt {
    pub title: String,
    pub primary_evidence: String,
}

/// Exactly one focused ask. Each variant maps to a concrete `DirectorIntent`,
/// so no prompt can present a verdict the contracts cannot honor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DirectorPrompt {
    Judgment(JudgmentPrompt),
    Gate(GatePrompt),
    Review(ReviewPrompt),
}
