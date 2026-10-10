---
title: Revisions and migration
description: How blueprint revisions work, why records stay pinned to one, and how to move them to a newer revision.
---

## Drafts, published revisions, and pinning

Each blueprint has a sequence of revisions: 1, 2, 3, and so on.

- A **draft** revision can be edited and deleted. No record can use it.
- A **published** revision is frozen. Its TOML, attributes, views, and schemas never change again.
- The **current** revision is the highest published one. New records use it.

Every record is pinned to the revision it was created with, or last migrated to. Its values are validated by that revision's rules and shown with that revision's layout. Publishing revision 3 changes nothing for a record on revision 2 until someone migrates it.

As a result, a value keeps the meaning it had when it was written, even after the definition moves on.

## Create a new revision

In the web app, open the blueprint under **Manage → Blueprints** and create a new revision from the current one. With the CLI:

```sh
acli blueprint revision <blueprint-id> --file product-v2.toml
acli blueprint publish <blueprint-id> 2
```

The `code` must stay the same. Everything else can change.

## Outdated records

Once a newer revision is published, records on older revisions are **outdated**. You can find them in several places:

- The Explorer defaults to the current revision and shows a notice with a link to hidden older records.
- Selecting **All versions** in the Explorer and sorting by the **Schema** column puts the oldest revisions first.
- The record page marks an outdated record and links to its upgrade page.
- **Manage → Data health** counts outdated records per blueprint.

## Migrate one record

Open the record and choose to upgrade it. Attricat compares its current values with the target revision and reports one of these outcomes:

| Outcome | Meaning | What to do |
| --- | --- | --- |
| `ready` | Every value fits the new revision. | Migrate. |
| `needs_input` | The new revision needs something the record does not have, such as a newly required attribute. | Fill in the missing values on the upgrade page, then migrate. |
| `blocked` | Existing values cannot fit, for example a value of a changed type or relationships that exceed a new cardinality. | Fix or discard the conflicting values, then migrate. |

The target is always the current published revision. Migration copies values into the new revision, validates them, rebuilds the record's search data, and records the migration in one step. If anything fails, nothing changes.

With the CLI:

```sh
acli record migrate <record-id>
```

## Migrate many records

### From the web app

When a new revision can store everything the previous revision could, the blueprint page offers **Migrate compatible records**. It starts a background batch that looks at every record pinned to any older revision of the blueprint. Each record whose migration is `ready` is migrated. The rest are left for review.

The blueprint's **Migrations** tab lists every batch with its target revision, progress, and counts of migrated, needs-review, and failed records. While a batch for the current revision is queued or running, the action is disabled.

| Batch status | Meaning |
| --- | --- |
| `queued` | Waiting for a worker. |
| `running` | Being processed. |
| `completed` | Every eligible record was examined. Some may still need review. |
| `superseded` | Replaced by a newer batch. |

Two server settings tune batches: `BLUEPRINT_MIGRATION_PAGE_SIZE` and `BLUEPRINT_MIGRATION_CONCURRENCY`. See the [configuration reference](/reference/configuration/#catalog-behavior-and-limits).

### From the CLI

`record migrate-bulk` migrates every `ready` record from one source revision and reports the rest:

```sh
acli record migrate-bulk --blueprint product --from-version 1 --dry-run
acli record migrate-bulk --blueprint product --from-version 1
```

The summary groups records into `ready`, `needs_input`, `blocked`, and `failed`. Records that need input or are blocked are skipped; resolve them on their upgrade pages, then run the command again.

## Designing revisions that migrate cleanly

- **Add, don't change.** A new optional attribute keeps every record `ready`.
- **Don't reuse codes.** If `weight` changes from grams to kilograms, add `weight_kg` and retire `weight`. Reusing the code makes old values look valid and wrong.
- **Loosen before you tighten.** Publish a revision that adds a field, fill it in across the catalog, then publish a revision that makes it required.
- **Watch cardinality.** Changing a relationship from `many` to `one` blocks every record that has more than one target.
- **Bump includes on purpose.** A new mixin revision only reaches a blueprint when you publish a blueprint revision that points at it.
