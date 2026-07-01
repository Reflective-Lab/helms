# SessionOwnershipLayer — Gap #1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire `SessionOwnershipLayer` into `helm-session-host` and Quorum so concurrent service instances cannot concurrently mutate the same session.

**Architecture:** `SessionHostModule` gains a `leases: Arc<dyn LeaseStore>` field and applies `SessionOwnershipLayer` inside its `HelmModule::router()` method. `SessionHostService` is unchanged. Quorum's `assemble_domain_router` in `domain_host.rs` applies the layer to the `authenticated` sub-router (inside auth, so `AuthContext` is available). Both repos thread `storage.leases.clone()` from `StorageKit`.

**Tech stack:** Rust, axum 0.8, `runway_app_host::SessionOwnershipLayer`, `runway_storage::LeaseStore`, `RedbLeaseStore` (local / test), `tokio::test`

## Global Constraints

- `SessionOwnershipLayer` lives in `runtime-runway/crates/runway-app-host/src/ownership.rs` and is re-exported from the `runway_app_host` crate root. Do NOT change it.
- `.path_param("session_id")` for helm-session-host; `.path_param("id")` for Quorum.
- GET / HEAD / OPTIONS are unconditionally exempt — no lease acquired.
- The layer requires `AuthContext` in request extensions (from upstream `AuthLayer`) with a non-empty `org_id`. Without `AuthContext`: `400 ownership_requires_auth`. Without `org_id`: `400 ownership_requires_org`.
- Without an explicit `.holder_id()` override, the layer uses `process_holder_id()` — a **process-wide** static string. Two layer instances in the same test process therefore share a holder ID and do NOT conflict. Ownership integration tests **must** call `.holder_id("h-a")` / `.holder_id("h-b")` explicitly.
- `SessionHostService::from_hub(hub, app_id)` signature is **unchanged** — service unit tests are untouched.
- Existing `http.rs` unit tests call `http::router(service)` directly (bypassing `module.router()`) — they are untouched.
- Existing `host_mount_test.rs` GET test is untouched except for one added `storage.leases.clone()` argument.
- `StorageKit::local().leases` is a `RedbLeaseStore`. It always grants a lease for a given holder_id (idempotent renewal) and conflicts only when a *different* holder holds the same key.

---

## File Map

### Task 1 — `helms` repo

| Op | Path |
|---|---|
| Modify | `helms/crates/helm-session-host/src/module.rs` |
| Modify | `helms/crates/helm-session-host/src/host.rs` |
| Modify | `helms/crates/helm-session-host/tests/host_mount_test.rs` |
| Create | `helms/crates/helm-session-host/tests/ownership_test.rs` |

`service.rs`, `types.rs`, `http.rs`, `store.rs`, `delivery.rs`, `Cargo.toml` — **no changes**.

### Task 2 — `marquee-apps` repo

| Op | Path |
|---|---|
| Modify | `marquee-apps/quorum-sense/crates/quorum-server/src/domain_host.rs` |
| Modify | `marquee-apps/quorum-sense/crates/quorum-server/src/main.rs` |

---

## Task 1: helm-session-host — SessionOwnershipLayer via SessionHostModule

### Files
- Modify: `helms/crates/helm-session-host/src/module.rs`
- Modify: `helms/crates/helm-session-host/src/host.rs`
- Modify: `helms/crates/helm-session-host/tests/host_mount_test.rs`
- Create: `helms/crates/helm-session-host/tests/ownership_test.rs`

### Interfaces
- Consumes: `runway_app_host::SessionOwnershipLayer` (re-exported from crate root), `runway_storage::LeaseStore`, `SessionHostService::app_id() -> &str` (already exists at `service.rs:43`)
- Produces: `mount_session_host(hub: EventHubHandle, app_id: impl Into<String>, leases: Arc<dyn LeaseStore>) -> Arc<SessionHostModule>`

---

- [ ] **Step 1: Write failing tests**

Create `helms/crates/helm-session-host/tests/ownership_test.rs`:

