# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

### Changed (RFL-154 — helm operator seam cut)
- `helm-module-contracts` now owns the complete operator vocabulary: `operator_receipts` module (`JobReadinessPacket`, `OperatorLedgerEntry`, all receipt families, `OperatorControlError`, hash/ledger helpers) and `operator_preview` module (`OperatorControlPreview`, `OperatorControlPreviewBacking`, `OperatorReceiptFamilyView`, `operator_receipt_families()`). sha2 added as direct dep.
- `prio-agent-ops` is now manifest-only: capability advertisement remains, operator-control vocabulary removed. Dep-tree no longer includes truth-catalog→prism-analytics→polars; that transitive chain is Plan-B scope.
- `workbench-backend` repoints operator-control types to `helm-module-contracts`.
- `helm-operator-control` depends on `helm-module-contracts` only (drops `workbench-backend`, `prio-agent-ops`, `polars`). Seed loading extracted to `seed-gen` via `ShowcaseSeedSource` injection. No shim re-exports.
- `seed-gen` gains a library target (`src/lib.rs`) exposing `pub mod showcase_seed`, removing the `#![allow(dead_code)]` suppression.
- arena `cross-extension-smoke` repoint (prio-agent-ops → helm-module-contracts) lands in arena-tests immediately after this merge (RFL-154 T6).

- `helm-module-contracts` bumped to 0.3.0: three new public modules (`showcase_pipeline`, `operator_receipts`, `operator_preview`) plus `sha2` as a direct runtime dep constitute minor surface expansion (RFL-154). All in-repo consumers updated.

