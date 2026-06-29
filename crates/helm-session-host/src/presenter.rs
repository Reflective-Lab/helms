// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

//! Quorum-specific [`DomainPresenter`] — maps opaque payloads to director copy.

use helm_client::{DomainPresenter, GateCopy};
use director_contracts::NowTask;
use helm_client::formation::LocalFormationIntent;
use helm_session_contracts::GatedDecision;

/// Default presenter for Quorum server-side director projection.
///
/// Reads optional string fields from opaque gate/push payloads when present;
/// otherwise supplies neutral copy so live snapshots remain usable before
/// per-session copy is fully curated.
#[derive(Debug, Clone, Copy, Default)]
pub struct QuorumDomainPresenter;

impl DomainPresenter for QuorumDomainPresenter {
    fn now_task(&self, intent: &LocalFormationIntent) -> NowTask {
        NowTask {
            objective: intent.description.clone(),
            needed_from_user: None,
            estimated_minutes: None,
        }
    }

    fn gate_copy(&self, gate: &GatedDecision) -> GateCopy {
        let reason = gate
            .payload
            .get("reason")
            .or_else(|| gate.payload.get("title"))
            .and_then(|value| value.as_str())
            .unwrap_or("Decision required")
            .to_string();
        let consequence = gate
            .payload
            .get("consequence")
            .or_else(|| gate.payload.get("body"))
            .and_then(|value| value.as_str())
            .unwrap_or("Formation paused until resolved")
            .to_string();
        GateCopy { reason, consequence }
    }

    fn idle_title(&self) -> String {
        "Waiting for session activity".into()
    }
}
