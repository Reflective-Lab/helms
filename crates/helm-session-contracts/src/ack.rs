// crates/helm-session-contracts/src/ack.rs
// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

use crate::finding::FindingId;
use crate::participant::ParticipantId;
use serde::{Deserialize, Serialize};

/// Body for `POST /v1/sessions/{id}/ack/delivery`.
/// Native layer sends this immediately when `ClientHelmAction::SpawnFormation`
/// or `ClientHelmAction::PauseAndInject` is returned by `handle_push`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryAck {
    pub participant_id: ParticipantId,
    pub finding_id: FindingId,
}

/// Body for `POST /v1/sessions/{id}/ack/completion`.
/// Native layer sends this when a formation completes with no temperature signal
/// (i.e., `drain_submissions()` returns no `ClientSubmission::Temperature` entry
/// referencing this finding). When a temperature signal IS produced, the ack is
/// carried implicitly via `PendingSubmission::triggered_by`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionAck {
    pub participant_id: ParticipantId,
    pub finding_id: FindingId,
    pub produced_output: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivery_ack_round_trips() {
        let ack = DeliveryAck {
            participant_id: ParticipantId::from_string("p-1"),
            finding_id: FindingId::from_string("f-1"),
        };
        let json = serde_json::to_string(&ack).unwrap();
        let back: DeliveryAck = serde_json::from_str(&json).unwrap();
        assert_eq!(back.participant_id.as_str(), "p-1");
        assert_eq!(back.finding_id.as_str(), "f-1");
    }

    #[test]
    fn completion_ack_no_output_round_trips() {
        let ack = CompletionAck {
            participant_id: ParticipantId::from_string("p-2"),
            finding_id: FindingId::from_string("f-2"),
            produced_output: false,
        };
        let json = serde_json::to_string(&ack).unwrap();
        let back: CompletionAck = serde_json::from_str(&json).unwrap();
        assert!(!back.produced_output);
    }
}
