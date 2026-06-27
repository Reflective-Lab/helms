// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

use crate::ids::LoopId;
use std::collections::HashMap;

/// Per-loop wall-clock budget guard.
///
/// Converge enforces `max_cycles`/`max_facts` but not `time_limit`; the engine
/// loop never inspects wall-clock. This guard bounds on-device formations.
/// It is pure — it holds no clock. Callers supply `started_at_ms` (from the
/// triggering `SessionPush.session_context.timestamp_ms`) and `now_ms` (from
/// the native layer) on each tick.
#[derive(Debug, Default)]
pub struct WallClockGuard {
    armed: HashMap<String, ArmedBudget>,
}

#[derive(Debug, Clone)]
struct ArmedBudget {
    loop_id: LoopId,
    started_at_ms: u64,
    max_ms: u64,
}

impl WallClockGuard {
    #[must_use]
    pub fn new() -> Self {
        Self {
            armed: HashMap::new(),
        }
    }

    /// Arm a wall-clock budget for a loop. `started_at_ms` is the spawn time.
    pub fn arm(&mut self, loop_id: &LoopId, started_at_ms: u64, max_ms: u64) {
        self.armed.insert(
            loop_id.as_str().to_string(),
            ArmedBudget {
                loop_id: loop_id.clone(),
                started_at_ms,
                max_ms,
            },
        );
    }

    /// Disarm a loop (completed, failed, or paused — no longer time-bounded).
    pub fn disarm(&mut self, loop_id: &LoopId) {
        self.armed.remove(loop_id.as_str());
    }

    /// Loop ids whose budget has elapsed at `now_ms`. Expired loops are
    /// disarmed, so each is reported at most once.
    #[must_use]
    pub fn expired(&mut self, now_ms: u64) -> Vec<LoopId> {
        let keys: Vec<String> = self
            .armed
            .iter()
            .filter(|(_, b)| now_ms.saturating_sub(b.started_at_ms) >= b.max_ms)
            .map(|(k, _)| k.clone())
            .collect();
        keys.iter()
            .filter_map(|k| self.armed.remove(k))
            .map(|b| b.loop_id)
            .collect()
    }

    #[must_use]
    pub fn is_armed(&self, loop_id: &LoopId) -> bool {
        self.armed.contains_key(loop_id.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lid() -> LoopId {
        LoopId::new()
    }

    #[test]
    fn new_guard_has_nothing_armed() {
        let mut g = WallClockGuard::new();
        assert!(g.expired(u64::MAX).is_empty());
    }

    #[test]
    fn armed_budget_not_expired_before_max() {
        let mut g = WallClockGuard::new();
        let id = lid();
        g.arm(&id, 1_000, 5_000);
        assert!(g.expired(3_000).is_empty());
        assert!(g.is_armed(&id));
    }

    #[test]
    fn armed_budget_expires_at_or_after_max() {
        let mut g = WallClockGuard::new();
        let id = lid();
        g.arm(&id, 1_000, 5_000);
        let expired = g.expired(6_000);
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].as_str(), id.as_str());
    }

    #[test]
    fn expired_loops_are_disarmed_once() {
        let mut g = WallClockGuard::new();
        let id = lid();
        g.arm(&id, 0, 1_000);
        assert_eq!(g.expired(2_000).len(), 1);
        assert!(g.expired(2_000).is_empty());
        assert!(!g.is_armed(&id));
    }

    #[test]
    fn disarm_removes_budget() {
        let mut g = WallClockGuard::new();
        let id = lid();
        g.arm(&id, 0, 1_000);
        g.disarm(&id);
        assert!(g.expired(u64::MAX).is_empty());
    }
}
