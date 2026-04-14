# Gemini CLI Entrypoint

Read and follow `AGENTS.md` first. It is the canonical entrypoint for this repository.

## Gemini-Specific Notes

- Lazy-load `kb/` from `kb/Home.md`; do not bulk-read the vault.
- Prefer targeted search over opening large files.
- Project knowledge belongs in `kb/`, not in tool memory.
- Current planning and scope live in `kb/Planning/Milestones.md`.
- Before implementing any core or foundational capability here, check `../converge/CAPABILITIES.md` and `../organism/CAPABILITIES.md`.
- Reuse upstream capabilities when available; keep this repo application-specific.
