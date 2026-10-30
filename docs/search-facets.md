# Relationship Tree Facets

The record explorer can filter a search through a hierarchical relationship and
show a count beside each node. A product/category catalogue is the usual case:
`product.categories` points to category records, and a self-targeting category
relationship such as `category.parent` supplies the tree edges.

## Use In The Explorer

On `lg` screens the Explorer shows a **Browse by** rail beside the results
(`ExplorerBrowseRail`). It lists the targets of one relationship field of the
selected blueprint, or of the last hop of a relationship path, through the
children endpoint below, and loads a node's children when it is expanded.

1. Candidate fields are relationships with a single target blueprint. A field
   is a tree when its target has a self-targeting relationship with
   `hierarchy` set (`tree` preferred over `acyclic`); otherwise the rail is a
   one-level list. Plain self-relationships are never treated as trees.
   The selected blueprint's own hierarchy field is its **own tree**: the rail
   labels it with the blueprint's name and lists the blueprint's records.
   After the direct fields, the picker offers relationship paths such as
   `model.brand`, labelled by their hops ("Model → Brand"), built by
   `useBrowseBy` from the target blueprints. A path follows up to three
   relationships that each have a single target blueprint and
   `cardinality: one`, are not hidden from Explorer, and never return to a
   blueprint the path has passed. At most 15 paths are offered, shortest
   first, plus the URL, stored and blueprint-default choices when they are
   paths beyond that. A path is a tree when its last target has a hierarchy.
2. The field comes from the URL's `browseBy`, then the person's last choice for
   the blueprint (local storage, keyed by blueprint ID), then the blueprint's
   `views.explorer.browse_by`, then the own tree, then the first direct tree
   field (a path opens the rail only when one of the earlier sources names
   it). `browseBy: false`
   in the URL or storage, or `browse_by: false` in the blueprint, hides the
   rail. A field the current revision no longer has falls through to the
   next source.
3. Choosing a node writes a `relationshipFacets` entry with one selected ID.
   For a tree field the entry also stores `hierarchyField`, and the search
   sends it as a `relationship_tree_facets` item so descendants match; entries
   without one are sent as exact `relationship_filters`. An own-tree entry
   also stores `ownTree: true` and is sent without `source_relationship_field`.
   For a path, the entry's `field` is the dotted path, which the API resolves
   hop by hop in both cases. The selection shows as a filter pill and is kept
   by saved views and share links like other facets; saved states also keep
   `browseBy`, `hierarchyField` and `ownTree`.

Tree edges and facet assignments resolve in the Explorer's selected context.

## Blueprint Requirements

The source attribute must be a relationship with a target blueprint. A
hierarchy field, when supplied, must be a relationship on that target blueprint
that targets the same blueprint. Without one, the facet is a one-level picker.

A source may also be a relationship path of up to three hops, such as
`model.brand` on a vehicle whose `model` targets `model` and whose model's
`brand` targets `brand`. Every hop must be a relationship with a single target
blueprint and `cardinality: one`, the rule for sortable relationship table
paths, so each source reaches at most one node. Hops after the first are read
from the latest published revision of the previous hop's target. The last
hop's target holds the facet's nodes and supplies the hierarchy field.

```yaml
attributes:
  # Product
  - code: categories
    value_type: relationship
    target_blueprint: category
```

```yaml
attributes:
  # Category
  - code: parent
    value_type: relationship
    target_blueprint: category
```

The API validates these conditions for every facet request. A target blueprint
without a self-targeting relationship is rendered as a one-level facet.

## Context Semantics

The selected context applies to both category parent edges and source-to-category
relationships. Values are resolved through the selected context's ancestor
chain, respecting each attribute's `context_fallback` policy. This ensures the
displayed tree and the filter use the same effective relationships. On a path,
every hop resolves this way on its own: a vehicle's model and that model's
brand may each come from a different context on the chain, and a hop with
`context_fallback: none` reads only the selected context.

