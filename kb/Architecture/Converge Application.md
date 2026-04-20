# Converge Application Model

## Position

Helm is a Converge application, not a parallel runtime.

- Converge owns proposals, facts, authority, promotion, budgets, and convergence
- Helm owns business-domain state, module boundaries, and the public application surfaces
- truths are the translation layer between business jobs and Converge execution

## What Lives Here

This repository keeps:

- current business projections (Organization, Person, Opportunity, etc.)
- durable state and projections via the application kernel
- module-specific commands and queries
- truth catalog definitions and per-truth executors
- current manual converge agent implementations for each truth
- criterion evaluators that check success against converged context
- gRPC and HTTP application boundaries
- CLI and workbench-facing operator surfaces

This repository does not recreate:

- a separate promotion gate (agents emit proposals, converge promotes)
- a separate fact constitution (Converge types are authoritative)
- a separate authority model (AuthorityGrant, Actor from Converge)
- a separate convergence evaluator (engine loop with fixed-point detection)
- a separate experience ledger (ExperienceEventObserver captures events during runs)

## Truth Execution (Live)

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

`qualify-inbound-lead` remains the simplest reference path through the Converge engine:

1. `ExecuteTruth` gRPC call enters `application-server`
2. Dispatcher routes to `truth_runtime/qualify_inbound_lead.rs`
3. Executor loads `TruthConvergeBinding` -> `TypesRootIntent`
4. Engine runs with pack-scoped agents and `TypesRunHooks` (evaluator + observer)
5. Both agents emit proposals through converge's promotion gate
6. `QualifyInboundLeadEvaluator` checks criteria against converged context
7. Server projects facts into the application kernel in a single `write_with_events` transaction
8. Response includes convergence result, experience events, and projected entities

Fact content uses typed JSON codecs. Confidence mapping is explicit via `converge_confidence_to_bps()`.

Phase 1 billing integration now has a concrete application boundary on the Helm application side:

- `POST /v1/integrations/billing/events`
- bearer auth via `CRM_BILLING_INGRESS_TOKEN`
- normalized event kinds map to truths instead of bypassing the job layer
- successful deliveries are cached by idempotency key so duplicate runtime retries do not double-project business state

Truth execution now distinguishes three important end states:

- `criteria-met` for completed truths
- `human-intervention-required` for truths that converged into approval-needed state
- `converged` for truths that stabilized without satisfying required criteria and without an explicit blocked condition

## Current Pattern vs Direction

The current live integration pattern in this repo is still manual:

- `prio-truths` derives `TruthConvergeBinding`
- bindings write `TypesRootIntent.active_packs` explicitly
- `application-server` executors register pack agents manually per truth

This works, but it is now a transitional level-1 pattern.

Organism has moved forward and now provides a stronger upstream direction:

- `organism-pack` for the structured planning loop contract
- admission control before engine start
- adversarial review before commit
- simulation and budget-envelope checks before commit
- resolution support that can infer packs and capability needs from the truth rather than relying only on local static bindings

For Helm, that means future truth work should prefer:

- truths that declare what should become true
- Organism resolution that determines what packs and capabilities are needed
- early surfacing of missing prerequisites such as provider credentials or unavailable upstream capabilities

The most relevant upstream examples are:

- `../organism/examples/expense-approval`
  maps to expense and reimbursement truths with admission, adversarial review, and budget simulation
- `../organism/examples/resolution-showcase`
  maps to truth-to-pack auto-resolution and prerequisite surfacing, which is the likely replacement for parts of `prio-truths`
- `../organism/examples/vendor-selection`
  maps to strategic sourcing and multi-criteria vendor evaluation truths

Local manual bindings should still be maintained where already live, but new foundational truth plumbing should be justified against these upstream patterns first.

## Truth to Converge Bridge

Each truth derives a Converge binding:

- a `TypesRootIntent` with deterministic `truth:{key}` intent ID
- pack IDs inferred from touched module suites, written into `intent.active_packs`
- required success criteria derived from desired outcomes
- hard constraints derived from guardrails
- risk posture: Conservative if approval points exist, Balanced otherwise

Pack mapping:

| Suite | Pack ID |
|-------|---------|
| Foundation | `prio-foundation-pack` |
| Relationship Core | `prio-relationship-pack` |
| Commercial Core | `prio-commercial-pack` |
| Usage & Revenue Core | `prio-revenue-pack` |
| Work Core | `prio-work-pack` |
| Trust Core | `trust` |
| Intelligence Core | `knowledge` |

