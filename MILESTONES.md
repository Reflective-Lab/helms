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

## Current: Stage 1 — Converge Browser Extension (Whatfix Demo)

**Deadline: 2026-04-18** (15 days)

**Goal:** A Chrome extension side panel that shows governed job state powered by
Converge. Demo-ready for Whatfix meeting. See `DEMO.md` for full narrative.

**Thesis:** DAP instruments applications. Converge governs jobs.

### Deliverables

#### Extension shell
- [ ] Chrome manifest v3 extension with side panel
- [ ] Svelte UI in side panel (reuse desktop component patterns)
- [ ] Connect to crm-server HTTP API (localhost for demo)

#### Job state view
- [ ] Active truth execution list with convergence progress
- [ ] Fact timeline per job (proposed → promoted, with agents identified)
- [ ] Blocked step rendering with policy reason visible
- [ ] Approval action button (calls truth execute / approval endpoint)

#### Live updates
- [ ] Polling or SSE from server for real-time job state changes
- [ ] Side panel updates when billing event arrives

#### Demo scenario
- [ ] Seed data script: Acme Corp onboarding (org, contact, catalog item, subscription)
- [ ] Scripted flow: billing event → truth fires → 3 steps converge → 1 blocked → approve → done
- [ ] Repeatable reset (re-run seed to start fresh)

#### Narrative
- [ ] 3-5 slide deck framing the thesis (DAP + Converge positioning)
- [ ] 90-second demo script practiced and timed

### Converge.zone primitives driven

- [ ] SSE or webhook surface for external consumers of convergence state (if needed)

### Not in scope

- DOM injection or page reading
- Integration with any specific enterprise app
- Authentication or multi-tenancy
- Production deployment
- New truths (existing 9 are sufficient)

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
