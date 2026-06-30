// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

use crate::formation::{LocalFormationIntent, SeedContext};
use crate::ids::LoopId;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum RegistryError {
    #[error("loop {0} not found")]
    NotFound(String),
    #[error("loop {0} is not in Running state")]
    NotRunning(String),
    #[error("loop {0} is not in Paused state")]
    NotPaused(String),
    #[error("a sequential loop is already Running")]
    AlreadyRunning,
}

/// Where a registered formation actually runs.
///
/// `Local` formations run Converge on this device and contend for the single
/// local-running slot. `ServerHandle` entries run on the **server** — the device
/// only tracks and surfaces them — so they never occupy the local slot. This is
/// the "DD job while I wait" case: heavy/parallel work is offloaded, not run on
/// device. `server_formation_id` is an opaque handle assigned by the server
/// (kept as a `String` here to preserve the crate's zero-Converge-deps rule).
#[derive(Debug, Clone)]
pub enum LoopKind {
    Local,
    ServerHandle { server_formation_id: String },
}

/// Current lifecycle state of a local formation.
#[derive(Debug, Clone)]
pub enum LoopState {
    Running,
    Paused { injected_context: SeedContext },
    Completed(Vec<serde_json::Value>),
    Failed(String),
}

/// A single formation entry in the registry.
#[derive(Debug, Clone)]
pub struct LoopEntry {
    pub loop_id: LoopId,
    /// Local (runs here) or ServerHandle (runs on the server, tracked here).
    pub kind: LoopKind,
    /// Opaque, Converge-free representation of the formation's RootIntent.
    pub intent: LocalFormationIntent,
    pub formation_type: String,
    pub seed_context: SeedContext,
    pub state: LoopState,
}

/// Read-only view of a LoopEntry for UI / FFI exposure.
#[derive(Debug, Clone)]
pub struct LoopEntryView {
    pub loop_id: String,
    pub formation_type: String,
    pub description: String,
    pub state_label: &'static str,
    /// "local" or "server_handle".
    pub kind_label: &'static str,
    /// Present only for ServerHandle entries.
    pub server_formation_id: Option<String>,
}

/// Manages the lifecycle of formations the participant is running.
///
/// Invariant: at most one `Local` entry is `Running` at a time. `ServerHandle`
/// entries run on the server and never occupy the local-running slot, so any
/// number may coexist with the single local formation.
pub struct LoopRegistry {
    entries: HashMap<String, LoopEntry>,
}

