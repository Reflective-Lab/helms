---
name: focus
description: Session opener — reads MILESTONES.md, shows current milestone and days remaining, scopes the session. TRIGGER at the start of every conversation.
user-invocable: true
allowed-tools: Read, Grep, Bash
---

# Session Focus

Read `MILESTONES.md` at the repo root. Identify the current milestone (marked with "Current:").

## Output

```
── Session Focus ──────────────────────────────────

Milestone:   <name>
Deadline:    <date> (<N> days remaining)
Progress:    <done>/<total> deliverables

Remaining:
- <unchecked deliverable 1>
- <unchecked deliverable 2>
- ...

────────────────────────────────────────────────────
```

## Rules

- Only list unchecked `[ ]` deliverables from the current milestone.
- If deadline is within 7 days, add: "⚠ <N> days left — keep scope tight."
- If deadline has passed, add: "⛔ Deadline was <date> — flag what's blocking."
- Do not suggest work. Just show the state. The user decides what to work on.
- Keep it short — this should take 5 seconds to read.