Trust Core and Intelligence Core reuse converge-native pack names where the runtime already has constitutional responsibility.

## Upstream Primitives

These currently arrive through `converge-core` in live code, but the target teaching surface for new work is `converge-kernel` + `converge-model` + `converge-pack`, and `converge-provider-api`/`converge-provider` where capability access is needed:

- `TypesRootIntent.active_packs` + `engine.run_with_types_intent_and_hooks()` for intent-driven pack activation
- `TruthCatalog` trait + `TruthDefinition` for platform-native truth shape
- `CriterionEvaluator` + `CriterionResult` (Met/Unmet/Indeterminate) for success criteria evaluation
- `CriterionResult::Blocked` + `StopReason::HumanInterventionRequired` for post-convergence approval-needed outcomes
- `TypesRunHooks` with optional criterion evaluator and event observer
- `ContextStore` trait for durable context snapshots across runs
- `ConvergeError::stop_reason()` for application-level error projection

## Next Revenue Truths

`activate-subscription` and `refill-prepaid-ai-credits` are now live end-to-end. They return structured commercial projection data through the truth response:

- projected subscription lifecycle state
- projected entitlements
- projected ledger entries

`upgrade-subscription-plan` is now live end-to-end:

- standard upgrades converge to a governed plan change, entitlement replacement, and `Adjustment` ledger entry
- non-standard deltas or override terms converge to `human-intervention-required` with an approval workflow and no revenue mutation

`refill-prepaid-ai-credits` is the first payment-gated policy truth in the revenue path:

- confirmed payment leads to a governed `CreditGrant` ledger entry and entitlement increase
- unconfirmed payment or elevated risk converges to an approval workflow with no credit grant applied

The live billing ingress maps runtime-side normalized events into those revenue truths:

- `prepaid_top_up_settled` -> `refill-prepaid-ai-credits`
- `subscription_activation_requested` -> `activate-subscription`
- `subscription_payment_failed` -> `suspend-service-on-payment-failure`
- `ledger_reconciliation_requested` -> `reconcile-model-usage-against-customer-ledger`

That keeps Stripe/provider concerns in runtime adapters while CRM remains the truth-executing application boundary.

`suspend-service-on-payment-failure` is now live end-to-end and is the first CRM truth that reuses a converge-domain agent directly:

- `OverdueDetectorAgent` from the Money pack detects overdue invoice state inside the truth pipeline
- CRM-local policy then converges to one of three governed outcomes: suspend, defer inside grace, or block for strategic-account approval
- suspended subscriptions now prevent downstream credit grants through the same revenue-domain kernel surface

`reconcile-model-usage-against-customer-ledger` is now live end-to-end and proves the auditor pattern:

- source adapters load runtime usage, provider billing, CRM ledger, and CRM entitlement state into a comparable truth context
- `ReconciliationMatcherAgent` from the Money pack performs the first provider-to-ledger matching pass
- CRM-local assessment then converges to one of three governed outcomes: clean reconciliation, routed exception, or approval-gated manual review
- projection writes only facts and workflow cases, never balance adjustments or entitlement mutations

Future revenue truths should reuse the same revenue-domain kernel surface rather than introducing a parallel balance model.

## Analytical Storage Direction

Analytical and retrieval-heavy paths should converge on Parquet as the interchange format, separate from the transactional record path:

- website usage ingestion from `www.converge.zone` should land as Parquet batches for Polars-backed analytics truths
- audit and timeline history should be exportable to Parquet for long-horizon analytical queries
- LanceDB integration should treat Parquet and Arrow as the zero-copy interchange boundary for semantic retrieval

SurrealDB remains the transactional projection store. Parquet is the analytical batch and interchange format. Do not collapse those two concerns into one store abstraction.

## Kernel as Projection Store

The CRM kernel is not the orchestration layer. It is a durable projection store that the server writes to after Converge execution completes. The pattern:

```
truth key + inputs
  -> TruthConvergeBinding -> TypesRootIntent
  -> engine.run_with_types_intent_and_hooks(context, intent, hooks)
  -> ConvergeResult with criteria_outcomes
  -> project_<truth>(store, inputs, result, actor)
  -> kernel.write_with_events(|kernel| { ... })
  -> TruthProjection with CRM entities
```

Converge owns the convergence loop. The kernel owns the business-state shape. The projection function is the seam between them.
