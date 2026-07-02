# Helm Milestones

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
- [x] Shared application layer (`workbench-backend` crate)
- [x] Svelte/Tauri desktop shell with 5 routes
- [x] 5 end-to-end integration flows passing
- [x] 105 tests green

---

## Deferred: Converge Browser Extension (Whatfix Demo)

Moved to backlog 2026-04-10. See [[../Product/Converge Browser Extension Demo]] for original narrative.

---

## Current: Stage 1 — Desktop End-to-End Showcase

**Deadline: 2026-04-25**

**Goal:** Desktop app runs a governed multi-truth pipeline end-to-end,
from inbound signal to scheduled meeting, with live convergence visibility
and human-in-the-loop approvals. One compelling use case that spans
data analysis, routing, scheduling, and HITL — all visible in real time.

**Showcase pipeline:**
`score-inbound-fit` → `qualify-inbound-lead` → `schedule-strategic-meetings`

#### Deliverables

**Epic:** E5

#### Synthetic data generation
- [x] `just gen-seed-data` recipe writes Parquet to `data/seed/` (2026-04-10)
- [x] Website behavior events: ~129k rows across 8 prospects, session clustering, working hours bias, duration tracking (2026-04-10)
- [x] Account context signals: firmographic data, tech stacks, email opens, meeting requests, support tickets, LinkedIn connections (2026-04-10)
- [x] Calendar/availability blocks for scheduling step — 3 reps, 5 days, 30-min slots, 70% availability (2026-04-10)
- [x] Uses polars Parquet writer (converge-analytics provides feature extraction downstream, not write path) (2026-04-10)
- [x] Deterministic seed SEED=42 (reproducible across runs) (2026-04-10)

#### Pipeline orchestration
- [x] Pipeline coordinator: score → qualify → schedule as a chained truth sequence (2026-04-19, pipeline.rs)
- [ ] Each step visible as a distinct convergence run in the UI
- [x] Pipeline state persists across steps — output→input mapping with org_id and fit_score threading (2026-04-19)
- [x] `score-inbound-fit` consumes Parquet via seed loader → usage_events_json → `extract_temporal_features()` + batch inference (2026-04-19)

#### Live convergence visibility
- [x] SSE endpoint for truth execution progress — `/v1/pipeline/showcase/stream` (2026-04-19)
- [x] Desktop UI subscribes to SSE and renders convergence in real time — pipeline route + EventSource (2026-04-19)
- [x] Fact timeline updates live as agents propose and converge promotes (2026-04-19)

#### HITL approval flow
- [x] Qualification gate: ambiguous fit pauses pipeline, surfaces for human review (2026-04-19)
- [x] Meeting booking gate: proposed slate requires human confirmation (2026-04-19)
- [x] Desktop approval UI: review evidence, approve/reject, pipeline resumes (2026-04-19)

#### Desktop connectivity
- [x] Desktop app talks to application-server HTTP API — apiBase + fetch (2026-04-19)
- [x] Operator cockpit shows pipeline progress across all three truths — step indicators (2026-04-19)
- [x] Blocked-step rendering shows which agent is waiting and why — yellow state + reason (2026-04-19)

#### Demo scenario
- [x] Seed script: realistic inbound lead with behavioral Parquet data (2026-04-10)
- [x] Full pipeline run: score → qualify → schedule, with HITL stops — POST /v1/pipeline/showcase/run (2026-04-19)
- [x] Repeatable reset (re-gen seed data + clear kernel state) — POST /v1/pipeline/showcase/reset (2026-04-19)

#### Not in scope

- Chrome extension (deferred)
- Production deployment
- Authentication or multi-tenancy
- New truth runtimes beyond the three in the showcase pipeline

#### Architecture guardrail during Stage 1

Do not deepen the old manual truth-binding pattern unless it is required to finish the showcase milestone.

Before adding new foundational truth plumbing, evaluate the current upstream Organism examples:

- `cargo run -p example-resolution-showcase`
  use as the reference for truth-to-pack and capability resolution
- `cargo run -p example-expense-approval`
  use as the reference for expense and approval truths with admission, adversarial review, and simulation
- `cargo run -p example-vendor-selection`
  use as the reference for strategic sourcing and vendor evaluation truths

