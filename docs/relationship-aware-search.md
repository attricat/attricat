# Relationship-Aware Explore Search

> **Status: planned.** This document defines the implementation target for issue
> #123; it does not describe the behavior currently deployed.

Explore will find entities of the selected blueprint from values on the entity
itself and values on entities connected to it through relationships. It also
introduces a small, typed query language so that later search features extend a
query plan rather than ad-hoc SQL text matching.

The query pipeline is application-owned Rust code: it parses and validates the
query, plans every term, performs breadth-first traversal, and combines
candidate ID sets. PostgreSQL is used only for parameterized batched set reads;
no recursive SQL, database functions, triggers, or other database-resident
query behavior is introduced.

## Matching Model

Search produces candidate IDs for the selected blueprint, then applies the
existing version, system-tag, outdated, facet, ordering, and cursor filters.

For a free-text term, the server:

1. Finds non-relationship scalar values that match the term.
2. Includes matching entities of the selected blueprint (depth 0).
3. Traverses **incoming active relationship edges** in breadth-first batches:
   an edge from Product to Color allows a matching Color value to select that
   Product.
4. Stops after three edges, de-duplicates every frontier, and never revisits an
   entity. Deleted entities and inactive edges are excluded at every step.

The depth limit is server controlled, defaults to three, and has a hard
configuration cap. Intermediate ID sets may be large; traversal deliberately
uses set queries per breadth-first level rather than N+1 queries.

Every query term independently produces a candidate set. Multiple whitespace-
separated terms are intersected (implicit `AND`). Relationship-tree facet
counts, selected-facet filtering, and paginated result pages consume that same
candidate set, so they cannot disagree about which source entities match.

## Query Language

| Form | Meaning |
| --- | --- |
| `red` | Free text across the reachable entity graph. |
| `color:red` | Match values on entities reached through selected-blueprint relationship `color`. |
| `color.name:red` | As above, restricted to related attribute `name`. |
| `sku:123*` | Match selected-blueprint attribute `sku`. |
| `product.sku:123*` | Explicit selected-blueprint form of the preceding query. |

Selectors are validated against the selected blueprint revision before the
query executes:

- `relationship` must name a relationship on the selected blueprint.
- `relationship.attribute` must name an attribute on that relationship's target
  blueprint.
- `attribute` is selected-blueprint shorthand.
- `blueprint.attribute` is valid only when `blueprint` is the selected
  blueprint and the attribute exists there.

Unknown or incompatible names and malformed terms return a clear `400` input
validation error. `*` is a query-language wildcard only in a trailing position
and means prefix matching. Values are always passed to SQL as parameters;
query wildcards are never interpolated into SQL.

Explicit multi-hop selectors (such as `category.parent.name:summer`), boolean
operators other than implicit `AND`, ranking, and match explanations are out of
scope.

## API and UI Contract

`POST /v1/entities/search` continues to take the query in its existing `query`
string field. An empty or absent query preserves current browse behavior. The
Explore query field and URL query parameter preserve the supplied query text;
the UI will show examples of supported selector and wildcard forms. API errors
are rendered through the existing Explore error alert.

No client-selected traversal depth is introduced initially. The server owns the
default and hard cap so large traversals cannot be requested by a browser or
CLI client.

## Implementation Boundaries

The backend will parse into typed query terms, compile validated terms using
blueprint metadata, and invoke a shared candidate-ID resolver. The main entity
search and relationship-tree facet child/count paths must both invoke this
resolver. Direct-text predicates embedded in individual facet queries will be
removed so all paths share semantics.

No migration is required. Index and query-telemetry work is deferred until
representative catalogue data is available.

## Acceptance Coverage

Integration tests must cover direct and multi-hop matches, depth bounds, cycles,
deleted entities, inactive relationships, structured selectors, wildcard
matching, invalid syntax/selectors, AND intersections, pagination, and facet
counts/filtering. Frontend tests must cover URL parsing/submission and request
forwarding for structured query text.
