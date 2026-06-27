// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

use uuid::Uuid;

/// Unique identifier for a local formation entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LoopId(String);

impl LoopId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for LoopId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for LoopId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
