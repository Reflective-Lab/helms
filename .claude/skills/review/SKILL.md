---
name: review
description: Review a PR, then approve+merge or fix issues — e.g. /review 37 OK, /review 37 fix the typo in line 42
disable-model-invocation: true
argument-hint: [pr-number] [OK | feedback...]
allowed-tools: Bash, Read, Edit, Write, Grep, Glob
---

# Review PR

Parse $ARGUMENTS into a PR number and an optional verdict.

- `/review 37` — review only, show findings
- `/review 37 OK` — approve and merge
- `/review 37 <feedback>` — fix the issues, push, then show updated diff

## Review (always runs first)

1. **Read the PR**
```bash
gh pr view <number>
gh pr diff <number>
```

2. **Review for:** security, correctness, style, operations. Summarize as blockers / suggestions / questions.

## If verdict is "OK"

1. Approve and squash-merge:
```bash
gh pr merge <number> --squash --delete-branch
```

2. Switch back to main and pull:
```bash
git checkout main
git pull --rebase origin main
```

3. Output:
```
── Merged ─────────────────────────────────────────
PR #<number> merged to main. Branch deleted.
────────────────────────────────────────────────────
```

## If verdict is feedback (anything other than "OK")

1. Check out the PR branch:
```bash
gh pr checkout <number>
```

2. Implement the requested changes.

3. Commit and push:
```bash
git add <changed files>
git commit -m "Address review: <summary of changes>"
git push
```

4. Output the updated diff and:
```
── Updated ────────────────────────────────────────
PR #<number> updated with: <what changed>
Ready for another /review <number> OK or more feedback.
────────────────────────────────────────────────────
```

## Rules

- Do NOT leave GitHub PR comments or reviews — just act.
- On "OK", always squash-merge and clean up.
- On feedback, stay on the branch after pushing so the user can continue.
- If the PR has merge conflicts, flag it instead of force-merging.
