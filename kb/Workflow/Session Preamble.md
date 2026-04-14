# Session Preamble

> Paste this at the start of any LLM session working on Outcome Workbench.
> Update the "Current milestone" line as you progress.

---

## Paste this:

```
You are helping me build Outcome Workbench, a JTBD-driven entrepreneur workbench built as an application layer on top of Converge and Organism.

## Current milestone: Stage 1 — TODO (deadline: TODO)

### Rules
- Scope ALL work to Stage 1 deliverables listed in `kb/Planning/Milestones.md`.
- If I ask for something outside Stage 1, flag it: "This isn't in Stage 1 — park it or swap something out?"
- Side-fixes get 15 min max, then back to the milestone.
- End every session with `/done`: what moved, what's left, any date risk.
- Use `just <recipe>` when a Justfile exists. Never suggest raw cargo commands.
- Never push to main without confirmation. Never commit secrets.

### Architecture
- crates/ — 30 Rust crates in 7 module suites
- apps/desktop/ — Svelte/Tauri operator UI
- proto/ — gRPC definitions
- truths/ — Gherkin feature files (jobs, policies, invariants)
- Converge owns governance and truth execution.
- Organism owns reusable intelligence capabilities.
- This repo owns application state, truth composition, and the desktop workbench.
- Legacy `crm-*` and `prio-*` names still exist in code. Treat them as temporary implementation names, not target architecture names.

### Key conventions
- converge-core is the governance layer — never reimplement it
- The kernel is a projection store, not the orchestration layer
- Agents emit proposals, not direct facts
- Every mutation emits DomainEvent + AuditEntry + TimelineEntry
```
