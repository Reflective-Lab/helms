// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use helm_session_contracts::FindingId;

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
    /// The FindingId of the SessionPush that triggered the formation which
    /// produced this temperature signal. `None` when the formation was not
    /// triggered by an inbound push (e.g. user-initiated). The server reads
    /// this to close the completion delivery record without a separate ack call.
    pub triggered_by: Option<FindingId>,
}

/// Queue for outbound temperature signals.
/// Deduplicated by idempotency key; drain → submit → if fails, re-enqueue.
pub struct TemperatureQueue {
    pending: HashMap<String, (TemperatureSignal, Option<FindingId>)>,
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
    pub fn enqueue(
        &mut self,
        signal: TemperatureSignal,
        idempotency_key: String,
        triggered_by: Option<FindingId>,
    ) {
        if self.pending.contains_key(&idempotency_key) {
            return;
        }
        self.order.push(idempotency_key.clone());
        self.pending.insert(idempotency_key, (signal, triggered_by));
    }

    /// Consume all pending signals. Queue is empty after this call.
    #[must_use]
    pub fn drain(&mut self) -> Vec<PendingSubmission> {
        let mut out = Vec::with_capacity(self.order.len());
        for key in self.order.drain(..) {
            if let Some((signal, triggered_by)) = self.pending.remove(&key) {
                out.push(PendingSubmission {
                    signal,
                    idempotency_key: key,
                    triggered_by,
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

    fn signal(position: &str) -> TemperatureSignal {
        TemperatureSignal {
            position: position.into(),
            conviction: "high".into(),
            subject_ref: "quorum://hypothesis/h-1".into(),
        }
    }

    #[test]
    fn enqueue_and_drain() {
        let mut q = TemperatureQueue::new();
        q.enqueue(signal("agree"), "key-1".into(), None);
        let drained = q.drain();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].idempotency_key, "key-1");
        assert!(drained[0].triggered_by.is_none());
    }

    #[test]
    fn triggered_by_is_preserved_through_drain() {
        let mut q = TemperatureQueue::new();
        let fid = FindingId::from_string("find-42");
        q.enqueue(signal("agree"), "key-2".into(), Some(fid.clone()));
        let drained = q.drain();
        assert_eq!(drained[0].triggered_by.as_ref().unwrap().as_str(), "find-42");
    }

    #[test]
    fn drain_is_empty_after_call() {
        let mut q = TemperatureQueue::new();
        q.enqueue(signal("disagree"), "key-3".into(), None);
        let _ = q.drain();
        assert!(q.drain().is_empty());
    }

    #[test]
    fn duplicate_key_is_deduplicated() {
        let mut q = TemperatureQueue::new();
        q.enqueue(signal("agree"), "key-dup".into(), None);
        q.enqueue(signal("agree"), "key-dup".into(), None);
        assert_eq!(q.drain().len(), 1);
    }
}
