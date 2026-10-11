# Relationship Tree Facets

The record explorer can filter a search through a hierarchical relationship and
show a count beside each node. A product/category catalogue is the usual case:
`product.categories` points to category records, and a self-targeting category
relationship such as `category.parent` supplies the tree edges.

## Use In The Explorer

On `lg` screens the Explorer shows a **Browse by** rail beside the results
(`ExplorerBrowseRail`). It lists the targets of one direct relationship field of
the selected blueprint, through the children endpoint below, and loads a node's
children when it is expanded.

1. Candidate fields are relationships with a single target blueprint. A field
   is a tree when its target has a self-targeting relationship with
   `hierarchy` set (`tree` preferred over `acyclic`); otherwise the rail is a
   one-level list. Plain self-relationships are never treated as trees.
2. The field comes from the URL's `browseBy`, then the person's last choice for
   the blueprint (local storage, keyed by blueprint ID), then the blueprint's
   `views.explorer.browse_by`, then the first tree field. `browseBy: false`
   in the URL or storage, or `browse_by = false` in the blueprint, hides the
   rail. A field the current revision no longer has falls through to the
   next source.
3. Choosing a node writes a `relationshipFacets` entry with one selected ID.
   For a tree field the entry also stores `hierarchyField`, and the search
   sends it as a `relationship_tree_facets` item so descendants match; entries
   without one are sent as exact `relationship_filters`. The selection shows as
   a filter pill and is kept by saved views like other facets.

Tree edges and facet assignments resolve in the Explorer's selected context.

## Blueprint Requirements

The source attribute must be a relationship with a target blueprint. A
hierarchy field, when supplied, must be a relationship on that target blueprint
that targets the same blueprint. Without one, the facet is a one-level picker.

```toml
# Product
[[attributes]]
code = "categories"
value_type = "relationship"
target_blueprint = "category"
```

```toml
# Category
[[attributes]]
code = "parent"
value_type = "relationship"
target_blueprint = "category"
```

The API validates these conditions for every facet request. A target blueprint
without a self-targeting relationship is rendered as a one-level facet.

## Context Semantics

The selected context applies to both category parent edges and source-to-category
relationships. Values are resolved through the selected context's ancestor
chain, respecting each attribute's `context_fallback` policy. This ensures the
displayed tree and the filter use the same effective relationships.

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
is expanded and uses `next_cursor` for that node's **Load more** action.

## Performance

The API aggregates complete matching result sets in PostgreSQL so counts remain
stable across record pagination, but only returns the requested root or child
page. `RELATIONSHIP_FACET_MAX_NODES` sets the server-side maximum number of
nodes returned for each page (default: 100).

Large taxonomies should retain indexes for relationship target lookups. The
current relationship target index supports the facet's source and parent edge
queries. If production measurements show repeated hierarchy traversal is a
bottleneck, add a maintained category-ancestry closure table rather than
issuing N+1 queries.
