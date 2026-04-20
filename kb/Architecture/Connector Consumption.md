---
tags: [architecture, integrations]
source: mixed
---
# Connector Consumption

How Helms (the application layer) consumes external system connectivity from
the layers below.

## Helms Does NOT Build Connectors

Connectors live below:

- **Generic tools** (Salesforce, Jira, Slack) → Converge ToolRegistry (MCP/OpenAPI)
- **Domain-semantic ports** (LinkedIn enrichment, social normalization) → Organism intelligence traits
- **AI backends** (LLMs, embeddings) → Converge providers

Helms composes these into application-facing seams.

## Application Ports

Application ports in Helms are thin orchestration layers that:

1. Select which external ports matter for the operator's environment
2. Compose Organism capabilities with application rules
3. Manage the raw → enriched → published data lifecycle
4. Schedule enrichment runs and publish to the workbench

```
port.social.linkedin = organism_intelligence::social + application rules
port.web.capture     = organism_intelligence::web + raw-run storage
port.crm.sync        = Converge Tool (MCP) + application projection
```

## The Rule

Per the [[Port Capability Taxonomy]]:

- Reusable capture/normalization → lives in Organism
- Application orchestration, scheduling, publishing → lives here
- Generic CRUD against external systems → lives in Converge as Tools

## When to Add a New Application Port

Add an application port here when:

1. The external system interaction requires **application-specific logic**
   (scheduling, storage rules, publishing decisions)
2. Raw results need to pass through the **raw → enriched → published** lifecycle
3. The workbench needs to **expose the flow** to the operator

Do NOT add an application port when:

- The interaction is generic enough to be a Converge Tool
- The capability is reusable across multiple applications (put it in Organism)

## Strategic Context: API-Only Infrastructure

The layers below (Converge, Organism) are **API-only infrastructure**. Helms
consumes them as API clients, not as embedded libraries with coupled UIs.

This means:

- Application ports here are thin API orchestration, not reimplementations
- The same Converge/Organism APIs that Helms uses are available to any other
  consumer — there is no privileged access
- Switching from embedded to remote deployment changes nothing about the
  connector architecture
- External system connectivity is always via standard protocols (MCP, OpenAPI)
  at the infrastructure layer

## Connector Architecture (Full Stack)

```
┌─────────────────────────────────────────────────┐
│  Helms (Application)                            │
│  Application ports — orchestration, scheduling  │
│  port.social.linkedin, port.web.capture         │
└──────────────────────┬──────────────────────────┘
                       │ composes
┌──────────────────────┴──────────────────────────┐
│  Organism (Intelligence)                        │
│  Typed port traits — domain-semantic            │
│  LinkedInProvider, WebCaptureProvider, etc.      │
└──────────────────────┬──────────────────────────┘
                       │ uses
┌──────────────────────┴──────────────────────────┐
│  Converge (Infrastructure)                      │
│  ToolRegistry — MCP, OpenAPI, GraphQL           │
│  Provider backends — LLM, embed, search         │
└──────────────────────┬──────────────────────────┘
                       │ discovers
┌──────────────────────┴──────────────────────────┐
│  Ecosystem                                      │
│  Community MCP servers, OpenAPI specs, etc.      │
└─────────────────────────────────────────────────┘
```

See also: [[Port Capability Taxonomy]], [[Integration Plan]]
