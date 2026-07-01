# Helm Milestones

> Single source of truth for what ships and when.
> Every session starts by reading this file. Scope work to the current milestone.
> Detailed deliverables live in `kb/Planning/Milestones.md`.
>
> See `~/dev/reflective/bedrock-platform/EPIC.md` for the coarse-grained outcomes these milestones advance.

---

## Current: Boundary refactor closeout + Scale unlock
**Deadline:** TBD | **Epic:** E5
Tie off the trailing work from the just-shipped Runtime Runway boundary refactor: get Quorum/Atlas running in production, restore CI on the post-rename workspace, and close the recovery audit on what the application-server deletion may have collaterally removed.

**RR D5 is shipped (2026-07-01).** `StorageKit.leases` is available. Helms can apply
`SessionOwnershipLayer` to its mutating routes now — this unblocks multi-instance
deployment and makes gaps 2–5 below the critical path.

**Remaining scale gaps (all Helms work):**
- [ ] Apply `SessionOwnershipLayer` to Quorum/Atlas mutating routes (RR D5 now available)
- [ ] Durable coordination state — migrate `SessionRegistry`, `PresenceRegistry` to `DocumentStore`
- [ ] Persistent `DecisionLedger` — gate decisions must survive process restart
- [ ] Topology consistency model — formalize read-consistency semantics under concurrent admission writes
- [ ] Mobile SSE resilience — server-side delivery tracking + client ack (delivery + completion) — **in progress**

**Boundary refactor closeout:**
- [ ] Phase 10 — Cloud Run deploy for Quorum and Atlas (not Catalyst)
- [ ] CI repair — replace `actions/checkout` `path: ../X` with direct git clone for sibling-repo path-deps
- [ ] Recovery audit A — `http_api.rs` route-by-route consumer survey (~2,200 deleted lines)
- [ ] Recovery audit B — `IdentityGrpc` / `TruthCatalogGrpc` / `ModuleRegistryGrpc` consumer survey
- [ ] Recovery audit C — 5 subscription truth bodies: decide commerce-rails-truths vs plain operations
- [ ] Quorum boundary Wave 1 — execute HELMS `H-2026-06-15-01` through `H-2026-06-15-05` in `QUALITY_BACKLOG.md`

Detailed steps for recovery items A–C live in `runtime-runway/docs/superpowers/plans/2026-05-30-recover-lost-functionality.md`.

## Next: Stage 1.5 — Notes & Intelligence Daily Driver
**Deadline:** TBD | **Epic:** E5
Notes app on the runway after platform-polish closes. Core vault is live; source imports (Google, web, LinkedIn, IG, FB, X), intelligence enrichment (OCR, PDF, vision, entity extraction), desktop UX, and receipt-OCR for expenses are still open. Full deliverable list in `kb/Planning/Milestones.md`.

## Future: Stage 1.75 — Surface Alignment
**Deadline:** TBD | **Epic:** E5
Align code, API, and CLI to the Helm Surface Model. CLI command taxonomy from truth catalog, surface-neutral API namespaces, crm-* → helm-* renames.

## Future: Stage 2 — Live Revenue
**Deadline:** TBD | **Epic:** E5
Billing hardened, CLI ships, first paying customer.

## Future: Stage 3 — Platform Signal
**Deadline:** TBD | **Epic:** E5
Analytics-backed truths, second vertical.

## Future: Stage 4 — Creative Convergence
**Deadline:** TBD | **Epic:** E5
Convergence loops that generate, verify, and deploy executable code (Wasm via Axiom).

---

## Shipped

### Runtime Runway execution-container boundary (2026-05-30)
Generic server ownership decoupled from Helm. `application-server` work classified and rehomed: host concerns (auth, middleware, telemetry, storage, deploy) into `runtime-runway`; operator-control + governed-job routes extracted into `helm-operator-control` and `helm-governed-jobs` as mountable `HelmModule`s; domain truths into per-app packets. Catalyst, Quorum, and Atlas backends all migrated to `RunwayAppHost`. `application-server` deleted (Phase 9).

### Stage 1 — Desktop End-to-End Showcase (v0.1.1, 2026-04-25)
Governed multi-truth pipeline (score → qualify → schedule) with live convergence visibility and HITL approvals. SSE endpoint, desktop pipeline page, helm-notes capture, EXP-001/EXP-002 confirmed.
