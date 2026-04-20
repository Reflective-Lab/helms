# Helm Surface Model

## Position

Helm is the operator environment for governed business truths.

It is not just:

- a CRM
- a desktop app
- an intent codec
- a second runtime on top of Converge

Helm exists to make **Business Truths** usable through real operator surfaces:

- CLI
- application APIs
- interactive workbenches
- automation and event ingress

Converge remains the governed execution substrate.
Organism remains the reusable intelligence foundation.
Axiom remains the truth-authoring and truth-validation foundation.

Helm is where those foundations become a usable product.

## Who It Is For

Helm is for people and teams who run decision-bearing work:

- founders and general managers
- revenue and go-to-market operators
- finance and procurement operators
- partnerships and strategy teams
- operations leads coordinating humans, systems, and agents

The current milestone narrows the showcase to an inbound revenue pipeline.
The product framing should stay broader than that slice.

The common pattern is not "sales" or "CRM."
The common pattern is:

- declare what should become true
- gather evidence from multiple sources
- surface contradictions
- govern approval
- project the result into durable state

That is the pattern described in [[Writing/Business Truths Article]].

## Core Terms

### Business Truth

A declarative outcome contract for something that should become true.

It includes:

- success criteria
- guardrails
- authority and approval points
- traceability
- budgets or stopping conditions when relevant

### Capability

A reusable operational capability such as documents, approvals, subscriptions, payments, notes, or workflow.

Capabilities are building blocks.
They are not user surfaces.

### Projection

Durable application state shaped for operators, APIs, timelines, and workbench views.

### Surface

Any way a human or system invokes Helm:

- CLI
- HTTP/gRPC API
- desktop workbench
- browser workbench
- mobile workbench
- automation ingress

### Workbench

The interactive operator surface family.

It is where humans inspect evidence, supervise truths, review blockers, and move between work contexts.
The workbench is broader than the desktop app.

### Desktop

The current packaged workbench client.

It is important, but it is not the entire product.

### Foundation Binding

A typed integration from Helm into Converge, Organism, or Axiom.

These are not external ports in the hexagonal sense.
They are internal foundations that Helm composes.

### External Port

A boundary to outside systems such as:

- email
- calendar
- accounting
- banking
- ERP/CRM systems
- SaaS tools
- web and social sources

## Hexagonal Model

### Inside The Hexagon

Helm core should own:

- business truth catalog and truth bindings
- application policies
- operator projections and timelines
- surface manifests and session state
- application orchestration around governed outcomes

### Inbound Ports

These are first-class entry points into Helm:

- CLI commands
- HTTP/gRPC APIs
- workbench clients
- event ingress
- scheduled jobs

### Foundation Bindings

These sit below the application core but are not "outside adapters":

- Converge for governed execution
- Organism for reusable intelligence
- Axiom for truth definitions and validation

### Outbound Ports

These connect Helm to external systems:

- accounting
- payments
- banking
- email
- calendar
- web capture
- social capture
- document stores

### Adapters

Concrete adapters should stay thin:

- Tauri commands
- axum handlers
- tonic services
- CLI binaries
- provider and integration adapters

Business truth semantics should not live in UI-only code.

## Surface Rules

### 1. CLI Is First-Class

Every executable truth should be invokable from a CLI command.

Examples:

- execute a truth
- inspect a truth definition
- seed or import data
- replay or audit a run
- list projections
- resume or approve blocked work

### 2. API Is First-Class

Everything important should also be invokable through an application API.

The API is the shared boundary for:

- desktop
- browser
- mobile
- automation
- external orchestration

### 3. Workbench Consumes The Same Core

The workbench should consume the same application boundary rather than carrying private business logic that only exists in the UI.

### 4. Desktop Is A Client, Not The Whole Product

The desktop client is the current best interactive surface.
It should not become the only way to run Helm.

### 5. Automation Must Not Depend On The Workbench

If a truth matters, it should be runnable without opening the desktop.

## Repo Working Structure

The easiest way to work on this repo is to think in six layers:

1. **Truths**
   Outcome contracts and governed jobs.
2. **Application Core**
   Projections, policies, sessions, orchestration, and state.
3. **CLI Surface**
   Fast local, automation, and operator command entry points.
4. **API Surface**
   Shared remote boundary for all clients and integrations.
5. **Workbench Surfaces**
   Desktop today, more clients later.
6. **External Adapters**
   Connectors to outside systems.

Converge, Organism, and Axiom stay below this as foundations.

## Naming Guidance

Use these meanings consistently:

- `Helm` = product family and operator environment
- `Business Truth` = the core outcome noun
- `Workbench` = interactive operator surface family
- `Desktop` = current packaged workbench client
- `CLI` = command surface
- `application-*` = application core and shared boundaries
- `capability-*` = reusable capabilities
- `truth-*` = truth catalogs and truth tooling

Avoid these confusions:

- calling the whole architecture "CRM"
- treating the desktop app as the whole product
- collapsing capability, truth, and surface into the same noun

## Stage 1 Note

Stage 1 still ships a desktop end-to-end showcase:

- `score-inbound-fit`
- `qualify-inbound-lead`
- `schedule-strategic-meetings`

That is the current demo slice, not the full product boundary.

The right mental model is:

- current milestone = one showcase surface and decision flow
- target architecture = a broader operator environment for governed business truths
