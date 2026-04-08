# Handover Brief: UX Implementor

You are the UX implementor for crm.prio.ai. You work alongside a main implementor (backend/kernel) and a QA/Security hardener. A coordinator (Karl) sequences the work.

## Context

crm.prio.ai is a JTBD-driven CRM built as a Converge application. The backend has 9 executable truths, 20 capability modules, and a full domain model — but everything is in-memory, there are limited query surfaces, and the desktop app is a static shell.

Your job is to build the shared application layer and the desktop operator cockpit. The TUI (Ratatui) comes later, but the shared layer must support both from the start.

Read `docs/coordinator-handoff.md` for full architectural context. Key principle: **this is an operator-first, job-centric interface — not a traditional record-navigation CRM.**

## Architecture decision: shared application layer

Both desktop (Svelte/Tauri) and TUI (Ratatui) consume the same Rust application layer. Svelte does NOT call the HTTP server directly. The data flow is:

```
Svelte component → Tauri command → crm-app crate → KernelStore
Ratatui widget   → crm-app crate → KernelStore (direct, no IPC)
```

The `crm-app` crate is transport-agnostic. No axum, no tonic, no HTTP types. It takes a `KernelStore` and exposes operator-oriented use cases.

## Your current deliverables (Phase 0 + start of Phase 4)

### Priority 1: Shared application layer crate

**Create:** `crates/crm-app/Cargo.toml`

```toml
[package]
name = "crm-app"
version = "0.1.0"
edition = "2021"

[dependencies]
crm-kernel = { path = "../crm-kernel" }
crm-storage = { path = "../crm-storage" }
prio-truths = { path = "../prio-truths" }
uuid = { workspace = true }
chrono = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
```

Add `"crates/crm-app"` to the workspace members in the root `Cargo.toml`.

**Create:** `crates/crm-app/src/lib.rs` — the operator application layer.

This crate defines:

#### View model types

```rust
pub struct OperatorDashboard {
    pub active_jobs: Vec<JobSummary>,
    pub pending_approvals: Vec<ApprovalListItem>,
    pub recent_exceptions: Vec<ExceptionItem>,
    pub recent_timeline: Vec<TimelineEventItem>,
}

pub struct TruthListItem {
    pub key: String,
    pub display_name: String,
    pub kind: String,          // "job", "policy", "invariant"
    pub summary: String,
    pub executable: bool,
}

pub struct TruthExecutionSession {
    pub truth_key: String,
    pub state: ExecutionState,
    pub cycles: Option<u32>,
    pub stop_reason: Option<String>,
    pub criteria_outcomes: Vec<CriterionOutcomeItem>,
    pub projection_summary: Option<ProjectionSummary>,
    pub experience_event_count: usize,
}

pub enum ExecutionState {
    Idle,
    Running,
    Completed,
    Blocked,
    Failed,
}

pub struct CriterionOutcomeItem {
    pub criterion_id: String,
    pub description: String,
    pub required: bool,
    pub status: String,       // "met", "unmet", "blocked", "indeterminate"
    pub detail: Option<String>,
}

pub struct ApprovalListItem {
    pub id: Uuid,
    pub truth_key: String,
    pub reason: String,
    pub created_at: DateTime<Utc>,
    pub related_to: Option<String>,
}

pub struct WorkflowCaseListItem {
    pub id: Uuid,
    pub definition_key: String,
    pub state: String,
    pub related_to: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub struct AccountSummary {
    pub organization: OrganizationView,
    pub people: Vec<PersonView>,
    pub opportunities: Vec<OpportunityView>,
    pub subscriptions: Vec<SubscriptionView>,
    pub entitlements: Vec<EntitlementView>,
    pub recent_timeline: Vec<TimelineEventItem>,
}

pub struct TimelineEventItem {
    pub id: Uuid,
    pub kind: String,
    pub summary: String,
    pub actor_name: String,
    pub actor_kind: String,
    pub timestamp: DateTime<Utc>,
    pub related_to: Option<String>,
}

// Simplified view types for the sub-views above.
// These are NOT the full kernel types — they carry only what the UI needs.
pub struct OrganizationView { pub id: Uuid, pub name: String, pub domain: Option<String> }
pub struct PersonView { pub id: Uuid, pub display_name: String, pub email: Option<String>, pub role: Option<String> }
pub struct OpportunityView { pub id: Uuid, pub title: String, pub stage: String, pub value: Option<i64> }
pub struct SubscriptionView { pub id: Uuid, pub plan_name: String, pub status: String, pub activated_at: Option<DateTime<Utc>> }
pub struct EntitlementView { pub id: Uuid, pub kind: String, pub label: String, pub remaining: Option<i64> }
pub struct JobSummary { pub id: Uuid, pub truth_key: String, pub state: String, pub started_at: DateTime<Utc> }
pub struct ExceptionItem { pub id: Uuid, pub kind: String, pub summary: String, pub timestamp: DateTime<Utc> }
pub struct ProjectionSummary { pub entities_created: usize, pub entities_updated: usize, pub events_emitted: usize }
```

