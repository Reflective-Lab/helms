# CRM Kernel For Converge

This repository starts from the premise that CRM is a domain substrate, not the product surface.

- `Converge` is the system of interaction and orchestration.
- This backend is the system of record and Converge application boundary.
- The thin desktop shell is a job-centric operator surface, not a classic record browser.

The workspace is now aligned on Rust 2024 with a local toolchain floor of `rustc 1.94.0`.

## What This Baseline Includes

- a Rust workspace with a headless CRM kernel
- explicit capability-module crates for future extraction and parallel ownership
- a gRPC contract centered on operational memory and workflow state
- in-memory runtime storage plus explicit SurrealDB and LanceDB configuration shapes
- a minimal Svelte/Tauri desktop shell that visualizes the intended JTBD-oriented UX

## Workspace Layout

- `crates/crm-kernel`: business-domain model and temporary local projection layer
- `crates/crm-storage`: storage modes, runtime state container, and local-first config
- `crates/crm-server`: gRPC server, protocol generation, and health endpoint
- `crates/prio-module-core`: shared capability-module descriptor types
- `crates/prio-modules`: registry of first-wave business modules
- `crates/prio-*`: module ownership scaffolds for independent implementation work
- `crates/prio-truths`: declarative JTBD truth catalog that composes modules
- `proto/prio/*/v1/*.proto`: module-specific public backend contracts plus shared common types
- `truths/`: Gherkin-compatible truth files for jobs, policies, and module-local invariants
- `apps/desktop`: thin Svelte/Tauri shell for jobs, approvals, account summary, and timeline
- `docs/architecture.md`: bounded contexts and design notes
- `docs/module-map.md`: suites, module boundaries, and extraction plan
- `docs/truths-layer.md`: four-layer architecture and starter JTBD truth library
- `docs/converge-application.md`: how truths map into Converge intents and packs
- `docs/platform-roadmap.md`: future `prio.ai` domain surface and bounded-context expansion
- `contracts/module-registry.yaml`: machine-readable module and API naming map

## Why This Shape

The backend tracks:

- organizations and people
- relationships and opportunities
- activities, notes, documents, and communication events
- workflow cases, permissions, facts, and audit history
- metadata for custom objects, fields, relationships, and views

That gives Converge a stable operational memory layer behind job execution and agent coordination without forcing the product into a traditional CRM UI.

The governance runtime is intentionally moving toward `converge-core`. Truths now expose a Converge binding so the application API can tell clients which packs a job activates and which intent packet should be handed to the runtime.

`converge-core` now has a native `active_packs` field on `TypesRootIntent`, so pack activation is no longer just CRM-side metadata.

## Commands

```bash
just server
just test
just fmt
```

The desktop app follows the Wolfgang pattern and lives under `apps/desktop`.

## gRPC Surface

The backend contract is intentionally layered. Current capability and catalog packages include:

- `prio.identity.v1`
- `prio.parties.v1`
- `prio.catalog.v1` as a reserved module contract name
- `prio.opportunities.v1`
- `prio.conversations.v1`
- `prio.documents.v1`
- `prio.workflow.v1`
- `prio.facts.v1`
- `prio.metadata.v1`
- `prio.modules.v1`
- `prio.truths.v1`
- `prio.common.v1` for shared record and enum types

`prio.truths.v1` now includes a Converge execution binding per truth, so the public API exposes:

- the declarative job definition
- the Converge intent request derived from that truth
- the pack IDs that should be activated
