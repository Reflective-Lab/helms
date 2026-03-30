# Handover Brief: QA / Security Hardener

You are the QA and security hardener for crm.prio.ai. You work alongside a main implementor and a UX implementor. A coordinator (Karl) sequences the work.

## Context

crm.prio.ai is a JTBD-driven CRM built as a Converge application. 9 truths execute end-to-end. The kernel is an in-memory projection store. Two correctness bugs have been identified by the implementor (store transactionality and billing idempotency) — he is fixing both. Your job is to harden the foundation while those fixes land.

Read `docs/coordinator-handoff.md` for full architectural context.

## Your current deliverables (Phase 0)

### Priority 1: Status enum migration

**File:** `crates/crm-kernel/src/model.rs`

Six status fields are still `String` and need migration to enums. Follow the existing `SubscriptionStatus` pattern (search for it in model.rs — it has `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]`).

Create these enums and replace the corresponding `String` fields:

```rust
pub enum LeadStatus {
    New,
    Contacted,
    Qualified,
    Disqualified,
    Converted,
}

pub enum TaskStatus {
    Open,
    InProgress,
    Blocked,
    Done,
    Cancelled,
}

pub enum QuoteStatus {
    Draft,
    Sent,
    Accepted,
    Rejected,
    Expired,
}

pub enum JobState {
    Pending,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

pub enum AgentRunStatus {
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

pub enum WorkflowRunStatus {
    Running,
    Succeeded,
    Failed,
    Cancelled,
}
```

**After creating the enums:** grep through the entire codebase for sites that set these fields with string values. Key locations to check:

- `crates/crm-kernel/src/kernel.rs` — kernel methods that create or update these entities
- `crates/crm-server/src/truth_runtime/*.rs` — all 9 truth executors create entities with status fields during projection
- `crates/crm-kernel/src/lib.rs` — tests that construct these types

Replace every `status: "some_string".into()` or `status: String::from("...")` with the appropriate enum variant.

### Priority 2: Error taxonomy expansion

**File:** `crates/crm-kernel/src/error.rs`

Current `KernelError` has only 3 variants: `Validation`, `NotFound`, `Invariant`. Expand to cover the operations the kernel actually performs:

```rust
#[derive(Debug, thiserror::Error)]
pub enum KernelError {
    #[error("validation failed: {0}")]
    Validation(String),

    #[error("record not found: {kind} {id}")]
    NotFound { kind: &'static str, id: String },

    #[error("invariant violated: {0}")]
    Invariant(String),

    #[error("conflict: {kind} {id} — {reason}")]
    Conflict { kind: &'static str, id: String, reason: String },

    #[error("unauthorized: {action} — {reason}")]
    Unauthorized { action: String, reason: String },

    #[error("invalid state transition: {kind} from {from} to {to}")]
    StateTransition { kind: &'static str, from: String, to: String },

    #[error("quota exceeded: {resource} limit {limit}, requested {requested}")]
    QuotaExceeded { resource: String, limit: i64, requested: i64 },

    #[error("external dependency failed: {service} — {message}")]
    ExternalDependency { service: String, message: String },
}
```

**File:** `crates/crm-storage/src/lib.rs` — expand `StorageError`:

```rust
pub enum StorageError {
    LockPoisoned,
    Kernel(KernelError),
    ConnectionFailed { backend: String, message: String },
    SerializationFailed { message: String },
    Timeout { operation: String },
}
```

The new `StorageError` variants prepare for SurrealDB (Phase 1). They won't be used yet but the types should exist.

Where the implementor's idempotency fix adds a duplicate-payment check, it should return `KernelError::Conflict`. Coordinate with them on the variant shape.

### Priority 3: Partial-failure and idempotency tests

**Depends on:** Implementor landing the store transactionality fix (Priority 1 in their brief) and the idempotency fix (their Priority 2).

Once those fixes land, write these tests:

**Test 1 — Partial projection failure (store transactionality)**

**File:** `crates/crm-storage/src/lib.rs` (in `#[cfg(test)]` module) or a new integration test file.

```
Scenario:
1. Create a store with one organization already in it
2. Call write_with_events with a closure that:
   a. Creates a person (succeeds)
   b. Performs an operation that returns Err(KernelError::Invariant(...))
3. Assert: the person from step 2a does NOT exist in the store
4. Assert: the original organization still exists unchanged
5. Assert: no domain events were emitted
```

This proves the clone-and-swap pattern works. If this test passes on the current code before the fix, something is wrong.

**Test 2 — Duplicate credit grant across restart boundary**

**File:** `crates/crm-kernel/src/lib.rs` (in tests) or dedicated integration test.

```
Scenario:
1. Create a store, create an organization and subscription
2. Apply a credit grant with payment_reference = "pay_abc123"
3. Assert: ledger entry exists, balance updated
4. Create a NEW store (simulating restart), load same data
5. Attempt to apply a credit grant with the same payment_reference = "pay_abc123"
6. Assert: returns KernelError::Conflict (not a successful double-grant)
7. Assert: ledger balance unchanged
```

**Test 3 — Truth execution with projection failure**

**File:** `crates/crm-server/src/truth_runtime/` (pick a truth executor, e.g., `activate_subscription.rs`).

```
Scenario:
1. Execute activate-subscription truth with valid inputs
2. Truth converges successfully
3. Projection phase: simulate a failure (e.g., subscription ID not found in kernel)
4. Assert: no domain state was committed
5. Assert: experience events from the truth execution are still available (the truth ran, the projection failed)
6. Assert: kernel state is unchanged from before execution
```

### Priority 4: Security audit of current HTTP surface

**File:** `crates/crm-server/src/http_api.rs`

Review and document:
- Is the billing ingress token comparison constant-time? (Should use `subtle::ConstantTimeEq` or equivalent to avoid timing attacks)
- Are there any endpoints that should require auth but don't? (The implementor is adding new GET endpoints — verify they have appropriate access control or document that auth is deferred)
- Does the billing ingress validate all required fields before executing a truth? (Check for missing field → panic paths)
- Is `serde(deny_unknown_fields)` used on `BillingIngressRequest` to reject unexpected input?

Document findings. Fix anything that's a clear vulnerability. Flag architectural auth decisions for the coordinator.

## What you should NOT do

- Do not implement the store transactionality fix itself — the implementor is doing that. You write the tests.
- Do not add query/list methods to the kernel — the implementor is doing that.
- Do not touch Svelte, Tauri, or UX code — UX implementor owns that.
- Do not add new truth executors or modify truth execution logic.
- Do not add comments or documentation to code you didn't change.

## Dependencies

- Your status enum migration (Priority 1) has no dependencies — start immediately.
- Your error taxonomy (Priority 2) should coordinate with the implementor on `KernelError::Conflict` shape for the idempotency fix.
- Your tests (Priority 3) depend on the implementor's store fix and idempotency fix landing first.
- Your security audit (Priority 4) can happen anytime.

## Verification

- `cargo test --workspace` green after each priority
- All 9 truth executors still pass after status enum migration (this is the main risk — string values changing to enums will break compilation at every construction site)
- The partial-failure test must FAIL on the pre-fix code and PASS on the post-fix code
- `cargo clippy --workspace` clean