All view model types should derive `Debug, Clone, Serialize, Deserialize`.

#### Operator contract

```rust
pub struct OperatorApp<S: KernelStore> {
    store: S,
    truth_catalog: StaticTruthCatalog,
}

impl<S: KernelStore> OperatorApp<S> {
    pub fn new(store: S) -> Self { ... }

    // Dashboard
    pub fn dashboard(&self) -> OperatorDashboard { ... }

    // Truths
    pub fn list_truths(&self) -> Vec<TruthListItem> { ... }
    pub fn execute_truth(&self, key: &str, inputs: HashMap<String, String>) -> Result<TruthExecutionSession, AppError> { ... }

    // Organizations
    pub fn list_organizations(&self) -> Vec<OrganizationView> { ... }
    pub fn account_summary(&self, org_id: Uuid) -> Result<AccountSummary, AppError> { ... }

    // Revenue
    pub fn list_subscriptions(&self) -> Vec<SubscriptionView> { ... }

    // Workflow
    pub fn list_workflow_cases(&self) -> Vec<WorkflowCaseListItem> { ... }

    // Timeline
    pub fn recent_timeline(&self, limit: usize) -> Vec<TimelineEventItem> { ... }

    // --- Deferred: implement when backend surfaces exist ---
    // pub fn list_opportunities(&self) -> Vec<OpportunityView>  // kernel method exists, add when UX needs it
    // pub fn list_leads(&self) -> Vec<LeadView>                 // kernel method deferred
    // pub fn list_approvals(&self) -> Vec<ApprovalListItem>     // approval lifecycle not rich enough yet
}
```

The implementations read from the `KernelStore` via `store.read(|kernel| ...)` and map kernel types to view model types. Start with the methods where kernel queries already exist (`list_organizations`, `list_opportunities`, `list_entitlements`, `list_timeline`). Stub the rest with empty vecs until the implementor lands the missing kernel methods.

#### Error type

```rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("truth execution failed: {0}")]
    ExecutionFailed(String),
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
}
```

### Priority 2: Tauri command layer

**File:** `apps/desktop/src-tauri/src/main.rs` (or split into `src/commands.rs`)

Register Tauri commands that delegate to `OperatorApp`:

```rust
#[tauri::command]
fn get_dashboard(state: tauri::State<AppState>) -> Result<OperatorDashboard, String> {
    Ok(state.app.dashboard())
}

#[tauri::command]
fn list_truths(state: tauri::State<AppState>) -> Result<Vec<TruthListItem>, String> {
    Ok(state.app.list_truths())
}

#[tauri::command]
fn execute_truth(state: tauri::State<AppState>, key: String, inputs: HashMap<String, String>) -> Result<TruthExecutionSession, String> {
    state.app.execute_truth(&key, inputs).map_err(|e| e.to_string())
}

// ... one command per OperatorApp method
```

**AppState:**
```rust
struct AppState {
    app: OperatorApp<InMemoryKernelStore>,  // SurrealDbKernelStore in Phase 1
}
```

Initialize in Tauri's `setup` hook. For now, use `InMemoryKernelStore`. When the implementor finishes Phase 1 (SurrealDB), swap to `SurrealDbKernelStore` — one line change.

### Priority 3: Desktop shell — operator cockpit layout

**Directory:** `apps/desktop/src/`

Replace the current static shell with a real operator cockpit. Use Svelte 5. You can reference `../wolfgang` for structural inspiration (app shell, store patterns, nav conventions) but do NOT reuse Wolfgang's chat-centric layout.

