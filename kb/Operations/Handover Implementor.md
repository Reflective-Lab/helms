# Handover Brief: Implementor

You are the main implementor for Outcome Workbench. You work alongside a QA/Security hardener and a UX implementor. A coordinator (Karl) sequences the work.

## Your current deliverables (Phase 0 + Phase 0.5)

### Priority 1: Store transactionality fix

**File:** `crates/crm-storage/src/lib.rs` — `InMemoryKernelStore::write_with_events()` at line 222.

**Bug:** The current implementation mutates the live `CrmKernel` in place, drains events, then checks if the closure returned `Ok`. If a projection closure does `op1 → op2 → fail`, `op1` remains committed and the emitted events are discarded. This is a correctness bug.

**Fix:** Clone-and-swap pattern. Clone the kernel before calling the closure. Only swap the clone into the live state on success.

```rust
fn write_with_events<R>(&self, f: impl FnOnce(&mut CrmKernel) -> KernelResult<R>) -> StorageResult<StoreWriteResult<R>> {
    let mut kernel = self.kernel.write().map_err(|_| StorageError::LockPoisoned)?;
    let mut snapshot = kernel.clone();
    let result = f(&mut snapshot)?;
    let events = snapshot.drain_events();
    *kernel = snapshot;
    Ok(StoreWriteResult { value: result, events })
}
```

Ensure `CrmKernel` derives or implements `Clone`. It should already (all fields are `HashMap<Uuid, T>` and `Vec<T>`).

Update the `KernelStore` trait implementation to use the same pattern.

**Why this is urgent:** The SurrealDB store (Phase 1) will build on this same write semantics. Fixing it now means Phase 1 inherits correct behavior.

### Priority 2: Durable idempotency for credit grants

**File:** `crates/crm-kernel/src/kernel.rs` — `apply_credit_grant()` around line 1473.

**Bug:** The billing ingress keeps `processed_billing_events` in memory (`crates/crm-server/src/http_api.rs:22`), but the kernel itself does not enforce uniqueness on `payment_reference` for `apply_credit_grant()`. A duplicate delivery after restart or across replicas can double-grant credits.

**Fix (two parts):**

1. **Model change:** Add `external_reference: Option<String>` to `LedgerEntry` in `crates/crm-kernel/src/model.rs:484`. Currently `LedgerEntry` has no field for payment reference — it only appears in the `description` text, which is not queryable. The new field is the durable idempotency key.

2. **Kernel uniqueness check:** In `apply_credit_grant` at `kernel.rs:1473`, before inserting, check if any existing `LedgerEntry` has the same `external_reference` value (when `Some`). Return `KernelError::Conflict` if duplicate.

This means the idempotency guarantee lives in the store, not in the HTTP handler's in-memory cache. The HTTP cache can remain as a fast-path optimization, but correctness must not depend on it.

### Priority 3: Query/list surfaces for operator UX

The UX implementor is building a shared application layer (`workbench-backend` crate) that needs these kernel methods. The current kernel has `list_organizations`, `list_people`, `list_opportunities`, `list_entitlements`, `list_ledger_entries`, `list_timeline`, but is missing several operator-critical queries.

**File:** `crates/crm-kernel/src/kernel.rs` — add the minimum operator-critical methods:

```rust
pub fn list_subscriptions(&self, organization_id: Option<Uuid>) -> Vec<&OrderSubscription>
pub fn list_catalog_items(&self, active_only: bool) -> Vec<&CatalogItem>
pub fn list_workflow_cases(&self, state: Option<&str>) -> Vec<&WorkflowCase>
pub fn get_organization(&self, id: Uuid) -> KernelResult<&Organization>
pub fn get_subscription(&self, id: Uuid) -> KernelResult<&OrderSubscription>
```

Note: `list_organizations`, `list_people`, `list_opportunities`, `list_entitlements`, `list_ledger_entries`, `list_timeline` already exist.

**Deferred (not needed for first UX pass):** `list_leads`, `list_tasks`, `list_approvals`, `list_facts`, `list_audit_entries`. The approval lifecycle is not rich enough yet to warrant a dedicated surface — operator work currently flows through workflow cases.

**File:** `crates/crm-kernel/src/capabilities.rs` — expand `RevenueCommands` and `WorkflowCommands` with the new methods.

### Priority 4: REST endpoints for operator consumption

**File:** `crates/crm-server/src/http_api.rs` — currently only has `/health`, `/v1/system/profile`, and `/v1/integrations/billing/events`.

Add the minimum operator REST surface. These are thin wrappers over kernel methods — `store.read(|k| k.list_organizations())` with JSON serialization. Do not build a second application model beside gRPC.

```
GET  /v1/organizations
GET  /v1/organizations/:id/summary    (account summary: org + people + opps + subs + timeline)
GET  /v1/subscriptions
GET  /v1/catalog
GET  /v1/workflow/cases
GET  /v1/timeline
GET  /v1/truths                       (truth catalog listing)
POST /v1/truths/:key/execute          (execute a truth with JSON inputs)
```

**Deferred:** `/v1/leads`, `/v1/tasks`, `/v1/approvals`, `/v1/opportunities`. Add when UX needs them.

Response shapes should be JSON. Keep them close to the kernel types — the UX implementor's `workbench-backend` crate handles view-model shaping.

## What you should NOT do

- Do not build the SurrealDB store yet — that's Phase 1 after these fixes land
- Do not touch status enum fields (Lead.status, Task.status, etc.) — QA/Security is handling that migration
- Do not build Tauri commands or Svelte code — UX implementor owns that
- Do not add gRPC services for the new queries yet — REST first, gRPC later if needed
- Do not add error handling beyond what's needed — QA/Security is expanding the error taxonomy

## Dependencies

- QA/Security will write the partial-failure and duplicate-delivery tests once your store fix and idempotency fix land. Coordinate timing.
- UX implementor needs query methods (Priority 3) to wire real data into the shared app layer. Land those as soon as Priority 1-2 are done.

## Verification

- `cargo test --workspace` green after each priority
- All 9 truth executors still pass
- The store fix should be verifiable by writing a test where a closure does two mutations then fails — only the pre-mutation state should survive
