# Helm Milestones

> Single source of truth for what ships and when.
> Every session starts by reading this file. Scope work to the current milestone.
> Detailed deliverables live in `kb/Planning/Milestones.md`.
>
> See `~/dev/reflective/stack/bedrock-platform/EPIC.md` for the coarse-grained outcomes these milestones advance.

---

## Current: Stage 1.5 — Notes & Intelligence Daily Driver
**Deadline:** TBD | **Epic:** E5
Notes app fully implemented: Google import, social capture (LinkedIn/X/Instagram/Facebook), OCR, PDF extraction, object detection. Expenses integrated with OCR pipeline.

## Next: Stage 1.75 — Surface Alignment
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

### Stage 1 — Desktop End-to-End Showcase (v0.1.1, 2026-04-25)
Governed multi-truth pipeline (score → qualify → schedule) with live convergence visibility and HITL approvals. SSE endpoint, desktop pipeline page, helm-notes capture, EXP-001/EXP-002 confirmed.