```rust
// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

//! Integration tests for SessionOwnershipLayer wiring in helm-session-host.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::middleware;
use axum::response::Response;
use axum::routing::post;
use axum::Router;
use runway_app_host::{EventHub, HelmModule, SessionOwnershipLayer};
use runway_auth::{AuthContext, FirebaseClaims};
use runway_storage::{LeaseStore, StorageKit};
use tower::ServiceExt;

use helm_session_host::mount_session_host;

async fn inject_test_auth(mut req: Request<Body>, next: middleware::Next) -> Response {
    req.extensions_mut().insert(AuthContext {
        claims: FirebaseClaims {
            uid: "u1".into(),
            email: None,
            org_id: Some("test-org".into()),
            apps: vec![],
            role: None,
        },
    });
    next.run(req).await
}

fn ack_delivery_request(session_id: &str) -> Request<Body> {
    let body = serde_json::json!({"participant_id": "p-1", "finding_id": "f-1"});
    Request::builder()
        .method(Method::POST)
        .uri(format!("/v1/sessions/{session_id}/ack/delivery"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

/// The module wires SessionOwnershipLayer. Without AuthContext, POST returns 400
/// (not the service-level 204/404), proving the layer is present.
#[tokio::test]
async fn ownership_layer_is_wired_returns_400_without_auth() {
    let dir = tempfile::tempdir().unwrap();
    let storage = StorageKit::local(dir.path()).await.unwrap();
    let hub = EventHub::with_capacity(8);
    let module = mount_session_host(hub.handle(), "test.session-host", storage.leases.clone());

    let router = Arc::clone(&module).router();
    // No auth middleware — should get 400 "ownership_requires_auth"
    let res = router
        .oneshot(ack_delivery_request("sess-noauth"))
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::BAD_REQUEST,
        "ownership layer must reject mutating requests without AuthContext"
    );
}

/// GET /stream is exempt from the ownership layer (pass-through).
#[tokio::test]
async fn get_stream_is_exempt_from_ownership_layer() {
    let dir = tempfile::tempdir().unwrap();
    let storage = StorageKit::local(dir.path()).await.unwrap();
    let hub = EventHub::with_capacity(8);
    let module = mount_session_host(hub.handle(), "test.session-host", storage.leases.clone());

    let router = Arc::clone(&module).router();
    // No auth middleware — GET is exempt, so it reaches the handler
    let res = router
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/v1/sessions/sess-stream/stream")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let ct = res.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.starts_with("text/event-stream"));
}

/// Two holders targeting the same session simultaneously: exactly one gets 409.
/// Uses explicit holder_ids because process_holder_id() is process-wide (both
/// instances would otherwise share the same ID and renew instead of conflict).
#[tokio::test]
async fn two_holders_on_same_session_one_gets_409() {
    let dir = tempfile::tempdir().unwrap();
    let storage = StorageKit::local(dir.path()).await.unwrap();
    let leases = storage.leases.clone();

    // Minimal router that stands in for the real ack route.
    async fn ok_handler() -> StatusCode {
        StatusCode::OK
    }
    let base = Router::new().route("/v1/sessions/{session_id}/ack/delivery", post(ok_handler));

    let make_router = |holder: &str| {
        let leases: Arc<dyn LeaseStore> = leases.clone();
        let holder = holder.to_string();
        base.clone()
            .layer(
                SessionOwnershipLayer::for_app("test.session-host", leases)
                    .path_param("session_id")
                    .holder_id(holder),
            )
            .layer(middleware::from_fn(inject_test_auth))
    };

    let router_a = make_router("holder-a");
    let router_b = make_router("holder-b");

    let (res_a, res_b) = tokio::join!(
        router_a.oneshot(ack_delivery_request("sess-conflict")),
        router_b.oneshot(ack_delivery_request("sess-conflict")),
    );

    let sa = res_a.unwrap().status();
    let sb = res_b.unwrap().status();
    let conflicts = [sa, sb]
        .iter()
        .filter(|&&s| s == StatusCode::CONFLICT)
        .count();
    assert_eq!(
        conflicts, 1,
        "exactly one holder must get 409 Conflict; got ({sa}, {sb})"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail (compile error expected)**

```bash
cd /Users/kpernyer/dev/reflective/bedrock-platform/helms
cargo test -p helm-session-host 2>&1 | head -40
```

Expected: compile error — `mount_session_host` does not accept 3 arguments, `SessionOwnershipLayer` not in scope for test.

- [ ] **Step 3: Modify `module.rs`**

Replace the entire file contents:

```rust
// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

//! `helm.session-host` as a mountable `HelmModule`.

use std::sync::Arc;

use async_trait::async_trait;
use axum::Router;
use helm_module_contracts::{HelmModuleReadiness, HelmModuleState, HelmModuleStatus};
use runway_app_host::{HelmModule, HostContext, ModuleState, SessionOwnershipLayer};
use runway_storage::LeaseStore;

use crate::http;
use crate::service::SessionHostService;

const MODULE_ID: &str = "helm.session-host";

/// Server-side Session Helm — routes findings to participants via SSE.
pub struct SessionHostModule {
    service: Arc<SessionHostService>,
    leases: Arc<dyn LeaseStore>,
}