### Added (RFL-154 — helm operator seam cut)
- trybuild compile-fail suite in `helm-module-contracts`: one case proves private validation helpers (`validate_sha256`) are not callable from external code (parse-don't-validate gate).
- trybuild regression guards in `helm-operator-control`: two compile-fail cases prove `workbench_backend` and `prio_agent_ops` are no longer dep-resolvable.
- `#[ignore]`d soak test in `helm-operator-control/tests/module_test.rs` (`soak_packet_ledger_preview_no_drift`): 100 000 iterations of packet→ledger→preview via `StaticReadinessFeed`, asserts no id/hash drift; proof run at 10 000 iter in 1.65s (6 061 iter/s).
- `kb/Architecture/Foundation Contracts.md` — operator vocabulary row in contracts surface table.
- `kb/Architecture/Operator Control Common Module.md` — reflects RFL-154 ownership; contracts is the canonical import path.
- `kb/Architecture/Module Map.md` — `prio-agent-ops` scoped to manifest-only; new `Seam Contracts` section for `helm-module-contracts`.

### Changed
- `HelmModule` trait and `ModuleState` enum extracted from `runtime-runway/runway-app-host` into `helm-module-contracts` (RFL-128). `init()` no longer takes `&HostContext` — parameter was unused by all five modules. `helm-operator-control` and `helm-truth-execution` now import directly from `helm-module-contracts` with no `runway-app-host` dep; `helm-coordination`, `helm-governed-jobs`, and `helm-session-host` retain the dep for EventHub/SSE/SessionOwnershipLayer under approved `# RP-HELMS-SUBSTRATE-SEAM` seams.

### Added
- `helm-coordination` crate — multi-operator coordination for Helm's headless surface under an optimistic model: `OperatorPrincipal` + `PrincipalResolver` identity seam (`RequestActorResolver`), workspace-scoped `SessionRegistry` (heartbeat-leased), advisory `PresenceRegistry` (soft-claims, no locks), optimistic `DecisionLedger` (idempotent-agree / divergent-conflict), `AuthorityResolver` (`PermissiveAuthority` default), a `/v1/coordination/` router, a workspace-scoped SSE `/stream`, and a `CoordinationModule` (`helm.coordination`) mountable alongside `GovernedJobsModule`. See `kb/Architecture/Operator Coordination.md`.
- `helm-coordination` gate front door `POST /v1/coordination/gates/{ref_id}/decision` — authority-checked, deduped gate approvals that record in the ledger and only signal `JobStreamState::signal_gate` on a fresh accepted decision; emits attributed `decision.recorded` / `decision.conflict` / `decision.denied` events.
- `helm-governed-jobs` now attributes run events: `Publisher` stamps `EventEnvelope.actor` and `JobRunTask.initiator: Option<Actor>` threads an initiating principal through `run_job_task` (falling back to a system actor). `StreamJobRequest` accepts an optional `actor` tag.
- `helm-governed-jobs::JobStreamState::gate_timeout` (Duration) — configurable HITL gate wait timeout. Defaults to 600s, preserving the original RealtimeHub contract for `quorum-server` / `atlas-server`. Integration tests can override via struct-literal + `..default()`.
- `helm-governed-jobs` now emits a `gate.timeout` event before `job.failed` when the gate-wait timeout fires, distinct from the cancellation path.
- `helm-governed-jobs::{JobRunTask, run_job_task}` pub so integration tests can drive the governed-job loop directly without going through the HTTP layer.
- `helm-governed-jobs/tests/gate_test.rs` — integration coverage for gate-rejected and gate-timeout paths preserved by the Phase 4b redo. Both tests run in <500ms.
- `experiments/code/EXP-002/generate_data_transformer.rs` — preserved the convergence-step proof from `a63811c^` (pre-Phase-6a) after `application-server` deletion. Includes revival README pointing back at `experiments/EXP-002.md`.
- `prio-agent-ops` operator-control common module slice: deterministic `JobReadinessPacket`, clause-level evidence readiness, receipt families, non-authoritative `OperatorLedgerEntry`, content-addressed ids, and unit tests for replay stability and authority boundaries.
- Workbench operator-control preview surface: `/v1/workbench/operator-control/preview` exposes the first injected live readiness packet, matching non-authoritative ledger entry, and receipt-family catalog for Helm UI integration.
- Workbench operator-control previews surface: `/v1/workbench/operator-control/previews` returns injected live readiness packets so apps can share the same UI/API contract without Helm shipping a static app portfolio.
- Desktop workbench Operator Control tab renders the standard readiness packet, evidence states, operator actions, ledger entry, and receipt-family catalog in both embedded and remote backend modes.
- Operator Control no longer synthesizes app-specific readiness packets when no live feed is supplied; absent or empty feeds return an empty preview list and the singular preview route returns an operator-control error.
- Operator Control readiness packets can now carry optional fuzzy traces produced by app-owned Suggestor paths while Helm remains display-only.
- Fuzzy readiness traces can now include an optional typed defuzzified score, including method, score basis points, output domain, and domain steps. Helm hashes and validates the scalar beside memberships/rules without turning it into domain authority.
- `kb/Architecture/Operator Control Common Module.md` documents the Helm side of the Axiom app-probe extraction: long-running job receipts, temporal-evidence receipts, and content/publication receipts share deterministic backlink mechanics while app payloads remain local.
- Helm architecture docs now recognize Converge `SubjectRef` as the typed subject identity bridge for truth execution, operator-control readiness packets, and Mosaic-backed Suggestor results while keeping subject vocabulary and authority semantics app-owned.

### Fixed
- Desktop Tauri Rust crate now belongs to the root Helms workspace, uses the same local Organism/Converge dependency graph, and relies on the root `Cargo.lock`; this removes the duplicate `converge_core` / `converge_model` type identity split in embedded desktop checks.

## [0.2.0] - 2026-05-08

### Added
- `truth-catalog::intent_compile` — `compile_intent_for_truth` runs axiom's typed `compile_intent_from_source` against a truth's `.feature` source plus a per-truth helms overlay (context JSON, expires, bare-string constraints/authority). Replaces the legacy `organism_recipe` IntentPacket-by-hand path
- `truth-catalog::admission` — `admit_truth_intent` stages a Truth's IntentPacket through Converge's typed admission boundary via `Runtime::admit_intent`, plus `select_formation_for_intent` + `default_helms_capabilities` for the formation-selection front half
- `truth-catalog::orchestration` — tournament policy primitives: `AutoRunOptions` (count-based cost model: `race_alternates`, `max_candidates`, `relative_cutoff`), `prepare_candidates` (slate filter with primary-always-survives + clamps), and explicit refusal of `Reversibility::Irreversible + race_alternates`. Six unit tests covering the policy decisions
- Governance blocks (`Intent: Outcome:`) added to the four migrated `.feature` files: `qualify_inbound_lead`, `submit_expense_report`, `evaluate_acquisition_target`, `plan_outbound_campaign`
- `kb/Architecture/HITL Admission Gate.md` — ADR locating the human-in-the-loop gate at `truth-catalog::admission::admit_truth_intent` (pre-`Runtime::admit_intent`), keeping the kernel pure and giving helms a single choke point. Tournaments forbidden for irreversibles, approval per-execution (bound to `IntentPacket::id`), failure mode is rejection
- `packages/helm-flow-ui` — Svelte/TypeScript replay adapter and `FlowContainer` component
- `crates/notes` — replaces `crates/helm-notes` (rename)
- `data/receipts/` fixtures
- 3 truth executors (qualify-inbound-lead, evaluate-acquisition-target, plan-outbound-campaign) now log the FormationGuru's chosen template + alternates after admission

### Changed
- `axiom-truth`: `git tag v0.6.0` → local path `0.8.1`
- `organism-*`: `1.7.x` → `1.8.0` (path)
- Converge extensions: workspace deps switched to canonical `converge-{prism-analytics,atelier-domain,mnemos-knowledge,arbiter-policy}` package names; alias keys preserved so existing `use converge_*` keeps working
- `[patch.crates-io]` extended with axiom-truth, all renamed converge extensions, and `converge-manifold-adapters` (`features = ["_chat"]` so the local-patched manifold's gated chat-backend selector resolves for axiom's `guidance` module)
- `organism_binding_for_truth` / `display_pack_names_for_truth` now take `&Registry`; the helms-static `default_organism_registry` factory moved out of `truth-catalog` into `workbench-backend` (registry is host-layer state, not catalog metadata)
- `truth-catalog` drops its `organism-domain` workspace dep
- `MILESTONES.md`: Stage 1 marked shipped at v0.1.1 (2026-04-25); Stage 1.5 promoted to current

### Removed
- `truth-catalog::organism::organism_recipe` and the per-truth IntentPacket match arms (~100 lines). Equivalence-gate tests retired now that the legacy recipe is gone
- `crates/helm-notes` (renamed to `crates/notes`)
- `examples/capture-*`, `examples/extract-*`, `examples/describe-image`, `examples/helm-capture` — superseded
- `apps/desktop/src/routes/notes/+page.svelte`

### Fixed
- `ContextFact::{id,content}` private-field migration: ~67 call-site fixes across the truth runtime to use the new getter methods
- `AgentEffect` struct-literal construction replaced with `with_proposal` / `with_proposals` / `builder` API (9 sites)
- `ConvergeError`: handle new `InvalidAdmission` / `InvalidSnapshot` variants in `status_from_converge`
- `UnitInterval` no longer implements `Mul<f64>` — basis-point conversions go through `.as_f64()` first
- `evaluate-acquisition-target` truth: replaced retired `DdLlm` trait with `DynChatBackend` (now from `converge-provider`); `StubDdLlm` → `StubChatBackend`

## [0.1.1] - 2026-04-25

### Added
- `evaluate-acquisition-target` truth — convergent DD via organism formation (EXP-001 confirmed)
- Code generation as convergence step — CodeGenSuggestor + CodeVerifierSuggestor (EXP-002 confirmed)
- Pipeline coordinator — score → qualify → schedule chained truth sequence
- SSE endpoint (`/v1/pipeline/showcase/stream`) for live convergence visibility
- HITL approval flow — `/v1/approvals/pending`, approve/reject endpoints
- Desktop pipeline page with SSE-driven timeline and approval UI
- `helm-notes` crate — smart capture (social, web, OCR, PDF) with vault write
- `helm-capture` CLI — single entry point for all capture types
- 5 CLI examples (capture-social, capture-web, extract-ocr, extract-pdf, describe-image)
- Capability Binding architecture doc (Options A/B/C for external systems)
- Business Truths article draft
- Experiments framework (LOG.md, EXP-001, EXP-002)
- Stage 1.75 (Surface Alignment) and Stage 4 (Creative Convergence) milestones

### Changed
- Renamed crates: crm-* → application-*/workbench-backend, prio-truths → truth-catalog, prio-modules → capability-registry, prio-module-core → capability-core
- Migrated all truth executors from deprecated Agent → Suggestor API (converge v3.4.0)
- Migrated all truth executors to async fn execute + .await
- Seed-gen crate for data generation
- Extension shell scaffold
- New truth scaffolding for Stage 1
- Milestone reprioritization for Stage 1

## 2026-04-10

### Added
- SurrealDB persistence for desktop
- Operator cockpit UI
- 105 tests green
- Executable truth runtime
- Revenue workflows (score → qualify → schedule)

## 2026-04-06

### Added
- Initial project structure
