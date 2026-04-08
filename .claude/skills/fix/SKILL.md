---
name: fix
description: Fix a GitHub issue by number — reads issue, implements fix, creates PR
disable-model-invocation: true
argument-hint: [issue-number]
allowed-tools: Bash, Read, Edit, Write, Grep, Glob
---

# Fix GitHub Issue #$ARGUMENTS

## Steps

1. **Read the issue**
```bash
gh issue view $ARGUMENTS
```

2. **Create a branch**
```bash
git checkout -b fix/$ARGUMENTS
```

3. **Understand the codebase** — explore relevant files based on the issue description.

4. **Implement the fix** — make the minimum changes needed. Follow existing patterns. Don't refactor unrelated code.

5. **Verify** — check that the code compiles:
```bash
just check
```

6. **Commit** with a message referencing the issue:
```
Fix #$ARGUMENTS: <description>
```

7. **Create PR**
```bash
git push -u origin HEAD
gh pr create --title "Fix #$ARGUMENTS: <description>" --body "Closes #$ARGUMENTS"
```

8. **Return the PR URL.**

9. **Handoff to human.** After the PR is created, output a test plan the user can follow to verify the change manually. Format:

```
── Ready for review ───────────────────────────────

PR: <url>
Branch: fix/<number>

How to test:
1. <exact command to run the app, using just recipes>
2. <step-by-step what to do in the app>
3. <what to look for — expected behavior>

Prerequisites:
- <anything that must be in place first, e.g., "need a KB with 2+ experts imported">
- <env vars, services, test data>

What changed (files):
- <file>: <one-line summary>

────────────────────────────────────────────────────
```

Keep it concrete. Use just recipes, not raw commands. Call out blockers (missing test data, services not running, etc.) so the user doesn't waste time.
