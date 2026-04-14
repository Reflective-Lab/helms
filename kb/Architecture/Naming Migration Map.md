# Naming Migration Map

This repository is now named **Outcome Workbench** at the application surface.

The codebase still contains many legacy `crm-*` and `prio-*` names. Those should be
removed in staged batches, not by ad hoc edits.

## Naming Rule

Use names based on architectural role:

- `outcome-workbench` for the product and desktop surface
- `application-*` for application-layer runtime and projection crates
- `workbench-*` for desktop-facing backend or UI-host concepts
- `capability-*` for reusable capability modules and registries
- `truth-*` for truths, truth bindings, and truth catalogs
- `intent-*` for intent-session concepts

Avoid:

- `crm-*` for new names
- `prio-*` for new names
- product branding as the backbone of architecture

## Current Chosen Product Name

- Product: `Outcome Workbench`
- Desktop package/binary: `outcome-workbench-desktop`
- Desktop env family: `OUTCOME_WORKBENCH_*`
- Desktop local namespace default: `outcome_workbench`

## Staged Rename Direction

### Application Layer Crates

| Current | Target |
|---|---|
| `crm-contracts` | `application-contracts` |
| `crm-kernel` | `application-kernel` |
| `crm-storage` | `application-storage` |
| `crm-server` | `application-server` |
| `crm-app` | `workbench-backend` |

### Capability Foundation Crates

| Current | Target |
|---|---|
| `prio-module-core` | `capability-core` |
| `prio-modules` | `capability-registry` |
| `prio-truths` | `truth-catalog` |

### Capability Leaf Crates

Rule:

- `prio-<name>` -> `capability-<name>`

Examples:

- `prio-catalog` -> `capability-catalog`
- `prio-documents` -> `capability-documents`
- `prio-workflow` -> `capability-workflow`
- `prio-expenses` -> `capability-expenses`

### Utility / Local-Only Crates

These can follow clearer problem-shaped names instead of the generic capability rule:

| Current | Target |
|---|---|
| `prio-vault` | `note-vault` |
| `prio-apple-notes` | `apple-notes-import` |
| `prio-apple-notes-cli` | `apple-notes-import-cli` |

## Proto / Package Namespace Direction

This also needs a staged migration.

Preferred target direction:

| Current | Target |
|---|---|
| `prio.common.v1` | `application.common.v1` |
| `prio.modules.v1` | `capability.registry.v1` |
| `prio.truths.v1` | `truth.catalog.v1` |
| `prio.<capability>.v1` | `capability.<capability>.v1` |

Note:

- namespace migration should happen after the crate rename map is settled
- keep compatibility windows where generated code or external clients still depend on old names

## Environment Variable Direction

### Preferred Product-Surface Prefix

Use `OUTCOME_WORKBENCH_*` for desktop-app-specific environment variables.

Examples already in use:

- `OUTCOME_WORKBENCH_DESKTOP_MODE`
- `OUTCOME_WORKBENCH_DESKTOP_ORG_NAME`

### Compatibility Policy

Keep fallbacks temporarily for:

- `WORKBENCH_DESKTOP_*`
- `PRIO_CRM_DESKTOP_*`

Remove those only after the rename batch and verification pass are complete.

## Execution Order

1. Product surface and docs
2. Desktop/package/env names
3. Application-layer crate names
4. Capability registry / truth catalog names
5. Leaf capability crates
6. Proto/package namespaces
7. Compatibility cleanup

## What Is Safe To Change Now

Safe now:

- docs
- desktop product strings
- desktop package/bin names
- desktop env names with fallbacks
- comments and grouping in workspace metadata

Not safe during foundation churn:

- large crate graph renames
- broad proto namespace churn
- generated-code moves tied to active Converge or Organism API changes
