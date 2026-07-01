# Design: SessionOwnershipLayer — Gap #1
**Date:** 2026-07-01
**Status:** Approved for implementation
**Repos touched:** `helms` (`helm-session-host`), `marquee-apps` (`quorum-server`)
**Repos not touched:** `atlas-integration` (deferred — no single session identifier), `runtime-runway` (no changes)

---

## Problem

With RR D5 shipped, `StorageKit.leases` is available. Without `SessionOwnershipLayer`, multiple instances of Quorum or a Helms-hosted service can concurrently mutate the same session's state — producing split-brain: two instances each believe they own the session, apply conflicting writes, and leave coordination state inconsistent. This is the primary blocker for safe multi-instance deployment of both Quorum and the Helms session host.

---

## Out of Scope

- Atlas (`atlas-integration`): mutating routes use heterogeneous path params (`{step_id}`, `{question_id}`) with no shared session identifier. Deferred to a later slice; Atlas's mutation surface is narrow and low-concurrency relative to Quorum and the session host.
- Changes to `SessionOwnershipLayer` itself (lives in `runtime-runway`, shipped with D5).
- Changes to `runway-storage` or `LeaseStore` implementations.
- Lease TTL tuning (use defaults from `SessionOwnershipLayer::for_app`).

---

## What `SessionOwnershipLayer` Does

Defined in `runtime-runway/crates/runway-app-host/src/ownership.rs`.

- Constructed via `SessionOwnershipLayer::for_app(app_id, leases: Arc<dyn LeaseStore>)` with optional builder methods `path_param(name)`, `ttl(duration)`, `renew_interval(duration)`, `holder_id(id)`.
- Added to an axum `Router` via `.layer(...)`.
- **GET / HEAD / OPTIONS** requests pass through unconditionally — no lease acquired.
- **Mutating requests (POST, PUT, PATCH, DELETE)** with the configured `path_param` present in the URL: the middleware attempts to acquire or renew a distributed lease on `(app_id, session_key)` via `LeaseStore`. If another holder owns the lease, returns **409 Conflict** immediately.
- **Mutating requests without the configured path param**: pass through (no lock acquired). This is the correct behavior for creation routes (`POST /inquiry`, `POST /inquiry/intent/compile`) and system routes (`/sensemap/recompute`) that don't target a specific existing session.
- On successful acquire: spawns a background renewal task. On connection close, fires-and-forgets a lease release.

---

## Participant Identity

Lease scope key:
- **helm-session-host**: `session_id` extracted from `{session_id}` path param
- **Quorum**: `inquiry_id` extracted from `{id}` path param

The `app_id` passed to `for_app` distinguishes leases between services sharing the same `LeaseStore`.

---

## Changes by Repo

### `helms` — `helm-session-host`

| Change | Detail |
|---|---|
| Modified `src/types.rs` | `SessionHostState` gains `leases: Arc<dyn LeaseStore>` field |
| Modified `src/service.rs` | `SessionHostService` exposes `leases() -> Arc<dyn LeaseStore>` and `app_id() -> &str` accessors |
| Modified `src/http.rs` | `router(service)` applies `.layer(SessionOwnershipLayer::for_app(service.app_id(), service.leases()).path_param("session_id"))` after `.with_state(service)` |
| Modified `src/module.rs` | `SessionHostModule` gains `leases` field; constructor updated |
| Modified `src/lib.rs` | `mount_session_host(hub, app_id, leases)` — adds `leases: Arc<dyn LeaseStore>` parameter |

**Covered routes (ownership enforced):**
- `POST /v1/sessions/{session_id}/ack/delivery`
- `POST /v1/sessions/{session_id}/ack/completion`
- Any future mutating session routes

**Passed through (no lock):**
- `GET /v1/sessions/{session_id}/stream` — GET, exempt by middleware

**Tests:** Update `mount_session_host` call sites to pass `StorageKit::local().leases`. The local lease store always grants — no behavioral change to existing tests.

---

### `marquee-apps` — `quorum-server`

| Change | Detail |
|---|---|
| Modified `src/main.rs` (ownership layer) | Apply `SessionOwnershipLayer::for_app("quorum", storage.leases.clone()).path_param("id")` to the assembled Quorum router before `.serve()` |
| Modified `src/main.rs` (session host) | Update `mount_session_host(hub, app_id, storage.leases.clone())` call to pass leases |

**Covered routes (ownership enforced, `{id}` present):**
- `POST /inquiry/{id}/signal`
- `POST /inquiry/{id}/consent`
- `POST /inquiry/{id}/probes/allocate`
- `POST /inquiry/{id}/mnemos/recall`
- `POST /inquiry/{id}/rounds/next`
- `POST /inquiry/{id}/rounds/{round_id}/phase`
- `POST /inquiry/{id}/rounds/{round_id}/reveal`
- `POST /inquiry/{id}/rounds/score`
- `POST /inquiry/{id}/trials`
- `POST /inquiry/{id}/trials/{trial_id}/signal`
- `POST /inquiry/{id}/decision`
- `POST /inquiry/{id}/amendments`
- `POST /inquiry/{id}/redirects`
- `DELETE /inquiry/{id}/consent/{participant_id}`
- (and remaining `/inquiry/{id}/...` routes)

**Passed through (no lock — no `{id}` param):**
- `POST /inquiry` — creation, no existing inquiry to lock
- `POST /inquiry/intent/compile`, `/stream`, `/contracted`
- `POST /api/session/start`, `/api/sessions/{inquiry_id}/join`
- `POST /acquisition/unresolved-questions/originate`
- `POST /sensemap/anticipatory-signals/detect`, `/sensemap/recompute`
- `POST /api/director/dev/push`, `/api/director/dev/intent`

**Known gap:** `/api/sessions/{inquiry_id}/join` uses `{inquiry_id}` not `{id}` — this route is not covered by the layer. Join is a participant-registration operation with low concurrent-write risk; acceptable for this slice.

---

## Error Handling

| Case | Behaviour |
|---|---|
| Lease held by another instance | `409 Conflict` — client should retry with backoff |
| `LeaseStore` unavailable | Middleware propagates the error as `503 Service Unavailable` (LeaseStore contract) |
| Request has no `{id}` / `{session_id}` param | Pass-through — no ownership enforced |
| Duplicate request from same holder | Lease renewed — `200`/`204` as normal |

---

## Testing

- **helm-session-host unit tests:** No behavioral change. Pass `StorageKit::local().leases` to `mount_session_host`. Existing tests continue to pass — local lease store always grants.
- **helm-session-host ownership integration test:** Two concurrent `POST /v1/sessions/s1/ack/delivery` requests from different `holder_id`s — second one returns `409`. Uses `LocalLeaseStore` with a short TTL.
- **Quorum smoke test:** Verify `POST /inquiry/{id}/signal` from two holders returns `409` for the second. Use an in-process Quorum with `StorageKit::local()`.
- **Quorum pass-through test:** Verify `POST /inquiry` (no `{id}`) is not blocked — returns `2xx` regardless of any held lease.
