# crm.prio.ai

## What This Is

A JTBD-driven CRM/ERP built as a Converge application. Not a standalone system. Converge owns governance (proposals, facts, authority, promotion, convergence). This repo owns business domain state, module boundaries, and the public application API. Truths are the bridge.

## Critical Rules

- **converge-core is the governance layer.** Do not reimplement promotion gates, fact constitution, authority models, convergence evaluators, or experience ledgers. Use converge-core types.
- **The kernel is a projection store.** Converge runs the job. The CRM kernel projects results into durable business state. Kernel methods are not the orchestration layer.
- **Truths compile into Converge intent packets.** Each truth maps to a `TypesRootIntent` via `TruthConvergeBinding`. Pack activation, success criteria, and hard constraints flow from the truth catalog into the engine.
- **Agents emit proposals, not direct facts.** Everything goes through converge's promotion gate. The routing agent fix (second pass) is the correct pattern.
- **Fact content uses typed JSON codecs.** No pipe-delimited or custom string formats. Fact IDs are the stable contract between agents.

## Build & Test

```bash
just test      # cargo test --workspace
just server    # cargo run -p crm-server
just fmt       # cargo fmt --all
```

Rust 2024, edition 2021 for formatting, toolchain floor `rustc 1.94.0`.

## Converge Dependency

`converge-core` is at `../../../converge.zone/crates/core` (local path). Only `prio-truths` depends on it directly; `crm-server` gets it transitively. Both workspaces must build cleanly.

## Truth Execution Pattern

Per-truth executors live in `crates/crm-server/src/truth_runtime/<truth_key>.rs`.

Pattern for each truth:
1. Load `TruthConvergeBinding` from truth catalog
2. Validate required inputs
3. Create `Engine`, register agents in packs via `register_in_pack()`
4. Run `engine.run_with_types_intent_and_hooks()` with `CriterionEvaluator` + `RecordingObserver`
5. If `persist_projection`, project converge context facts into CRM kernel via single `write_with_events` transaction
6. Return `TruthExecutionArtifacts { result, experience_events, projection }`

Currently executable: `qualify-inbound-lead`, `activate-subscription`, `upgrade-subscription-plan`, `suspend-service-on-payment-failure`, `refill-prepaid-ai-credits`, `score-inbound-fit`, `plan-outbound-campaign`, `match-renewal-context`.

`refill-prepaid-ai-credits` is the reference payment-gated revenue truth. Confirmed payments project a `CreditGrant` ledger entry plus an updated credit entitlement. Unconfirmed or risky top-ups project an approval workflow and no balance mutation.

## Module Structure

20 capability modules in 7 suites. Each module crate exports a `pub const MODULE: CapabilityModule`. Module suites map to converge pack IDs:

- Foundation -> `prio-foundation-pack`
- Relationship Core -> `prio-relationship-pack`
- Commercial Core -> `prio-commercial-pack`
- Usage & Revenue Core -> `prio-revenue-pack`
- Work Core -> `prio-work-pack`
- Trust Core -> `trust` (converge-native)
- Intelligence Core -> `knowledge` (converge-native)

## Domain Conventions

- `confidence_bps: u16` (0-10000 basis points) for CRM-side confidence. Use `converge_confidence_to_bps()` for the mapping.
- `Actor { Human, Agent, System }` on every mutation. Agents must identify themselves.
- Every kernel mutation emits `DomainEvent` + `AuditEntry` + `TimelineEntry`.
- `KernelStore` trait abstracts storage. `InMemoryKernelStore` is the current implementation.
- Truth evaluators may return `CriterionResult::Blocked`; this should surface as `human-intervention-required`, not plain convergence.
- Status fields that are still `String` need migration to enums before their truth can execute.

## What Still Needs Work

- Status enums for `Lead`, `Task`, `OfferQuote`, `OrderSubscription`, `Job`, `AgentRun`, `WorkflowRun`
- Domain error taxonomy (specific `KernelError` variants per operation)
- next revenue-domain truths over the same revenue-domain kernel surface, starting with deeper Money pack reuse in `refill-prepaid-ai-credits` and then `reconcile-model-usage-against-customer-ledger`
- follow-up revenue hardening: richer error variants, list/query surfaces, and module-specific gRPC contracts for catalog, subscriptions, entitlements, and ledger
- `ContextStore` implementation for durable state across runs
- Parquet-aware analytical path for website usage batches, audit/timeline export, and LanceDB interchange. Do not mix this with the SurrealDB transactional path.