The target direction is:

- Helm keeps app-specific truths, projections, Tauri commands, and UX
- Organism increasingly owns generic planning-loop and resolution behavior
- Converge remains the execution and governance foundation

#### Stretch: Mobile Daily Priorities App

**Epic:** E5

A mobile-first JTBD surface for daily operator work. The user opens the app,
spends 2 minutes swiping left/right on surfaced cards to triage and prioritize
their day, then agent flows fire with well-defined truths behind each action.

#### Deliverables
- [ ] React Native or Tauri Mobile shell (single screen: card stack + done state)
- [ ] Priority card feed: pulls today's pending approvals, blocked truths, scheduled meetings, and open opportunities from application-server HTTP API
- [ ] Swipe-left (defer/dismiss) / swipe-right (act now) gesture with haptic
- [ ] Swipe-right triggers the associated truth execution (e.g. approve meeting, advance qualification, acknowledge incident)
- [ ] Card rendering: one-line context, agent reasoning summary, urgency signal, HITL action label
- [ ] "Day started" projection: after triage, the system has a prioritized work queue and launched agent flows
- [ ] Connects to same application-server HTTP API + SSE as desktop (no separate backend)

#### Design constraints
- 2-minute session target — no deep navigation, no settings, no dashboards
- Cards are generated from truth state, not manually curated
- Every swipe is a governed action (proposal → promotion, not a raw mutation)
- Offline-safe: queue swipe decisions locally, sync when connected

---

## Stage 1.5 — Notes & Intelligence Daily Driver

**Deadline: TBD**
**Epic:** E5 (Helm is a daily tool)

**Goal:** The Notes app and Expense tracker work well enough for everyday use. Import from real sources, enrich with intelligence, iterate based on daily friction.

#### Notes application (full implementation)

**Epic:** E5

#### Core vault (already live)
- [x] Vault CRUD with frontmatter freshness
- [x] Tree browser with metadata
- [x] Apple Notes import via AppleScript
- [x] Web snapshot capture with provenance
- [x] Cleanup analysis (dedup, similarity, merge suggestions)

#### Source imports
- [ ] **Google Notes/Keep import** — Takeout export → Markdown, or API adapter
- [ ] **Website fetching** — capture and summarize web pages into vault notes
- [ ] **LinkedIn capture** — profile/post extraction via `organism-intelligence` social provider
- [ ] **Instagram capture** — public profile/post extraction
- [ ] **Facebook capture** — public page extraction
- [ ] **X (Twitter) capture** — tweet/thread extraction into vault notes

#### Intelligence enrichment
- [ ] **OCR in notes** — extract text from images embedded in or attached to notes (`organism-intelligence` OCR)
- [ ] **PDF extraction** — parse PDFs into structured Markdown notes (Mistral OCR / cloud backends)
- [ ] **Object detection** — "tell me what's in this picture" via vision models (Claude, GPT-4o, Gemini, Pixtral)
- [ ] **Enrichment pipeline** — entity extraction, auto-tagging, backlink suggestions

#### Desktop UX
- [ ] Inline OCR trigger from note editor (select image → extract text)
- [ ] PDF drop → auto-extract to new note
- [ ] Social capture from URL bar (paste LinkedIn/X URL → structured note)
- [ ] Vision panel: drag image → get scene description + extracted text

#### Expenses & Receipts (OCR integration)

**Epic:** E5

- [ ] **Receipt OCR pipeline** — photo → structured data (vendor, amount, date, line items)
- [ ] Wire `organism-intelligence` OCR backends (Mistral cloud, Tesseract local, Apple Vision)
- [ ] Receipt photo → auto-create expense item with extracted fields
- [ ] Side-by-side OCR comparison UI (already scaffolded, needs real backends)
- [ ] HITL approval for ambiguous extractions (low-confidence fields flagged for review)

#### Not in scope

- Billing / revenue features (Stage 2)
- Production deployment
- Intent Codec (future — Notes is a slice of it)

---

## Stage 1.75 — Surface Alignment

**Deadline: TBD (immediately after Stage 1 demo)**

Align code, API, and CLI to the Helm Surface Model documented in `kb/Architecture/Helm Surface Model.md`. The framing shift: Business Truth is the core noun, Helm is the operator environment, surfaces (CLI, API, workbench) are peer entry points.

