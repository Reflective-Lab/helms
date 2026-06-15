# Operating Authority Boundary

**Owner:** `[HELMS-ARCH]`
**Status:** Canonical Helm boundary document linked from the active quorum-sense handoff.
**Source review:** `/Users/kpernyer/dev/reflective/REVIEW_quorum-sense_2026-06-15.md`
**Workspace registry:** `/Users/kpernyer/dev/reflective/BOUNDARY_REGISTRY.md`
**Active handoff:** `/Users/kpernyer/dev/reflective/HANDOFF_quorum-sense_2026-06-15.md`
**Helm ledger:** `QUALITY_BACKLOG.md`
**Commercial inventory:** [[Commercial Authority Inventory]]

> Implementor reading order: start with
> `/Users/kpernyer/dev/reflective/BOUNDARY_REGISTRY.md`, then the active
> handoff, then this boundary document when your change touches Helm authority.

Helm is the operator-facing product layer. It owns trust-transfer surfaces,
operator workbench flows, HITL presentation, truth catalog binding, governed-job
views, and readiness/audit shapes. It does not own runtime substrate or
commercial authority.

## Authority Decision

Helm owns what the operator can inspect, approve, reject, and audit.

Helm does not own who can act, where code runs, who paid, what was granted, or
the source-of-truth domain state of a marquee app.

| Concern | Owner | Helm position |
|---|---|---|
| Runtime, deploy, auth, app shell, storage, telemetry, session ownership | Runtime-Runway | Consume the app execution container. Do not rebuild it locally. |
| Subscriptions, plans, entitlements, payments, revenue share, provider refs, webhook receipts, plan-to-app grants | Commerce-Rails | Consume by contract as read-only projections or receipts. Do not hold source-of-truth commercial state. |
| Trust-transfer surfaces, HITL approvals, operator-control readiness, governed-job ledger shape, workbench projections | Helm | Own and stabilize these contracts. Keep them advisory unless a lower layer or app grants authority. |
| App domain semantics, product flows, subject refs, process receipts | Marquee app | Display and correlate; do not bypass the app state machine. |

## No Commercial State Rule

Helm must not be the source of truth for:

- subscriptions;
- plans and plan-to-app mappings;
- entitlements and quota grants;
- payments, refunds, settlements, provider refs, or webhook receipts;
- metering used for billing authority;
- commercial ledger authority or revenue reconciliation.

Allowed in Helm:

- read-only operator projections backed by Commerce-Rails contracts;
- receipt views that show what Commerce-Rails decided;
- HITL surfaces that ask an operator to review a commercial action before the
  commercial command is sent to Commerce-Rails;
- historical/demo fixtures clearly marked as non-authoritative.

Forbidden in Helm:

- mutating commercial source-of-truth state directly;
- exposing Helm commercial crates as reusable app contracts;
- inferring entitlement from Stripe/provider IDs, emails, invoices, webhook
  payloads, or custom claims;
- treating a Helm readiness packet as commerce authorization.

## Expanded Rehoming Scope

The signed review found that the problem is broader than the original
`prio-*` commercial crates. At minimum, `H-2026-06-15-02` covers:

- `crates/prio-subscriptions`;
- `crates/prio-entitlements`;
- `crates/prio-payments`;
- `crates/prio-metering`;
- active commercial mutations and models in `crates/application-kernel`;
- commercial command execution in `crates/workbench-backend`;
- subscription/payment/entitlement truth framing in `truth-catalog`;
- commercial entries in `contracts/module-registry.yaml`;
- commercial/revenue ownership language in `kb/Architecture/Module Map.md`.

Adjacent commercial surfaces such as catalog and ledger must be audited during
the same inventory. Some may remain as read-only operator projections, but none
may remain Helm authority without a new panel review.

The active inventory and demotion ledger is [[Commercial Authority Inventory]].

## Migration Mechanic

`HELMS-ADR-001` chooses **in-place demotion first**:

1. Freeze new Helm commercial writes and new marquee-app imports of Helm
   commercial crates.
2. Inventory every commercial path and classify it as move-to-CR,
   CR-backed Helm projection, or historical/demo-only.
3. Move source-of-truth command/state ownership to Commerce-Rails contracts.
4. Convert Helm workbench views to projections over those contracts.
5. Mark historical/demo fixtures explicitly and attach removal dates during the
   inventory.
6. Update registry, module map, truth catalog, and milestone docs as the code
   moves.

This is not a feature-flagged sunset. There must be no flag, env toggle, or
strict-mode switch that keeps source-of-truth commercial authority alive in
Helm for new apps.

## Operator-Control Boundary

Helm readiness packets, operator ledger entries, and governed-job views are
advisory trust-transfer artifacts.

They must not authorize:

- Quorum or other app domain action;
- commerce action or entitlement grant;
- deployment;
- claim refresh;
- app writeback.

For quorum-sense live readiness, the minimum live evidence is:

- process receipt;
- integrity proof;
- adapter receipt;
- Axiom report.

The feed contract for that evidence is `OperatorControlReadinessFeed` in
`helm-operator-control`. Quorum supplies already-derived readiness packets and
ledger entries; Helm reports `shell-default` or `live` through
`HelmModuleReadiness`. RR D1 check 2 should use `HelmModuleReadiness`, not
`runway_app_host::HelmModule`; mounting routes is not live-readiness evidence.
Until RR observes `live`, Quorum's manifest should keep `helm.operator-control`
at `mount_kind: "planned"`.

Raw inquiry state, transcript content, entitlement state, and deployment
authority stay outside Helm operator-control unless a later dated panel review
grants a projection.

## PR-Rejectable Rules

- New Helm code that mutates subscription, entitlement, payment, metering,
  provider-ref, or plan-to-app source-of-truth state is rejected.
- New marquee-app code importing `prio-subscriptions`, `prio-entitlements`,
  `prio-payments`, or `prio-metering` from Helm is rejected.
- A manifest or doc that marks a default `helm.operator-control` or
  `helm.governed-jobs` shell as live is rejected.
- A readiness packet or operator ledger entry with domain, commerce, deploy, or
  writeback authority is rejected.
- A static portfolio preview presented as live app state is rejected.

## Cross-Repo Links

- Runtime-Runway boundary: `/Users/kpernyer/dev/reflective/runtime-runway/kb/Architecture/App Execution Container.md`
- Commerce-Rails boundary: `/Users/kpernyer/dev/reflective/commerce-rails/kb/Architecture/Operating Authority Boundary.md`
- Workspace registry: `/Users/kpernyer/dev/reflective/BOUNDARY_REGISTRY.md`
- Review: `/Users/kpernyer/dev/reflective/REVIEW_quorum-sense_2026-06-15.md`
- Active handoff: `/Users/kpernyer/dev/reflective/HANDOFF_quorum-sense_2026-06-15.md`
- Local commercial inventory: [[Commercial Authority Inventory]]
