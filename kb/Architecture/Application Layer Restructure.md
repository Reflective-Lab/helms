# Application Layer Restructure

## Position

This repository should not become another foundation.

The foundation is:

- `../converge`: runtime, governance, convergence, truth execution, promotion, HITL
- `../organism`: reusable intelligent capabilities, content understanding, OCR, note and document intelligence, semantic helpers

This repository should be the application layer for decision-bearing operators:

- founders and general managers
- revenue, finance, strategy, and operations leads
- people trying to get governed work done during the day
- people who want the system to adapt to intent, not force a fixed back-office workflow

The current milestone uses a revenue and inbound-pipeline showcase.
The architecture should stay broader than that slice.

That means this repo should own:

- application-facing business projections
- app composition for the desktop workbench
- job definitions and truth bindings for this operator
- the experience model of "what should be true by the end of the day?"

It should not own:

- a second runtime
- a second OCR platform
- a second generic intelligence framework
- generic reusable foundations that belong in `organism` or `converge`

## The Core Concepts

### 1. Capability Module

A capability module is a reusable business capability.

Examples:

- `expenses`
- `conversations`
- `documents`
- `workflow`
- `approvals`

These are backend capabilities, not UI concepts.

Current home:

- `crates/prio-*`
- registry in `crates/prio-modules`

### 2. Truth

A truth is an outcome contract.

It describes:

- what the operator is trying to make true
- which capabilities are involved
- what counts as success
- what approvals or policies apply

Current home:

- `crates/prio-truths`
- `crates/crm-server/src/truth_runtime`

### 3. Workbench Surface

A workbench surface is a user-facing work surface inside Helm.

The desktop app is the current packaged client.
It is not the only valid surface.

Examples:

- Notes
- Customer Calls
- Receipt Management
- Revenue Review

A workbench surface is not the same thing as a capability module.

Examples:

- the Receipt Management app may use `expenses`, `documents`, `workflow`, `approvals`, and Organism OCR
- the Customer Calls app may use `conversations`, `tasks`, `facts`, `documents`, and truth suggestions
- the Notes app may use `documents`, `memory`, `facts`, and semantic helpers from Organism

This distinction is the most important structural clarification for the repo.

### 4. Workbench

The workbench is the interactive operator surface family.

Its job is to:

- host work surfaces
- show work in progress
- expose system state and exceptions
- let the user move between concrete surfaces quickly
- later adapt those surfaces based on intent

Today the desktop app is mostly a routed shell in `apps/desktop/src/routes`.
Later Helm should support a broader workbench model with:

- desktop client
- browser clients when useful
- mobile or lightweight supervision surfaces when useful

The key point is that the workbench is broader than the current desktop package.

### 5. Surface Model

Helm should expose three first-class operator surfaces:

- CLI for direct execution, debugging, automation, and local operations
- API for remote clients, automation, and integrations
- workbench clients for high-context human supervision

Every important truth should be runnable through CLI and API, not only from the desktop surface.

### 6. Intent Session

An intent session is the top-level object for the operator's day.

Example:

- "When I go home today, reimburse the March receipts, send follow-ups to two leads, and clean up my meeting notes."

An intent session should eventually contain:

- desired truths
- active apps
- current blockers
- proposed next actions
- evidence gathered during the day

This is the right place for the future LLM layer.

The LLM should not replace apps.
It should:

- decode intent
- propose truths
- open or compose the right work surfaces
- fill obvious gaps
- keep the operator in control

### 7. Projection

This repo should remain the projection store for the operator-facing application state.

That means:

- local durable business state
- UX-shaped summaries
- app-specific read models
- synchronization with the runtime boundary

Current home:

- `crates/crm-kernel`
- `crates/crm-storage`
- `crates/crm-app`

## Proposed Layer Model

### Layer 0: Foundations

- `converge`
- `organism`

No application-specific UX assumptions should leak down here.

### Layer 1: Application Core

This repo's core should be:

- capability composition
- truth catalog for decision-bearing operators
- application projections
- application policies and defaults
- foundation bindings

This is where the app decides how reusable capabilities become one opinionated operator product.

### Layer 2: Operator Surfaces

The Tauri desktop shell should become one workbench client and app host.

It needs:

- app registry
- app manifests
- navigation model
- workbench layout
- shared services like search, notifications, file access, sync state

For local development and testing, the preferred boundary is:

- desktop shell as a client
- CLI as a peer client
- `application-server` as the application and truth boundary
- storage and runtime hidden behind the server

An embedded backend inside the desktop app is a temporary bootstrap path, not the long-term layering target.

### Layer 3: Intent Decoder

This is the later layer on top.

It should:

- interpret natural language intent
- propose a day plan
- activate apps
- draft inputs for truths
- watch what is blocked or missing

It should orchestrate the workbench, not bypass it.

## The Key Structural Distinctions

There are three different nouns that should stay separate:

1. capability module
2. truth
3. desktop app

If these collapse into one concept, the architecture will drift again.

Good examples:

- `expenses` is a capability module
- `submit my March reimbursement` is a truth or job
- `Receipt Management` is a desktop app

- `documents` is a capability module
- `leave today with my notes structured and searchable` is a truth
- `Notes` is a desktop app

- `conversations` is a capability module
- `follow up with warm leads before close of business` is a truth
- `Customer Calls` is a desktop app

## What In The Current Repo Already Fits

These parts are directionally right:

- `prio-*` module crates as reusable capabilities
- `prio-truths` as the job and policy layer
- `application-kernel` as a projection-oriented application state store
- `workbench-backend` as the first version of an application-facing shell
- `apps/desktop` as the permanent workbench surface

