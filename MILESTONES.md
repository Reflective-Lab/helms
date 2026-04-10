# Prio CRM Milestones

> This file is the single source of truth for what ships and when.
> Every session starts by reading this file and scoping work to the **current milestone**.
>
> Rules:
> - No feature work outside the current milestone without explicit approval.
> - Side-fixes get 15 min max, then back to the milestone.
> - Each session ends with a checkpoint: what moved, what's left, any date risk.
> - When a deliverable is done, mark it `[x]` and note the date.

---

## Completed: Desktop UX Milestone

**Reached: 2026-03-30** (commit eb6daef)

- [x] SurrealDB persistence with snapshot pattern
- [x] Kernel query surfaces (list/get for subscriptions, catalog, workflow, orgs)
- [x] REST endpoints for operator consumption
- [x] Shared application layer (crm-app crate)
- [x] Svelte/Tauri desktop shell with 5 routes
- [x] 5 end-to-end integration flows passing
- [x] 105 tests green

---

## Deferred: Converge Browser Extension (Whatfix Demo)

Moved to backlog 2026-04-10. See `DEMO.md` for original narrative.

---

## Current: Stage 1 — Desktop End-to-End Showcase

**Deadline: 2026-04-25**

**Goal:** Desktop app runs a governed multi-truth pipeline end-to-end,
from inbound signal to scheduled meeting, with live convergence visibility
and human-in-the-loop approvals. One compelling use case that spans
data analysis, routing, scheduling, and HITL — all visible in real time.

**Showcase pipeline:**
`score-inbound-fit` → `qualify-inbound-lead` → `schedule-strategic-meetings`

### Deliverables

#### Synthetic data generation
- [ ] `just gen-seed-data` recipe writes Parquet to `data/seed/`
- [ ] Website behavior events: ~50-100k rows per prospect, 30-60 day window (pageviews, feature clicks, docs reads, pricing visits, time-on-page)
- [ ] Account context signals: firmographic data, past interactions, email engagement
- [ ] Calendar/availability blocks for scheduling step
- [ ] Uses `converge-analytics` Parquet write path (`write_parquet_to_store` / local)
- [ ] Deterministic seed (reproducible across runs)

#### Pipeline orchestration
- [ ] Pipeline coordinator: score → qualify → schedule as a chained truth sequence
- [ ] Each step visible as a distinct convergence run in the UI
- [ ] Pipeline state persists across steps (output of step N seeds step N+1)
- [ ] `score-inbound-fit` consumes Parquet via `extract_temporal_features()` + batch inference

#### Live convergence visibility
- [ ] SSE endpoint for truth execution progress (fact proposals, promotions, blocks)
- [ ] Desktop UI subscribes to SSE and renders convergence in real time
- [ ] Fact timeline updates live as agents propose and converge promotes

#### HITL approval flow
- [ ] Qualification gate: ambiguous fit pauses pipeline, surfaces for human review
- [ ] Meeting booking gate: proposed slate requires human confirmation
- [ ] Desktop approval UI: review evidence, approve/reject, pipeline resumes

#### Desktop connectivity
- [ ] Desktop app talks to crm-server HTTP API (not just Tauri IPC)
- [ ] Operator cockpit shows pipeline progress across all three truths
- [ ] Blocked-step rendering shows which agent is waiting and why

#### Demo scenario
- [ ] Seed script: realistic inbound lead with behavioral Parquet data
- [ ] Full pipeline run: score → qualify → schedule, with 2 HITL stops
- [ ] Repeatable reset (re-gen seed data + clear kernel state)

### Not in scope

- Chrome extension (deferred)
- Production deployment
- Authentication or multi-tenancy
- New truth runtimes beyond the three in the showcase pipeline

### Stretch: Mobile Daily Priorities App

A mobile-first JTBD surface for daily operator work. The user opens the app,
spends 2 minutes swiping left/right on surfaced cards to triage and prioritize
their day, then agent flows fire with well-defined truths behind each action.

#### Deliverables
- [ ] React Native or Tauri Mobile shell (single screen: card stack + done state)
- [ ] Priority card feed: pulls today's pending approvals, blocked truths, scheduled meetings, and open opportunities from crm-server HTTP API
- [ ] Swipe-left (defer/dismiss) / swipe-right (act now) gesture with haptic
- [ ] Swipe-right triggers the associated truth execution (e.g. approve meeting, advance qualification, acknowledge incident)
- [ ] Card rendering: one-line context, agent reasoning summary, urgency signal, HITL action label
- [ ] "Day started" projection: after triage, the system has a prioritized work queue and launched agent flows
- [ ] Connects to same crm-server HTTP API + SSE as desktop (no separate backend)

#### Design constraints
- 2-minute session target — no deep navigation, no settings, no dashboards
- Cards are generated from truth state, not manually curated
- Every swipe is a governed action (proposal → promotion, not a raw mutation)
- Offline-safe: queue swipe decisions locally, sync when connected

---

## Stage 2 — Live Revenue

**Deadline: TBD**

Billing integration hardened for real money. First paying customer on the platform.

### Deliverables

- [ ] Durable idempotency (external_reference on LedgerEntry, survives restart)
- [ ] Runtime billing adapter (Stripe webhook → normalize → billing ingress)
- [ ] Status enum migration (Lead, Task, Quote, Job, AgentRun, WorkflowRun)
- [ ] Error taxonomy expansion (Conflict, Unauthorized, StateTransition, QuotaExceeded)
- [ ] Production deployment target (fly.io or similar)

---

## Stage 3 — Platform Signal

**Deadline: TBD**

Multi-domain proof point. Analytics-backed truths. Second vertical beyond CRM.

### Deliverables

- [ ] Website usage ingestion (Parquet-landed first-party events)
- [ ] converge-analytics integration (behavioral cohorts, account scoring)
- [ ] converge-optimization integration (lead routing, queue balancing)
- [ ] detect-abnormal-token-burn truth (analytics-backed)
- [ ] Second domain vertical scoped and prototyped
