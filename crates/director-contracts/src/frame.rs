// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

use crate::action::{PrimaryAction, SecondaryAction};
use crate::context::{ContextLevel, PresenceHint};
use crate::prompt::DirectorPrompt;
use serde::{Deserialize, Serialize};

/// Who the current scene is blocked on.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WaitingFor {
    Nobody,
    Participants { actor_labels: Vec<String> },
    Server,
}

/// How hard the current moment blocks progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockingState {
    NotBlocking,
    BlocksFormation,
    BlocksSession,
}

/// The one task requiring human attention, in human terms.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NowTask {
    pub objective: String,
    pub needed_from_user: Option<String>,
    pub estimated_minutes: Option<u32>,
}

/// The current scene the user should see. Computed by `helm-client` from ordered
/// SSE / session / gate / loop state; **rendered, never computed** by Swift /
/// Kotlin / Svelte. This is the Rust→FFI/UI projection boundary — distinct from
/// `helm-session-contracts` (the server↔client wire boundary).
///
/// Domain-readable fields (`title`, `subtitle`, `now`, prompt copy) are filled by
/// the per-app FFI via `helm-client`'s `DomainPresenter` seam, because
/// `helm-client` treats session payloads as opaque.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectorFrame {
    pub frame_id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub now: Option<NowTask>,
    pub waiting_for: WaitingFor,
    pub primary: PrimaryAction,
    pub secondary: Vec<SecondaryAction>,
    pub prompt: Option<DirectorPrompt>,
    pub presence: Vec<PresenceHint>,
    pub context_trail: Vec<ContextLevel>,
    pub blocking: BlockingState,
}

/// An immutable, versioned snapshot. `version` is the upstream SSE `sequence` the
/// frame was computed at (the `runway-app-host` hub sequence consumed by
/// `helm-client`) — **not** a new mobile counter — so ordering and dedup are
/// consistent end-to-end. `helm-client` produces this; `mobile-core` may wrap or
/// re-export it as its FFI envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectorSnapshot {
    pub version: u64,
    pub frame: DirectorFrame,
}
