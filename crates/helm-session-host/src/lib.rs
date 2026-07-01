// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

//! helm-session-host — server-side Session Helm for the Session Intelligence Spine.
//!
//! Receives promoted coordinator findings (later slices), manages delivery state,
//! and emits [`SessionPush`] on the shared [`runway_app_host::EventHubHandle`].
//! Transport is delegated to upstream Runway (`sse::event_stream`); this crate
//! owns session-host semantics only.
//!
//! Plan 2 slice 1: wire types + SSE mount test. See
//! `KB/08-roadmap/2026-06-26-spine-plan-2-helm-session-host.md`.

mod delivery;
mod events;
mod host;
mod http;
mod module;
mod presenter;
mod service;
mod store;
mod types;

pub use events::{
    SESSION_GATE_OPENED, SESSION_GATE_RESOLVED, SESSION_PUSH, is_session_host_type, publish_push,
};
pub use host::mount_session_host;
pub use module::SessionHostModule;
pub use presenter::QuorumDomainPresenter;
pub use service::SessionHostService;
pub use types::{DecisionSessionId, SessionHostState};

pub use helm_module_contracts::{
    HelmModuleReadiness as SessionHostModuleReadiness, HelmModuleState as SessionHostModuleState,
    HelmModuleStatus as SessionHostModuleStatus,
};

// Re-export wire types consumers need at the host boundary.
pub use helm_session_contracts::{
    CoordinatorFinding, FindingId, FindingType, GateCondition, GateId, GatedDecision,
    SessionContext, SessionPush, UrgencyIntent,
};
