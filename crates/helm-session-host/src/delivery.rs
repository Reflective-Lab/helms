// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

use helm_session_contracts::{FindingId, ParticipantId, SessionPush};

/// One delivery record per (participant, finding).
pub struct DeliveryRecord {
    pub push: SessionPush,
    pub delivered_at_version: u64,
    pub delivery_acked_at_ms: Option<u64>,
    pub completed_acked_at_ms: Option<u64>,
    pub produced_output: Option<bool>,
}

/// Tracks Disruptive/Preemptive push delivery per (session_id, ParticipantId, FindingId).
/// Only these urgencies are tracked; Informational and Advisory are fire-and-forget.
#[derive(Default)]
pub struct DeliveryTable {
    records: std::collections::HashMap<(String, ParticipantId, FindingId), DeliveryRecord>,
}

impl DeliveryTable {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that a push was published to a participant at a given hub sequence.
    pub fn record(
        &mut self,
        session_id: &str,
        participant_id: ParticipantId,
        finding_id: FindingId,
        push: SessionPush,
        version: u64,
    ) {
        let key = (session_id.to_string(), participant_id, finding_id);
        self.records.entry(key).or_insert(DeliveryRecord {
            push,
            delivered_at_version: version,
            delivery_acked_at_ms: None,
            completed_acked_at_ms: None,
            produced_output: None,
        });
    }

    /// Mark delivery as acked. Returns `true` if the record exists (idempotent on
    /// already-acked). Returns `false` if no record exists for this key.
    pub fn ack_delivery(
        &mut self,
        session_id: &str,
        participant_id: &ParticipantId,
        finding_id: &FindingId,
        now_ms: u64,
    ) -> bool {
        let key = (
            session_id.to_string(),
            participant_id.clone(),
            finding_id.clone(),
        );
        let Some(record) = self.records.get_mut(&key) else {
            return false;
        };
        if record.delivery_acked_at_ms.is_none() {
            record.delivery_acked_at_ms = Some(now_ms);
        }
        true
    }

    /// Mark completion as acked. Returns `true` if the record exists, `false` if not.
    pub fn ack_completion(
        &mut self,
        session_id: &str,
        participant_id: &ParticipantId,
        finding_id: &FindingId,
        produced_output: bool,
        now_ms: u64,
    ) -> bool {
        let key = (
            session_id.to_string(),
            participant_id.clone(),
            finding_id.clone(),
        );
        let Some(record) = self.records.get_mut(&key) else {
            return false;
        };
        if record.completed_acked_at_ms.is_none() {
            record.completed_acked_at_ms = Some(now_ms);
            record.produced_output = Some(produced_output);
        }
        true
    }

