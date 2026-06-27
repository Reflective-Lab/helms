# Helm Quality Backlog

**Owner:** `[HELMS-ARCH]`
**Filed:** 2026-06-15
**Source review:** `/Users/kpernyer/dev/reflective/REVIEW_quorum-sense_2026-06-15.md`
**Workspace registry:** `/Users/kpernyer/dev/reflective/BOUNDARY_REGISTRY.md`
**For implementors:** start with the active handoff,
`/Users/kpernyer/dev/reflective/HANDOFF_quorum-sense_2026-06-15.md`.
This backlog is the Helm commitment ledger, not the app work queue.

This backlog tracks Helm-owned follow-up from the quorum-sense multi-architect
review. It is the Helm implementation ledger for the H-IDs signed in Round 3.

## State

| ID | State | Note |
|---|---|---|
| `H-2026-06-15-01` | Done 2026-06-15 (Helm side) | `HelmModuleReadiness::module_state()` and status APIs landed; `helm.operator-control` has an app-owned live readiness feed contract; mounted modules bridge that state into Runway's `HelmModule::module_state()`; apps still need to implement their feeds. |
| `H-2026-06-15-02` | In progress 2026-06-15 | Commercial authority inventory filed; H-02b registry authority proposal filed in `kb/Architecture/Registry Authority Field Proposal.md`; source-of-truth demotion still needs implementation. |
| `H-2026-06-15-03` | Done 2026-06-18 | `helm-operator-control` re-exports operator-control packet and ledger helpers; operator-control previews are feed-only and no longer synthesize a static app portfolio; grep found no direct `prio-agent-ops` imports in marquee apps, Runtime-Runway, or Commerce-Rails. |
| `H-2026-06-15-04` | Done 2026-06-15 | Wave 3 cross-references landed; this item moved through in-progress during this cleanup pass and is now closed. |
| `H-2026-06-15-05` | Done 2026-06-15 (Helm side) | Helm exposes and tests `shell-default`/`live` through `HelmModuleReadiness` and maps it into Runway's `HelmModule::module_state()` for RR D1 check 2. |

## Severity

- **A:** Blocks a trustworthy production or scaling claim.
- **B:** Blocks app #2 from copying a clean platform contract, but does not
  immediately block quorum-sense security stopgaps.

## Ledger

| ID | Severity | Owner | Scope | Acceptance criteria | Dependencies | Ship signal |
|---|---|---|---|---|---|---|
| `H-2026-06-15-01` | A | Helms + apps | Live Helm readiness contract for `helm.operator-control` and `helm.governed-jobs`; default shells must not be presented as live. | `helm-operator-control` and `helm-governed-jobs` implement `HelmModuleReadiness::module_state()` / `readiness_status()` and bridge that result into Runway's `HelmModule::module_state()`; `helm.operator-control` accepts an app-owned `OperatorControlReadinessFeed` with process receipt, integrity proof, adapter receipt, Axiom report, and at least one packet/ledger snapshot; absent or empty feeds do not fall back to a static app portfolio; RR strict verifier can reject a shell advertised as live. | RR `D1`; app live readiness feed; `H-2026-06-15-05`. | App operator-control can be marked live only when the verifier sees live module state and live app evidence. |
| `H-2026-06-15-02` | A | Helms | Remove commercial authority from Helm. | Inventory classifies `prio-subscriptions`, `prio-entitlements`, `prio-payments`, `prio-metering`, active commercial paths in `application-kernel`, `workbench-backend`, `truth-catalog`, `contracts/module-registry.yaml`, and `kb/Architecture/Module Map.md` as move-to-CR, CR-backed projection, or historical/demo-only; no Helm path remains a source of truth for subscriptions, entitlements, payments, metering, provider refs, or plan-to-app grants. | CR contract availability; `HELMS-ADR-001`. | New marquee apps cannot import or call Helm commercial authority; Helm commercial views, if any, are read-only CR projections. |
| `H-2026-06-15-03` | B | Helms | Stabilize Helm-named operator-control imports and feed-only previews. | Legitimate `prio-agent-ops` readiness/ledger helpers are re-exported through `helm-operator-control`; marquee apps stop importing `prio-agent-ops` directly; Helm no longer contains a hard-coded app portfolio, and previews come from app-owned feeds. | `H-2026-06-15-01`. | App code depends on Helm-named operator-control contracts, not legacy `prio-*` names. |
| `H-2026-06-15-04` | B | Helms | Stale Helm milestone and architecture cleanup. | `MILESTONES.md`, `kb/Planning/Milestones.md`, `kb/Architecture/Foundation Contracts.md`, `kb/Architecture/Module Map.md`, and related KB pages point to the current boundary and do not describe commercial/revenue modules as Helm authority. | `H-2026-06-15-02`. | A new session reading Helm docs reaches the same boundary as the signed review without reading the whole review. |
| `H-2026-06-15-05` | B | Helms + RR | Mount vocabulary and live-readiness transport. | RR D1 check 2 reads Runway's `HelmModule::module_state()` on mounted modules; Helm derives that value from `HelmModuleReadiness` and distinguishes `shell-default` from `live`; the live-readiness transport is documented as provider/feed/packet, not direct domain-state access; apps keep `mount_kind: "planned"` until RR sees `live`. | RR `D1`, RR `D2`; `H-2026-06-15-01`. | `runway.app.json` can express planned/shell/live Helm mounts without ambiguity. |

## Standards To Promote

- `HP-NO-COMMERCIAL-STATE`: Helm must not be source of truth for subscriptions,
  plans, entitlements, payments, metering, revenue ledger, provider refs, or
  plan-to-app grants.
- `HP-READINESS-IS-NON-AUTHORITY`: Helm readiness packets and operator ledger
  entries are advisory trust-transfer artifacts; they do not authorize domain,
  commerce, deployment, claim refresh, or writeback actions.
- `HP-NO-STATIC-SHELL-AS-LIVE`: A mounted Helm module is not live unless it is
  backed by live app evidence and reports live module state.
