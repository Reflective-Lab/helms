// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

/// A participant's position and conviction on a specific subject.
/// Sent to the server admission boundary as a ProposedFact.
#[derive(Debug, Clone)]
pub struct TemperatureSignal {
    /// "agree" | "disagree" | "uncertain" | "need_more_evidence"
    pub position: String,
    /// "low" | "medium" | "high" | "critical"
    pub conviction: String,
    /// SubjectRef string — e.g. "quorum://hypothesis/h-1"
    pub subject_ref: String,
}

/// A temperature signal ready for submission to the server.
#[derive(Debug, Clone)]
pub struct PendingSubmission {
    pub signal: TemperatureSignal,
    pub idempotency_key: String,
}

/// Queue for outbound temperature signals.
/// Deduplicated by idempotency key; drain → submit → if fails, re-enqueue.
pub struct TemperatureQueue {
    pending: HashMap<String, TemperatureSignal>,
    order: Vec<String>,
}

impl TemperatureQueue {
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
            order: Vec::new(),
        }
    }

    /// Enqueue a signal. If the key already exists, the existing entry is kept (idempotent).
    pub fn enqueue(&mut self, signal: TemperatureSignal, idempotency_key: String) {
        if self.pending.contains_key(&idempotency_key) {
            return;
        }
        self.order.push(idempotency_key.clone());
        self.pending.insert(idempotency_key, signal);
    }

    /// Consume all pending signals. Queue is empty after this call.
    #[must_use]
    pub fn drain(&mut self) -> Vec<PendingSubmission> {
        let mut out = Vec::with_capacity(self.order.len());
        for key in self.order.drain(..) {
            if let Some(signal) = self.pending.remove(&key) {
                out.push(PendingSubmission {
                    signal,
                    idempotency_key: key,
                });
            }
        }
        out
    }
}

impl Default for TemperatureQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enqueue_and_drain() {
        let mut q = TemperatureQueue::new();
        q.enqueue(
            TemperatureSignal {
                position: "agree".into(),
                conviction: "high".into(),
                subject_ref: "quorum://hypothesis/h-1".into(),
            },
            "key-1".into(),
        );
        let drained = q.drain();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].idempotency_key, "key-1");
    }

    #[test]
    fn drain_is_empty_after_call() {
        let mut q = TemperatureQueue::new();
        q.enqueue(
            TemperatureSignal {
                position: "disagree".into(),
                conviction: "critical".into(),
                subject_ref: "quorum://hypothesis/h-2".into(),
            },
            "key-2".into(),
        );
        let _ = q.drain();
        assert!(q.drain().is_empty());
    }

    #[test]
    fn duplicate_key_is_deduplicated() {
        let mut q = TemperatureQueue::new();
        let sig = TemperatureSignal {
            position: "agree".into(),
            conviction: "low".into(),
            subject_ref: "quorum://hypothesis/h-3".into(),
        };
        q.enqueue(sig.clone(), "key-dup".into());
        q.enqueue(sig, "key-dup".into());
        assert_eq!(q.drain().len(), 1);
    }
}
