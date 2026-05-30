# Helm

Helm is the operator-facing application layer and desktop workbench built on top of `../converge` and `../organism`.

The repo and some code names still reflect the earlier `Outcome Workbench` naming while the migration is in flight.

Long-form documentation now lives in the Obsidian knowledgebase under `kb/`.

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
