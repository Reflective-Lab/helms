//! What an operator is coordinating on.
//!
//! A [`SubjectRef`] is an advisory pointer used by presence, soft-claims, and
//! gate decisions. It is intentionally free-form (`kind` is a string) so apps
//! can attach presence to any coordination target without a schema change.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A reference to a coordination subject (a gate, a job run, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SubjectRef {
    pub kind: String,
    pub id: String,
}

impl SubjectRef {
    pub fn new(kind: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            id: id.into(),
        }
    }

    /// A HITL approval gate, keyed by its scoped `ref_id`.
    pub fn gate(ref_id: impl Into<String>) -> Self {
        Self::new("gate", ref_id)
    }

    /// A governed job run.
    pub fn run(run_id: impl Into<String>) -> Self {
        Self::new("run", run_id)
    }
}

impl fmt::Display for SubjectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind, self.id)
    }
}
