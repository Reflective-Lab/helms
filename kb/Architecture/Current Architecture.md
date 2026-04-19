# Backend Architecture

## Positioning

This backend is a Converge application with a CRM-shaped business substrate.

- system of record: this repository
- system of interaction and constitutional runtime: Converge
- system of work: job-to-be-done flows
- system of trust: Converge governance plus application-level approvals and audit

## Layers

1. Storage layer
   SurrealDB, LanceDB, files, and event history.
2. Business capability modules
   Reusable CRM/ERP-style capabilities with owned objects, commands, queries, events, and local invariants.
3. Truths / JTBD layer
   Declarative job, policy, and module-local truth definitions that compose modules without replacing them, and now compile into Converge intent packets plus pack activation sets.
4. Converge runtime
   The orchestration layer that interprets truths, coordinates agents and humans, enforces promotion and authority, and owns convergence.

## Bounded Contexts

### Directory

- `Organization`
- `Person`
- `Relationship`

This is the durable graph of who the company sells to, buys from, and works with.

### Revenue

- `Lead`
- `Opportunity`
- `OfferQuote`
- `OrderSubscription`
- `CatalogItem`

This context turns conversations into pipeline truth without assuming a sales-led UI.

### Work

- `Activity`
- `Task`
- `Intent`
- `WorkflowCase`

This is the execution layer where humans and agents collaborate around jobs.

### Memory

- `Note`
- `Document`
- `Conversation`
- `CommunicationEvent`
- `Fact`

This is the operational memory layer used by Converge for recall, summarization, and fact promotion.

### Trust

- `PermissionGrant`
- `AuditEntry`

This is where approvals, scope, and replayable business history live.

### Metadata

- `ObjectDefinition`
- `FieldDefinition`
- `RelationshipDefinition`
- `ViewDefinition`

This is the part worth taking seriously from Twenty. Standard CRM objects and custom objects should be configured through the same metadata layer so the API can stay model-driven instead of hard-coded around a fixed schema forever.

### Runtime Bridge

- `Job`
- `WorkflowCase`
- `Intent`
- `ProposedFact`
- `Fact`
- `Approval`
- `AgentRun`
- `Policy`

This is the application-facing bridge into Converge. The goal is not to recreate Converge primitives locally. The goal is to give Converge durable business state, domain packs, and truth definitions that it can execute.

## Storage Direction

- primary record store: SurrealDB
- vector and semantic recall: LanceDB
- analytical batch and interchange format: Parquet
- current scaffold: in-memory runtime store with explicit storage configuration objects

The code keeps persistence behind a storage boundary so the business model can evolve without coupling domain rules to transport or a specific database client. Durable projections stay in this repository even as the constitutional runtime moves into Converge.

The intended split is:

- SurrealDB for transactional CRM and revenue projections
- LanceDB for vector search and semantic recall
- Parquet for analytical ingestion batches, audit and timeline export, and Arrow-native interchange into retrieval paths

Do not force the transactional and analytical paths through the same abstraction just because they both persist data.

## API Shape

The gRPC surface is module-oriented and favors job-oriented operations. Current packages are split by capability, with shared record types in `prio.common.v1`.

The operations currently exposed cover:

- create or update account context
- attach people and relationships
- move opportunities and workflow cases
- append activity, notes, communication, documents, and facts
- define custom objects, fields, and saved views
- retrieve account summaries and stream timeline context

This keeps the API aligned with Converge flows instead of forcing every client through a generic CRUD layer.

The truth catalog sits above those module APIs. It describes which jobs exist, which modules they compose, which guardrails or approvals apply, and which Converge packs and intent packet should be used to execute the job.

## Capability Modules

The long-term target is not one giant CRM crate. It is a set of capability modules and domain packs that Converge consumes.

The current scaffold now makes that explicit through:

- a shared module descriptor crate
- first-wave module crates
- second-wave scaffolds that the JTBD catalog depends on, including `catalog`, `payments`, `audit`, `memory`, and `agent-ops`
- a module registry exposed by the server profile endpoint
- a separate truth catalog exposed through `prio.truths.v1`
- a Converge binding for each truth, derived from the same module map

See [[Module Map]], [[Truths Layer]], [[Converge Application]], and `contracts/module-registry.yaml`.
