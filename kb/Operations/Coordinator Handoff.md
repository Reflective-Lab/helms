# Coordinator Handoff

This document is the current implementor view of the project. It is a working assumption set, not a product contract.

## Project Thesis

Outcome Workbench is not a classic CRM product.

It is a headless business substrate for Converge:

- Converge is the runtime and governance layer
- CRM is the durable business-state and application boundary
- Truths are the JTBD contract between the two

The intended user experience is job-centric, not record-centric.

## System Boundary Assumptions

### Converge owns

- proposals, promotion, and fact governance
- convergence loop and pack-scoped agent orchestration
- authority, budgets, and blocked/HITL semantics
- reusable domain packs such as Money, Trust, and Delivery

### Outcome Workbench owns

- business entities and durable projections
- truth catalog content for this application
- module boundaries and public APIs
- truth executors and projection logic
- operator workflows, approvals, and summaries

### External systems own

- `converge-runtime`: provider/webhook-facing integrations
- Stripe or other PSPs: payment-provider truth
- `www.converge.zone`: first-party usage-event source
- Wolfgang: reference patterns only, not a runtime dependency

## Architectural Assumptions

The current architecture is four layers:

1. Storage and projection layer
2. Capability modules
3. Truths / JTBD layer
4. Converge runtime

Important assumption:

- modules own reusable business capabilities
- truths own business jobs
- Converge owns orchestration
- facts and approvals own trust

## Domain Assumptions

The minimum serious commercial substrate is already the center of gravity:

- organizations, people, relationships
- catalog and plan semantics
- subscriptions
- entitlements
- ledger
- workflow and approvals
- facts, audit, and timelines

The CRM kernel is assumed to be a projection store, not the orchestration layer.

## Current Implementation Assumptions

At the time of writing, the repo has nine executable truths:

- `qualify-inbound-lead`
- `activate-subscription`
- `upgrade-subscription-plan`
- `suspend-service-on-payment-failure`
- `reconcile-model-usage-against-customer-ledger`
- `refill-prepaid-ai-credits`
- `score-inbound-fit`
- `plan-outbound-campaign`
- `match-renewal-context`

These prove:

- analytics-backed truth execution
- optimization-backed truth execution
- knowledge-backed truth execution
- direct reuse of Converge domain-pack agents
- blocked/HITL outcomes as first-class results
- payment-gated commercial mutations
- auditor truths that project facts/workflows but do not mutate balances

## Integration Assumptions

Phase 1 billing integration is intentionally split:

- runtime billing normalizes provider events
- CRM receives normalized billing events
- CRM maps those events to truths
- truths project business state

Current CRM-side ingress:

- `POST /v1/integrations/billing/events`
- bearer auth via `CRM_BILLING_INGRESS_TOKEN`
- normalized event kinds map to revenue truths

Current expected mapping:

- `prepaid_top_up_settled` -> `refill-prepaid-ai-credits`
- `subscription_activation_requested` -> `activate-subscription`
- `subscription_payment_failed` -> `suspend-service-on-payment-failure`
- `ledger_reconciliation_requested` -> `reconcile-model-usage-against-customer-ledger`

Assumption:

- runtime adapters should call truths, not kernel commands

## Data Assumptions

Transactional and analytical storage are separate concerns.

- SurrealDB is the likely transactional record store
- LanceDB is the likely vector and retrieval store
- Parquet is the intended analytical interchange format

Assumption:

- do not collapse transactional state, analytics batches, and vector retrieval into one abstraction too early

## Product Assumptions

The next high-value business outcomes are likely:

1. live runtime billing integration
2. renew-contract
3. create-customer-workspace
4. detect-abnormal-token-burn
5. resolve-support-incident

Assumption:

- live billing integration is more important than adding many more truths right now

## Team Assumptions

### Coordinator / project lead

Should manage sequencing, dependency decisions, and cross-role clarity:

- runtime integration milestones
- QA scope and failure semantics
- UX scope for thin operator surfaces
- truth prioritization vs infrastructure hardening

### QA

Should focus on:

- honest stopping
- blocked vs completed semantics
- idempotency
- partial failure
- retries and duplicates
- projection consistency
- agent misbehavior and malformed outputs

### UX

Should assume:

- thin UI first
- conversational and operator-centered surfaces
- job dashboards, approvals, summaries, exceptions
- no attempt to build a traditional record-navigation CRM first

## Current Risks

These are the main engineering risks as I currently see them:

1. transactional projection semantics need hardening
2. billing idempotency must become durable, not only process-local
3. some status fields still need enum migration
4. the kernel is still integrated more tightly than the module map implies
5. `ContextStore` and durable multi-run Converge state are not finished
6. provider/runtime adapter implementations are still thinner in design than in production reality

## Working Principles I Assume The Coordinator Should Protect

- Do not reimplement Converge primitives in CRM
- Do not bypass truths for business mutations
- Do not let integrations write kernel state directly
- Do not build a heavy UI before the job and revenue flows are stable
- Do not optimize storage architecture before the truth and projection contracts settle
- Prefer explicit blocked/approval states over silent repair or guesswork

## What I Would Watch Closely Next

- whether runtime billing actually calls the truth ingress cleanly
- whether duplicate or replayed billing events can double-apply business state
- whether projection writes are truly atomic under failure
- whether QA finds gaps in blocked/HITL retry behavior
- whether UX starts drifting toward classic CRM instead of job-centric operations
