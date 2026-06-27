// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

/// How urgently the server coordinator wants a participant to respond.
///
/// Derived by CoordinatorSuggestor from evidence topology changes.
/// Never assigned by Helms — always passed through from the promoted Fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UrgencyIntent {
    /// FYI — no action required; client app may surface as ambient state.
    Informational,
    /// Worth attention when convenient; do not interrupt active work.
    Advisory,
    /// Spawn a parallel local formation; surface prominently.
    Disruptive,
    /// Suspend the active local formation; on accept, spawn a FRESH formation
    /// seeded with its accumulated context + this context. Not a Converge resume.
    Preemptive,
}
