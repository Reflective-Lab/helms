//! Operator identity for the coordination surface.
//!
//! Helm consumes identity; it does not authenticate. Runtime Runway owns
//! authentication and (eventually) verified claims. The [`PrincipalResolver`]
//! trait is the seam between that upstream identity and Helm's coordination
//! model: today [`RequestActorResolver`] trusts the self-declared actor on the
//! request (the current platform behavior), and a future `RunwayClaimsResolver`
//! can swap in verified claims from `HostContext` without changing callers.

use application_kernel::{Actor, ActorKind};
use serde::{Deserialize, Serialize};

use crate::error::CoordinationError;

/// An identified operator acting within a workspace.
///
/// Derived from [`application_kernel::Actor`] plus the workspace the operator is
/// coordinating in. Coordination is non-authority over domain/commerce state; a
/// principal only scopes presence, sessions, and gate-decision attribution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorPrincipal {
    pub actor_id: String,
    pub display_name: String,
    pub kind: ActorKind,
    pub workspace_id: String,
}

impl OperatorPrincipal {
    pub fn new(
        actor_id: impl Into<String>,
        display_name: impl Into<String>,
        kind: ActorKind,
        workspace_id: impl Into<String>,
    ) -> Self {
        Self {
            actor_id: actor_id.into(),
            display_name: display_name.into(),
            kind,
            workspace_id: workspace_id.into(),
        }
    }

    /// Project into a kernel [`Actor`] for downstream domain calls.
    #[must_use]
    pub fn to_actor(&self) -> Actor {
        Actor {
            actor_id: self.actor_id.clone(),
            display_name: self.display_name.clone(),
            kind: self.kind,
        }
    }

    /// Stable string used to stamp `EventEnvelope.actor`.
    #[must_use]
    pub fn actor_tag(&self) -> String {
        format!(
            "{}:{}",
            workspace_or_global(&self.workspace_id),
            self.actor_id
        )
    }
}

fn workspace_or_global(workspace_id: &str) -> &str {
    if workspace_id.is_empty() {
        "global"
    } else {
        workspace_id
    }
}

/// The self-declared identity material attached to an inbound request.
///
/// This is the raw claim before resolution. A resolver decides whether to trust
/// it (request-actor mode) or to ignore it in favor of verified upstream claims.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrincipalClaim {
    #[serde(default)]
    pub actor_id: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub kind: Option<ActorKind>,
    #[serde(default)]
    pub workspace_id: Option<String>,
}

/// Resolves a [`PrincipalClaim`] into a verified [`OperatorPrincipal`].
///
/// The identity seam. Implementations decide the trust model. Helm never
/// authenticates here; a production resolver reads identity that Runtime Runway
/// has already authenticated.
pub trait PrincipalResolver: Send + Sync + 'static {
    fn resolve(&self, claim: &PrincipalClaim) -> Result<OperatorPrincipal, CoordinationError>;
}

/// Trusts the self-declared actor on the request.
///
/// This mirrors the current platform behavior (`approvals.rs` trusts
/// `body.actor`). It is the default for the first coordination increment. Swap
/// it for a claims-backed resolver once Runtime Runway exposes verified
/// identity through `HostContext`.
#[derive(Debug, Clone, Default)]
pub struct RequestActorResolver;

impl PrincipalResolver for RequestActorResolver {
    fn resolve(&self, claim: &PrincipalClaim) -> Result<OperatorPrincipal, CoordinationError> {
        let actor_id = claim
            .actor_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| CoordinationError::MissingIdentity("actor_id is required".to_string()))?
            .to_string();

        let workspace_id = claim
            .workspace_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                CoordinationError::MissingIdentity("workspace_id is required".to_string())
            })?
            .to_string();

        let display_name = claim
            .display_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| actor_id.clone());

        Ok(OperatorPrincipal {
            actor_id,
            display_name,
            kind: claim.kind.unwrap_or(ActorKind::Human),
            workspace_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_resolver_trusts_full_claim() {
        let resolver = RequestActorResolver;
        let principal = resolver
            .resolve(&PrincipalClaim {
                actor_id: Some("alice".into()),
                display_name: Some("Alice".into()),
                kind: Some(ActorKind::Human),
                workspace_id: Some("ws-1".into()),
            })
            .expect("claim resolves");
        assert_eq!(principal.actor_id, "alice");
        assert_eq!(principal.display_name, "Alice");
        assert_eq!(principal.workspace_id, "ws-1");
    }

    #[test]
    fn request_resolver_defaults_display_name_to_actor_id() {
        let resolver = RequestActorResolver;
        let principal = resolver
            .resolve(&PrincipalClaim {
                actor_id: Some("bob".into()),
                display_name: None,
                kind: None,
                workspace_id: Some("ws-1".into()),
            })
            .expect("claim resolves");
        assert_eq!(principal.display_name, "bob");
        assert_eq!(principal.kind, ActorKind::Human);
    }

    #[test]
    fn request_resolver_requires_actor_and_workspace() {
        let resolver = RequestActorResolver;
        assert!(matches!(
            resolver.resolve(&PrincipalClaim::default()),
            Err(CoordinationError::MissingIdentity(_))
        ));
        assert!(matches!(
            resolver.resolve(&PrincipalClaim {
                actor_id: Some("alice".into()),
                workspace_id: Some("   ".into()),
                ..Default::default()
            }),
            Err(CoordinationError::MissingIdentity(_))
        ));
    }

    #[test]
    fn principal_projects_to_kernel_actor() {
        let principal = OperatorPrincipal::new("alice", "Alice", ActorKind::Human, "ws-1");
        let actor = principal.to_actor();
        assert_eq!(actor.actor_id, "alice");
        assert_eq!(actor.kind, ActorKind::Human);
        assert_eq!(principal.actor_tag(), "ws-1:alice");
    }
}