impl SessionHostModule {
    #[must_use]
    pub fn new(service: Arc<SessionHostService>, leases: Arc<dyn LeaseStore>) -> Self {
        Self { service, leases }
    }

    #[must_use]
    pub fn service(&self) -> Arc<SessionHostService> {
        self.service.clone()
    }

    #[must_use]
    pub fn module_state(&self) -> HelmModuleState {
        HelmModuleState::Live
    }

    #[must_use]
    pub fn readiness_status(&self) -> HelmModuleStatus {
        HelmModuleStatus::new(
            MODULE_ID,
            self.module_state(),
            "session-host SSE push surface is wired to EventHubHandle",
        )
        .with_live_requirements(["event_hub"])
    }
}

impl HelmModuleReadiness for SessionHostModule {
    fn module_state(&self) -> HelmModuleState {
        SessionHostModule::module_state(self)
    }

    fn readiness_status(&self) -> HelmModuleStatus {
        SessionHostModule::readiness_status(self)
    }
}

#[async_trait]
impl HelmModule for SessionHostModule {
    fn module_id(&self) -> &'static str {
        MODULE_ID
    }

    async fn init(&self, _ctx: &HostContext) -> anyhow::Result<()> {
        tracing::info!(module = MODULE_ID, "initialized");
        Ok(())
    }

    fn router(self: Arc<Self>) -> Router {
        http::router(self.service.clone()).layer(
            SessionOwnershipLayer::for_app(self.service.app_id(), self.leases.clone())
                .path_param("session_id"),
        )
    }

    fn module_state(&self) -> ModuleState {
        ModuleState::Live
    }
}
```

- [ ] **Step 4: Modify `host.rs`**

Replace entire file:

```rust
// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

//! App-host wiring for the session-host module.

use std::sync::Arc;

use runway_app_host::EventHubHandle;
use runway_storage::LeaseStore;

use crate::{SessionHostModule, SessionHostService};

/// Build a [`SessionHostModule`] over the shared hub and lease store.
///
/// The module applies [`runway_app_host::SessionOwnershipLayer`] to all mutating
/// session routes. GET / HEAD / OPTIONS pass through unconditionally.
/// Mutating routes require `AuthContext` in request extensions (provided by the
/// upstream `AuthLayer` on `RunwayAppHost`) — without it the layer returns 400.
#[must_use]
pub fn mount_session_host(
    hub: EventHubHandle,
    app_id: impl Into<String>,
    leases: Arc<dyn LeaseStore>,
) -> Arc<SessionHostModule> {
    Arc::new(SessionHostModule::new(
        Arc::new(SessionHostService::from_hub(hub, app_id)),
        leases,
    ))
}
```

- [ ] **Step 5: Update `tests/host_mount_test.rs` call site**

Update only the `mount_session_host` call (line 39). The `storage` variable is already created 2 lines above via `StorageKit::local(dir.path()).await.expect("local storage")`:

```rust
    let module = mount_session_host(hub.handle(), "test.session-host", storage.leases.clone());
```

(Replace the existing `mount_session_host(hub.handle(), "test.session-host")` on that line.)

- [ ] **Step 6: Run tests**

```bash
cd /Users/kpernyer/dev/reflective/bedrock-platform/helms
cargo test -p helm-session-host 2>&1 | tail -30
```

Expected: all existing tests pass PLUS the 3 new ownership tests pass.

If `two_holders_on_same_session_one_gets_409` is flaky (concurrent tokio::join! resolves too fast), see fix note in Step 7.

- [ ] **Step 7: Commit**

```bash
cd /Users/kpernyer/dev/reflective/bedrock-platform/helms
git add crates/helm-session-host/src/module.rs \
        crates/helm-session-host/src/host.rs \
        crates/helm-session-host/tests/host_mount_test.rs \
        crates/helm-session-host/tests/ownership_test.rs
