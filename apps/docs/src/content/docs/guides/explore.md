---
title: Explore records
description: Find records with search, filters, relationship facets, and sorting, then save and share the result.
---

The **Record explorer** is the home page of Attricat. It lists the records of one blueprint at a time and lets you narrow them down with a search query, attribute filters, and relationship facets.

## Choose what to browse

Pick a blueprint from the selector, or from a shortcut in the sidebar. Workspace administrators decide which blueprints appear as sidebar shortcuts.

The **Version scope** selector controls which blueprint revisions you see:

- **Current** (the default) shows records on the latest published revision. If older records exist, a notice tells you how many are hidden and links to them.
- A specific older **Version** shows only records pinned to that revision.
- **All versions** shows everything. Add the **Schema** column and sort by it to see the oldest records first. See [Revisions and migration](/builders/revisions/).

## Search

Type in the search box and press Enter. A plain term such as `linen` matches values on the chosen blueprint's own attributes. It does not look inside related records.

To search further, qualify the term:

| Query | Finds |
| --- | --- |
| `linen` | Records with "linen" in any of their own text values. |
| `sku:ABC*` | Records whose `sku` starts with `ABC`. |
| `colors:red` | Records linked through `colors` to a record with "red" in any value. |
| `colors.name:red` | Same, but only the linked record's `name`. |
| `family.product_type.name:laptop` | Follows two relationships, then matches `name`. |
| `*:red` | Records with "red" on themselves or on anything linked within three steps. |
| `@id:ID-1,ID-2` | Exactly the listed records. |
| `colors.@id:ID-1,ID-2` | Records linked through `colors` to one of the listed records. |

Several terms separated by spaces must all match. The full grammar is in [Search syntax](/guides/search-syntax/).

Open **Search info** on a result to see why it matched, for example *red via 1 relationship*.

## Filters

**Add filter** narrows results by an attribute's value in the selected [context](#context). Operators depend on the type:

| Type | Operators |
| --- | --- |
| String | Equals, Contains, Starts with |
| Number, integer, date, datetime, time | Equals, Greater than, Greater than or equal, Less than, Less than or equal |
| Boolean | Equals |
| Status | Equals, chosen from the status labels |
| User or team | Equals a person or team, or **Assigned to me (or my teams)** |
| File | Presence only |

Every type also offers **Presence**: choose **Has a value** or **Not set**. For a file attribute, **Has a value** finds records with at least one attached file, and **Not set** finds records without one.

You can filter on an attribute of a related record too, such as `brand.name`, through up to three relationships.

You can also start a filter from the table: open a cell's menu and choose **Filter by**.

Every active filter shows as a pill under the search box. All filters, facets, and the search query must match together.

## Relationship facets

Each relationship attribute of the blueprint gets a facet in the sidebar. Select one or more targets to keep only records linked to them.

When the target blueprint has a relationship to itself, such as `category.parent`, the facet becomes a tree:

- Counts next to each node include everything below it. A record assigned to two subcategories is counted once.
- Selecting a parent also matches records assigned to its descendants.
- Several selections in one facet mean *any of these*. Selections in different facets must all match.
- Counts reflect the search query and version scope, but not the facet's own selection, so you can still see sibling counts after choosing a node. Attribute filters do not affect facet counts.

**Tree options** lets you pick which self-referencing field builds the tree and which context to resolve relationships in. Changing either clears the facet's selection.

## Context

The **Context** selector shows values as they resolve in a context: the context's own value if it has one, otherwise the value it inherits. See [Contexts](/guides/contexts/).

The table columns, the record label, filters, and sorting all use the selected context. For example, with a `pl` context selected, a translated name is shown, filtered, and sorted in Polish, and a record without a Polish name shows the inherited one. Text search matches values in every context.

If the selected context is a [publication channel](/guides/publishing/), the table gains a **Publication** column showing whether each record is published there. It can be sorted to put unpublished records first.

## Sort and arrange columns

Click a column header to sort. A column can be sorted when it holds a single scalar value per record. Columns that follow a many-valued relationship are displayed but cannot be sorted.

In **All versions**, sorting on a related value works only when every matching record is on the same revision.

**Columns** lets you show, hide, and reorder columns. The arrangement is saved in your browser.

## Save and share searches

Everything you set up in the Explorer is kept in the page URL, so reloading the page or sending the link restores it.

- **Save search** stores the search under a name. Choose **Private** to keep it to yourself, or **Workspace** to share it with every member who can read records. Open saved searches from the **Saved searches** list.
- Changing a saved search marks it as having unsaved changes. **Save changes** updates it; **Save search** saves a copy.
- A [solution pack](/builders/solution-packs/#rules-workflows-and-saved-searches) can add ready-made **Workspace** searches, such as review queues. They appear in the same list, and the person who applied the pack owns them.
- **Share search** copies a link. Long searches are stored as a snapshot and shared as a short link.

A link never grants access. The person opening it needs to be a member of the workspace with permission to read records, and sees only what their role allows.

## Act on several records

**Select records** turns on selection mode. You can select up to 50 records. The selection stays when you change the query, sort, or filters, so you can collect records from several searches of the same blueprint. Click **N selected** to review the list and remove records from it.

The **Actions** menu works on the selection:

- **Send to agent conversation** sends the records to an [agent conversation](/guides/agents/) with instructions.
- **Create saved search** saves a search that matches exactly these records.
- Installed extensions can add their own bulk actions.

## Extension actions and runs

An extension can offer an action, such as generating documents, from a record preview, from a row's menu, or for the current selection. These actions always use saved data. If you have unsaved edits for a selected record in the same browser tab, the dialog tells you and links to the editor so you can save first.

Starting the action creates a run in the background. You can close the dialog or leave the page; the run continues and you are notified when it finishes. Open **Profile → Extension runs** to follow your runs, cancel one that is still in progress, and download its results. The status shows whether the run finished; the extension reports separately how many records succeeded, failed, or were skipped.

Only you, and workspace members who manage extensions, can see your runs. You can download results only while you can still read every record in the run. Results become available when the run completes and are kept for 30 days.

## Open a record

Click a result to open its preview. From there you can edit it, see its history, or open related records. See [Work with records](/guides/records/).