    /// Return (finding_id, delivered_at_version, push) for all unacked records in
    /// `session_id` for `participant_id` where `delivered_at_version <= max_version`.
    #[must_use]
    pub fn unacked_for_replay(
        &self,
        session_id: &str,
        participant_id: &ParticipantId,
        max_version: u64,
    ) -> Vec<(FindingId, u64, SessionPush)> {
        self.records
            .iter()
            .filter(|((sid, pid, _), record)| {
                sid == session_id
                    && pid == participant_id
                    && record.delivery_acked_at_ms.is_none()
                    && record.delivered_at_version <= max_version
            })
            .map(|((_, _, fid), record)| {
                (
                    fid.clone(),
                    record.delivered_at_version,
                    record.push.clone(),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use helm_session_contracts::{SessionContext, UrgencyIntent};

    fn push(session_id: &str, finding_id: FindingId, urgency: UrgencyIntent) -> SessionPush {
        SessionPush {
            finding_id,
            urgency_intent: urgency,
            payload: serde_json::json!({"test": true}),
            session_context: SessionContext {
                session_id: session_id.to_string(),
                phase: "hypothesis".into(),
                cycle: 1,
                timestamp_ms: 1,
            },
        }
    }

    fn participant(id: &str) -> ParticipantId {
        ParticipantId::from_string(id)
    }

    #[test]
    fn record_and_unacked_for_replay_returns_push() {
        let mut table = DeliveryTable::new();
        let fid = FindingId::from_string("f-1");
        let p = push("sess", fid.clone(), UrgencyIntent::Disruptive);
        table.record("sess", participant("p-1"), fid.clone(), p, 5);

        let unacked = table.unacked_for_replay("sess", &participant("p-1"), 10);
        assert_eq!(unacked.len(), 1);
        assert_eq!(unacked[0].0.as_str(), "f-1");
        assert_eq!(unacked[0].1, 5);
    }

    #[test]
    fn unacked_for_replay_excludes_versions_after_cursor() {
        let mut table = DeliveryTable::new();
        let fid = FindingId::from_string("f-2");
        let p = push("sess", fid.clone(), UrgencyIntent::Preemptive);
        table.record("sess", participant("p-1"), fid.clone(), p, 20);

        // cursor = 15, finding was published at version 20 → not eligible
        let unacked = table.unacked_for_replay("sess", &participant("p-1"), 15);
        assert!(unacked.is_empty());
    }

    #[test]
    fn delivery_ack_marks_record_idempotent() {
        let mut table = DeliveryTable::new();
        let fid = FindingId::from_string("f-3");
        let p = push("sess", fid.clone(), UrgencyIntent::Disruptive);
        table.record("sess", participant("p-1"), fid.clone(), p, 3);

        assert!(table.ack_delivery("sess", &participant("p-1"), &fid, 1000));
        // Second ack is idempotent — returns true, no panic
        assert!(table.ack_delivery("sess", &participant("p-1"), &fid, 2000));

        // After ack, not returned in unacked_for_replay
        let unacked = table.unacked_for_replay("sess", &participant("p-1"), 100);
        assert!(unacked.is_empty());
    }

    #[test]
    fn delivery_ack_returns_false_for_unknown_finding() {
        let mut table = DeliveryTable::new();
        let fid = FindingId::from_string("f-unknown");
        assert!(!table.ack_delivery("sess", &participant("p-1"), &fid, 1000));
    }

    #[test]
    fn completion_ack_sets_produced_output_and_timestamp() {
        let mut table = DeliveryTable::new();
        let fid = FindingId::from_string("f-4");
        let p = push("sess", fid.clone(), UrgencyIntent::Preemptive);
        table.record("sess", participant("p-1"), fid.clone(), p, 7);

        assert!(table.ack_completion("sess", &participant("p-1"), &fid, true, 5000));
        let key = ("sess".to_string(), participant("p-1"), fid);
        let record = table.records.get(&key).unwrap();
        assert_eq!(record.completed_acked_at_ms, Some(5000));
        assert_eq!(record.produced_output, Some(true));
    }

    #[test]
    fn completion_ack_returns_false_for_unknown_finding() {
        let mut table = DeliveryTable::new();
        let fid = FindingId::from_string("f-unknown");
        assert!(!table.ack_completion("sess", &participant("p-1"), &fid, false, 1000));
    }

    #[test]
    fn unacked_for_replay_only_returns_undelivery_acked_records() {
        let mut table = DeliveryTable::new();
        let fid_a = FindingId::from_string("f-a");
        let fid_b = FindingId::from_string("f-b");
        let pa = push("sess", fid_a.clone(), UrgencyIntent::Disruptive);
        let pb = push("sess", fid_b.clone(), UrgencyIntent::Preemptive);
        table.record("sess", participant("p-1"), fid_a.clone(), pa, 1);
        table.record("sess", participant("p-1"), fid_b.clone(), pb, 2);

        // Ack delivery for f-a only
        table.ack_delivery("sess", &participant("p-1"), &fid_a, 999);

        let unacked = table.unacked_for_replay("sess", &participant("p-1"), 100);
        assert_eq!(unacked.len(), 1);
        assert_eq!(unacked[0].0.as_str(), "f-b");
    }

    #[test]
    fn empty_table_returns_no_unacked_findings() {
        let table = DeliveryTable::new();
        let unacked = table.unacked_for_replay("sess", &participant("p-1"), u64::MAX);
        assert!(unacked.is_empty());
    }

    #[test]
    fn different_participants_are_tracked_independently() {
        let mut table = DeliveryTable::new();
        let fid = FindingId::from_string("f-5");
        let p1 = push("sess", fid.clone(), UrgencyIntent::Disruptive);
        let p2 = push("sess", fid.clone(), UrgencyIntent::Disruptive);
        table.record("sess", participant("p-1"), fid.clone(), p1, 4);
        table.record("sess", participant("p-2"), fid.clone(), p2, 4);

        // Ack for p-1 only
        table.ack_delivery("sess", &participant("p-1"), &fid, 1000);

        assert!(
            table
                .unacked_for_replay("sess", &participant("p-1"), 100)
                .is_empty()
        );
        assert_eq!(
            table
                .unacked_for_replay("sess", &participant("p-2"), 100)
                .len(),
            1
        );
    }
}
