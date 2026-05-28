# Runway Execution Container Boundary

Helm should not become the platform server container for every marquee app.

Helm owns operator-control semantics. Runway owns the execution container that
hosts app backends. Marquee apps instantiate that container with domain packets.

## Decision

`application-server` is currently the practical reference host, but it should be
treated as a transitional composition point:

```text
Today:
  Helm application-server
    -> HTTP/SSE/gRPC host
    -> operator control
    -> truth execution routes
    -> app probes

Target:
  Runway execution container
    -> auth, middleware, telemetry, secrets, storage, health, deploy
    -> mounted Helm operator-control/job module
    -> app packet with truths, subject refs, projections, fixtures, copy
```

New generic host work should move toward Runway. New operator-control work stays
in Helm.

## Helm Owns

- operator-control read models;
- job readiness packets;
- HITL approval and rejection semantics;
- receipt families and operator ledger views;
- app-facing realtime/job stream semantics;
- workbench and local/operator UX;
- app-neutral routes such as `/v1/jobs/{key}/stream` when they express governed
  work rather than deployment substrate.

## Runway Owns

- process lifecycle and ports;
- health checks;
- auth, claims, CORS, middleware, request ids, and error formatting;
- secrets and environment bootstrap;
- telemetry and tracing;
- storage/event-log implementations;
- Cloud Run packaging and deployment;
- public transport defaults.

## Apps Own

- app id and display metadata;
- domain truths and fixtures;
- app subject references;
- product copy and UX;
- domain projections/writeback adapters;
- optional product-specific HTTP routes.

Apps should not own reusable HTTP/gRPC/GraphQL servers, auth stacks, telemetry
bootstrap, or realtime parsers.

## Practical Migration Rule

Before adding to `application-server`, classify the change:

| Change | Destination |
|---|---|
| health, auth, middleware, telemetry, secrets, storage, deploy, event-log backend | Runway |
| operator packet, approval, readiness, receipt, job stream, workbench state | Helm |
| app vocabulary, copy, fixture, domain projection, product route | app packet or app repo |
| convergence, promotion, runtime receipt semantics | Converge |
| formation selection or specialist choice | Organism |
| truth validation or intent artifacts | Axiom |

If the change is generic host machinery, avoid deepening Helm. Add only the
small adapter needed to keep the current proof working, then extract the host
concern to Runway.

## Current Application-Server Split

| Current Helm area | Future owner | Notes |
|---|---|---|
| `main.rs` HTTP listener, CORS, trace layer, route prefix concerns | Runway | Replace with `runway-app-host` once Helm routes are mountable. |
| `main.rs` gRPC service assembly for Helm application APIs | Helm module or internal service | Keep only if the typed service is genuinely Helm-specific. |
| `realtime.rs` event envelope and replay hub | Helm module, backed by Runway event log | Semantics remain Helm/Converge-facing; durable backend comes from Runway. |
| `job_stream.rs` `/v1/jobs/{key}/stream` | Helm governed-job module | This is the first route to expose as a mountable router. |
| `sse.rs` operator-control/pipeline compatibility routes | Helm operator-control module | Keep compatibility, but avoid owning the process host. |
| `http_api.rs` workbench/operator-control endpoints | Helm workbench/operator module | Route module should mount into Runway host. |
| `truth_runtime/*` product truth executors | Helm truth module until Axiom/Organism packet path takes over | Do not copy into apps. |

## Immediate Target

Catalyst is the first proof. The next backend shape should be:

```text
Catalyst UI
  -> Runway-hosted app backend
  -> Helm operator-control/job module
  -> Catalyst app packet
  -> Axiom/Organism/Converge/Mosaic lower-layer contracts
```

That replaces the weaker framing where Catalyst simply calls a Helm
`application-server`.
