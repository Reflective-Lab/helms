---
name: status
description: Check the health of Prio CRM services and infrastructure
disable-model-invocation: true
allowed-tools: Bash
---

# Service Status Check

Check all Prio CRM services and report status.

## Steps

1. **Local server** — can it start?
```bash
just test 2>&1 | tail -5
```

2. **Desktop app** — does it build?
```bash
just desktop-check 2>&1 | tail -5
```

3. **Rust workspace health**
```bash
cargo check --workspace 2>&1 | tail -5
```

<!-- TODO: Add production checks when deployment infra exists -->
<!-- Cloud Run, health endpoints, logs, etc. -->

Summarize findings in a clear status table: service, status (healthy/degraded/down), details.
