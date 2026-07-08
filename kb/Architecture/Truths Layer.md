# Truths Layer

## Role

Truths are the declarative JTBD layer above the capability modules.

- modules own reusable business capabilities
- truths own jobs, guardrails, and desired outcomes
- Converge owns orchestration
- facts own durable trust
- the truth catalog now also owns the bridge into Converge intent packets and pack selection

This is the intended stack:

1. storage layer
2. business capability modules
3. truths / JTBD layer
4. Converge runtime

## Truth Classes

### Job Truths

Cross-module business narratives such as:

- qualify inbound lead
- score inbound fit
- plan outbound campaign
- match renewal context
- create customer workspace
- activate subscription
- refill prepaid AI credits
- upgrade subscription plan
- suspend service on payment failure
- resolve support incident
- reconcile model usage against customer ledger
- detect abnormal token burn
- renew contract

### Policy Truths

Cross-module constraints such as:

- top-up requires confirmed payment
- overdue balance blocks entitlement increase
- promoted fact requires traceable evidence

### Module-Local Truths

Invariants that stay close to one capability boundary:

- ledger entry is immutable
- active subscription requires plan

## Mechanism / Content Split (RFL-172, Seam B)

The truth layer is split into a mechanism crate and a content crate:

- `crates/truth-catalog` — **mechanism**: `TruthDefinition`, `TruthCatalog`,
  `TruthKey` (kebab-case newtype, parse-don't-validate), `TruthConvergeBinding`,
  the `PackResolver` + `IntentOverlay` injection traits, intent compilation,
  admission, and orchestration (`prepare_candidates`). Carries zero
  `capability_registry` / `capability_core` imports — a trybuild compile-fail
  guard (`tests/compile_fail/capability_registry_not_a_dep.rs`) pins that edge
  as deleted.
- `crates/crm-truths` — **content**: the `TRUTHS` const, the `.feature` files,
  the CRM evaluators, `CrmPackResolver`, `CrmIntentOverlay`, and the assembled
  `CRM_CATALOG`.

Injection flow: mounting binaries construct `TruthCatalog::new(crm_truths::TRUTHS)`
(the same value as `CRM_CATALOG`) and inject catalog + overlay into
`helm-governed-jobs::JobStreamState` (T5). The desktop path is
`apps/desktop/src-tauri` (embedded-backend) → `workbench-backend` →
`crm_truths::find_truth(key)` → `CRM_CATALOG.find(&key)`; this chain is pinned
by `crates/crm-truths/tests/catalog_mount.rs`. `apps/crm-helm/` is orphaned —
it has no cargo edge to `helm-governed-jobs`; its truth files are legacy.

## Current Catalog

The catalog lives in:

- `crates/crm-truths` (content) over `crates/truth-catalog` (mechanism)
- `truths/jobs`
- `truths/policies`
- `truths/modules`

Each truth now also exposes a Converge binding:

- `request`: the job packet handed to Converge
- `pack_ids`: the domain packs activated for the job
- `required_success_criteria`: the desired outcomes lifted into intent criteria
- `hard_constraints`: the guardrails lifted into intent constraints
- `approval_points`: human gates that still need explicit runtime treatment

This keeps the split clean:

- truths specify the job contract
- modules specify reusable business capabilities
- Converge executes the contract against the selected packs

Today, nine truths are executable end-to-end through Converge: `qualify-inbound-lead`, `activate-subscription`, `upgrade-subscription-plan`, `suspend-service-on-payment-failure`, `reconcile-model-usage-against-customer-ledger`, `refill-prepaid-ai-credits`, `score-inbound-fit`, `plan-outbound-campaign`, and `match-renewal-context`.

## Design Rule

A capability belongs in a module if many different truths should be able to reuse it.

A behavior belongs in a truth if it mainly sequences multiple modules to achieve a job, enforce a policy, or state a business invariant in a declarative form.
