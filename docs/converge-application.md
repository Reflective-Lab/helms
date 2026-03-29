# Converge Application Model

## Position

`crm.prio.ai` should be treated as a Converge application, not as a parallel runtime.

- `converge-core` owns proposals, facts, authority, promotion, budgets, and convergence
- `crm.prio.ai` owns business-domain state, module boundaries, and the public application API
- truths are the translation layer between business jobs and Converge execution

## What Lives Here

This repository should keep:

- CRM and ERP-shaped business objects
- durable state and projections
- module-specific commands and queries
- truth catalog definitions
- gRPC, OpenAPI, and GraphQL application boundaries
- thin desktop and operator-facing surfaces

This repository should avoid recreating:

- a separate promotion gate
- a separate fact constitution
- a separate authority model
- a separate convergence evaluator
- a separate experience ledger model when Converge already provides it

## Truth To Converge Bridge

Each truth now derives a Converge binding made of:

- a `TypesRootIntent`
- a deterministic `truth:{key}` intent ID
- pack IDs inferred from the touched module suites
- required success criteria derived from desired outcomes
- hard constraints derived from guardrails

The upstream runtime now exposes `active_packs` on `TypesRootIntent`, so this mapping is no longer CRM-only metadata. The truth bridge can hand pack selection directly to Converge.

The current pack mapping is:

- `Foundation` -> `prio-foundation-pack`
- `RelationshipCore` -> `prio-relationship-pack`
- `CommercialCore` -> `prio-commercial-pack`
- `UsageRevenueCore` -> `prio-revenue-pack`
- `WorkCore` -> `prio-work-pack`
- `TrustCore` -> `trust`
- `IntelligenceCore` -> `knowledge`

The last two intentionally reuse Converge-native pack names where the runtime already has constitutional responsibility.

## Near-Term Migration

1. Keep the current kernel as a business-state projection while execution moves toward Converge.
2. Replace CRM-local governance types with converge-core types where the constitutional boundary matters.
3. Turn module operations into pack-owned agents or application adapters that Converge can invoke.
4. Move server-side job execution from direct kernel orchestration to `truth -> intent -> packs -> engine.run(...)`.
5. Upstream any missing primitives into Converge when they are generally useful, especially:
   - richer job outcome reporting
   - run-scoped event callbacks
   - durable projection/state boundaries across runs

## Important Constraint

The current Converge binding is a compile-time bridge and discovery surface. It does not yet mean `crm-server` is executing jobs through the engine. That is the next integration step.