The facet's context is chosen separately from the search's `context_code`,
which resolves the result table, filters, and sorting (see
[View Configuration](views.md#table-context)).

## Counts And Selection

Counts are the number of distinct result records assigned to each category or
one of its descendants. A result assigned to more than one category contributes
only once to a node's roll-up count.

Counts respect the text query and selected blueprint version, but intentionally
do not apply the category selection itself. This lets users see useful sibling
counts after choosing a category. Attribute filters apply to the main Explorer
search but are not included in the separately loaded facet-child request, so
they do not currently affect facet counts.

Without a search term, counts are cached for the workspace, per source
blueprint, version, field or path, hierarchy field and context, and may lag record
changes by about 30 seconds. The next request after that reloads them, and
requests arriving during the reload keep receiving the previous counts. Nodes themselves are always
current: a new or moved category appears at once, with the count it had when
the counts were cached (0 for a new one). Counts for a search term are exact.

Selection is an OR over the selected nodes and their descendants within a
facet. Selections across different facets are combined with AND semantics.
Selecting both a parent and child does not duplicate results.

## API Contract

Add `relationship_tree_facets` to `POST /v1/records/search`:

```json
{
  "blueprint": { "code": "product" },
  "query": "linen",
  "filters": [],
  "relationship_tree_facets": [
    {
      "source_relationship_field": "categories",
      "hierarchy_field": "parent",
      "context_id": "00000000-0000-4000-8000-000000000001",
      "selected_target_ids": ["e8b7a8d3-c954-4c0f-b658-0f686ba466a3"]
    },
    {
      "source_relationship_field": "color",
      "context_id": "00000000-0000-4000-8000-000000000001",
      "selected_target_ids": ["2a036fb3-5a05-4f34-9a7d-9450bd1bd7d2"]
    }
  ],
  "page": { "size": 25, "cursor": null }
}
```

`source_relationship_field` may be a dotted relationship path, as in
`relationship_filters` and table columns: `"model.brand"` lists brands and
counts the vehicles that reach each one through their model. A path with more
than three hops, a hop that is not a relationship with a single target, or a
hop of a multi-hop path without `cardinality: one` is rejected with
`422 invalid_input`. Single-field requests are unchanged.

Omit `source_relationship_field` to facet the searched blueprint by its own
`hierarchy_field`, which is then required. Each record is its own node, so a
selection matches the selected records and their descendants, and a node's
count includes the node itself.

The search response retains `items` and `next_cursor`. The facet tree is loaded
separately so searching does not transfer an entire taxonomy.

```json
{
  "items": []
}
```

`POST /v1/records/facets/relationship-tree/children` accepts the search
blueprint/query, facet fields and context, plus an optional `hierarchy_field`,
`parent_id`, and cursor. Omit `hierarchy_field` for a one-level picker. It
returns one configured-size page of direct children with `count`,
`has_children`, and `next_cursor`. The client requests a page only when a node
is expanded and uses `next_cursor` for that node's **Load more** action. With
`selected_target_ids`, the response also has `selected_paths`: for each selected
node, the IDs from a root down to the node itself, along the shortest way up and
the lowest parent IDs among equally short ones. A node whose every way up loops
follows its lowest parents until one repeats. `include_ids` names nodes of the
requested level to return even when they fall outside the page; they follow the
page's items and do not move `next_cursor`, and IDs outside the level are
ignored. Nodes of the level on a selected path are included the same way. The
Explorer's browse panel sends the selection with the root level when it opens,
then includes each path's node in the deeper levels, so a filter loaded from the
URL shows its node expanded into view however many siblings come before it.

## Performance

Counts cover complete matching result sets, so they stay stable across record
pagination, but the API only returns the requested root or child page. A page
reads the tree's parent edges fresh and takes counts from the cache described
in [Counts And Selection](#counts-and-selection); with a search term, it
resolves every matching source edge and counts the page's subtrees in memory.
For a relationship path, counting resolves each hop's edges with one query per
hop and joins them in memory. Without a search term the hops are read
concurrently in full. With a term matching at most 20,000 sources, the hops are
read in turn, each only for the records the hop before reached, so a large
middle hop costs in proportion to the matches; a broader term reads every hop
in full and keeps the matching sources. A selection walks down the selected
subtrees and then back one hop per query through the target-side indexes.
`apps/api/tests/facet_latency.rs` measures a direct field, a two-hop path with
a small middle hop, and two- and three-hop paths whose middle hop has a record
per source (see `perf/facets.md`). `RELATIONSHIP_FACET_MAX_NODES` sets the server-side maximum number of
nodes returned for each page (default: 100).

Large taxonomies should retain indexes for relationship target lookups. The
current relationship target index supports the facet's source and parent edge
queries. If production measurements show repeated hierarchy traversal is a
bottleneck, add a maintained category-ancestry closure table rather than
issuing N+1 queries.
