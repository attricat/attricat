# Relationship Tree Facets

The entity explorer can filter a search through a hierarchical relationship and
show a count beside each node. A product/category catalogue is the usual case:
`product.categories` points to category entities, and a self-targeting category
relationship such as `category.parent` supplies the tree edges.

## Use In The Explorer

1. Search an entity blueprint.
2. Under **Relationship facet**, select a relationship field, such as
   `categories`.
3. The explorer loads that relationship's target blueprint and discovers its
   self-targeting relationship fields. It uses the first field by default.
4. Select category nodes to restrict results. Selecting a parent includes its
   descendants.

**Tree options** is collapsed by default. It lets users select a different
self-targeting field under **Build tree using** and select the relationship
**Context**. Changing either option clears selected nodes because the tree or
effective relationship set may have changed.

The source field, hierarchy field, context code, and selected node IDs are in
the explorer URL. Reloading or sharing that URL restores the facet.

## Blueprint Requirements

The source attribute must be a relationship with a target blueprint. The
hierarchy field must be a relationship on that target blueprint that targets
the same blueprint.

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
without a self-targeting relationship cannot be rendered as a tree facet.

## Context Semantics

The selected context applies to both category parent edges and source-to-category
relationships. Values are resolved through the selected context's ancestor
chain, respecting each attribute's `context_fallback` policy. This ensures the
displayed tree and the filter use the same effective relationships.

The main result table does not otherwise become a context-specific explorer;
the context is scoped to this facet.

## Counts And Selection

Counts are the number of distinct result entities assigned to each category or
one of its descendants. A result assigned to more than one category contributes
only once to a node's roll-up count.

Counts respect the text query and selected blueprint version, but intentionally
do not apply the category selection itself. This lets users see useful sibling
counts after choosing a category. Other search filters, once supported, should
also be applied before the facet is aggregated.

Selection is an OR over the selected nodes and their descendants. Selecting
both a parent and child does not duplicate results.

## API Contract

Add `relationship_tree_facet` to `POST /v1/entities/search`:

```json
{
  "blueprint": { "code": "product" },
  "query": "linen",
  "filters": [],
  "relationship_tree_facet": {
    "source_relationship_field": "categories",
    "hierarchy_field": "parent",
    "context_id": "00000000-0000-4000-8000-000000000001",
    "selected_target_ids": ["e8b7a8d3-c954-4c0f-b658-0f686ba466a3"]
  },
  "page": { "size": 25, "cursor": null }
}
```

The response retains `items` and `next_cursor` and adds an optional flat facet
graph. The client uses `parent_ids` to render it as a tree.

```json
{
  "relationship_tree_facet": {
    "items": [
      {
        "id": "e8b7a8d3-c954-4c0f-b658-0f686ba466a3",
        "parent_ids": [],
        "display": "Clothing",
        "count": 42
      }
    ]
  }
}
```

## Performance

The API loads category nodes and effective relationship edges in bulk. It does
not issue a request per tree node. It aggregates the complete matching result
set rather than the current page, so counts are stable across pagination.

Large taxonomies should retain indexes for relationship target lookups. The
current relationship target index supports the facet's source and parent edge
queries. If production measurements show repeated hierarchy traversal is a
bottleneck, add a maintained category-ancestry closure table rather than
issuing N+1 queries.
