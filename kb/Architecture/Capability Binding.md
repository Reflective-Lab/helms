---
source: mixed
updated: 2026-04-19
---

# Capability Binding — How Truths Connect to External Systems

When a Truth says "gather customer data" or "send campaign", something must declare *where* that data comes from and *where* that action goes. This document describes three binding strategies and when to use each.

## The Problem

Organism packs declare **abstract capabilities** (web search, LLM, CRM, billing) but not concrete providers. The app must bridge the gap:

```
Truth: "evaluate-acquisition-target"
  → Pack: due_diligence (requires: web, llm)
  → But *which* web search? *Which* LLM?
```

## Option A: Static Capability Config (dev / demo)

Backends are injected at registration time by the app.

```rust
let search: Arc<dyn DdSearch> = Arc::new(TavilySearch::new(env!("TAVILY_API_KEY")));
let llm: Arc<dyn DdLlm> = Arc::new(AnthropicLlm::new(env!("ANTHROPIC_API_KEY")));

engine.register_suggestor_in_pack("dd", BreadthResearchSuggestor::new(company, budget, search));
```

**Pros:** Simple, explicit, no infrastructure needed.
**Cons:** Hardcoded per app. No discovery. No enterprise multi-tenancy.
**Use for:** Development, demos, single-tenant deployments.

### Readiness Probes (existing)

Organism already validates prerequisites before running:

- `PackProbe` — are required packs registered?
- `CredentialProbe` — are env vars present?
- `BudgetProbe` — is there token/spend budget?

## Option B: MCP Server Directory (production target)

Each enterprise maintains a registry of MCP servers that wrap their systems.

```yaml
# Enterprise capability directory
capabilities:
  crm:
    mcp_server: mcp://salesforce.acme.corp
    protocol: mcp-v1
    scopes: [contacts.read, leads.write]
  campaigns:
    mcp_server: mcp://hubspot.acme.corp
    protocol: mcp-v1
    scopes: [campaigns.create, contacts.read]
  search:
    mcp_server: mcp://tavily.acme.corp
    protocol: mcp-v1
  llm:
    mcp_server: mcp://anthropic.acme.corp
    protocol: mcp-v1
    model: claude-sonnet-4-6
```

A pack agent calls through the MCP protocol instead of direct HTTP:

```rust
// Instead of:
tavily_client.search(query).await

// Through MCP:
mcp.call("search", "web_search", json!({ "query": query })).await
```

**Pros:**
- Standardized contract (MCP protocol)
- Enterprise owns the server (data stays in their perimeter)
- Organism doesn't need to know Salesforce vs Dynamics vs Pipedrive
- New integrations = new MCP server, no Organism code change
- Auth, rate limiting, and schema translation are the MCP server's concern

**Cons:** Requires MCP infrastructure per enterprise.
**Use for:** Multi-tenant production, enterprise customers.

### How This Changes the DD Backend

```rust
// Option A (today):
struct TavilyDdSearch { api_key: String }
impl DdSearch for TavilyDdSearch { /* direct HTTP */ }

// Option B (target):
struct McpDdSearch { mcp: McpClient, capability: String }
impl DdSearch for McpDdSearch {
    async fn search(&self, query: &str) -> Result<Vec<SearchHit>, DdError> {
        let result = self.mcp.call(&self.capability, "search", json!({ "query": query })).await?;
        // MCP server wraps Tavily/Brave/whatever — we don't care which
        parse_search_hits(result)
    }
}
```

The `DdSearch` and `DdLlm` traits are the right abstraction — swapping direct HTTP for MCP is a one-line change per backend.

## Option C: Converge Capability Axioms (governance-enforced)

Capabilities become governed resources with explicit axioms:

```
capability "web_search" {
    requires credential "search_api_key"
    requires endpoint "search_base_url"
    max_calls_per_run 100
    data_residency "eu-west-1"
    audit_level "full"
}

capability "llm" {
    requires credential "llm_api_key"
    max_tokens_per_run 50_000
    max_spend_per_run_usd 25.0
    allowed_models ["claude-sonnet-4-6", "claude-haiku-4-5"]
}
```

The engine refuses to run a truth that needs "web_search" unless the axiom is satisfied. Budget enforcement, data residency, and audit requirements become part of the convergence contract.

**Pros:** Governance-enforced. Auditable. Budget-aware.
**Cons:** More infrastructure. Axiom authoring is an investment.
**Use for:** Regulated industries, high-stakes decisions, multi-model governance.

## Recommended Strategy

| Environment | Strategy | Why |
|-------------|----------|-----|
| Development | Option A (static) | Fast iteration, no infrastructure |
| Demo / POC | Option A (static) | Simple, self-contained |
| Single-tenant prod | Option A + C | Static backends with axiom governance |
| Multi-tenant prod | Option B + C | MCP directory with axiom governance |

Options B and C are complementary:
- **B** answers "where is the service?" (routing)
- **C** answers "is the service allowed?" (governance)

## Current State

- `DdSearch` and `DdLlm` traits exist in Organism (planning/dd.rs)
- `FailoverDdLlm` and `FailoverDdSearch` handle multi-provider failover
- Readiness probes check prerequisites before running
- Stub backends in Helm prove the wiring (EXP-001)
- Real backends exist in Monterro (Tavily, Brave, converge-provider)

## Code Generation as a Capability (Future — EXP-002)

A convergence loop may reach a point where it needs to *build something that didn't exist before* — a Wasm module, an API adapter, a data transformation. This is a special capability where the suggestor's output is executable code.

See `experiments/EXP-002.md` for the hypothesis that code generation can participate as a convergence step, producing Wasm artifacts that are verified before promotion.
