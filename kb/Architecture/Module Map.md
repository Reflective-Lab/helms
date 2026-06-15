# Module Map

## Why This Exists

The current `application-kernel` crate is still a practical integrated kernel, but the intended long-term architecture is modular. These module boundaries are explicit so separate agents or teams can take ownership without arguing about scope every time.

The transport layer now reflects those boundaries directly through module-specific gRPC packages under `proto/prio/<module>/v1`.

Truths are not modules. They sit above these capabilities and compose them into declarative jobs, policies, and local invariants.

> Boundary note (2026-06-15): commercial and revenue modules listed here are
> historical scaffolding, not Helm authority. Subscriptions, plans,
> entitlements, payments, metering used for billing, provider refs, commercial
> ledger authority, and plan-to-app grants belong to Commerce-Rails. Helm may
> keep read-only operator projections or historical/demo fixtures only. See
> [[Operating Authority Boundary]], [[Commercial Authority Inventory]], and
> [[ADRs/HELMS-ADR-001 Commercial Authority Migration]]. Implementors should
> start with `/Users/kpernyer/dev/reflective/BOUNDARY_REGISTRY.md` and the
> active handoff at
> `/Users/kpernyer/dev/reflective/HANDOFF_quorum-sense_2026-06-15.md` before
> treating any module listed here as reusable.

## First-Wave Capability Crates

- `prio-identity`: auth, roles, tenancy, workspace membership
- `prio-parties`: people, organizations, accounts, relationships
- `prio-conversations`: threads, messages, participants, summaries
- `prio-catalog`: products, plans, pricing, bundles, commercial packaging
- `prio-opportunities`: lead intake, qualification, pipeline, forecast context
- `prio-tasks`: work queues, dependencies, follow-up, completion state
- `prio-documents`: notes, files, attachments, extracted facts, versioning
- `prio-subscriptions`: orders, subscriptions, billing periods, usage plans
- `prio-metering`: usage events, meters, consumption, anomaly signals
- `prio-ledger`: balances, credits, debits, adjustments, ledger entries
- `prio-entitlements`: quotas, feature access, policy-based limits
- `prio-payments`: payments, refunds, provider reconciliation, settlement visibility
- `prio-workflow`: cases, steps, transitions, deadlines, wait states
- `prio-approvals`: HITL requests, decisions, rationale, escalation
- `prio-policies`: invariants, constraints, validations, violations
- `prio-facts`: proposed facts, promoted facts, evidence, provenance
- `prio-audit`: provenance, decision trace, evidence links, replayable history
- `prio-intents`: jobs, intent context, success criteria, outcomes, agent runs
- `prio-memory`: semantic memory, embeddings, entity graph, retrieval context
- `prio-agent-ops`: agent runs, operator-control readiness packets, receipt ledger entries, validation contracts, execution traceability

## Suites

### Foundation

- `identity`

### Relationship Core

- `parties`

### Commercial Core (Transitional Boundary Debt)

- `catalog`
- `opportunities`
- `subscriptions`

These entries must be audited under `H-2026-06-15-02`. Any source-of-truth
commercial authority moves to Commerce-Rails or becomes a CR-backed Helm
projection.

### Usage And Revenue Core (Transitional Boundary Debt)

- `metering`
- `ledger`
- `entitlements`
- `payments`

These entries must be audited under `H-2026-06-15-02`. They are not reusable
commercial contracts for marquee apps.

### Work Core

- `conversations`
- `tasks`
- `documents`
- `workflow`

### Trust Core

- `approvals`
- `policies`
- `facts`
- `audit`

### Intelligence Core

- `intents`
- `memory`
- `agent-ops`

`agent-ops` still contains the legacy implementation for the first Helm
operator-control slice. The public app-facing contract is
`helm-operator-control`: `JobReadinessPacket`, `OperatorLedgerEntry`, receipt
families, and the non-authority invariant for readiness views are imported
through that Helm-named crate. See [[Operator Control Common Module]].

## API Naming Convention

Each module owns three public surface names from day one:

- gRPC package and service
- OpenAPI tag and path prefix
- GraphQL query and mutation namespace

Those names are exposed in code through `prio-module-core::CapabilityModule` and aggregated by `prio-modules`.

## Current Extraction Strategy

For now:

- `application-kernel` remains the integrated working kernel
- module crates act as ownership and API boundary scaffolds
- the server exposes the module registry in `/v1/system/profile`
- the truths catalog sits above modules rather than inside the registry

Next step after this scaffold:

1. move module-specific commands and aggregates out of `application-kernel`
2. add module-specific application services behind the existing gRPC package boundaries
3. keep growing the truth library so job design drives extraction priority
4. add per-module OpenAPI and GraphQL contracts
5. let Converge call capability modules instead of a single kitchen-sink backend
