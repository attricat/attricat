---
title: Explore entities
description: Find, inspect, and maintain catalog records.
---

Open **Entity explorer** to browse the entities available in your workspace.

## Find records

Choose a blueprint to start exploring its entities. The version scope defaults to the current published revision; select an older published revision to inspect and sort it, or select **All versions** for a combined view. When current entities are shown, an informational notice links to hidden older entities and, when permitted, migration health.

A bare search term matches scalar values on that blueprint only. Use `*:red` to explicitly search connected records through up to three relationship edges. Queries can target a field, for example `sku:123*`, or explicitly search a related scalar through up to three relationships, for example `family.product_type.name:red`.

Use the context selector to see values as they resolve in a particular context. A value can come directly from that context or be inherited from its default value.

## Filter records

Relationship filters narrow results using connected records and hierarchies. Attribute filters appear directly below them and operate on scalar values in the default context. String attributes support equality, contains, and starts-with filters. Numbers, integers, dates, date-times, and times support equality and range comparisons; booleans support equality.

Related-value sorting is available for a selected revision when each relationship hop is single-valued. In **All versions**, it is available only when the complete matching result set uses one source revision; otherwise the header explains why sorting is unavailable.

Applied attribute and relationship filters appear as removable pills below the search box. Relationship pills use the target blueprint's dropdown-option view to name selected records. When that view cannot produce labels, the pill falls back to the number of selected records. Multiple filters are combined with each other and the text query, so every active condition must match. Filter selections remain in the page URL while editing and can be restored after a refresh. Use **Save search** to create a named, short `?savedView=<id>` link. Choose Private for a personal search or Workspace for one other authorized members can open. Select a saved search from the Explorer dropdown. Editing a saved search creates a draft URL; use **Save changes** to update the original or **Save search** to create a copy. **Copy link** copies the inline URL for small searches; for long searches it stores a snapshot and copies a short `?viewState=<id>` URL. Opening a link requires workspace membership and catalog read permission; sharing a link does not grant data access.

## Work with an entity

Open a result to inspect its values, schema revision, files, relationships, and change history. If you can edit the entity, save changes from its detail page. Read-only fields remain visible but are managed by a system integration rather than the browser. Users with `entities.delete` permission can delete an entity from its preview toolbar or an Explorer result's actions menu. Deletion requires confirmation, hides the entity from normal reads and search results, and retains its history.

When the entity's blueprint has a newer published revision, Attricat identifies the record as outdated. Review the migration before upgrading so changes to fields or validation are deliberate.

See [Contexts](/guides/contexts/) for how inherited values work.
