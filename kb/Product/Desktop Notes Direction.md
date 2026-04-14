# Desktop Notes Direction

## Position

The desktop app should become a local-first notes product with a normal filesystem-visible vault.

- Canonical notes live in `~/Notes`.
- Canonical note format is an Obsidian-compatible vault on disk.
- Tauri stays a thin shell.
- Rust owns note IO, parsing, import, indexing, search, sync, and external integrations.
- Converge, Organism, and cloud services connect through ports. They do not sit in the write path for basic note editing.

Companion policy:

- [[Notes Operating Model]]

This keeps the user in control of their files, avoids lock-in, and matches the workflow expectations set by Obsidian.

## Current Repo Fit

This direction fits the repo, but not by stretching the current operator cockpit into the canonical note model.

Relevant current seams:

- `apps/desktop/` is already the Tauri desktop surface.
- `apps/desktop/src-tauri/src/main.rs` already boots a local Rust application boundary and exposes Tauri commands.
- `crates/prio-documents/` already claims ownership of `document`, `note`, `file`, `attachment`, and `version`.
- `crates/prio-memory/` already exists as the semantic retrieval and embedding boundary.
- `crates/crm-kernel/` already has `Note` and `Document`, but those are closer to CRM timeline records than vault-native filesystem objects.

The practical consequence: use the existing module map, but let the desktop note product grow a vault-native domain shape under `documents` instead of forcing `crm-kernel::Note` to become the canonical user note file.

## Source Of Truth

### Canonical storage

Use the filesystem as the source of truth:

- root: `~/Notes`
- note files: UTF-8 Markdown
- directories: user-visible notebook hierarchy
- attachment files: relative files on disk, not blobs hidden in a database

Recommended attachment layout:

- `Folder/Note.md`
- `Folder/Note.assets/...`

That keeps relative links stable and avoids one global attachments dump.

### Derived local state

Keep rebuildable application state outside the vault, for example under:

- `~/Library/Application Support/PrioDesktop/`

This is where local-only derived state belongs:

- search index
- import checkpoints
- file watch cache
- parsed link graph
- embeddings or retrieval cache
- background job state

If SurrealDB, SQLite, LanceDB, or Parquet are useful here, use them here. They are secondary indexes and caches, not the source of truth for the user's notes.

## Markdown Contract

Yes: choose the Obsidian format as the product contract.

More precisely, choose an Obsidian-compatible vault format, because Obsidian is mostly a set of filesystem and Markdown conventions rather than one formal spec document.

Baseline support:

- CommonMark text
- fenced code blocks
- task lists
- tables
- YAML frontmatter
- `[[wikilinks]]`
- `![[embeds]]`
- tags like `#topic`
- relative attachment links

Unknown Markdown or frontmatter should pass through unchanged. The app should avoid reformatting a file unless the user explicitly edits it.

Things we should not make mandatory:

- a running Obsidian install
- Obsidian-specific JSON settings under `.obsidian/`
- any database that becomes more authoritative than the files

If `.obsidian/` exists, treat it as compatible workspace metadata. Do not make it part of the core contract for reading or writing notes.

## Why The Current CRM Note Model Is Not Enough

`crm-kernel::Note` is currently a good fit for append-only operational notes and timeline projection. It is not a complete vault file model.

Missing vault concerns include:

- filesystem path and rename semantics
- folder moves
- frontmatter
- backlinks and wikilinks
- attachment lifecycle
- file watch and external edits
- conflict handling for concurrent file changes
- preserving user formatting and exact Markdown text

So the right move is:

- keep CRM notes as business records and projections
- introduce a vault-native note model under `documents`
- project into CRM, memory, or workflow only when needed

## Rust Boundary

The desktop product should keep business logic in Rust and keep Tauri commands thin.

Recommended shape:

- `prio-documents`
  - vault domain types
  - Markdown parse and serialize rules
  - create/read/update/rename/move/delete note services
  - attachment import and reference rewriting
  - link graph
  - import pipeline
- `prio-memory`
  - embeddings
  - semantic recall
  - entity extraction
  - retrieval indexing
- desktop Tauri layer
  - command mapping only
  - no note business rules in TypeScript

That keeps the desktop app aligned with the existing module boundaries instead of creating a second logic stack in the frontend.

## Converge And Organism Boundaries

The integration rule should be simple:

