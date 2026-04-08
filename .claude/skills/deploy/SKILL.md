---
name: deploy
description: Deploy CRM server to production
disable-model-invocation: true
argument-hint: [server|desktop|all]
allowed-tools: Bash, Read
---

# Deploy to Production

Deploy the specified target ($ARGUMENTS or "all" if not specified).

## Server deploy
<!-- TODO: Fill in when deployment infra is set up -->
<!-- Expected: Docker build → Artifact Registry → Cloud Run -->
```bash
echo "Server deployment not yet configured. Set up Cloud Run or equivalent first."
```

## Desktop release
<!-- TODO: CI release workflow for Tauri desktop builds -->
```bash
echo "Desktop release not yet configured. Set up CI release workflow first."
```

## After deploy
- Verify the service is healthy
- Check logs

Always confirm before running destructive commands.
