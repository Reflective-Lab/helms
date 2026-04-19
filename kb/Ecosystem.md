# Helm

Helm is the operator-facing application layer built on top of Converge and Organism.

- **Converge** is the runtime: convergence, governance, promotion, authority, budgets
- **Organism** is the reusable intelligence foundation: OCR, note intelligence, document understanding, semantic helpers
- **This repo** is the application boundary: business-domain state, capability composition, truth catalog, workbench surfaces
- **Truths** are the bridge: declarative jobs that compile into Converge intent packets and execute through the engine

## Foundation-First Rule

Before implementing any core, basic, or foundational function in this repo:

1. Check `../converge/CAPABILITIES.md`
2. Check `../organism/CAPABILITIES.md`
3. Reuse an upstream capability if it already exists
4. If the need is generic and missing, treat it as an upstream capability gap rather than defaulting to a local implementation

Helm should keep application-specific composition, truths, projections, Tauri commands, and UX. Reusable foundations belong upstream.

Current code names are still mid-migration. The product surface is moving to `Helm`, while many
crates, proto packages, and some docs still use legacy `Outcome Workbench`, `crm-*`, and `prio-*` names. See
[[Architecture/Naming Migration Map]] for the staged rename direction.

## Architecture

```
Layer 4: Converge runtime          (orchestration, promotion gate, convergence loop)
Layer 3: Truths / JTBD             (outcome contracts and execution bindings)
Layer 2: Capability modules        (reusable backend business capabilities)
Layer 1: Application projections   (kernel, storage, workbench-facing summaries)
```

Truths compose capabilities into cross-functional jobs. Each truth maps to a `TypesRootIntent`
with pack activation, success criteria, hard constraints, and approval points. The engine runs
pack-scoped agents, evaluates criteria post-convergence, and the application server projects the
result into durable application state.

See [[Architecture/Converge Application]] for the full integration model.
See [[Architecture/Application Layer Restructure]] for the core concept split.
See [[Architecture/Agent Architecture Brief]] for the shareable agent brief.
See [[Operations/Coordinator Handoff]] for the current implementor assumptions and project framing.

## Truth Execution

Current executable truths:

- `qualify-inbound-lead`
- `activate-subscription`
- `upgrade-subscription-plan`
- `suspend-service-on-payment-failure`
- `reconcile-model-usage-against-customer-ledger`
- `refill-prepaid-ai-credits`
- `score-inbound-fit`
- `plan-outbound-campaign`
- `match-renewal-context`

The execution flow is the same for each truth:

1. `ExecuteTruth` gRPC call with truth key + inputs
2. Truth binding resolves to `TypesRootIntent` with pack IDs
3. Converge engine runs pack-scoped agents (`LeadQualificationAgent`, `LeadRoutingAgent`)
4. Both agents emit proposals through the promotion gate
5. `CriterionEvaluator` checks desired outcomes against converged context
6. Server projects facts into application kernel entities in a single transaction
7. Response includes convergence result, experience events, and projected entities

Per-truth executors live in `crates/crm-server/src/truth_runtime/`. Fact content uses typed JSON
codecs. Confidence mapping between Converge (`f32`) and application state (basis points) is
explicit.

Phase 1 billing integration is now live through a normalized HTTP ingress:

- `POST /v1/integrations/billing/events`
- bearer auth via `CRM_BILLING_INGRESS_TOKEN`
- successful events are idempotent by `idempotency_key` or `source:truth:event_id`
- runtime billing maps normalized events to truths instead of calling kernel commands directly

Current normalized billing event kinds:

- `prepaid_top_up_settled` -> `refill-prepaid-ai-credits`
- `subscription_activation_requested` -> `activate-subscription`
- `subscription_payment_failed` -> `suspend-service-on-payment-failure`
- `ledger_reconciliation_requested` -> `reconcile-model-usage-against-customer-ledger`

Blocked truths are now first-class at the runtime boundary:

- criteria can return `blocked` with an approval reference
- Converge emits `human-intervention-required` when a truth converges into a valid waiting state rather than a completed outcome

`activate-subscription` now projects structured commercial state through the truth response:

- projected subscription lifecycle state
- projected entitlements
- projected ledger opening balance

