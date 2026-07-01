// crates/helm-session-contracts/src/participant.rs
// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

/// Stable identity for a session participant.
///
/// Derived by the native layer as `sha256_hex(firebase_uid + ":" + device_install_id)`.
/// Opaque on the wire. The server validates membership but never generates this id.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ParticipantId(String);

impl ParticipantId {
    #[must_use]
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn participant_id_round_trips() {
        let id = ParticipantId::from_string("user-123:device-abc");
        let json = serde_json::to_string(&id).unwrap();
        let back: ParticipantId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn participant_id_as_str() {
        let id = ParticipantId::from_string("uid:did");
        assert_eq!(id.as_str(), "uid:did");
    }
}
