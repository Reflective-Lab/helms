//! Optimistic gate-decision ledger and authority seam.
//!
//! Coordination is optimistic: any authorized operator may decide a gate. The
//! ledger makes that conflict-safe after the fact:
//!
//! - the first decision for a gate `ref_id` is **recorded** and is the one that
//!   drives the governed job;
//! - an identical later decision is **idempotent** (returns the original
//!   receipt, no second side-effect);
//! - a divergent later decision is a **conflict** (rejected; nothing changes).

use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use helm_governed_jobs::GateDecision;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::principal::OperatorPrincipal;
use crate::subject::SubjectRef;

/// An operator's decision on a HITL gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GateDecisionKind {
    Approve,
    Reject,
}

impl GateDecisionKind {
    /// Map into the governed-jobs gate decision that signals the run waiter.
    #[must_use]
    pub fn to_gate_decision(self) -> GateDecision {
        match self {
            Self::Approve => GateDecision::Approved,
            Self::Reject => GateDecision::Rejected,
        }
    }
}

/// A recorded, attributed gate decision.
#[derive(Debug, Clone, Serialize)]
pub struct DecisionRecord {
    pub decision_id: Uuid,
    pub ref_id: String,
    pub principal: OperatorPrincipal,
    pub decision: GateDecisionKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub recorded_at: DateTime<Utc>,
}

/// Result of attempting to record a decision.
#[derive(Debug, Clone)]
pub enum DecisionOutcome {
    /// First decision for this gate. The caller should drive the job.
    Recorded(DecisionRecord),
    /// An identical decision already existed. No new side-effect; original returned.
    Idempotent(DecisionRecord),
    /// A divergent decision already existed. Rejected; nothing changed.
    Conflict {
        existing: DecisionRecord,
        attempted: GateDecisionKind,
        attempted_by: OperatorPrincipal,
    },
}

/// Append-once-per-gate decision ledger.
#[derive(Debug, Default)]
pub struct DecisionLedger {
    inner: Mutex<HashMap<String, DecisionRecord>>,
}

impl DecisionLedger {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a decision under optimistic conflict rules.
    pub fn record(
        &self,
        ref_id: impl Into<String>,
        principal: OperatorPrincipal,
        decision: GateDecisionKind,
        note: Option<String>,
    ) -> DecisionOutcome {
        let ref_id = ref_id.into();
        let mut ledger = self.guard();
        if let Some(existing) = ledger.get(&ref_id) {
            if existing.decision == decision {
                return DecisionOutcome::Idempotent(existing.clone());
            }
            return DecisionOutcome::Conflict {
                existing: existing.clone(),
                attempted: decision,
                attempted_by: principal,
            };
        }

        let record = DecisionRecord {
            decision_id: Uuid::new_v4(),
            ref_id: ref_id.clone(),
            principal,
            decision,
            note,
            recorded_at: Utc::now(),
        };
        ledger.insert(ref_id, record.clone());
        DecisionOutcome::Recorded(record)
    }

    #[must_use]
    pub fn get(&self, ref_id: &str) -> Option<DecisionRecord> {
        self.guard().get(ref_id).cloned()
    }

    fn guard(&self) -> std::sync::MutexGuard<'_, HashMap<String, DecisionRecord>> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Decides whether a principal may decide a given subject.
///
/// The first increment ships [`PermissiveAuthority`]. A `RoleAuthority` backed
/// by the kernel `Role`/`WorkspaceMember` scaffolding is a documented follow-up.
pub trait AuthorityResolver: Send + Sync + 'static {
    fn can_decide(&self, principal: &OperatorPrincipal, subject: &SubjectRef) -> bool;
}

/// Allows any resolved principal to decide. Default for the first increment.
#[derive(Debug, Clone, Default)]
pub struct PermissiveAuthority;

impl AuthorityResolver for PermissiveAuthority {
    fn can_decide(&self, _principal: &OperatorPrincipal, _subject: &SubjectRef) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use application_kernel::ActorKind;

    fn principal(actor: &str) -> OperatorPrincipal {
        OperatorPrincipal::new(actor, actor, ActorKind::Human, "ws-1")
    }

    #[test]
    fn first_decision_is_recorded() {
        let ledger = DecisionLedger::new();
        let outcome = ledger.record("g1", principal("alice"), GateDecisionKind::Approve, None);
        assert!(matches!(outcome, DecisionOutcome::Recorded(_)));
        assert!(ledger.get("g1").is_some());
    }

    #[test]
    fn identical_later_decision_is_idempotent() {
        let ledger = DecisionLedger::new();
        ledger.record("g1", principal("alice"), GateDecisionKind::Approve, None);
        let outcome = ledger.record("g1", principal("bob"), GateDecisionKind::Approve, None);
        match outcome {
            DecisionOutcome::Idempotent(record) => {
                // The original decider is preserved.
                assert_eq!(record.principal.actor_id, "alice");
            }
            other => panic!("expected idempotent, got {other:?}"),
        }
    }

    #[test]
    fn divergent_later_decision_conflicts() {
        let ledger = DecisionLedger::new();
        ledger.record("g1", principal("alice"), GateDecisionKind::Approve, None);
        let outcome = ledger.record("g1", principal("bob"), GateDecisionKind::Reject, None);
        match outcome {
            DecisionOutcome::Conflict {
                existing,
                attempted,
                attempted_by,
            } => {
                assert_eq!(existing.decision, GateDecisionKind::Approve);
                assert_eq!(attempted, GateDecisionKind::Reject);
                assert_eq!(attempted_by.actor_id, "bob");
            }
            other => panic!("expected conflict, got {other:?}"),
        }
        // The original decision is untouched.
        assert_eq!(
            ledger.get("g1").unwrap().decision,
            GateDecisionKind::Approve
        );
    }

    #[test]
    fn permissive_authority_allows_everyone() {
        let authority = PermissiveAuthority;
        assert!(authority.can_decide(&principal("alice"), &SubjectRef::gate("g1")));
    }
}
