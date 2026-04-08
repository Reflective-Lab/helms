# crm.prio.ai

A JTBD-driven CRM/ERP substrate built as a Converge application.

- **Converge** is the runtime: convergence, governance, promotion, authority, budgets
- **This repo** is the application boundary: business-domain state, module APIs, truth catalog, operator surfaces
- **Truths** are the bridge: declarative jobs that compile into Converge intent packets and execute through the engine

## Architecture

```
Layer 4: Converge runtime          (orchestration, promotion gate, convergence loop)
Layer 3: Truths / JTBD             (18 truths: 13 jobs, 3 policies, 2 invariants)
Layer 2: Capability modules        (20 modules in 7 suites)
Layer 1: Storage + projections     (in-memory kernel, SurrealDB/LanceDB shapes)
```

Truths compose modules into cross-functional jobs. Each truth maps to a `TypesRootIntent` with pack activation, success criteria, hard constraints, and approval points. The engine runs pack-scoped agents, evaluates criteria post-convergence, and the server projects results into durable CRM state.

See `docs/converge-application.md` for the full integration model.
See `docs/coordinator-handoff.md` for the current implementor assumptions and project framing.

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
6. Server projects facts into CRM kernel entities in a single transaction
7. Response includes convergence result, experience events, and projected entities

Per-truth executors live in `crates/crm-server/src/truth_runtime/`. Fact content uses typed JSON codecs. Confidence mapping between converge (f32) and CRM (basis points) is explicit.

Phase 1 billing integration is now live on the CRM side through a normalized HTTP ingress:

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

`suspend-service-on-payment-failure` is the first CRM truth to reuse a `converge-domain` pack agent directly:

- it registers `OverdueDetectorAgent` from the Money pack to detect overdue invoice state
- it then applies CRM-local suspension policy, recovery workflow, and strategic-account approval routing on top

`reconcile-model-usage-against-customer-ledger` is the first auditor truth in the revenue path:

- it normalizes runtime usage, provider billing, CRM ledger, and CRM entitlement state through explicit source adapters
- it reuses `ReconciliationMatcherAgent` from the Money pack for provider-to-ledger matching
- it projects only facts and workflows, never balance mutations

## Workspace Layout

```
crates/crm-kernel          Domain model, commands, invariants, domain events
crates/crm-storage          KernelStore trait, in-memory impl, config shapes
crates/crm-server           gRPC server, truth runtime, protocol generation
crates/prio-module-core     Shared capability-module descriptor types
crates/prio-modules         Registry of 20 first-wave business modules
crates/prio-truths          Truth catalog + Converge bridge (TruthConvergeBinding)
crates/prio-*               Module ownership scaffolds
proto/prio/*/v1/            Module-specific gRPC contracts + shared types
truths/                     Gherkin feature files for jobs, policies, invariants
apps/desktop                Svelte/Tauri shell (job-centric operator UX)
docs/                       Architecture, module map, truths layer, integration
```

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

`converge-core` is a local path dependency from `../../../converge/crates/core`. The CRM uses:

- `TypesRootIntent`, `TypesRunHooks` for intent construction and execution hooks
- `Agent`, `AgentEffect`, `ProposedFact` for pack-scoped agent implementations
- `CriterionEvaluator`, `CriterionResult` for success criteria evaluation
- `Engine::run_with_types_intent_and_hooks()` for truth execution
- `ExperienceEventObserver` for run-scoped event capture
- `TruthCatalog`, `TruthDefinition` for upstream truth shape

## Commands

```bash
just server    # cargo run -p crm-server
just test      # cargo test --workspace
just fmt        # cargo fmt --all
just desktop   # cd apps/desktop && bun run dev
```

Rust 2024, toolchain floor `rustc 1.94.0`.
