// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

//! Host-side state and functions for the WASM sandbox.
//!
//! This module defines the `HostState` that lives inside each `wasmtime::Store`
//! and the host function implementations that guest modules may import.
//!
//! # Host Functions
//!
//! | Function            | Capability Required | Description |
//! |---------------------|---------------------|-------------|
//! | `host_read_context` | `ReadContext`        | Read facts by `ContextKey` ordinal |
//! | `host_log`          | `Log`               | Structured logging |
//! | `host_now_millis`   | `Clock`             | Deterministic logical clock |

use super::contract::*;

/// Host-side state associated with each WASM module invocation.
///
/// Lives inside the `wasmtime::Store<HostState>` and is accessible
/// from host function implementations via `Caller::data()`.
pub struct HostState {
    /// Read-only context snapshot for this invocation.
    pub(crate) context: GuestContext,
    /// Resource quota for this invocation.
    pub(crate) quota: WasmQuota,
    /// Granted capabilities.
    pub(crate) capabilities: Vec<HostCapability>,
    /// Host call counter.
    pub(crate) host_call_count: u32,
    /// Host call records for tracing.
    pub(crate) host_calls: Vec<HostCallRecord>,
    /// Total result bytes written.
    pub(crate) result_bytes: u64,
    /// Log entries collected during execution.
    pub(crate) log_entries: Vec<LogEntry>,
}

/// A log entry produced by a guest module via `host_log`.
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// Log level (0=trace, 1=debug, 2=info, 3=warn, 4=error).
    pub level: u32,
    /// Log message.
    pub message: String,
}

impl HostState {
    /// Create a new host state for a WASM invocation.
    pub fn new(context: GuestContext, quota: WasmQuota, capabilities: Vec<HostCapability>) -> Self {
        Self {
            context,
            quota,
            capabilities,
            host_call_count: 0,
            host_calls: Vec::new(),
            result_bytes: 0,
            log_entries: Vec::new(),
        }
    }

    /// Check if a capability has been granted.
    pub fn has_capability(&self, cap: HostCapability) -> bool {
        self.capabilities.contains(&cap)
    }

    /// Increment host call counter, return false if quota exceeded.
    pub fn check_host_call_quota(&mut self) -> bool {
        self.host_call_count += 1;
        self.host_call_count <= self.quota.max_host_calls
    }

    /// Record a host call for tracing.
    pub fn record_host_call(
        &mut self,
        function: &str,
        args_summary: &str,
        duration_us: u64,
        success: bool,
    ) {
        self.host_calls.push(HostCallRecord {
            function: function.to_string(),
            args_summary: args_summary.to_string(),
            duration_us,
            success,
        });
    }

    /// Get deterministic elapsed time for trace ordering.
    pub fn elapsed_us(&self) -> u64 {
        u64::from(self.host_call_count)
    }

    /// Return a Lamport-like logical millisecond value for this invocation.
    pub fn logical_now_millis(&self) -> i64 {
        let value = self
            .context
            .version
            .saturating_mul(1_000)
            .saturating_add(u64::from(self.context.cycle))
            .saturating_add(u64::from(self.host_call_count));

        value.min(i64::MAX as u64) as i64
    }

    /// Map a ContextKey ordinal to its string name.
    ///
    /// Ordinals match the order in `converge_core::ContextKey`:
    /// 0=Seeds, 1=Hypotheses, 2=Strategies, 3=Constraints,
    /// 4=Signals, 5=Competitors, 6=Evaluations.
    ///
    /// Returns `None` for invalid ordinals. Proposals (7) and
    /// Diagnostic (8) are intentionally blocked.
    pub fn context_key_name(ordinal: u32) -> Option<&'static str> {
        match ordinal {
            0 => Some("Seeds"),
            1 => Some("Hypotheses"),
            2 => Some("Strategies"),
            3 => Some("Constraints"),
            4 => Some("Signals"),
            5 => Some("Competitors"),
            6 => Some("Evaluations"),
            _ => None, // Proposals (7) and Diagnostic (8) intentionally blocked
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_context() -> GuestContext {
        GuestContext {
            facts: HashMap::new(),
            version: 1,
            cycle: 0,
        }
    }

    #[test]
    fn host_state_creation() {
        let state = HostState::new(
            test_context(),
            WasmQuota::default(),
            vec![HostCapability::ReadContext, HostCapability::Log],
        );

        assert!(state.has_capability(HostCapability::ReadContext));
        assert!(state.has_capability(HostCapability::Log));
        assert!(!state.has_capability(HostCapability::Clock));
        assert_eq!(state.host_call_count, 0);
    }

    #[test]
    fn host_call_quota_enforcement() {
        let mut state = HostState::new(
            test_context(),
            WasmQuota {
                max_host_calls: 3,
                ..WasmQuota::default()
            },
            vec![],
        );

        assert!(state.check_host_call_quota()); // 1 <= 3
        assert!(state.check_host_call_quota()); // 2 <= 3
        assert!(state.check_host_call_quota()); // 3 <= 3
        assert!(!state.check_host_call_quota()); // 4 > 3
    }

    #[test]
    fn context_key_ordinal_mapping() {
        assert_eq!(HostState::context_key_name(0), Some("Seeds"));
        assert_eq!(HostState::context_key_name(1), Some("Hypotheses"));
        assert_eq!(HostState::context_key_name(2), Some("Strategies"));
        assert_eq!(HostState::context_key_name(3), Some("Constraints"));
        assert_eq!(HostState::context_key_name(4), Some("Signals"));
        assert_eq!(HostState::context_key_name(5), Some("Competitors"));
        assert_eq!(HostState::context_key_name(6), Some("Evaluations"));
        // Proposals and Diagnostic blocked
        assert_eq!(HostState::context_key_name(7), None);
        assert_eq!(HostState::context_key_name(8), None);
        assert_eq!(HostState::context_key_name(99), None);
    }

    #[test]
    fn host_call_recording() {
        let mut state = HostState::new(test_context(), WasmQuota::default(), vec![]);

        state.record_host_call("host_read_context", "key=Seeds", 5, true);
        state.record_host_call("host_log", "level=2", 1, true);

        assert_eq!(state.host_calls.len(), 2);
        assert_eq!(state.host_calls[0].function, "host_read_context");
        assert!(state.host_calls[0].success);
    }
}
