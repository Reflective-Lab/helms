# Helm — Claude Workspace Notes

Read `AGENTS.md` first. It is the canonical project entrypoint.

## Session Scope

- **Milestones:** Read `MILESTONES.md` at the start of every session. Scope work to the current milestone.
- **Changelog:** Update `CHANGELOG.md` when shipping notable changes.
- **Strategic context:** `~/dev/reflective/stack/bedrock-platform/EPIC.md`

## Claude-Specific Notes

- Long-form documentation lives in `kb/`.
- Scope current work using `kb/Planning/Milestones.md`.
- For repo navigation, start at `kb/Home.md` and lazy-load from there.
- **Available skills:** `/experiment` — hypothesis-driven development with evidence logging.
- Before adding any core or foundational function here, check `../converge/CAPABILITIES.md` and `../organism/CAPABILITIES.md`.
- Prefer reusing upstream capabilities over rebuilding them in Helm.

## Useful Commands

```bash
just test
just server
just desktop-check
just desktop-build-web
```

## Architecture Reminder

- `../converge` is the runtime and governance foundation.
- `../organism` is the reusable intelligence foundation.
- This repo is the application layer and workbench.
- Capability module != truth != desktop app.