These parts are currently overloaded:

- the repo still describes itself too much as a CRM substrate rather than an operator environment for governed business truths
- `workbench-backend` mixes "operator shell" and application services, but is close to becoming the workbench backend
- the desktop app is route-based and static, not yet an app host
- capability modules and UX apps are not yet clearly separated in naming or structure

## Recommended Restructure Direction

### 1. Formalize Foundation Integrations

Converge and Organism are not external ports in the hexagonal sense.

They are internal foundations with typed Rust APIs and are meant to be tightly integrated.

So the correct seam here is not:

- external port

It is:

- foundation binding
- foundation bridge
- application-facing integration seam

External ports should stay reserved for systems like:

- LinkedIn
- Salesforce
- email providers
- accounting systems

Every dependency on runtime intelligence or OCR should still go through a clear local seam, but
that seam is an internal integration boundary, not an "outside world" port.

The recent OCR bridge is the right direction.

### 2. Introduce A Workbench App Registry

Add a concept separate from `prio-modules`:

- `WorkbenchAppManifest`
- `WorkbenchAppId`
- `WorkbenchAppSurface`
- `WorkbenchAppCapabilityBinding`

This registry should answer:

- which apps exist
- which routes or panes they own
- which capability modules they depend on
- which truths they commonly launch

### 3. Make `workbench-backend` The Workbench Backend

`workbench-backend` is already close to the correct role.

It should become the application-facing service layer that serves:

- desktop app manifests
- workbench summaries
- app-specific read models
- intent-session state

It should not become another foundation library.

### 4. Add An Intent Session Model

Before building the full LLM layer, add first-class application concepts for:

- `IntentSession`
- `DesiredOutcome`
- `ProposedTruth`
- `ActiveWorkbenchContext`
- `DayPlan`

This keeps the future intent decoder anchored in application state rather than in ad hoc prompts.

### 5. Treat The Desktop As A Host, Not Just A Client

The Tauri shell should own:

- app loading
- workbench composition
- local affordances like filesystem, notifications, background sync
- local-first interaction where appropriate

That makes it the natural place for the future adaptive operator experience.

### 6. Use Capability Language Correctly

There are three different kinds of things in play:

- foundation capabilities
- application capabilities
- providers

Foundation capabilities are things like:

- `converge-analytics`
- `converge-provider`
- `converge-optimization`
- Organism OCR and document intelligence

These are backend capabilities or subsystems.

They are not external ports.

Providers are interchangeable implementations inside a capability.

Examples:

- OCR provider A vs OCR provider B
- LLM provider A vs provider B
- retrieval provider A vs provider B

Providers help agents and services do the same class of job through different concrete engines.

That is different from the capability itself.

## Suggested Package Direction

Not necessarily all at once, but directionally:

- keep `crates/prio-*` as reusable capability modules
- keep `crates/prio-truths` as the truth catalog
- evolve `crates/crm-app` toward a workbench backend
- keep `crates/crm-kernel` and `crates/crm-storage` as application projection/state layers
- add a new crate for desktop app registry and manifests
- add a new crate for intent-session types before adding LLM-driven orchestration

Possible new concepts:

- `crates/prio-workbench`
- `crates/prio-desktop-apps`
- `crates/prio-intent-session`

The exact names matter less than the separation of concerns.

## Naming Direction

The current codebase still leaks two kinds of names that should be phased out:

- brand names like `prio`
- product assumptions like `crm`

Both are architectural debt because they encode yesterday's framing into today's abstractions.

The rename should be deliberate and staged, not done piecemeal.

### What `crm` currently implies

`crm` makes the repo sound like:

- a classic CRM product
- a sales-led system
- a narrower product than the workbench you actually want

But the target is broader:

- entrepreneur workbench
- JTBD application layer
- desktop host for many work surfaces

### What `prio` currently implies

`prio` is a brand marker, not an architectural concept.

It should not be the backbone of crate names, proto packages, or module families.

### Safer Naming Families

Prefer neutral names based on role:

- `application-*`
- `capability-*`
- `truth-*`
- `workbench-*`
- `intent-*`

Example direction:

- `crm-kernel` -> `application-kernel`
- `crm-storage` -> `application-storage`
- `crm-server` -> `application-server`
- `crm-app` -> `workbench-backend`
- `prio-module-core` -> `capability-core`
- `prio-modules` -> `capability-registry`
- `prio-truths` -> `truth-catalog`

For transport namespaces, move away from `prio.*` toward a neutral namespace that describes the
application or workbench rather than the brand.

### Migration Strategy

1. First rename concepts in docs and code comments.
2. Then introduce neutral aliases in Rust crate/module boundaries.
3. Then rename crate packages and internal imports in batches.
4. Finally rename proto package namespaces and generated surfaces.

Do not start by renaming proto packages first. That creates maximum churn for minimum clarity.

## Migration Order

1. Clarify the concepts in code and docs: capability module vs truth vs desktop app.
2. Add a desktop app registry without changing the user experience yet.
3. Move the current desktop routes to registry-backed apps.
4. Add an intent-session model and keep it deterministic at first.
5. Put the LLM layer on top only after the deterministic workbench model exists.
6. Continue moving reusable intelligence and OCR concerns toward `organism`.
7. Continue moving runtime and constitutional concerns toward `converge`.

## Bottom Line

The right identity for this repo is:

- not a generic CRM
- not a second foundation
- not a runtime
- not a bag of random surfaces

It is:

- the operator application layer
- the projection layer for daily work
- the desktop workbench host
- the place where truths become concrete work surfaces
- eventually the place where intent is decoded into an adaptive day plan
