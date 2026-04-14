# Working with Codex

- Start with `AGENTS.md`.
- Keep the same canonical workflow names used in the Claude docs: `/focus`, `/sync`, `/next`, `/check`, `/fix`, `/pr`, `/done`, `/audit`
- In Codex, name the workflow directly in plain text: `focus`, `run focus`, `check`, `done`, `audit`, `fix issue 42`, `review PR 5`
- `/done` may still be backed by `checkpoint` internally, and `/check` by `quality`
- Treat `kb/` as lazy-loaded documentation, not startup context.
- Prefer `just` commands where they exist.
- Keep edits aligned to [[../Planning/Milestones]] and [[../Architecture/Naming Migration Map]].
