# Commercial Authority Inventory

**Owner:** `[HELMS-ARCH]`
**Status:** `H-2026-06-15-02` in progress - inventory filed; source-of-truth demotion still open.
**Filed:** 2026-06-15
**Source review:** `/Users/kpernyer/dev/reflective/REVIEW_quorum-sense_2026-06-15.md`
**Active handoff:** `/Users/kpernyer/dev/reflective/HANDOFF_quorum-sense_2026-06-15.md`
**Boundary:** [[Operating Authority Boundary]]
**ADR:** [[ADRs/HELMS-ADR-001 Commercial Authority Migration]]
**Schema proposal:** [[Registry Authority Field Proposal]]

> Implementor rule: no new marquee app may import a Helm commercial path during
> this migration. Treat every surface below as boundary debt until its
> post-delivery row says otherwise.

## Decision

Helm is not a source of truth for commercial authority.

Commercial authority means subscriptions, plans, plan-to-app grants,
entitlements, payments, refunds, settlements, provider refs, billing metering,
webhook receipts, and commercial ledger state. Those decisions belong to
Commerce-Rails. Helm may display operator projections, receipts, and HITL
review surfaces over those decisions, but it must not make or persist the
decision itself.

This inventory is the first H-02 implementation artifact. It deliberately does
not delete or rewrite behavior yet because the existing code is still used by
historical demos and the replacement CR contracts are not all available.

## Classification Key

- `move-to-cr`: source-of-truth command, state, truth, or contract must move to
  Commerce-Rails.
- `cr-backed-projection`: Helm may keep the operator view only if the backing
  data is read-only and comes from a Commerce-Rails contract.
- `historical-demo-only`: may remain temporarily for demos or fixtures, but it
  must be labeled non-authoritative and cannot be reused by new apps.
- `helm-owned-adjacent`: can remain Helm-owned only outside subscriptions,
  entitlements, payments, plan grants, provider refs, billing metering, and
  commercial ledger authority.
- `split-required`: one part moves to CR and another part may stay as a Helm
  projection or adjacent operator workflow.

## Inventory

