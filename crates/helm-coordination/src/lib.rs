//! helm-coordination — multi-operator coordination for Helm's headless surface.
//!
//! Helm's operator surface is already headless: mountable `HelmModule`s hosted
//! by Runtime Runway. This crate makes that surface usable by several operators
//! at once under an **optimistic** model:
//!
//! - **Sessions** ([`SessionRegistry`]) — who is connected, heartbeat-leased.
//! - **Presence + soft-claims** ([`PresenceRegistry`]) — who is looking at what;
//!   claims are advisory hints, never locks.
//! - **Optimistic decision ledger** ([`DecisionLedger`]) — any authorized
//!   operator may decide a gate; the first decision drives the job, an identical
//!   repeat is idempotent, a divergent one is a conflict.
//! - **Coordination stream** — sessions, presence, and attributed gate/job
//!   events on one workspace-scoped SSE stream.
//!
//! Boundary: Runtime Runway authenticates; Helm consumes identity through the
//! [`PrincipalResolver`] seam and owns session/presence/approval semantics.
//! Coordination is non-authority over domain and commercial state.

#![allow(clippy::result_large_err)]

mod error;
mod events;
mod http;
mod ledger;
mod module;
mod presence;
mod principal;
mod service;
mod session;
mod subject;

pub use error::CoordinationError;
pub use events::{
    is_coordination_type, is_job_type, CLAIM_ACQUIRED, CLAIM_RELEASED, DECISION_CONFLICT,
    DECISION_DENIED, DECISION_RECORDED, PRESENCE_FOCUS_CHANGED, PRESENCE_JOINED, PRESENCE_LEFT,
    SESSION_CLOSED, SESSION_OPENED,
};
pub use ledger::{
    AuthorityResolver, DecisionLedger, DecisionOutcome, DecisionRecord, GateDecisionKind,
    PermissiveAuthority,
};
pub use module::CoordinationModule;
pub use presence::{PresenceChange, PresenceEntry, PresenceRegistry};
pub use principal::{OperatorPrincipal, PrincipalClaim, PrincipalResolver, RequestActorResolver};
pub use service::CoordinationService;
pub use session::{Session, SessionRegistry, DEFAULT_SESSION_LEASE};
pub use subject::SubjectRef;

// Re-export the readiness contract types so consumers can name the module's
// state without depending on helm-module-contracts directly.
pub use helm_module_contracts::{
    HelmModuleReadiness as CoordinationModuleReadiness, HelmModuleState as CoordinationModuleState,
    HelmModuleStatus as CoordinationModuleStatus,
};
