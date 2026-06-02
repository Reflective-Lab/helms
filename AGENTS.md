# Helm — Agent Entrypoint

This is the canonical agent entrypoint for this repository. Long-form documentation lives in `kb/`.

## What This Repo Is

Helm is the operator-facing product surface for this repository. It is the **application layer and desktop workbench** built on top of:

- `../converge` as the runtime, governance, and truth-execution foundation
- `../organism` as the reusable intelligence foundation

This repo owns:

- application state and projections
- capability composition
- truth catalog content and truth bindings
- workbench surfaces and operator flows

This repo does **not** own:

- a second runtime
- a second OCR platform
- a second intelligence framework

## The Knowledgebase

`kb/` is an Obsidian vault and is the main documentation surface for this project.

Do not read the entire vault on startup. Lazy-load:

1. Read `kb/Home.md` only when you need to find something.
2. Follow one link to the specific page you need.
3. Read that page.
4. Keep going only if that page points to something required.

## Start Here

- `kb/Home.md`
- `kb/Ecosystem.md`
- `kb/Architecture/Application Layer Restructure.md`
- `kb/Architecture/Foundation Contracts.md`
- `kb/Architecture/Naming Migration Map.md`
- `kb/Planning/Milestones.md`

## Core Rule

Keep these three concepts separate:

- capability module
- truth
- desktop app

Also:

- Converge and Organism are internal foundations, not external ports.
- External ports are outside systems like email, accounting, banking, LinkedIn, Salesforce.
- Providers are interchangeable implementations inside a capability.

## Build

```bash
just test
just server
just desktop-check
just desktop-build-web
```

## Working Rules

- Use `just` commands when they exist.
- Read `kb/Planning/Milestones.md` at session start before doing feature work.
- Update `kb/` when architecture, process, or product framing changes.
- Git uses two durable branches only: `main` and `next`.
- Do normal implementation work on `next`; advance `main` only from validated `next`.
- Do not create topic branches or worktrees unless the human explicitly asks.
- Keep new naming aligned to `Helm` and the staged map in `kb/Architecture/Naming Migration Map.md`.
- Treat legacy `crm-*` and `prio-*` names as temporary implementation names, not architectural guidance.
- Before implementing any core, basic, or foundational capability here, check `../converge/CAPABILITIES.md` and `../organism/CAPABILITIES.md` first.
- When choosing lower-layer dependencies, prefer the curated surfaces in `../converge/kb/Architecture/Golden Path Matrix.md` and `../organism/kb/Architecture/API Surfaces.md`.
- If the capability already exists upstream, reuse it instead of rebuilding it locally.
- If it is generic but missing upstream, treat that as an upstream capability gap rather than immediately adding it to this repo.
- When touching truth resolution, governed workflows, or planning-loop behavior, inspect the current Organism examples first:
  `../organism/examples/resolution-showcase`, `../organism/examples/expense-approval`, and `../organism/examples/vendor-selection`.

## Milestones

Read `MILESTONES.md` at the start of every session. Scope all work to the current milestone. See `~/dev/reflective/bedrock-platform/EPIC.md` for the strategic context (this project = E5).
