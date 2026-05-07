# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

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