git commit -m "feat(session-host): apply SessionOwnershipLayer via mount_session_host(leases)"
```

> **Fix note (if `two_holders_on_same_session_one_gets_409` fails):** If `tokio::join!` ends up being sequential rather than truly concurrent, wrap each request in `tokio::spawn` instead:
> ```rust
> let handle_a = tokio::spawn(async move { router_a.oneshot(ack_delivery_request("sess-conflict")).await });
> let handle_b = tokio::spawn(async move { router_b.oneshot(ack_delivery_request("sess-conflict")).await });
> let sa = handle_a.await.unwrap().unwrap().status();
> let sb = handle_b.await.unwrap().unwrap().status();
> ```
> And if even that doesn't produce a conflict (TTL expires between requests), add `.ttl(std::time::Duration::from_secs(30))` to both `SessionOwnershipLayer` builders.

---

## Task 2: Quorum — SessionOwnershipLayer on inquiry/{id} routes

### Files
- Modify: `marquee-apps/quorum-sense/crates/quorum-server/src/domain_host.rs`
- Modify: `marquee-apps/quorum-sense/crates/quorum-server/src/main.rs`

### Interfaces
- Consumes: `runway_app_host::SessionOwnershipLayer`, `runway_storage::LeaseStore` (already imported in main.rs via `StorageKit`)
- Produces: all `/inquiry/{id}/...` mutating routes protected by ownership; `/inquiry` creation routes pass through

### Key ordering rule
In axum, `router.layer(A).layer(B)` means B is outermost (processes requests first). The ownership layer must be INNER relative to auth (so `AuthContext` is populated before ownership checks it):
```rust
authenticated
    .with_state(app)
    .layer(ownership_layer)   // inner — second to run, after auth
    .layer(auth_layer)        // outer — first to run, sets AuthContext
```

---

- [ ] **Step 1: Write failing tests**

Add to the existing `#[cfg(test)] mod tests` block in `main.rs`:

```rust
    #[tokio::test]
    async fn ownership_layer_blocks_second_holder_on_inquiry_mutation() {
        use axum::middleware;
        use axum::routing::post;
        use axum::Router;
        use runway_app_host::SessionOwnershipLayer;
        use runway_auth::{AuthContext, FirebaseClaims};
        use runway_storage::{LeaseStore, StorageKit};

        let dir = tempfile::tempdir().unwrap();
        let storage = StorageKit::local(dir.path()).await.unwrap();
        let leases = storage.leases.clone();

        async fn ok_handler() -> StatusCode {
            StatusCode::OK
        }
        async fn inject_auth(
            mut req: axum::http::Request<axum::body::Body>,
            next: middleware::Next,
        ) -> axum::response::Response {
            req.extensions_mut().insert(AuthContext {
                claims: FirebaseClaims {
                    uid: "u1".into(),
                    email: None,
                    org_id: Some("test-org".into()),
                    apps: vec![],
                    role: None,
                },
            });
            next.run(req).await
        }

        let make_router = |holder: &str| {
            let leases: Arc<dyn LeaseStore> = leases.clone();
            let holder = holder.to_string();
            Router::new()
                .route("/inquiry/{id}/signal", post(ok_handler))
                .layer(
                    SessionOwnershipLayer::for_app("quorum", leases)
                        .path_param("id")
                        .holder_id(holder),
                )
                .layer(middleware::from_fn(inject_auth))
        };

        let router_a = make_router("holder-a");
        let router_b = make_router("holder-b");

        let make_req = || {
            axum::http::Request::builder()
                .method(axum::http::Method::POST)
                .uri("/inquiry/inq-test-1/signal")
                .body(axum::body::Body::empty())
                .unwrap()
        };

        let (res_a, res_b) = tokio::join!(
            router_a.oneshot(make_req()),
            router_b.oneshot(make_req()),
        );
        let sa = res_a.unwrap().status();
        let sb = res_b.unwrap().status();
        let conflicts = [sa, sb]
            .iter()
            .filter(|&&s| s == StatusCode::CONFLICT)
            .count();
        assert_eq!(
            conflicts, 1,
            "exactly one holder must get 409 Conflict; got ({sa}, {sb})"
        );
    }

    #[tokio::test]
    async fn ownership_layer_passes_through_inquiry_creation_route() {
        use axum::middleware;
        use axum::routing::post;
        use axum::Router;
        use runway_app_host::SessionOwnershipLayer;
        use runway_auth::{AuthContext, FirebaseClaims};
        use runway_storage::{LeaseStore, StorageKit};

        let dir = tempfile::tempdir().unwrap();
        let storage = StorageKit::local(dir.path()).await.unwrap();

        async fn ok_handler() -> StatusCode {
            StatusCode::OK
        }
        async fn inject_auth(
            mut req: axum::http::Request<axum::body::Body>,
            next: middleware::Next,
        ) -> axum::response::Response {
            req.extensions_mut().insert(AuthContext {
                claims: FirebaseClaims {
                    uid: "u1".into(),
                    email: None,
                    org_id: Some("test-org".into()),
                    apps: vec![],
                    role: None,
                },
            });
            next.run(req).await
        }

        let router = Router::new()
            .route("/inquiry", post(ok_handler))
            .layer(
                SessionOwnershipLayer::for_app("quorum", storage.leases.clone())
                    .path_param("id")
                    .holder_id("holder-a"),
            )
            .layer(middleware::from_fn(inject_auth));

        let req = axum::http::Request::builder()
            .method(axum::http::Method::POST)
            .uri("/inquiry")
            .body(axum::body::Body::empty())
            .unwrap();
        let res = router.oneshot(req).await.unwrap();
        assert_eq!(
            res.status(),
            StatusCode::OK,
            "POST /inquiry (no {{id}}) must not be blocked by ownership layer"
        );
    }
```

