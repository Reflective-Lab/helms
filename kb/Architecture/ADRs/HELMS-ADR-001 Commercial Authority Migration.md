# HELMS-ADR-001: Commercial Authority Migration

**Status:** Accepted
**Date:** 2026-06-15
**Owner:** `[HELMS-ARCH]`
**Answers:** `[CR-ARCH]` OQ3 from the quorum-sense review.
**Related backlog:** `H-2026-06-15-02`

## Context

Helm still contains historical commercial scaffolding from before
Commerce-Rails existed:

- `prio-subscriptions`;
- `prio-entitlements`;
- `prio-payments`;
- `prio-metering`;
- commercial state and mutations in `application-kernel`;
- commercial command execution in `workbench-backend`;
- commercial truth framing in `truth-catalog`;
- commercial ownership language in `contracts/module-registry.yaml` and
  `kb/Architecture/Module Map.md`.

The 2026-06-15 multi-architect review settled the authority boundary:
Commerce-Rails owns commercial meaning and source-of-truth state. Helm may show
operator projections and receipts, but it must not own subscriptions,
entitlements, payments, metering, provider refs, or plan-to-app grants.

`[CR-ARCH]` asked HELMS to pick a migration mechanic: crate move, in-place
demotion, or dated sunset.

## Decision

Choose **in-place demotion first**.

Helm will not wholesale move crates into Commerce-Rails before the target CR
contracts exist. Helm will also not keep legacy commercial authority alive behind
a feature flag.

The migration rule is:

1. Freeze new source-of-truth commercial writes in Helm.
2. Freeze new marquee-app imports of Helm commercial crates.
3. Inventory each commercial path and classify it as:
   - move to Commerce-Rails;
   - convert to a CR-backed Helm projection;
   - disable as historical/demo-only.
4. Move or replace source-of-truth command/state paths after CR publishes the
   target contract.
5. Keep temporary compatibility only as deprecated projection or demo surfaces,
   never as alternate commercial authority.
6. Attach dated removal milestones during the inventory for every historical
   compatibility surface that remains.

## Rejected Options

### Crate move first

Rejected as the first step. A mechanical crate move would transfer historical
Helm assumptions into Commerce-Rails before CR has defined the correct contract.
Selective code movement may still happen after the inventory and CR contract
design.

### Feature-flagged sunset

Rejected. A feature flag, env toggle, or strict-mode switch would preserve a
second commercial authority path and violate the review's hard rules. If legacy
behavior must remain visible, it is a historical/demo fixture or a deprecated
projection, not a live authority.

### Blind deletion

Rejected. `application-kernel`, `workbench-backend`, and truth catalog paths may
still be referenced by demos and operator views. Deleting them before inventory
would create avoidable breakage and hide the migration plan.

## Consequences

- HELMS owns the inventory and demotion work under `H-2026-06-15-02`.
- CR owns the replacement commercial contracts and source-of-truth semantics.
- RR may reject marquee apps that import Helm commercial authority.
- Existing Helm demos may keep historical fixtures only when labeled
  non-authoritative and dated for removal.

## Acceptance

This ADR is complete when:

- `QUALITY_BACKLOG.md` carries `H-2026-06-15-02`;
- `kb/Architecture/Operating Authority Boundary.md` records the no-commercial
  state rule;
- implementation follow-up exists for inventory, demotion, and doc cleanup;
- future handoffs can cite this ADR as the answer to CR-OQ3.

## Referenced From

- `/Users/kpernyer/dev/reflective/HANDOFF_quorum-sense_2026-06-15.md`,
  Section 0, Rule 10: no new marquee app may import Helm commercial paths
  during the migration.
- `/Users/kpernyer/dev/reflective/BOUNDARY_REGISTRY.md`, the active workspace
  anchor for the Marquee App Contract and active handoff.
