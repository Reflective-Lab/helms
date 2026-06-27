// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

/// Input context injected into a local formation as seed facts.
///
/// `facts` are opaque JSON values — the native layer knows how to
/// marshal them into ProposedFacts for the local Converge formation.
/// `helm-client` never inspects them.
#[derive(Debug, Clone)]
pub struct SeedContext {
    pub facts: Vec<serde_json::Value>,
    pub description: String,
}

/// Output from a completed local formation.
///
/// `proposals` are opaque JSON values submitted to the server admission
/// boundary by the native layer. `helm-client` never inspects them.
#[derive(Debug, Clone)]
pub struct FormationOutput {
    pub proposals: Vec<serde_json::Value>,
    /// Optional temperature reading derived from the formation's fixed point.
    /// If present, the native layer submits it as a TemperatureSignal.
    pub temperature: Option<TemperatureReading>,
}

/// Position + conviction derived from a local formation's output.
/// The native layer converts this into a server-bound ProposedFact.
#[derive(Debug, Clone)]
pub struct TemperatureReading {
    /// "agree" | "disagree" | "uncertain" | "need_more_evidence"
    pub position: String,
    /// "low" | "medium" | "high" | "critical"
    pub conviction: String,
    /// SubjectRef string — what this temperature is about.
    pub subject_ref: String,
}

/// Converge-free, opaque representation of a local formation's `RootIntent`.
///
/// The spec's `LoopEntry` carries a `TypesRootIntent`
/// (`converge_core::types::intent`). `helm-client` has **zero Converge deps**, so
/// it cannot hold that type. This carries exactly what Client Helm needs from the
/// intent: a `description` to display, and the two **engine-enforced** budgets
/// (`max_cycles`, `max_facts`) the native layer uses to configure the local
/// Converge formation. The full `TypesRootIntent` is reconstructed native-side
/// from this carrier. (The wall-clock budget is separate — Converge does not
/// enforce `time_limit`; see `WallClockGuard` in Task 4b.)
#[derive(Debug, Clone)]
pub struct LocalFormationIntent {
    pub description: String,
    pub max_cycles: u32,
    pub max_facts: u32,
}

impl LocalFormationIntent {
    /// Default light on-device budgets for a personal formation.
    #[must_use]
    pub fn new(description: String) -> Self {
        Self {
            description,
            max_cycles: 8,
            max_facts: 64,
        }
    }
}
