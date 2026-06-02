# Helm — Capabilities

What Helm provides as an operator-facing application and what it demonstrates as a platform consumer.

## Notes Application

Obsidian-compatible local-first vault with intelligent ingestion and enrichment.

| Capability | Status | What it does |
|---|---|---|
| Vault CRUD | **Live** | Create, read, save, move notes in `~/Notes` with frontmatter freshness tracking |
| Tree browser | **Live** | Recursive vault listing with metadata, file types, depth |
| Apple Notes import | **Live** | macOS Notes.app → Markdown via AppleScript. Batch export, reuse detection, inline images |
| Google Notes/Keep import | Planned | Import from Google Takeout or API |
| Web snapshot capture | **Live** | URL → raw HTML/metadata + Markdown note in vault with provenance |
| Social media capture | **Live** | LinkedIn, X, Instagram, Facebook profile extraction via web capture |
| Markdown tree import | **Live** | Bulk import any directory of Markdown files |
| Cleanup analysis | **Live** | Exact duplicate detection, Jaccard similarity scoring, merge suggestions |
| Freshness & value analysis | **Live** | Score notes for freshness/current value and suggest promote, refresh, or demote actions |
| OCR integration | Planned | Extract text from images and scanned documents in notes |
| PDF extraction | Planned | Parse PDFs into structured Markdown notes |
| Object detection | Planned | "Tell me what's in this picture" — scene understanding via vision models |
| Enrichment pipeline | Partial | Freshness/value analysis live. Entity extraction, richer backlinks, embeddings, semantic search still planned |

**Powered by:** `organism-notes` (vault, sources, cleanup) + `organism-intelligence` (web, social, OCR, vision)

## Expense & Receipt Management

Receipt-to-expense pipeline with OCR extraction and governed approval.

| Capability | Status | What it does |
|---|---|---|
| Receipt OCR | Planned | Photo → structured data (vendor, amount, date, items) via Tesseract/cloud OCR |
| Receipt comparison | **Live** | Side-by-side OCR backend comparison (Tesseract vs Ollama) |
| Expense reports | **Live** | Group expense items into reports with approval workflow |
| HITL approval | Planned | Converge-governed approval gates for expense authorization |

**Powered by:** `organism-intelligence` (OCR cloud/local/receipt backends, vision)

## Truth Execution Engine

Declarative jobs that compile to Converge intents. The catalog currently holds
23 truth definitions; four have executable workbench bodies today.

| Truth | What it does |
|---|---|
| `qualify-inbound-lead` | Capture demand, verify fit, assign next step |
| `activate-subscription` | Order → entitlements lifecycle |
| `refill-prepaid-ai-credits` | Top-up with payment verification |
| `submit-expense-report` | Expense report capture, document workflow, and approval path |

## 21 Capability Modules

Reusable CRM/ERP building blocks organized into 8 suites:

| Suite | Modules |
|---|---|
| Foundation | identity |
| Relationship | parties |
| Commercial | catalog, opportunities, subscriptions |
| Revenue | metering, ledger, entitlements, payments |
| Work | conversations, tasks, documents, workflow |
| Trust | approvals, policies, facts, audit |
| Intelligence | intents, memory, agent-ops |
| Expenses | receipt extraction, OCR bridge |

## Desktop Application

Tauri 2 + SvelteKit 5 desktop app with 6 routes:

- **Dashboard** — operator cockpit with jobs, approvals, exceptions
- **Notes** — full vault interface with import, capture, cleanup
- **Accounts** — organization detail views
- **Expenses** — receipt OCR and expense management
- **Workflow** — case management
- **Revenue** — subscription tracking

## Live Convergence Visibility (Stage 1)

SSE-backed real-time view of truth execution:

- Fact proposals and promotions streaming live
- Pipeline progress across chained truth sequences
- Blocked-step rendering (which agent is waiting, why)
- HITL approval UI with evidence presentation

## What Helm depends on

- **Converge** — engine, policy (Cedar), experience store, optimization, analytics, LLM providers, object storage
- **Organism** — planning loop, notes vault, intelligence (OCR, vision, web, social), domain packs

## What Helm does NOT own

- The convergence engine or axioms (Converge)
- The planning loop or adversarial review (Organism)
- OCR/vision/web providers (Organism intelligence)
- Vault management primitives (Organism notes)
- Generic organizational packs (Organism domain)
