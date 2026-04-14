# Notes Operating Model

## Purpose

Define how the Notes product should work for both:

- the human operator using the vault every day
- the agents that capture, enrich, clean up, and publish around that vault

The key rule is simple:

- the vault is primary
- raw captures are evidence
- enrichments are derived artifacts
- published notes are the human-facing layer

This page is the operational companion to [[Desktop Notes Direction]].

## Core Position

The Notes app is not a chat transcript archive and not a hidden agent database.

It is a local-first working knowledge system with explicit layers:

1. `raw`
2. `enriched`
3. `published`

The system may compound over time, but it must do so without blurring:

- what came from a source
- what the machine inferred
- what a human actually stands behind

## Canonical Rules

- `~/Notes` is the canonical root.
- Markdown files on disk are the source of truth for human-facing notes.
- Hidden pipeline folders stay in the vault but out of the normal note tree.
- Agents must not silently rewrite raw evidence.
- Agents must not silently overwrite human-authored canonical notes.
- Derived outputs must remain marked as derived until explicitly promoted.
- Promotion is a separate action from ingestion or enrichment.

## Vault Layers

Recommended shape:

```text
~/Notes/
  Inbox/
  Projects/
  Areas/
  Resources/
  Archive/

  Imported/

  .raw/
    apple-notes/<run-id>/...
    web/<run-id>/...
    pdf/<run-id>/...
    image/<run-id>/...

  .enriched/
    cleanup/<run-id>/...
    ocr/<run-id>/...
    vision/<run-id>/...
    entities/<run-id>/...
    summaries/<run-id>/...

  INDEX.md
  LOG.md
```

Layer meaning:

- `published`
  visible notes in normal folders such as `Inbox`, `Projects`, `Areas`, `Resources`, `Archive`
- `raw`
  source-faithful captures and imported artifacts
- `enriched`
  machine-derived outputs only

`Imported/` is transitional and may still be useful for plain Markdown tree imports, but it is not the long-term answer for all external source ingestion.

## Artifact Classes

Every meaningful note-like artifact should fit one of these classes:

- `source_capture`
  imported or fetched source material
- `derived`
  machine-generated analysis or extraction
- `note`
  normal human-facing working note
- `draft`
  candidate note awaiting review or promotion

These classes should be explicit in frontmatter.

## Provenance Contract

The system must be able to distinguish:

- human-authored original
- imported source
- machine-derived
- human-reviewed machine-assisted

Suggested fields:

```yaml
kind: note | source_capture | derived | draft
provenance: human_authored | imported_source | machine_derived | human_reviewed_agent_assisted
review_state: canonical | draft | unreviewed | approved | rejected
derived_from:
  - .raw/web/20260413T120000Z/manifest.json
source_system: apple_notes | web_capture | pdf | image | manual
```

Do not rely only on directory names. Agents need note-local provenance.

## Frontmatter Conventions

### Human-facing note

```yaml
---
kind: note
provenance: human_authored
review_state: canonical
vault_created_at: "2026-04-13T10:00:00Z"
vault_touched_at: "2026-04-13T10:00:00Z"
---
```

### Raw source capture

```yaml
---
kind: source_capture
provenance: imported_source
source_system: apple_notes
captured_at: "2026-04-13T09:08:41Z"
immutable: true
---
```

### Derived artifact

```yaml
---
kind: derived
provenance: machine_derived
review_state: unreviewed
derivation_type: ocr
derived_from:
  - .raw/image/20260413T121500Z/photo.jpg
generator: apple_vision
generated_at: "2026-04-13T12:16:02Z"
---
```

### Promoted machine-assisted note

```yaml
---
kind: note
provenance: human_reviewed_agent_assisted
review_state: approved
derived_from:
  - .raw/web/20260413T123000Z/manifest.json
  - .enriched/summaries/20260413T123100Z/summary.md
---
```

## User Modus Operandi

The operator should be able to work in a simple way:

- write notes directly in the visible vault
- import or capture source material without deciding the final structure immediately
- let enrichments accumulate around the source material
- review drafts and promoted notes when the machine proposes structure

The operator should spend most of their time in published notes, not in `.raw` or `.enriched`.

The product should make it easy to answer:

- what do I know
- what is the source
- what changed
- what is only an inference

## Agent Modus Operandi

Agents should operate with asymmetric permissions:

- they may read broadly
- they may capture into `raw`
- they may write derived outputs into `enriched`
- they may propose drafts for publication
- they may not silently rewrite canonical published notes

Agent behavior should default to:

1. read `INDEX.md`
2. read relevant published notes
3. inspect raw evidence as needed
4. write derived outputs into `.enriched/...`
5. propose a promotion into a visible note or draft

## Source Intake Rules

### Plain text note

Keep it as a normal note when:

- the content itself is the durable thing
- there is no need to preserve source-fidelity beyond the note
- the operator is simply thinking, writing, planning, or recording

### URL in a note

Keep the URL as a plain link when:

- it is just a citation or bookmark
- the page does not need local evidence retention
- there is no near-term need for extraction or recurring follow-up

Capture the URL when:

- the page is evidence
- the page may change later
- canonical URL and page metadata matter
- the page is likely to be summarized, linked, or watched

URL capture should create:

- a raw snapshot under `.raw/web/<run-id>/`
- a published stub or note that points back to the raw snapshot

### Imported Apple Notes

Apple Notes import should remain source-faithful first.

That means:

- preserve folder/account shape in `raw`
- preserve note-local assets
- preserve note IDs and timestamps
- do not treat the imported Apple Notes tree as the final canonical published structure

The richer structure should be built on top of the imported corpus, not during import.

### Image attachment

Use the attachment as-is when:

- the image is supporting context and no further interpretation is needed

Run OCR when:

- the question is "what text is in this image?"

Run vision when:

- the question is "what is happening in this image?"
- layout, objects, scene, or context matters more than text

Run both OCR and vision when:

- the image is a screenshot
- the image is a whiteboard
- the image is an annotated photo
- both visible text and scene context matter

### PDF attachment

Use direct PDF extraction when:

- the PDF is text-native
- structure matters

Use OCR when:

- the PDF is scanned
- page images are the real source

Always preserve the original PDF in `raw`.

### Social URL

Treat social pages as URL captures first, not as special-case hidden ingestion.

Platform-specific normalization may happen later, but the first durable step is still:

- raw capture
- enrichment
- published note or watch entry

## Recurrence Rules

Do not make ordinary notes recurrent.

Use recurring capture only for explicit watch targets.

Examples:

- company pages
- press pages
- executive profiles
- market or competitor watchlists
- specific social accounts

Watch targets should create repeated raw snapshots over time.

The published layer should summarize evolution across snapshots rather than overwrite prior evidence.

## Enrichment Rules

Enrichment must remain clearly derived.

Examples of `enriched` outputs:

- OCR text
- scene descriptions
- extracted entities
- tag proposals
- backlink suggestions
- summaries
- contradiction flags
- cleanup and merge suggestions

Derived artifacts should cite their source inputs and the generator used.

## Promotion Rules

Promotion turns evidence plus derivation into a human-facing note.

Promotion should usually be explicit, not implicit.

Typical path:

1. ingest source into `raw`
2. derive OCR, summaries, entities, or scene understanding into `enriched`
3. create a draft note
4. review and promote into a visible canonical note
5. record the mutation in `LOG.md`

## INDEX And LOG

The vault should gain two explicit support files:

- `INDEX.md`
  curated map of major notes, entities, and directories
- `LOG.md`
  append-only record of important promotions, restructures, merges, and governance decisions

These are not raw event streams. They are operator-facing orientation tools.

## Privacy And Provider Rules

The imported corpus may contain highly sensitive material.

Therefore:

- local processing should be the default
- cloud enrichment should be explicit, capability-bounded, and reviewable
- provenance should record which provider or model touched derived artifacts
- sensitive raw captures must not be flattened into cloud-oriented workflows by default

## Current Implementation Notes

As of the current milestone:

- Apple Notes import is live and lands in `.raw/apple-notes/...`
- web snapshot capture is live and writes both raw artifacts and a visible note stub
- cleanup analysis is live and writes into `.enriched/...`
- note freshness and value analysis is live and writes promote, refresh, and demote suggestions into `.enriched/...`
- OCR, PDF extraction, scene understanding, entity extraction, and backlink suggestions are planned but not yet fully wired into the Notes app

That means the next major step is not another importer.

It is a proper publish layer that turns raw captures plus enrichments into canonical visible notes while preserving provenance.

## Inspiration

The AI wiki pattern is useful inspiration, especially for:

- compounding context
- explicit vault structure
- `INDEX.md`
- `LOG.md`
- write-back as a side effect of normal work

But this project should keep a stricter distinction between:

- evidence
- inference
- human-approved contributions

Reference:

- Aaron Fulkerson, "Karpathy's Pattern for an 'LLM Wiki' in Production"
  `https://aaronfulkerson.com/2026/04/12/karpathys-pattern-for-an-llm-wiki-in-production/`