#### Deliverables

**Epic:** E5

#### CLI command taxonomy
- [ ] Define CLI command tree from the truth catalog (`helm truth execute <key>`, `helm truth inspect <key>`, `helm truth list`)
- [ ] Pipeline commands (`helm pipeline run showcase`, `helm pipeline status`)
- [ ] Projection queries (`helm projection list`, `helm projection get <id>`)
- [ ] Approval commands (`helm approve <ref>`, `helm reject <ref> --reason "..."`)
- [ ] Seed and import commands (`helm seed generate`, `helm import parquet <path>`)
- [ ] Audit and replay (`helm audit <truth-key> --last 10`, `helm replay <run-id>`)

#### API namespace reshape
- [ ] Surface-neutral API namespaces: `/v1/truths/`, `/v1/projections/`, `/v1/approvals/`, `/v1/pipelines/`
- [ ] Move workbench-specific routes under `/v1/workbench/` (session state, UI preferences)
- [ ] CLI and automation consume the same `/v1/truths/` endpoints as the desktop
- [ ] SSE endpoint for pipeline/truth progress is surface-neutral (not desktop-specific)
- [ ] OpenAPI spec generated from route definitions

#### Naming migration (code + UI)
- [ ] Rename `crm-server` → `helm-server` (binary + crate)
- [ ] Rename `crm-kernel` → `application-kernel` (already partial — complete it)
- [ ] Rename `crm-storage` → `application-storage` (already partial — complete it)
- [ ] Rename `crm-app` → `workbench-backend` (already done — verify consistency)
- [ ] Update all UI strings from "CRM" / "Outcome Workbench" → "Helm" / "Workbench"
- [ ] Update proto package names to match new taxonomy
- [ ] Follow `kb/Architecture/Naming Migration Map.md` for the full rename schedule

#### Not in scope
- New surfaces (browser workbench, mobile) — those come in Stage 2+
- Actual CLI binary implementation — Stage 1.75 defines the taxonomy, Stage 2 ships it

---

## Stage 2 — Live Revenue

**Deadline: TBD**

Billing integration hardened for real money. First paying customer on the platform.

#### Deliverables

**Epic:** E5

- [ ] Durable idempotency (external_reference on LedgerEntry, survives restart)
- [ ] Runtime billing adapter (Stripe webhook → normalize → billing ingress)
- [ ] Status enum migration (Lead, Task, Quote, Job, AgentRun, WorkflowRun)
- [ ] Error taxonomy expansion (Conflict, Unauthorized, StateTransition, QuotaExceeded)
- [ ] Production deployment target (fly.io or similar)
- [ ] CLI binary ships (`helm` command, truth execution from terminal)

---

## Stage 3 — Platform Signal

**Deadline: TBD**

Multi-domain proof point. Analytics-backed truths. Second vertical beyond CRM.

#### Deliverables

**Epic:** E5

- [ ] Website usage ingestion (Parquet-landed first-party events)
- [ ] converge-analytics integration (behavioral cohorts, account scoring)
- [ ] converge-optimization integration (lead routing, queue balancing)
- [ ] detect-abnormal-token-burn truth (analytics-backed)
- [ ] Second domain vertical scoped and prototyped

---

## Stage 4 — Creative Convergence (Code Generation)

**Deadline: TBD**

Convergence loops that generate, verify, and deploy executable code when they discover missing capabilities. Proven in EXP-002; this stage wires it to production infrastructure.

#### Deliverables

**Epic:** E5

- [ ] Wire Axiom's `WasmCompiler::compile()` into CodeVerifierSuggestor (replaces structural checks with actual Wasm compilation)
- [ ] Build `WasmSuggestor` adapter in `helm-plugin-runtime` (lets verified Wasm modules execute as suggestors in subsequent loops)
- [ ] Connect generated modules back to organism-runtime Registry so future intents can resolve and use them
- [ ] LLM-backed CodeGenSuggestor (replaces stub with real generation from transformation specs)
- [ ] Acceptance test framework: Axiom Gherkin specs as runtime test cases for generated modules
- [ ] Module signing + provenance chain (generated → verified → signed → registered → invoked)
