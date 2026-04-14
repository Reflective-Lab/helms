# Port Capability Taxonomy

This document defines how external ports should map to reusable Organism
capabilities and application logic in this repository.

It is intended to prevent three kinds of architectural drift:

- putting external-system logic directly into desktop apps
- turning this repo into a second reusable capability foundation
- conflating generic capture with application-specific publishing

## Layering

### `../organism`

Owns reusable intelligent capabilities with typed Rust interfaces.

Examples:

- `organism_intelligence::web`
- `organism_intelligence::social`
- later `organism_intelligence::document_enrich`

These are reusable capabilities, not application surfaces.

### This Repository

Owns application logic built on top of those capabilities.

Examples:

- which external ports matter for this operator environment
- how raw captures are stored
- how enrichment runs are scheduled
- how curated content is published into the human-facing vault
- how the desktop workbench exposes these flows

### External Ports

External ports are outside systems.

Examples:

- LinkedIn
- X
- Instagram
- Facebook
- company websites
- press sites

## Capability To Port Mapping

### Reusable Organism Capabilities

`organism_intelligence::web`

- capture a public URL
- return typed page content and metadata
- normalize canonical URL, title, description, site name, links
- stay agnostic about the application using it

`organism_intelligence::social`

- normalize public social profile and page URLs
- detect platform from URL
- extract stable profile identifiers like handles when possible
- return a common social profile shape backed by a web capture

Future likely capabilities:

- `organism_intelligence::document_enrich`
- `organism_intelligence::vision`
- `organism_intelligence::ocr`

### Application Ports In This Repo

These should be modeled here as application-facing seams:

- `port.web.capture`
- `port.social.linkedin`
- `port.social.x`
- `port.social.instagram`
- `port.social.facebook`

Those ports should compose Organism capabilities rather than reimplement them.

Examples:

- `port.social.linkedin` uses `organism_intelligence::social` plus application rules
- `port.web.capture` uses `organism_intelligence::web` plus raw-run storage

## Provider Position

Providers are interchangeable implementations behind capabilities.

Examples:

- plain HTTP capture
- rendered browser capture
- FireCrawl-backed capture
- Brave-assisted discovery before capture

The important rule is:

- applications should depend on capability contracts
- capability crates can wrap providers
- providers must not become the dominant architecture vocabulary

## Data Flow

External capture should follow the same lifecycle shape as notes:

1. `raw`
2. `enriched`
3. `published`

### `raw`

Source-faithful captures and assets.

Examples:

- fetched HTML
- rendered Markdown
- screenshots
- extracted page metadata
- raw social profile snapshots

### `enriched`

Derived artifacts only.

Examples:

- OCR text
- image descriptions
- extracted entities
- summaries
- classification
- tags proposed by models

### `published`

Clean human-facing artifacts.

Examples:

- curated Obsidian notes
- linked contact or company summaries
- operator-facing watchlists
- task-ready research digests

## Rules

- Keep reusable capture and normalization capabilities in `../organism`.
- Keep application-specific orchestration, projection, publishing, and workbench surfaces here.
- Do not put social scraping logic directly into a desktop route or component.
- Do not make this repo the home of generic capture providers.
- Do not publish directly from external systems into the clean vault without a raw stage.
- Keep provider names secondary to capability names in architecture discussions and code structure.

## Current Direction

The current direction should be:

1. build reusable public-page capture in `../organism`
2. build reusable social normalization in `../organism`
3. consume those capabilities here through application ports
4. store results in raw runs
5. add enrichment and publishing flows on top

This keeps Organism reusable and keeps this repository focused on the operator
application layer and desktop workbench.