- [ ] **Step 2: Run tests to verify they compile and pass** (they test `SessionOwnershipLayer` in isolation — no domain code changes yet)

```bash
cd /Users/kpernyer/dev/reflective/marquee-apps/quorum-sense
cargo test -p quorum-server ownership 2>&1
```

Expected: both new tests PASS (they're self-contained layer tests with no code changes needed).

- [ ] **Step 3: Modify `domain_host.rs`**

Add `leases` to `DomainHostConfig` and apply the ownership layer in `assemble_domain_router`.

**In `DomainHostConfig`** — add the `leases` field after `accounts_state`:

```rust
#[derive(Clone)]
pub struct DomainHostConfig {
    pub app: AppState,
    pub session_host: Arc<SessionHostService>,
    pub firebase_project_id: String,
    pub local_dev: bool,
    pub auth_app: String,
    pub entitlement_ctx: EntitlementContext,
    pub accounts_state: AccountsState,
    pub leases: Arc<dyn runway_storage::LeaseStore>,
}
```

**In `assemble_domain_router`** — apply the ownership layer to `authenticated` BEFORE `auth_layer` (so it runs after auth in the request path):

```rust
fn assemble_domain_router(
    cfg: &DomainHostConfig,
    by_path: HashMap<String, MethodRouter<AppState>>,
) -> Router {
    let auth_layer = AuthLayer::new(
        FirebaseAuth::new(cfg.firebase_project_id.clone()),
        cfg.local_dev,
    )
    .requiring_app(cfg.auth_app.clone());

    let bare_auth_layer = AuthLayer::new(
        FirebaseAuth::new(cfg.firebase_project_id.clone()),
        cfg.local_dev,
    );

    let accounts_router =
        runway_accounts::protected_routes(cfg.accounts_state.clone()).layer(bare_auth_layer);

    let ownership_layer = runway_app_host::SessionOwnershipLayer::for_app(
        "quorum",
        cfg.leases.clone(),
    )
    .path_param("id");

    let mut authenticated = Router::new();
    for (path, method_router) in by_path {
        authenticated = authenticated.route(&path, method_router);
    }

    let public_router = runway_accounts::public_routes(cfg.accounts_state.clone());

    authenticated
        .with_state(cfg.app.clone())
        .layer(ownership_layer)  // inner — runs after auth_layer sets AuthContext
        .layer(auth_layer)       // outer — runs first, sets AuthContext
        .merge(accounts_router)
        .merge(public_router)
        .layer(Extension(cfg.entitlement_ctx.clone()))
        .layer(Extension(cfg.session_host.clone()))
}
```

- [ ] **Step 4: Modify `main.rs`** (two changes)

**Change A** — `mount_session_host` call (around line 1721): add `storage.leases.clone()`:

```rust
    let session_host_module = mount_session_host(hub.handle(), app_id.clone(), storage.leases.clone());
```

**Change B** — `DomainHostConfig` construction (around line 1736-1747): add `leases` field:

```rust
    let host_builder = domain_host::wire_domain_routes(
        RunwayAppHost::builder(packet),
        &domain_host::DomainHostConfig {
            app: app_state.clone(),
            session_host: session_host.clone(),
            firebase_project_id: firebase_project_id.clone(),
            local_dev,
            auth_app: auth_app.clone(),
            entitlement_ctx: entitlement_ctx.clone(),
            accounts_state: accounts_state.clone(),
            leases: storage.leases.clone(),
        },
    );
```

- [ ] **Step 5: Run tests**

```bash
cd /Users/kpernyer/dev/reflective/marquee-apps/quorum-sense
cargo test -p quorum-server 2>&1 | tail -30
```

Expected: all existing tests pass PLUS the 2 new ownership tests pass.

- [ ] **Step 6: Commit**

```bash
cd /Users/kpernyer/dev/reflective/marquee-apps/quorum-sense
git add crates/quorum-server/src/domain_host.rs \
        crates/quorum-server/src/main.rs
git commit -m "feat(quorum): apply SessionOwnershipLayer to /inquiry/{id} mutation routes"
```
