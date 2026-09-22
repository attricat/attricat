# Relationship-Aware Explore Search

> **Status: deployed.** This document describes the relationship-aware search
> behavior used by the Explorer and `POST /v1/entities/search`.

Explore finds entities of the selected blueprint from values on the entity
itself and values on entities connected to it through relationships. It also
introduces a small, typed query language so that later search features extend a
query plan rather than ad-hoc SQL text matching.

The query pipeline is application-owned Rust code: it parses and validates the
query, plans every term, performs breadth-first traversal, and combines
candidate ID sets. PostgreSQL is used only for parameterized batched set reads in the
relationship-aware candidate resolver; it introduces no database functions,
triggers, or other database-resident query behavior. Relationship-facet tree
queries may use recursive CTEs for hierarchy traversal.

## Matching Model

Search produces candidate IDs for the selected blueprint, then applies the
existing version, system-tag, outdated, facet, ordering, and cursor filters.

For a bare free-text term, the server searches only non-relationship scalar
values on entities of the selected blueprint (depth 0). It does not traverse
relationships.

A relationship-qualified term, such as `color.name:red` or
`family.product_type.name:graphics`, first finds matching scalar values on the
path's target blueprint, then traverses up to three named **incoming active
relationship edges** in batched steps:
an edge from Product to Color allows a matching Color value to select that
Product. Deleted entities and inactive edges are excluded.

The `*:` selector explicitly opts into global relationship-aware discovery. It
first finds matching scalar values across blueprints, then traverses incoming
active relationship edges in breadth-first batches for up to three edges before
returning selected-blueprint entities. Intermediate ID sets use batched set
reads rather than N+1 queries.

Every query term independently produces a candidate set. Multiple whitespace-
separated terms are intersected (implicit `AND`). Relationship-tree facet
counts, selected-facet filtering, and paginated result pages consume that same
candidate set, so they cannot disagree about which source entities match.

The resolver retains a match witness for every accepted result and term. The
search response includes `match_explanations` to each item: an array
with one deterministic witness per matched term containing the original term,
the matching entity ID, matching attribute code when applicable, traversal
depth, and the relationship-edge path from the returned entity to that match.
Depth `0` has an empty path. This data lets clients show why a record matched
without rerunning the search; it is explanatory metadata, not ranking input.

## Query Language

| Form | Meaning |
| --- | --- |
| `red` | Free text across scalar values on the selected blueprint only. |
| `*:red` | Explicit global relationship-aware search through up to three incoming edges. |
| `color:red` | Match values on entities reached through selected-blueprint relationship `color`. |
| `color.name:red` | As above, restricted to related attribute `name`. |
| `Produkt:czerwony` | Free text limited to the selected blueprint, identified by its user-specified name (its code, `product:czerwony`, also works). |
| `sku:123*` | Match selected-blueprint attribute `sku`. |
| `product.sku:123*` | Explicit selected-blueprint form of the preceding query; `product` may be its code or user-specified name. |

Selectors are validated against the selected blueprint revision before the
query executes:

- A one-part selector matching the selected blueprint's code or user-specified
  name (for example `Produkt:czerwony`) scopes free-text matching to that
  blueprint's own scalar values. This selected-blueprint alias takes precedence
  over a same-named relationship field.
- Otherwise, `relationship` must name a relationship on the selected blueprint.
- `relationship.attribute` must name an attribute on that relationship's target
  blueprint.
- `attribute` is selected-blueprint shorthand.
- `blueprint.attribute` is valid only when `blueprint` identifies the selected
  blueprint by either its code or its user-specified name, and the attribute
  exists there. Blueprint-name matching is case-insensitive; if multiple active
  blueprints share that name, the selector is rejected as ambiguous and the
  caller must use the blueprint code.

Unknown, ambiguous, or incompatible names and malformed terms return a clear
`400` input validation error. `*` is a query-language wildcard only in a trailing position
and means prefix matching. Values are always passed to SQL as parameters;
query wildcards are never interpolated into SQL.

Explicit selectors may contain up to three relationship hops followed by a
scalar leaf, such as `category.parent.name:summer`. Deeper selectors, implicit
relationship traversal, boolean operators other than implicit `AND`, and ranking
are out of scope. `*:` remains the only global multi-hop mode.

## API and UI Contract

`POST /v1/entities/search` continues to take the query in its existing `query`
string field. An empty or absent query preserves current browse behavior. Each
non-empty-query result additionally returns `match_explanations` as described
above. The Explore query field and URL query parameter preserve the supplied
query text; the UI will show examples of supported selector and wildcard forms
and can use explanation metadata to identify why a result matched. API errors
are rendered through the existing Explore error alert.

No client-selected traversal depth is introduced initially. The server owns the
default and hard cap so large traversals cannot be requested by a browser or
CLI client.

## Implementation Boundaries

The backend parses typed query terms, compiles validated terms using blueprint
metadata, and invokes a shared candidate-ID resolver. Main entity search and
relationship-tree facet child/count paths use that resolver so their text-query
semantics agree.

No migration is required. Only `*:` global traversal is resource-bounded: all
such terms in one request share caps for matching scalar rows, discovered
entities, and relationship edges. Traversal is an ordered, three-level BFS;
exhausting any cap fails the whole request with a typed `422` response (never a
partial page). Its scalar, edge, and selected-ID reads run in one transaction
with a 250 ms PostgreSQL `statement_timeout`. Counters, histograms, and warning
logs record global-search outcomes and the consumed dimensions. Bare and
qualified selectors retain their existing semantics and are not subject to
these global caps.

## Coverage

Integration tests cover direct matches, the fact that bare terms do not
traverse relationships, `*:` global traversal, explicit relationship matches,
deleted entities, inactive relationships, structured selectors (including
selected-blueprint code and user-specified-name aliases in both `blueprint:term`
and `blueprint.attribute:term` forms), wildcard matching, invalid or ambiguous
syntax/selectors, AND intersections, pagination, facet counts/filtering, and
deterministic per-term match explanations. Frontend tests must cover URL
parsing/submission and request forwarding for structured query text, plus
rendering or otherwise exposing the returned match rationale.
