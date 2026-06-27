// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

/// How far out the user has chosen to look. Default surface is `Task`; the user
/// escapes outward only on request (the Now Principle).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextLevel {
    Task,
    LocalContext,
    Session,
    Formation,
    Organization,
    Everything,
}

/// Minimal "someone else is here" awareness. Opaque labels — the renderer does
/// not interpret them as identities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceHint {
    pub actor_label: String,
    /// e.g. "viewing" | "deciding" | "away" — an opaque status label.
    pub status: String,
}
