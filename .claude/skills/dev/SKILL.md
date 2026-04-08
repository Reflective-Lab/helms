---
name: dev
description: Start local development environment
disable-model-invocation: true
argument-hint: [server|desktop|all]
allowed-tools: Bash
---

# Start Development Environment

Start the specified service locally ($ARGUMENTS or "all").

## Server
```bash
just server
```
Runs the gRPC CRM server (`crm-server` crate).

## Desktop
```bash
just desktop-dev
```
Tauri desktop app with Svelte frontend.

## All
Start server in background, then desktop in foreground.

```bash
just server &
just desktop-dev
```

## Useful checks
```bash
just test          # cargo test --workspace
just fmt           # cargo fmt --all
just desktop-check # bun check on Svelte
```
