# Registry Authority Field Proposal

**Owner:** `[HELMS-ARCH]`
**Status:** Proposal only - do not implement until CR and RR approve the schema.
**Filed:** 2026-06-15
**Backlog:** `H-2026-06-15-02b`
**Related:** [[Commercial Authority Inventory]], [[Operating Authority Boundary]]

## Purpose

`contracts/module-registry.yaml` currently lists capability modules but cannot
say whether a module is source authority, a projection, historical demo
scaffolding, or boundary debt. That makes transitional commercial modules look
canonical to app implementors.

This proposal adds machine-readable authority metadata later, after
Commerce-Rails and Runtime-Runway approve the field names and verifier impact.
No schema change lands in this slice.

## Proposed Fields

Each module entry may eventually carry:

```yaml
authority:
  owner: helms | commerce-rails | runtime-runway | app | external
  class: source-of-truth | projection | adapter | historical-demo | boundary-debt
  consumption: reusable-contract | read-only-projection | internal-only | forbidden-for-new-apps
  canonical_contract: path-or-contract-id
  migration:
    status: active | transitional | sunset
    backlog_id: H-2026-06-15-02
```

## Field Semantics

- `owner`: the system allowed to define source semantics for the module.
- `class`: what kind of authority the registry entry has.
- `consumption`: how new apps may consume the entry.
- `canonical_contract`: the API, crate, ADR, or contract document that should be
  read before use.
- `migration.status`: whether the registry entry is stable, transitional, or
  being removed.
- `migration.backlog_id`: the ledger item carrying the migration work.

## Initial Commercial Mapping

If approved, the current commercial entries should start as:

| Module | Proposed owner | Proposed class | Proposed consumption |
|---|---|---|---|
| `catalog` | `commerce-rails` | `boundary-debt` | `forbidden-for-new-apps` |
| `subscriptions` | `commerce-rails` | `boundary-debt` | `forbidden-for-new-apps` |
| `entitlements` | `commerce-rails` | `boundary-debt` | `forbidden-for-new-apps` |
| `payments` | `commerce-rails` | `boundary-debt` | `forbidden-for-new-apps` |
| `metering` | `commerce-rails` for billing metering; `helms` for non-billing operator projections | `boundary-debt` | `forbidden-for-new-apps` until split |
| `ledger` | `commerce-rails` for commercial ledger; `helms` for operator-control receipt ledger | `boundary-debt` | `forbidden-for-new-apps` until split |
| `opportunities` | `helms` | `source-of-truth` for pipeline state only | `reusable-contract` after commercial fields are constrained |
| `agent-ops` | `helms` | `source-of-truth` for operator-control read models | `reusable-contract` through `helm-operator-control` |

## Verifier Expectations

Runtime-Runway can eventually reject a marquee-app manifest that consumes a
registry entry with:

- `consumption: forbidden-for-new-apps`;
- `class: boundary-debt` without an explicit dated exception;
- `owner` that conflicts with the importing layer.

That verifier behavior belongs to RR. Helm's responsibility is to publish the
metadata and keep the authority claims honest.

## Non-Goals

- This does not move commercial source state out of Helm.
- This does not change `contracts/module-registry.yaml` today.
- This does not give Helm authority over Commerce-Rails contracts.
- This does not replace the signed handoff or boundary registry.

## Approval Needed

- `[CR-ARCH]`: confirm owner/class/consumption values for commercial modules.
- `[RR-ARCH]`: confirm field names and verifier behavior before schema changes.
- `[HELMS-ARCH]`: implement only after both approvals or a new dated panel
  review.
