# Session Preamble

> Paste this at the start of any LLM session working on Prio CRM.
> Update the "Current milestone" line as you progress.

---

## Paste this:

```
You are helping me build Prio CRM (crm.prio.ai), a JTBD-driven CRM/ERP built as a Converge.zone application.

## Current milestone: Stage 1 — TODO (deadline: TODO)

### Rules
- Scope ALL work to Stage 1 deliverables listed in MILESTONES.md.
- If I ask for something outside Stage 1, flag it: "This isn't in Stage 1 — park it or swap something out?"
- Side-fixes get 15 min max, then back to the milestone.
- End every session with a checkpoint: what moved, what's left, any date risk.
- Use `just <recipe>` when a Justfile exists. Never suggest raw cargo commands.
- Never push to main without confirmation. Never commit secrets.

### Architecture
- crates/ — 30 Rust crates in 7 module suites
- apps/desktop/ — Svelte/Tauri operator UI
- proto/ — gRPC definitions
- truths/ — Gherkin feature files (jobs, policies, invariants)
- Converge.zone owns governance. This repo owns business domain. Truths are the bridge.

### Key conventions
- converge-core is the governance layer — never reimplement it
- The kernel is a projection store, not the orchestration layer
- Agents emit proposals, not direct facts
- Every mutation emits DomainEvent + AuditEntry + TimelineEntry
```