**Layout:**
```
┌──────────┬─────────────────────────────┬──────────────┐
│          │                             │              │
│  Left    │     Center Pane             │  Right Rail  │
│  Nav     │     (primary content)       │  (approvals, │
│          │                             │   blockers,  │
│  Jobs    │                             │   exceptions)│
│  Accounts│                             │              │
│  Revenue │                             │              │
│  Workflow│                             │              │
│  Truths  │                             │              │
│  System  │                             │              │
│          │                             │              │
└──────────┴─────────────────────────────┴──────────────┘
```

**Svelte routes (v1 — narrowed scope):**

```
src/routes/
  +layout.svelte          — shell with left nav + right rail
  +page.svelte            — dashboard (OperatorDashboard)
  accounts/
    +page.svelte          — organization list
    [id]/+page.svelte     — account summary (AccountSummary)
  revenue/+page.svelte    — subscriptions, catalog, entitlements
  truths/
    +page.svelte          — truth catalog list
    [key]/+page.svelte    — truth detail + execute with inputs
  workflow/+page.svelte   — workflow cases
  system/+page.svelte     — system profile, modules, config
```

**Deferred routes:** `pipeline/` (opportunities/leads), dedicated approvals view. Add when backend surfaces exist. The right rail for approvals/exceptions is deferred — start with a simpler layout and add it when the approval lifecycle is richer.

**API client:** `src/lib/api.ts`

```typescript
import { invoke } from '@tauri-apps/api/core';

export async function getDashboard(): Promise<OperatorDashboard> {
  return invoke('get_dashboard');
}

export async function listTruths(): Promise<TruthListItem[]> {
  return invoke('list_truths');
}

export async function executeTruth(key: string, inputs: Record<string, string>): Promise<TruthExecutionSession> {
  return invoke('execute_truth', { key, inputs });
}

// ... one function per Tauri command
```

**TypeScript types:** `src/lib/types.ts` — mirror the Rust view model types. These must match the Serde JSON output of the Rust structs.

**First interactive flows (in priority order):**

1. Browse truth catalog → select a truth → fill typed inputs → execute → see convergence result, criteria outcomes, projection summary
2. Dashboard → see active jobs, pending approvals, recent timeline
3. Accounts → select account → see summary with people, opportunities, subscriptions, timeline
4. Workflow → see cases and approvals, understand why something is blocked

### Styling

Keep it intentional and CRM-specific. Clean, professional, information-dense. No decorative elements. Use a system font stack. Dark mode optional but not required for v1.

Do NOT inherit Wolfgang's theme. This is a different product with different information architecture.

## What you should NOT do

- Do not call the HTTP server from Svelte — go through Tauri commands → crm-app
- Do not implement kernel query methods — the implementor is adding those
- Do not modify truth executors, kernel types, or storage code
- Do not build the TUI yet — shared layer first, desktop first, TUI follows
- Do not build CRUD forms for creating/editing entities — truth execution is the write primitive in v1
- Do not add broad record-navigation flows — this is operator-first, not table-first

## Dependencies

- **From Implementor:** kernel query methods (`list_leads`, `list_tasks`, `list_subscriptions`, `list_workflow_cases`, `list_approvals`, etc.). These are being built in parallel. Stub the `OperatorApp` methods that need them with empty results, then wire in as they land.
- **From QA/Security:** status enum migration. Once landed, your view model types should use the enum names (e.g., `LeadStatus::Qualified` → `"qualified"` in JSON). Until then, expect string status fields.
- **From Implementor (Phase 1):** `SurrealDbKernelStore`. Until it lands, use `InMemoryKernelStore` — data won't persist across restarts but everything else works.

## What exists today

- `apps/desktop/` — Tauri v2 + SvelteKit 5 app with static mock content
- `crates/crm-server/src/http_api.rs` — 3 HTTP endpoints (health, profile, billing ingress)
- `crates/crm-server/src/truth_runtime/` — 9 truth executors that run end-to-end
- `crates/prio-truths/src/lib.rs` — full truth catalog with 18 definitions
- `crates/crm-kernel/` — domain model, kernel methods, capabilities traits

The `../wolfgang` directory has a working Svelte/Tauri app you can reference for patterns.

## Verification

- `cargo test --workspace` green (shared app layer has unit tests)
- `cargo build -p crm-app` compiles
- Desktop app launches, Tauri commands respond with data
- Can browse truths and execute one from the UI
- Can view account summary with real kernel data
- Dashboard renders jobs, approvals, timeline (even if lists are empty with fresh kernel)