`refill-prepaid-ai-credits` now reuses the same revenue substrate and adds a payment-gated credit grant path:

- confirmed top-ups append a `CreditGrant` ledger entry and increase the credit entitlement balance
- unconfirmed or risky top-ups stop honestly, open an approval workflow, and do not mutate balance state

`upgrade-subscription-plan` now reuses the same revenue substrate for governed plan changes:

- standard upgrades replace entitlements and append an `Adjustment` ledger entry for the commercial delta
- non-standard upgrades stop honestly, open an approval workflow, and do not mutate subscription state

`suspend-service-on-payment-failure` is the first application truth to reuse a `converge-domain`
pack agent directly:

- it registers `OverdueDetectorAgent` from the Money pack to detect overdue invoice state
- it then applies local suspension policy, recovery workflow, and strategic-account approval routing on top

`reconcile-model-usage-against-customer-ledger` is the first auditor truth in the revenue path:

- it normalizes runtime usage, provider billing, ledger state, and entitlement state through explicit source adapters
- it reuses `ReconciliationMatcherAgent` from the Money pack for provider-to-ledger matching
- it projects only facts and workflows, never balance mutations

## Workspace Layout

```
crates/crm-kernel          Directory for the `application-kernel` crate
crates/crm-storage         Directory for the `application-storage` crate
crates/crm-server          Directory for the `application-server` crate
crates/crm-app             Directory for the `workbench-backend` crate
crates/prio-module-core    Current capability descriptor crate name
crates/prio-modules        Current capability registry crate name
crates/prio-truths         Current truth catalog crate name
crates/prio-*              Current capability leaf crate family
proto/prio/*/v1/           Current proto package family
truths/                    Gherkin feature files for jobs, policies, invariants
apps/desktop               Svelte/Tauri workbench shell
kb/                        Obsidian knowledgebase and project documentation
```

These are current names, not target names. The staged target map is in
[[Architecture/Naming Migration Map]].

## Module Suites

| Suite | Modules |
|-------|---------|
| Foundation | identity |
| Relationship Core | parties |
| Commercial Core | catalog, opportunities, subscriptions |
| Usage & Revenue Core | metering, ledger, entitlements, payments |
| Work Core | conversations, tasks, documents, workflow |
| Trust Core | approvals, policies, facts, audit |
| Intelligence Core | intents, memory, agent-ops |

## gRPC Surface

Current capability packages:

- `prio.common.v1` shared record and enum types
- `prio.identity.v1` auth, roles, workspace membership
- `prio.parties.v1` organizations, people, relationships, account summaries
- `prio.opportunities.v1` pipeline, qualification
- `prio.conversations.v1` threads, messages
- `prio.documents.v1` notes, files, attachments
- `prio.workflow.v1` cases, state transitions
- `prio.facts.v1` proposed and promoted facts
- `prio.metadata.v1` custom objects, fields, views
- `prio.modules.v1` capability module registry
- `prio.truths.v1` truth catalog, Converge bindings, `ExecuteTruth` RPC

Current HTTP integration packages:

- `/health` liveness
- `/v1/system/profile` runtime config + module registry
- `/v1/integrations/billing/events` normalized billing ingress for runtime adapters

## Converge Dependency

Current live code still depends directly on `converge-core` in places because the truth-execution boundary predates the curated Converge surfaces.

Target for new work:

- `converge-kernel` for execution
- `converge-model` for governed semantic types
- `converge-pack` for authoring contracts
- `converge-provider-api` + `converge-provider` for capability access

Legacy `converge-core` usage remains until the truth runtime is migrated. Today it is used for:

- `TypesRootIntent`, `TypesRunHooks` for intent construction and execution hooks
- `Agent`, `AgentEffect`, `ProposedFact` for pack-scoped agent implementations
- `CriterionEvaluator`, `CriterionResult` for success criteria evaluation
- `Engine::run_with_types_intent_and_hooks()` for truth execution
- `ExperienceEventObserver` for run-scoped event capture
- `TruthCatalog`, `TruthDefinition` for upstream truth shape

## Commands

```bash
just server    # cargo run -p application-server
just test      # cargo test --workspace
just fmt       # cargo fmt --all
just desktop   # cd apps/desktop && bun run dev
```

Rust 2024, toolchain floor `rustc 1.94.0`.