- typing, saving, renaming, moving, and browsing notes must work fully offline
- Converge is for governed automation and downstream facts, not basic note persistence
- Organism is for local intelligence capabilities such as OCR, transcription, extraction, and other attachment-heavy tasks

Examples of good ports:

- `port.notes.import.apple_notes`
- `port.notes.enrich.ocr`
- `port.notes.enrich.entities`
- `port.notes.publish.fact`
- `port.notes.sync.cloud`

The note vault stays primary. Everything else is a consumer or enrichment layer.

## Apple Notes Import

Do not start with direct SQLite parsing as the main path.

Reasons:

- Apple Notes storage format has changed over time.
- Third-party parsers document a move from older clear-text-ish store files to `NoteStore.sqlite` with non-clear-text payloads.
- locked notes add password and crypto edge cases
- attachments and consistency handling are more complex than plain note text
- reverse-engineering the database is a brittle maintenance burden

Use a staged import strategy instead.

### Stage 1

Support import from a directory tree of Markdown files plus attachments.

This is the real durable capability anyway. It also lets the product import from Obsidian, exported Apple Notes, and any future exporter with the same pipeline.

### Stage 2

Add an Apple Notes adapter that consumes exported data rather than parsing the live database.

Two practical entry paths already exist:

- official Apple Notes Markdown export for individual notes on current macOS
- external exporters that already convert Apple Notes into Markdown plus attachments

### Stage 3

Only if the exporter path is not sufficient, add a read-only database parser adapter behind the same import port.

If that happens, keep it:

- macOS-specific
- read-only
- best-effort
- version-gated
- isolated from the main vault model

## Apple Notes Reality Check

Verified on 2026-04-12:

- Apple's current Notes guide for macOS Tahoe 26 documents `File > Export as > Markdown` and `File > Import Markdown`, but the export flow is per note, not bulk.
- Third-party tools show two viable bulk-export patterns:
  - AppleScript and UI-driven export with attachments preserved
  - direct database export to Markdown and metadata, with locked-note handling

That means the clean first implementation is not "parse Apple internals first". It is "normalize imported Markdown trees first, then plug Apple Notes into that importer".

## Import Mapping

Recommended mapping from Apple Notes into the vault:

- Apple folder -> vault directory
- Apple note title -> file name slug
- original title -> frontmatter `title` when needed
- note body -> Markdown body
- created and updated times -> frontmatter metadata
- Apple note identifier -> frontmatter source metadata
- attachments -> note-local `.assets/` directory
- links between notes -> convert to relative links or `[[wikilinks]]` when resolvable

Suggested import metadata shape:

```yaml
---
title: Example note
created: 2026-04-10T08:12:14Z
updated: 2026-04-11T19:42:03Z
tags:
  - imported/apple-notes
source:
  system: apple_notes
  note_id: "..."
  account: "iCloud"
---
```

Keep imported metadata minimal. Do not flood frontmatter with every Apple-specific field unless it is operationally useful.

## First Implementation Slice

1. Expand `prio-documents` from manifest-only scaffolding into a real vault domain.
2. Add a filesystem-backed note service rooted at `~/Notes`.
3. Expose thin Tauri commands for:
   - list tree
   - read note
   - write note
   - create note
   - rename or move note
   - import Markdown directory
4. Add file watching so external edits from Obsidian or Finder are reflected immediately.
5. Add a simple desktop UI for note tree plus editor, while keeping existing CRM views secondary.
6. Add Apple Notes import as an adapter on top of the Markdown import pipeline.
7. Add retrieval and Converge projection later, after the vault model is stable.

## Recommendation

The right first bet is:

- canonical vault in `~/Notes`
- Obsidian-compatible Markdown behavior
- Rust-owned note logic under `prio-documents`
- derived local indexes outside the vault
- Apple Notes import via exporter-driven Markdown intake first

That gives a stable local-first product core and keeps Apple Notes migration from dictating the internal architecture.

## References

- Apple Support, Notes User Guide for macOS Tahoe 26:
  `https://support.apple.com/en-lamr/guide/notes/not201900c07/mac`
- `storizzi/notes-exporter`:
  `https://github.com/storizzi/notes-exporter`
- `yirogue/apple_notes_export`:
  `https://github.com/yirogue/apple_notes_export`
- `ChrLipp/notes-import`:
  `https://github.com/ChrLipp/notes-import`
