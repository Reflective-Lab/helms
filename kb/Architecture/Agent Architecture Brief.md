# Agent Architecture Brief

## Purpose

This document is the working architectural intent for this repository.

It is written for agents and implementors so they can make local design decisions
without drifting the repo into the wrong shape.

## One Sentence

This repository is the application layer and desktop workbench for **Outcome Workbench**,
built for an SMB entrepreneur,
built on top of `../converge` and `../organism`, with the long-term goal of becoming an
intent-adaptive operator environment.

## Non-Negotiable Position

This repository is not:

- a second runtime
- a second intelligence framework
- a second OCR platform
- a classic CRM product
- a generic foundation library

This repository is:

- the entrepreneur application layer
- the projection store for business and work state
- the desktop workbench host
- the place where truths become concrete work surfaces
- eventually the place where daily intent is decoded into an adaptive day plan

## Foundations

There are two internal foundations:

- `../converge`
- `../organism`

They are not "ports" in the external hexagonal sense.

They are internal foundational systems with typed Rust APIs and are intended to be
tightly integrated.

### `converge`

Owns:

- runtime
- convergence
- truth execution
- authority
- promotion
- budgets
- HITL/governance
- reusable runtime-side capabilities like analytics and optimization

Examples of foundation capabilities in Converge:

- `converge-analytics`
- `converge-provider`
- `converge-optimization`

### `organism`

Owns reusable intelligent capabilities such as:

- OCR
- document understanding
- note and content intelligence
- semantic helpers
- reusable local intelligence building blocks

## External Ports

External ports are reserved for outside systems.

Examples:

- LinkedIn
- Salesforce
- email providers
- accounting systems
- banking systems
- booking systems

Do not describe `converge` or `organism` as external ports.

## Providers

Providers are interchangeable concrete implementations inside a capability.

Examples:

- OCR provider A vs OCR provider B
- LLM provider A vs LLM provider B
- retrieval provider A vs retrieval provider B

Providers are not the same thing as capabilities.

## Three Concepts That Must Stay Separate

### 1. Capability Module

A reusable backend business capability.

Examples:

- expenses
- documents
- conversations
- workflow
- approvals

These are not UI concepts.

### 2. Truth

An outcome contract.

A truth describes:

- what should become true
- which capabilities are involved
- what counts as success
- what constraints or approvals apply

### 3. Desktop App

A user-facing work surface inside the Tauri workbench.

Examples:

- Notes
- Customer Calls
- Receipt Management
- Revenue Review

A desktop app is not the same as a capability module.

Example:

- `expenses` is a capability module
- `submit my March reimbursement` is a truth
- `Receipt Management` is a desktop app

If these concepts collapse into one, the architecture will drift.

## Identity Of This Repo

This repo should be the opinionated application layer for one user archetype:

- the SMB entrepreneur
- the operator trying to get important things done today

The user experience should evolve from:

- route-based application shell

toward:

- dynamic desktop workbench

and later toward:

- intent-adaptive workbench

## Core Layers

### Layer 0: Foundations

- `converge`
- `organism`

These should remain reusable beyond this app.

### Layer 1: Application Core

This repo's core should own:

- capability composition
- truth catalog for this operator archetype
- application projections
- application defaults and policies
- internal foundation bindings to Converge and Organism

### Layer 2: Desktop Workbench

The Tauri desktop app is permanent.

It should become a host for multiple apps and work surfaces.

It should own:

- app loading
- workbench layout
- navigation
- search
- notifications
- local filesystem affordances
- local-first interaction where useful

### Layer 3: Intent Decoder

This is the later layer on top of the workbench.

It should:

- interpret what the user wants to make true today
- propose truths
- propose next actions
- activate or compose work surfaces
- fill obvious gaps

It should orchestrate the workbench, not replace it.

## Intent Session

The future LLM layer should not be built as a free-floating chat overlay.

It should be anchored in a first-class application concept:

- `IntentSession`

An intent session should eventually contain:

- desired truths
- proposed truths
- current blockers
- active apps
- current workbench context
- suggested next actions
- evidence gathered during the day
- completion state for "what should be true when I go home today"

## Current Repo Reality

Directionally correct:

- module crates exist
- truth catalog exists
- Converge integration exists
- desktop exists
- `workbench-backend` is already close to an application-facing backend shell

Currently overloaded or misleading:

- the repo still describes itself too much as "CRM"
- `crm-*` naming is too narrow for the intended product
- `prio-*` naming is brand-driven rather than architectural
- desktop is still mostly a static routed shell
- capability modules and desktop apps are not clearly separated in structure

## Naming Direction

See [[Naming Migration Map]] for the current canonical rename map.

Two naming families should be phased out:

- `crm`
- `prio`

### Why `crm` is wrong now

It implies:

- a classic CRM
- a sales-led product
- a narrower product than the actual workbench being built

### Why `prio` is wrong now

It is a brand marker, not an architectural concept.

Brand names should not be the backbone of the codebase.

### Preferred Naming Direction

Use neutral names based on role:

- application
- workbench
- capability
- truth
- intent
- projection

Examples of target direction:

- `crm-kernel` -> `application-kernel`
- `crm-storage` -> `application-storage`
- `crm-server` -> `application-server`
- `crm-app` -> `workbench-backend`
- `prio-module-core` -> `capability-core`
- `prio-modules` -> `capability-registry`
- `prio-truths` -> `truth-catalog`

This is a staged migration, not a piecemeal rename.

## Internal Integration Rule

Dependencies on Converge or Organism should go through clear local seams,
but those seams are internal integration boundaries, not external ports.

Good terms:

- foundation binding
- bridge
- adapter
- integration seam

The recent OCR bridge is a good example of this pattern.

## Architectural Rule For Agents

When adding functionality, ask:

1. Is this a reusable foundation concern?
   If yes, it probably belongs in `converge` or `organism`, not here.
2. Is this application composition for the SMB entrepreneur?
   If yes, it belongs here.
3. Is this a backend capability, a truth, or a desktop app?
   Do not mix them.
4. Is this an external system?
   If yes, model it as a port/integration.
5. Is this just one concrete implementation choice?
   If yes, it is probably a provider.

## Concrete Near-Term Restructure

### 1. Keep the foundations clean

- move generic OCR/intelligence concerns toward `organism`
- move runtime/governance concerns toward `converge`

### 2. Keep this repo application-shaped

- keep projections here
- keep operator-specific truths here
- keep workbench surfaces here

### 3. Add a desktop app registry

Separate from capability modules.

Needed concepts:

- `DesktopAppManifest`
- `DesktopAppId`
- `DesktopAppSurface`
- `DesktopAppCapabilityBinding`

### 4. Evolve the backend shell

The current `workbench-backend` role should evolve into a workbench backend that serves:

- desktop app manifests
- workbench summaries
- app-specific read models
- intent-session state

### 5. Add intent-session types before full LLM orchestration

Do this before building a large adaptive layer.

Suggested concepts:

- `IntentSession`
- `DesiredOutcome`
- `ProposedTruth`
- `DayPlan`
- `ActiveWorkbenchContext`

## Migration Order

1. Align docs and terminology.
2. Define neutral canonical names for crates and transport namespaces.
3. Introduce aliases or new crates with neutral names.
4. Add a desktop app registry.
5. Move the current routed desktop surfaces behind that registry.
6. Add intent-session types.
7. Add the LLM intent-decoder layer on top of the deterministic workbench model.
8. Continue extracting reusable foundation concerns into `organism` and `converge`.

## Bottom Line

The target shape is:

- reusable foundations below
- one application layer here
- one permanent desktop workbench
- one later intent-decoder layer on top

This repo should become the entrepreneur's adaptive workbench, not another foundation and not just a CRM.
