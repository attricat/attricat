---
title: Explore entities
description: Find entities with search, filters, relationship facets, and sorting, then save and share the result.
---

The **Entity explorer** is the home page of Attricat. It lists the entities of one blueprint at a time and lets you narrow them down with a search query, attribute filters, and relationship facets.

## Choose what to browse

Pick a blueprint from the selector, or from a shortcut in the sidebar. Workspace administrators decide which blueprints appear as sidebar shortcuts.

The **Version scope** selector controls which blueprint revisions you see:

- **Current** (the default) shows entities on the latest published revision. If older entities exist, a notice tells you how many are hidden and links to them.
- A specific older **Version** shows only entities pinned to that revision.
- **All versions** shows everything. Add the **Schema** column and sort by it to see the oldest entities first. See [Revisions and migration](/builders/revisions/).

## Search

Type in the search box and press Enter. A plain term such as `linen` matches values on the chosen blueprint's own attributes. It does not look inside related entities.

To search further, qualify the term:

| Query | Finds |
| --- | --- |
| `linen` | Entities with "linen" in any of their own text values. |
| `sku:ABC*` | Entities whose `sku` starts with `ABC`. |
| `colors:red` | Entities linked through `colors` to an entity with "red" in any value. |
| `colors.name:red` | Same, but only the linked entity's `name`. |
| `family.product_type.name:laptop` | Follows two relationships, then matches `name`. |
| `*:red` | Entities with "red" on themselves or on anything linked within three steps. |
| `@id:ID-1,ID-2` | Exactly the listed entities. |
| `colors.@id:ID-1,ID-2` | Entities linked through `colors` to one of the listed entities. |

Several terms separated by spaces must all match. The full grammar is in [Search syntax](/guides/search-syntax/).

Open **Search info** on a result to see why it matched, for example *red via 1 relationship*.

## Filters

**Add filter** narrows results by an attribute's value in the default context. Operators depend on the type:

| Type | Operators |
| --- | --- |
| String | Equals, Contains, Starts with |
| Number, integer, date, datetime, time | Equals, Greater than, Greater than or equal, Less than, Less than or equal |
| Boolean | Equals |

You can filter on an attribute of a related entity too, such as `brand.name`, through up to three relationships.

You can also start a filter from the table: open a cell's menu and choose **Filter by**.

Every active filter shows as a pill under the search box. All filters, facets, and the search query must match together.

## Relationship facets

Each relationship attribute of the blueprint gets a facet in the sidebar. Select one or more targets to keep only entities linked to them.

When the target blueprint has a relationship to itself, such as `category.parent`, the facet becomes a tree:

- Counts next to each node include everything below it. An entity assigned to two subcategories is counted once.
- Selecting a parent also matches entities assigned to its descendants.
- Several selections in one facet mean *any of these*. Selections in different facets must all match.
- Counts reflect the search query and version scope, but not the facet's own selection, so you can still see sibling counts after choosing a node. Attribute filters do not affect facet counts.

**Tree options** lets you pick which self-referencing field builds the tree and which context to resolve relationships in. Changing either clears the facet's selection.

## Context

The **Context** selector shows values as they resolve in a context: the context's own value if it has one, otherwise the value it inherits. See [Contexts](/guides/contexts/).

If the selected context is a [publication channel](/guides/publishing/), the table gains a **Publication** column showing whether each entity is published there. It can be sorted to put unpublished entities first.

## Sort and arrange columns

Click a column header to sort. A column can be sorted when it holds a single scalar value per entity. Columns that follow a many-valued relationship are displayed but cannot be sorted.

In **All versions**, sorting on a related value works only when every matching entity is on the same revision.

**Columns** lets you show, hide, and reorder columns. The arrangement is saved in your browser.

## Save and share searches

Everything you set up in the Explorer is kept in the page URL, so reloading the page or sending the link restores it.

- **Save search** stores the search under a name. Choose **Private** to keep it to yourself, or **Workspace** to share it with every member who can read entities. Open saved searches from the **Saved searches** list.
- Changing a saved search marks it as having unsaved changes. **Save changes** updates it; **Save search** saves a copy.
- **Share search** copies a link. Long searches are stored as a snapshot and shared as a short link.

A link never grants access. The person opening it needs to be a member of the workspace with permission to read entities, and sees only what their role allows.

## Act on several entities

**Select entities** turns on selection mode. You can select up to 50 entities. The selection stays when you change the query, sort, or filters, so you can collect entities from several searches of the same blueprint. Click **N selected** to review the list and remove entities from it.

The **Actions** menu works on the selection:

- **Send to agent conversation** sends the entities to an [agent conversation](/guides/agents/) with instructions.
- **Create saved search** saves a search that matches exactly these entities.
- Installed extensions can add their own bulk actions.

## Open an entity

Click a result to open its preview. From there you can edit it, see its history, or open related entities. See [Work with entities](/guides/entities/).
