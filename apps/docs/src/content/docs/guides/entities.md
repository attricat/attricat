---
title: Work with entities
description: Create, edit, and delete entities, work with contextual values, files, and relationships, and use the change history.
---

An entity is one catalog record, such as a product or a category. Its blueprint decides which attributes it has, how they are validated, and how its page is laid out.

## Create an entity

In the Explorer, choose **Create entity** (or **Create *Blueprint*** when a blueprint is selected). Fill in the form and save. New entities always use the blueprint's current published revision.

Values you enter when creating an entity are stored in the default context. Attributes with a `default_value` start filled in.

**Duplicate entity** on an existing entity opens the create form pre-filled with its values.

## Edit values

Open an entity and choose **Edit entity**. The form shows the values for one context at a time; switch with the **Context** selector.

The form tells you where each value comes from:

- A value typed directly in this context is its own.
- **Inherited from *X* context** means this context has no value and shows its ancestor's. Typing a value creates an override for this context only. Clearing an override brings the inherited value back.
- **Managed in Default** means the attribute can only be edited in the default context.
- **Managed by system actions** means the attribute is `readonly`: an integration, workflow, or agent maintains it.

Saving validates the whole entity in every context. If a change would make any context invalid, nothing is saved and the form points to the problem. See [Validation](/builders/validation/).

### Unsaved drafts

The edit form keeps your unsaved changes for the browser tab. If you reload or come back, Attricat offers **Restore draft** or **Discard draft**. A draft is cleared once you save. Drafts never contain passwords or files.

## Relationships

Relationship fields open a picker. Search for targets, **Select** them, and **Apply**. Single-select relationships replace the current target. Each option can be previewed in a new tab.

Relationship values are contextual, like any other value: a product can have a different set of categories in one market.

## Files

File fields accept uploads once the entity has been saved. Choose or drop files; each one is checked against the attribute's rules for type, extension, and size.

A new file goes through these statuses: **Uploading**, **Queued**, **Processing**, and **Ready**. Images get a thumbnail and a display-sized version. A file can be downloaded once it is ready. If processing fails, the file shows **Failed**; an administrator can retry it.

## Additional attributes

Some entities need a field their blueprint does not have. If your workspace has published [reusable attributes](/builders/modeling/#reuse-attributes), choose **Add custom attribute or attribute group** on the edit form to attach one, or a whole group, to this entity only.

## Smart fill

When agents are enabled, **Smart fill** on the edit form takes pasted text, such as a supplier's product description, and proposes values for the current context. The proposals go into the form; review them and save as usual. Nothing is saved automatically.

## History

**Changes** lists every change to the entity: what changed, in which context, when, and by whom. Changes made by an agent show the agent and the person who approved them.

Previous attribute values are kept for 90 days by default. From the value history you can restore an earlier value; the restore is recorded as a new change.

## Data quality findings

If [data quality rules](/builders/rules/) flag the entity, its page shows the number of open findings.

## Upgrade to a newer revision

When the blueprint has a newer published revision, the entity shows **Schema is outdated** and offers **Upgrade entity**. The upgrade page compares the entity's values with the new revision and asks for anything missing. See [Revisions and migration](/builders/revisions/).

## Publish

If your workspace uses publication channels, the entity page shows its status per channel and **Publish** / **Unpublish** actions. See [Publishing](/guides/publishing/).

## Ask an agent

**Ask about this entity** starts an [agent conversation](/guides/agents/) about the entity you are looking at.

## Delete an entity

**Delete entity** is available with the `entities.delete` permission, from the entity toolbar or the Explorer row menu. A deleted entity disappears from search, previews, and relationship pickers. Its history is kept.

## From the command line

Everything on this page is also available through the [CLI](/reference/cli/) and [API](/reference/api/). For example:

```sh
acli entity create --blueprint product --values product.toml
acli entity update <entity-id> --values changes.toml --context-id <context-id>
acli entity value-history <entity-id>
acli entity restore-value <entity-id> <history-id>
acli file upload <entity-id> main_photo --file photo.jpg
```
