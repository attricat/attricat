---
title: Search syntax
description: The Explorer query language, from plain terms to qualified relationship paths and wildcards.
---

The Explorer search box and the `query` field of `POST /v1/entities/search` share one query language. Queries are checked against the blueprint before they run. A query that names an attribute or relationship that does not exist fails with a clear error instead of returning nothing.

## Terms

A query is one or more terms separated by spaces. An entity must match **every** term.

```text
linen shirt
```

matches entities that contain both "linen" and "shirt".

There is no `OR`, `NOT`, or grouping.

## Plain terms

A term without a colon searches the text of the chosen blueprint's own scalar attributes. It does not follow relationships.

```text
linen
```

## Wildcards

A trailing `*` matches any ending:

```text
sku:ABC-12*
```

`*` is only allowed at the end of a term.

## Qualified terms

`selector:term` narrows where a term is matched. A selector is a dotted path.

| Form | Example | Matches |
| --- | --- | --- |
| `attribute:term` | `sku:ABC*` | The blueprint's own `sku`. |
| `blueprint:term` | `product:linen` | Any own attribute of the chosen blueprint. The blueprint can be named by code or display name, so `Produkt:lniana` also works. |
| `blueprint.attribute:term` | `product.sku:ABC*` | Same as `sku:ABC*`, written explicitly. |
| `relationship:term` | `colors:red` | Any attribute of an entity linked through `colors`. |
| `relationship.attribute:term` | `colors.name:red` | The `name` of an entity linked through `colors`. |
| `rel.rel.attribute:term` | `category.parent.name:summer` | Follows up to three relationships, then matches one attribute. |

Blueprint display names are matched without regard to case. If two blueprints share a display name, use the code.

When a one-part selector could be either the blueprint's name or one of its relationships, the blueprint wins.

## Entity IDs

`@id:` matches entities by ID instead of by value. List several IDs separated by commas, without spaces. An entity matches when its ID is any one of them:

```text
@id:0190a6f2-7c1e-7b3a-9c4d-2e5f6a7b8c9d,0190a6f2-7c1e-7b3a-9c4d-2e5f6a7b8c9e
```

Put `@id` at the end of a relationship path to match entities linked to one of the listed IDs:

| Form | Example | Matches |
| --- | --- | --- |
| `@id:ids` | `@id:…9c9d,…9c9e` | The chosen blueprint's entities with one of these IDs. |
| `blueprint.@id:ids` | `product.@id:…9c9d` | Same as `@id:…`, written explicitly. |
| `relationship.@id:ids` | `colors.@id:…4a1b,…4a1c` | Entities linked through `colors` to one of these entities. |
| `rel.rel.@id:ids` | `category.parent.@id:…77e0` | Follows up to three relationships, then matches the linked entity's ID. |

A term lists at most 100 IDs, and wildcards are not allowed. IDs that do not exist, or that belong to another blueprint, match nothing. Other terms still apply, so `@id:… linen` keeps only the listed entities that also contain "linen".

Because the list is part of the query, saving the search as a [saved search](/guides/explore/#save-and-share-searches) keeps a fixed selection of entities.

## Global search

`*:term` searches every blueprint, then walks back along relationships, up to three steps, to find entities of the chosen blueprint connected to a match.

```text
*:red
```

finds products that are red themselves, products linked to a color named red, and products linked to a family linked to something red.

Global search is the most expensive query form. It has server-side limits on how many values, entities, and links it will examine and a 250 ms time budget. If a query exceeds them, the request fails with `422` rather than returning partial results. Narrow the query with a qualified selector when that happens.

## Why did this match?

Each result includes a match explanation per term: the entity and attribute that matched, and the relationship path from the result to it. In the Explorer, open **Search info** on a result.

## Combining with filters and facets

The query, [attribute filters](/guides/explore/#filters), [relationship facets](/guides/explore/#relationship-facets), version scope, and system-tag filters all narrow the same result set. Facet counts are computed from the same matches, so a count never disagrees with the result list.