| Surface | Evidence | Current authority risk | Classification | Next Helm action |
|---|---|---|---|---|
| `crates/prio-subscriptions` | Module manifest claims `order`, `subscription`, `subscription_item`, `billing_period`, `usage_plan`, `credit_balance`, and `/v1/subscriptions`. | Presents Helm as owner of subscription/order authority. | `move-to-cr`; temporary `historical-demo-only`. | Stop exposing as reusable app contract; convert any future Helm surface to a CR-backed projection. |
| `crates/prio-entitlements` | Module manifest claims entitlement, quota, limit, feature flag, and access policy objects. | Duplicates CR entitlement authority and risks stale grants. | `move-to-cr`; temporary `historical-demo-only`. | No new imports; replace with `is_entitled` / `entitlement_projection` from CR. |
| `crates/prio-payments` | Module manifest claims payments, payment methods, transactions, refunds, and settlements. | Duplicates payment-rail authority and provider receipt handling. | `move-to-cr`; temporary `historical-demo-only`. | Demote to historical scaffolding until CR projection contracts exist. |
| `crates/prio-metering` | Module manifest claims usage events, meters, consumption records, token classes, pricing units, and anomalies. | Billing metering could become commercial source of truth. | `split-required`. | Billing metering moves to CR; Helm may keep non-billing operational usage projections only. |
| `crates/prio-catalog` | Module manifest claims products, plans, prices, bundles, offers, and pricing rules. | Plan/pricing authority conflicts with CR plan and checkout contracts. | `move-to-cr` for plan/pricing source; `cr-backed-projection` for operator display. | Treat catalog data as CR-owned for app #2; do not extend Helm plan authority. |
| `crates/prio-ledger` | Module manifest claims balances, credit grants, ledger entries, debits, credits, and adjustments. | Commercial ledger authority conflicts with CR billing and entitlement accounting. | `move-to-cr` for commercial ledger; `cr-backed-projection` for operator display. | Keep distinct from Helm operator ledger/readiness receipts. |
| `crates/prio-opportunities` | Module manifest claims leads, opportunities, scoring, renewal candidates, forecasts, and proposed terms. | Pipeline state can remain Helm-owned, but proposed terms can accidentally become pricing authority. | `helm-owned-adjacent` with commercial constraints. | Keep lead/pipeline workflow; resolve plans/prices/grants through CR contracts only. |
| `crates/application-kernel/src/capabilities.rs` | `RevenueCommands` exposes catalog, subscription, entitlement, and ledger commands and queries. | Public kernel trait makes Helm a commercial command surface. | `move-to-cr`. | Freeze new callers; replace write commands with CR client calls or remove them after CR coverage lands. |
| `crates/application-kernel/src/kernel.rs` | Kernel stores `orders`, `entitlements`, `ledger_entries`, and `catalog_items`; mutates subscription activation, plan changes, suspension, and credit grants. | Active source-of-truth commercial state inside Helm. | `move-to-cr`. | In-place demotion: label as legacy/demo, then migrate or delete once CR contracts replace the writes. |
| `crates/workbench-backend/src/lib.rs` | Truth executors call kernel commercial writes, including activation, plan changes, suspension, and prepaid credit grants. | Operator workflow can directly mutate commercial authority. | `split-required`. | HITL review can stay in Helm; post-approval commercial mutation must go to CR. |
| `crates/workbench-backend/src/views.rs` | Workbench exposes subscription, catalog, entitlement, and ledger view models. | Views can be legitimate only if they are not backed by Helm-owned commercial state. | `cr-backed-projection`. | Keep the operator UI shape, replace backing source with CR projections. |
| `crates/truth-catalog/src/converge.rs` | Commercial and revenue packs evaluate subscription, payment, credit, entitlement, and reconciliation facts. | Truth ownership can imply Helm commercial decision authority. | `split-required`. | Move CR-owned commercial truth authority to CR; Helm keeps operator/trust framing only. |
| `truths/jobs/*.feature` and `truths/policies/*.feature` commercial files | Subscription activation, plan upgrade, refill credits, payment failure, overdue balance, and top-up truths. | Declarative truths may be reused as Helm authority by app teams. | `split-required`. | Mark commercial truth bodies as CR-owned or demo-only during truth-catalog migration. |
| `contracts/module-registry.yaml` | Registry lists commercial/revenue modules and API roots under Helm. | Makes transitional modules look canonical to implementors. | `historical-demo-only` until schema supports authority owners. | Add boundary note now; future registry schema should carry authority classification. |
| `crates/capability-registry/src/lib.rs` | Aggregates commercial modules into the runtime-visible module list. | New consumers can discover and reuse boundary-debt modules. | `historical-demo-only` until demotion. | Add boundary note; do not expose these entries as app contracts. |
| `kb/Architecture/Module Map.md` | Commercial and usage/revenue modules are documented as first-wave crates. | Documentation can override the signed boundary if read alone. | `historical-demo-only` with active warning. | Keep warning linked to this inventory and the boundary registry. |

## Demotion Plan

1. Label every Helm commercial surface as boundary debt in docs and registry
   hints. This inventory completes that first slice.
2. Block new marquee-app imports of Helm commercial crates in review.
3. When CR ships the public contracts, replace Helm write paths with CR calls:
   checkout/session creation, entitlement projection, payment receipt handling,
   plan-to-app mapping, and commercial ledger updates.
4. Convert workbench commercial screens into read-only CR-backed projections and
   HITL review surfaces.
5. Move or retire commercial truth bodies. Helm keeps trust-transfer/operator
   truths; CR owns commercial decision truths.
6. Remove historical/demo-only commercial authority after demos no longer need
   it or after a dated panel review grants an exception.

## PR-Rejectable During Migration

- Do not merge a new marquee-app import of `prio-subscriptions`,
  `prio-entitlements`, `prio-payments`, `prio-metering`, `prio-catalog`, or
  `prio-ledger`.
- Do not merge new Helm code that writes subscription, entitlement, payment,
  billing-metering, provider-ref, plan-to-app, or commercial ledger state.
- Do not merge new app contracts that treat Helm module registry entries as
  commercial authority.
- Do not merge a readiness packet or operator ledger entry that authorizes
  commerce.

## Open Implementation Slices

- `H-02a`: add source-level boundary notes to commercial crates and registry
  exports without changing behavior.
- `H-02b`: propose a machine-readable registry authority field after CR and RR
  confirm the schema impact. Draft proposal: [[Registry Authority Field Proposal]].
- `H-02c`: replace workbench commercial mutations with CR-backed command calls
  once CR contracts exist.
- `H-02d`: split or relocate commercial truth bodies.
- `H-02e`: remove historical/demo-only commercial authority after replacement
  coverage is proven.
