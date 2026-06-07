# Helm

Helm is the operator-facing application layer and desktop workbench built on top of `../converge` and `../organism`.

The repo and some code names still reflect the earlier `Outcome Workbench` naming while the migration is in flight.

Long-form documentation now lives in the Obsidian knowledgebase under `kb/`.

## Current Code-Backed Shape

- Workspace version: `0.2.0`.
- Rust workspace members: 35 crates, including the desktop Tauri shell,
  workbench backend, application kernel/storage, Helm operator modules,
  capability crates, the truth catalog, notes, and seed tooling.
- Truth catalog: 23 registered definitions in `crates/truth-catalog`.
- Truth keys exposed through `workbench-backend` today: `qualify-inbound-lead`,
  `submit-expense-report`, `activate-subscription`, and
  `refill-prepaid-ai-credits`.
- Wire compatibility: proto packages still live under `proto/prio/*/v1` while
  naming migrates in stages.

Helm owns operator UX, app projections, plugin hosting, and product-specific
truth composition. Axiom owns truth compilation and run verification, Organism
owns formation selection, Converge owns promotion authority, Mosaic owns
specialist capabilities, Runtime Runway owns deployment/runtime plumbing, and
Commerce Rails owns commercial truth.

## Boundary

> Owns: trust-transfer surfaces, workbench views, operator-facing consequence, manifest intake, operator review, truth-catalog binding, sandbox lifecycle, approval points, audit visibility. Does NOT own: applet authority/schema (→ Axiom); domain mutation in product apps (→ marquee/studio repos); commercial state (→ Commerce Rails).

— Canonical claim: [Helms](https://github.com/Reflective-Lab/reflective/blob/main/KB/04-architecture/current-system-map.md#helms) in the boundary registry. Update there first; this README quotes that source.

The paragraph below ("Helm owns operator UX...") restates the same boundary in narrower prose; the blockquote above is the canonical version.

## A New World

Software was built around the assumption that humans operate workflows. Models and orchestration invert that: humans declare intent, the system converges on it, and the human surface becomes a place to author invariants, set bounds, approve irreversibles, and redirect when the world changes. Helm is that surface.

**Why it matters.** A new world without a clear operator surface degenerates into either reckless autonomy or a smarter dashboard. Helm is the trust-transfer layer — every interaction is a typed event, every authority recompute is visible, and the success metric inverts: the product is most valuable when you barely use it.

Start here:

- `AGENTS.md` for the canonical agent entrypoint
- `CLAUDE.md` for Claude-specific workspace notes
- `GEMINI.md` for Gemini-specific notes
- `kb/Home.md` for the vault index

## Operator Console Package

`packages/helm-console` is the shared receipt-backed console layer for
Marquee apps. It provides the `ConsoleAdapter` descriptor schema, command
cards, event timelines, connection bar, proof artifact panel, and first app
profiles for Quorum, Atlas, and Warden. The package owns reusable operator UX;
apps keep their domain nouns, command payloads, evidence rules, and custom
panels.
