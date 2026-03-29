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

## Current Catalog

The starter catalog lives in:

- `crates/prio-truths`
- `truths/jobs`
- `truths/policies`
- `truths/modules`

It is exposed through the `prio.truths.v1.TruthCatalogService` gRPC package.

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

## Design Rule

A capability belongs in a module if many different truths should be able to reuse it.

A behavior belongs in a truth if it mainly sequences multiple modules to achieve a job, enforce a policy, or state a business invariant in a declarative form.
