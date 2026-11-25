---
title: Search syntax
description: The Explorer query language, from plain terms to qualified relationship paths and wildcards.
---

The Explorer search box and the `query` field of `POST /v1/records/search` share one query language. Queries are checked against the blueprint before they run. A query that names an attribute or relationship that does not exist fails with a clear error instead of returning nothing.

## Terms

A query is one or more terms separated by spaces. A record must match **every** term.

```text
linen shirt
```

matches records that contain both "linen" and "shirt".

There is no `OR`, `NOT`, grouping, or quoting, so a term cannot contain a space.

## Plain terms

A term without a colon searches the chosen blueprint's own scalar attributes. It matches anywhere in a value and ignores case. Numbers, dates, and times match by their stored form, such as `2026-05`, so `4` also finds 40 and 14. To find an exact number, name its attribute: see [Numbers](#numbers). It does not follow relationships.

```text
linen
```

## Similar words

A text attribute tagged [`search:fuzzy`](/reference/blueprint/#search-tags) also matches values similar to a term of four letters or more, so a term finds other forms of the same word and small typos: `tarcze` finds "Tarcza tnąca 125 mm". A value that contains the term still matches as usual. The record's search info names the similar value. Terms ending in `*` and `*:` terms match exactly.

## Wildcards

A trailing `*` matches any ending:

```text
sku:ABC-12*
```

`*` is only allowed at the end of a term, and not in a term on a number or true/false attribute.

## Qualified terms

`selector:term` narrows where a term is matched. A selector is a dotted path.

| Form | Example | Matches |
| --- | --- | --- |
| `attribute:term` | `sku:ABC*` | The blueprint's own `sku`. |
| `blueprint:term` | `product:linen` | Any own attribute of the chosen blueprint. The blueprint can be named by code or display name, so `Produkt:lniana` also works. |
| `blueprint.attribute:term` | `product.sku:ABC*` | Same as `sku:ABC*`, written explicitly. |
| `relationship:term` | `colors:red` | Any attribute of a record linked through `colors`. |
| `relationship.attribute:term` | `colors.name:red` | The `name` of a record linked through `colors`. |
| `rel.rel.attribute:term` | `category.parent.name:summer` | Follows up to three relationships, then matches one attribute. |

Blueprint display names are matched without regard to case. If two blueprints share a display name, use the code.

The last part of a longer path must be a scalar attribute, not a relationship, file, or JSON attribute. A term can contain only one colon.

A relationship can link to records of several blueprints, such as an `item` that is either a product or a material. A term through it matches the linked records of all of them: `item:steel` searches every attribute of the linked product or material, and `item.name:steel` searches `name` on those that have it. An attribute only one of them has, such as `item.sku:HK*`, matches only that blueprint's records. A path can continue through such a relationship into a relationship its blueprints declare alike: `item.brand.name:acme` follows `brand` on the products that have it. If the blueprints declare the next part differently, for example as a relationship to different blueprints, the term is rejected.

When a one-part selector could be either the blueprint's name or one of its attributes or relationships, the blueprint wins.

## Numbers

A qualified term on a `number` or `integer` attribute compares the value as a number instead of matching its text. `width_mm:4` finds 4 and 4.0, but not 40 or 14.

| Form | Example | Matches |
| --- | --- | --- |
| `attribute:n` | `width_mm:4` | Values equal to 4. `width_mm:=4` is the same. |
| `attribute:>n` | `width_mm:>4` | Values greater than 4. |
| `attribute:>=n` | `width_mm:>=4` | 4 and greater. |
| `attribute:<n` | `width_mm:<4` | Values less than 4. |
| `attribute:<=n` | `width_mm:<=4` | 4 and less. |
| `attribute:from..to` | `width_mm:40..60` | 40 to 60, both included. |

Write decimals with a point or a comma: `width_mm:2,5` is the same as `width_mm:2.5`. The forms also work at the end of a relationship path, such as `spec.width_mm:40..60`. Through a relationship to several blueprints, the term compares the values of those that have the attribute, which must all declare it with the same type.

An `integer` attribute takes whole numbers only, so `qty:2,5` is rejected. A value that is not a number, such as `width_mm:abc`, a wildcard, such as `width_mm:4*`, or a range that starts after it ends, such as `width_mm:60..40`, is rejected too.

A `boolean` attribute matches `true` or `false` exactly, in any case: `in_stock:true`.

Plain terms and `*:` terms name no attribute, so they still match numbers by their text. Dates, times, and timestamps match by their text in every term, so `due:2026-05` finds dates in May 2026.

## Record IDs

`@id:` matches records by ID instead of by value. List several IDs separated by commas, without spaces. A record matches when its ID is any one of them:

```text
@id:0190a6f2-7c1e-7b3a-9c4d-2e5f6a7b8c9d,0190a6f2-7c1e-7b3a-9c4d-2e5f6a7b8c9e
```

Put `@id` at the end of a relationship path to match records linked to one of the listed IDs:

| Form | Example | Matches |
| --- | --- | --- |
| `@id:ids` | `@id:…9c9d,…9c9e` | The chosen blueprint's records with one of these IDs. |
| `blueprint.@id:ids` | `product.@id:…9c9d` | Same as `@id:…`, written explicitly. |
| `relationship.@id:ids` | `colors.@id:…4a1b,…4a1c` | Records linked through `colors` to one of these records. |
| `rel.rel.@id:ids` | `category.parent.@id:…77e0` | Follows up to three relationships, then matches the linked record's ID. |

A term lists at most 100 IDs, and wildcards are not allowed. IDs that do not exist, or that belong to another blueprint, match nothing. Other terms still apply, so `@id:… linen` keeps only the listed records that also contain "linen".

Because the list is part of the query, saving the search as a [saved search](/guides/explore/#save-and-share-searches) keeps a fixed selection of records.

## Global search

`*:term` searches every blueprint, then walks back along relationships, up to three steps, to find records of the chosen blueprint connected to a match.

```text
*:red
```

finds products that are red themselves, products linked to a color named red, and products linked to a family linked to something red.

Global search is the most expensive query form. It has server-side limits on how many values, records, and links it will examine and a 250 ms time limit on each database read. If a query exceeds them, the request fails with `422` rather than returning partial results. Narrow the query with a qualified selector when that happens.

## Why did this match?

Each result includes a match explanation per term: the record and attribute that matched, and the relationship path from the result to it. In the Explorer, open **Search info** on a result.

## Combining with filters and facets

The query, [attribute filters](/guides/explore/#filters), [relationship facets](/guides/explore/#relationship-facets), version scope, and system-tag filters all narrow the same result set. Facet counts reflect only the query and version scope. Attribute filters and other facets do not change them, so a count can be higher than the filtered result list.