impl LoopRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Spawn a sequential **local** formation. Fails if one is already Running.
    pub fn try_spawn_sequential(
        &mut self,
        formation_type: String,
        seed_context: SeedContext,
    ) -> Result<LoopId, RegistryError> {
        if self.running_entry().is_some() {
            return Err(RegistryError::AlreadyRunning);
        }
        Ok(self.insert(formation_type, seed_context, LoopKind::Local))
    }

    /// Spawn a **local** formation unconditionally (use when no loop is running).
    #[must_use]
    pub fn spawn(&mut self, formation_type: String, seed_context: SeedContext) -> LoopId {
        self.insert(formation_type, seed_context, LoopKind::Local)
    }

    /// Record a **server-side** formation the participant spawned (e.g. the "DD
    /// job while I wait"). It runs on the server; the device only tracks it. It
    /// never occupies the local-running slot, so it is always allowed.
    /// `server_formation_id` is the opaque handle the server assigned.
    #[must_use]
    pub fn spawn_server_handle(
        &mut self,
        server_formation_id: String,
        formation_type: String,
        seed_context: SeedContext,
    ) -> LoopId {
        self.insert(
            formation_type,
            seed_context,
            LoopKind::ServerHandle {
                server_formation_id,
            },
        )
    }

    fn insert(
        &mut self,
        formation_type: String,
        seed_context: SeedContext,
        kind: LoopKind,
    ) -> LoopId {
        let id = LoopId::new();
        let intent = LocalFormationIntent::new(seed_context.description.clone());
        self.entries.insert(
            id.as_str().to_string(),
            LoopEntry {
                loop_id: id.clone(),
                kind,
                intent,
                formation_type,
                seed_context,
                state: LoopState::Running,
            },
        );
        id
    }

    /// Pause a Running entry and inject server context.
    pub fn pause(
        &mut self,
        loop_id: &LoopId,
        injected_context: SeedContext,
    ) -> Result<(), RegistryError> {
        let entry = self
            .entries
            .get_mut(loop_id.as_str())
            .ok_or_else(|| RegistryError::NotFound(loop_id.to_string()))?;
        if !matches!(entry.state, LoopState::Running) {
            return Err(RegistryError::NotRunning(loop_id.to_string()));
        }
        entry.state = LoopState::Paused { injected_context };
        Ok(())
    }

    /// Resume a Paused entry.
    pub fn resume(&mut self, loop_id: &LoopId) -> Result<(), RegistryError> {
        let entry = self
            .entries
            .get_mut(loop_id.as_str())
            .ok_or_else(|| RegistryError::NotFound(loop_id.to_string()))?;
        if !matches!(entry.state, LoopState::Paused { .. }) {
            return Err(RegistryError::NotPaused(loop_id.to_string()));
        }
        entry.state = LoopState::Running;
        Ok(())
    }

    /// Mark a formation as completed.
    pub fn complete(
        &mut self,
        loop_id: &LoopId,
        proposals: Vec<serde_json::Value>,
    ) -> Result<(), RegistryError> {
        let entry = self
            .entries
            .get_mut(loop_id.as_str())
            .ok_or_else(|| RegistryError::NotFound(loop_id.to_string()))?;
        entry.state = LoopState::Completed(proposals);
        Ok(())
    }

    /// Mark a formation as failed.
    pub fn fail(&mut self, loop_id: &LoopId, reason: String) -> Result<(), RegistryError> {
        let entry = self
            .entries
            .get_mut(loop_id.as_str())
            .ok_or_else(|| RegistryError::NotFound(loop_id.to_string()))?;
        entry.state = LoopState::Failed(reason);
        Ok(())
    }

    /// The single `Local` Running entry, if any. `ServerHandle` entries are
    /// never counted — they run on the server, not the local-running slot.
    #[must_use]
    pub fn running_entry(&self) -> Option<&LoopEntry> {
        self.entries
            .values()
            .find(|e| matches!(e.kind, LoopKind::Local) && matches!(e.state, LoopState::Running))
    }

    #[must_use]
    pub fn get(&self, loop_id: &LoopId) -> Option<&LoopEntry> {
        self.entries.get(loop_id.as_str())
    }

    /// All entries — for UI inspection.
    #[must_use]
    pub fn entries(&self) -> Vec<&LoopEntry> {
        self.entries.values().collect()
    }

    /// Read-only views for FFI / UI.
    #[must_use]
    pub fn entry_views(&self) -> Vec<LoopEntryView> {
        self.entries
            .values()
            .map(|e| LoopEntryView {
                loop_id: e.loop_id.as_str().to_string(),
                formation_type: e.formation_type.clone(),
                description: e.intent.description.clone(),
                state_label: match &e.state {
                    LoopState::Running => "running",
                    LoopState::Paused { .. } => "paused",
                    LoopState::Completed(_) => "completed",
                    LoopState::Failed(_) => "failed",
                },
                kind_label: match &e.kind {
                    LoopKind::Local => "local",
                    LoopKind::ServerHandle { .. } => "server_handle",
                },
                server_formation_id: match &e.kind {
                    LoopKind::Local => None,
                    LoopKind::ServerHandle {
                        server_formation_id,
                    } => Some(server_formation_id.clone()),
                },
            })
            .collect()
    }
}

impl Default for LoopRegistry {
    fn default() -> Self {
        Self::new()
    }
}
