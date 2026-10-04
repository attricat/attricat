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
- **Locked while the record is *X*** means the record's status freezes this field. See [Controlled records](#controlled-records).

Saving validates the whole entity in every context. If a change would make any context invalid, nothing is saved and the form points to the problem. See [Validation](/builders/validation/).

The blueprint can also have checks, such as "valid until must not be before valid from" or "every facility belongs to this supplier", and your workspace can enforce data quality rules. When one fails, the message names the check or rule and says what is wrong. Change the fields it mentions, in the contexts it lists, and save again. If it points to a linked record, such as a facility that is not approved, fix or replace that record first.

### Unsaved drafts

The edit form keeps your unsaved changes for the browser tab. If you reload or come back, Attricat offers **Restore draft** or **Discard draft**. A draft is cleared once you save. Drafts never contain passwords or files.

## Change a status

A status field, such as *Draft*, *In review*, or *Released*, is a select. A blueprint can limit which statuses can follow the current one and can set conditions for a change. You might not be able to choose or save a status for one of these reasons:

- **The option is disabled.** The blueprint does not allow that change from the current status. For example, a released document may have to go back to *Draft* before it can be reviewed again. Move through the allowed statuses instead.
- **The option is disabled and shows *Blocked*** with the unmet conditions. The change is allowed, but the saved entity does not meet its conditions yet. Fix what they ask for and save, then choose the status.
- **Saving is rejected** with a message such as *status transition conditions are not met: Set an approver before release (approver-set)*. The change is allowed, but the values you are saving do not meet its conditions. A similar message starting *enforcing rules are violated* means a data quality rule guards the change. Nothing is saved.

To recover, read the listed conditions and fix what they ask for. Missing values can go in the same save as the status change: fill in the approver and choose *Released* together. Some conditions depend on other records, for example "every corrective action is closed"; update those records first, then change the status again.

Conditions are checked in every context where the status changes. If the status is inherited, a change in the default context must satisfy the conditions in every child context too.

To see which statuses are available and why others are blocked, without trying to save, call `GET /v1/entities/{id}/status-transitions` with an optional `context_id`. Each transition shows `allowed`, a `denial_reason` when blocked, and the `unmet` conditions.

## Relationships

Relationship fields open a picker. Search for targets, **Select** them, and **Apply**. Single-select relationships replace the current target. Each option can be previewed in a new tab.

Relationship values are contextual, like any other value: a product can have a different set of categories in one market.

## Files

File fields accept uploads once the entity has been saved. Choose or drop files; each one is checked against the attribute's rules for type, extension, and size.

A new file goes through these statuses: **Uploading**, **Queued**, **Processing**, and **Ready**. Images get a thumbnail and a display-sized version. A file can be downloaded once it is ready. If processing fails, the file shows **Failed**; an administrator can retry it.

Image-only file fields show an image gallery. Select an image to open a larger preview with zoom and next/previous controls. On the edit form you can add and remove images, and move them earlier or later when the attribute's file order is meaningful. Removing an image detaches it from the entity.

## Controlled records

Some blueprints use statuses to control a record's lifecycle. Then:

- The status select disables the transitions you may not make and says why: the transition needs a permission or role you do not have, or someone other than you must make it (for example, the person who submitted a document cannot approve it).
- In a finalized status, such as *Released*, some or all fields are read-only, files cannot be added or removed, and the entity cannot be deleted. To correct the record, change its status with the correction transition the blueprint provides, save, and then edit. The correction is recorded in the audit log.
- When a record is approved, the approval is tied to the exact content that was reviewed. Editing that content voids the approval, and the record returns to an earlier status in the same save.
- Files of finalized records can be kept under a retention hold until a set date.

The **Record control** panel on the entity page lists approvals, with who approved, when, and whether they are still valid, and the retention holds on the entity's files.

## Comments

The entity page has a **Comments** panel. Anyone who can read the entity can add a comment, written in Markdown; **Preview Markdown** shows how it will look. You can edit only your own comments. If someone else's change reaches a comment before your edit is saved, the edit is rejected; reload the comments and restore your draft to reconcile it.

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

**Delete entity** is available with the `entities.delete` permission, from the entity toolbar or the Explorer row menu. A deleted entity disappears from search, previews, and relationship pickers. Its history is kept. A record in a locked status cannot be deleted.

## From the command line

Everything on this page is also available through the [CLI](/reference/cli/) and [API](/reference/api/). For example:

```sh
acli entity create --blueprint product --values product.toml
acli entity update <entity-id> --values changes.toml --context-id <context-id>
acli entity value-history <entity-id>
acli entity restore-value <entity-id> <history-id>
acli file upload <entity-id> main_photo --file photo.jpg
```
