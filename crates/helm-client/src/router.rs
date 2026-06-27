// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

use crate::formation::SeedContext;
use crate::ids::LoopId;
use helm_session_contracts::urgency::UrgencyIntent;

/// The action Client Helm instructs the native layer to take.
#[derive(Debug)]
pub enum RoutingDecision {
    /// B — no active loop: spawn a new local formation with these seeds.
    SpawnNew { seed_context: SeedContext },
    /// B — a local formation is already running: offload this work to the
    /// **server** as a sub-formation (tracked as a `ServerHandle`). It does NOT
    /// spawn a second local formation — at most one local formation runs at a
    /// time, and heavy/parallel work belongs on the server. The native layer
    /// requests the sub-formation; when the server returns an id it calls
    /// `ClientHelm::server_formation_started` to record the handle.
    OffloadToServer { seed_context: SeedContext },
    /// Queue this push and show a UI notification; active loop continues.
    QueueAndNotify {
        urgency: UrgencyIntent,
        seed_context: SeedContext,
    },
    /// C — suspend the active loop and capture server context. On user accept,
    /// the native layer spawns a FRESH formation seeded with the suspended
    /// loop's accumulated context + this injected context. This is local
    /// registry bookkeeping, NOT a Converge `Engine::resume`.
    PauseAndInject {
        loop_id_to_pause: LoopId,
        injected_context: SeedContext,
    },
}

/// Stateless router: maps (urgency, active loop, context) → routing decision.
///
/// Routing table from spec Section 4:
/// - No running loop (any urgency) → SpawnNew (local)
/// - Running + Informational / Advisory → QueueAndNotify
/// - Running + Disruptive → OffloadToServer (server sub-formation, ServerHandle — never a second local loop)
/// - Running + Preemptive → PauseAndInject (suspend + fresh spawn, NOT Converge Engine::resume)
pub struct SeverityRouter;

impl SeverityRouter {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Decide what to do with an incoming SessionPush.
    ///
    /// `active_loop_id` is the LoopId of the currently Running sequential
    /// formation, if any.
    #[must_use]
    pub fn decide(
        &self,
        urgency: UrgencyIntent,
        active_loop_id: Option<&LoopId>,
        seed_context: SeedContext,
    ) -> RoutingDecision {
        let Some(running_id) = active_loop_id else {
            return RoutingDecision::SpawnNew { seed_context };
        };

        match urgency {
            UrgencyIntent::Informational | UrgencyIntent::Advisory => {
                RoutingDecision::QueueAndNotify { urgency, seed_context }
            }
            UrgencyIntent::Disruptive => RoutingDecision::OffloadToServer { seed_context },
            UrgencyIntent::Preemptive => RoutingDecision::PauseAndInject {
                loop_id_to_pause: running_id.clone(),
                injected_context: seed_context,
            },
        }
    }
}

impl Default for SeverityRouter {
    fn default() -> Self {
        Self::new()
    }
}
