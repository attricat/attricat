# Relationship-Aware Explore Search

> **Status: deployed.** This document describes the relationship-aware search
> behavior used by the Explorer and `POST /v1/records/search`.

Explore finds records of the selected blueprint from values on the record
itself and values on records connected to it through relationships. It uses a typed query language parsed and validated before candidate resolution.

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
values on records of the selected blueprint (depth 0). It does not traverse
relationships.

A relationship-qualified term, such as `color.name:red` or
`family.product_type.name:graphics`, first finds matching scalar values on the
path's target blueprint, then traverses up to three named **incoming active
relationship edges** in batched steps:
an edge from Product to Color allows a matching Color value to select that
Product. Deleted records and inactive edges are excluded.

The `*:` selector explicitly opts into global relationship-aware discovery. It
first finds matching scalar values across blueprints, then traverses incoming
active relationship edges in breadth-first batches for up to three edges before
returning selected-blueprint records. Intermediate ID sets use batched set
reads rather than N+1 queries.

Every query term independently produces a candidate set. Multiple whitespace-
separated terms are intersected (implicit `AND`). Relationship-tree facet
counts, selected-facet filtering, and paginated result pages consume that same
candidate set, so they cannot disagree about which source records match.


## Query Language

| Form | Meaning |
| --- | --- |
| `red` | Free text across scalar values on the selected blueprint only. |
| `*:red` | Explicit global relationship-aware search through up to three incoming edges. |
| `color:red` | Match values on records reached through selected-blueprint relationship `color`. |
| `color.name:red` | As above, restricted to related attribute `name`. |
| `Produkt:czerwony` | Free text limited to the selected blueprint, identified by its user-specified name (its code, `product:czerwony`, also works). |
| `sku:123*` | Match selected-blueprint attribute `sku`. |
| `width_mm:4`, `width_mm:>=4`, `width_mm:40..60` | Compare number attribute `width_mm` by value (see [Typed leaves](#typed-leaves)). |
| `product.sku:123*` | Explicit selected-blueprint form of the preceding query; `product` may be its code or user-specified name. |
| `@id:ID-1,ID-2` | Selected-blueprint records whose ID is one of the comma-separated UUIDs (`product.@id:…` is equivalent). |
| `color.@id:ID-1,ID-2` | Selected-blueprint records linked through `color` to one of the listed records. |

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

- `@id` as the final part (case-insensitive) matches record IDs instead of a
  scalar leaf. The preceding parts, if any, must be the selected-blueprint alias
  or up to three relationships. The value is a comma-separated list of at most
  100 UUIDs; wildcards are rejected. Listed IDs seed the same incoming-edge
  traversal as scalar matches, and witnesses have no matching attribute code.

Unknown, ambiguous, or incompatible names and malformed terms return a clear
`400` input validation error. When the targets of a multi-target hop declare
the leaf with different value types, the term returns `422`
`relationship_path_type_conflict`, as table columns and filters do. `*` is a query-language wildcard only in a trailing position
and means prefix matching. Values are always passed to SQL as parameters;
query wildcards are never interpolated into SQL.

### Typed leaves

Once a qualified selector resolves to one attribute (`attribute:`,
`blueprint.attribute:`, or a relationship path ending in a scalar leaf), the
leaf's value type decides how the term compares values. Multi-target hops
resolve the leaf by the same rule as other relationship paths, so every target
that declares it has one value type.

- `number` and `integer` leaves compare by value, never by text:
  `width_mm:4` (or `=4`) matches 4 and 4.0 but not 40, 14, or 4.5. `>`, `>=`,
  `<`, `<=` bound one end, and `from..to` is a closed range including both
  ends. Decimals may use a point or a comma (`2,5`). A value is an optional
  sign, digits, and an optional fraction; exponents, `NaN`, and infinities are
  not numbers. A non-number, a trailing `*`, or a range that starts after it
  ends is a `400`. An `integer` leaf accepts only whole numbers (`4,0` is 4;
  `4,5` is a `400`, not an empty result), so a mistyped quantity is reported
  rather than silently matching nothing. The term compares the leaf's own
  column (`value_number`, or `value_integer` with `bigint` bounds) so the
  per-attribute value indexes apply. Bounds are SQL parameters; only the
  comparison operators, chosen from the parsed form, are part of the SQL
  text.
- `boolean` leaves match `true` or `false` exactly, ignoring case; anything
  else, including a wildcard, is a `400`.
- `string` leaves keep substring and `*` prefix matching and `search:fuzzy`
  similarity. `date`, `datetime`, and `time` leaves keep matching their
  canonical text, so `due:2026-05` still finds a month.

Terms that name no single leaf keep text matching on every scalar value:
bare terms, `blueprint:term`, `relationship:term`, and `*:term`. A bare `4`
therefore still matches 40 and 14; free text means "this text anywhere", and
a qualified term is the way to ask for an exact number.

Follow-up: `date` and `datetime` leaves could accept the same `=`, comparison,
and range forms. That needs a rule for a day against a timestamp's time zone
and would change today's partial-date matching (`2026-05`), so it is not part
of the number change.

Explicit selectors may contain up to three relationship hops followed by a
scalar leaf, such as `category.parent.name:summer`. Deeper selectors, implicit
relationship traversal, boolean operators other than implicit `AND`, and ranking
are out of scope. `*:` remains the only global multi-hop mode.

## API and UI Contract

`POST /v1/records/search` continues to take the query in its existing `query`
string field. An empty or absent query preserves current browse behavior. The Explore query field and URL query parameter preserve the supplied
query text. The UI shows selector and wildcard examples. API errors
are rendered in the Explore error alert.

There is no client-selected traversal depth. The server owns the
default and hard cap so large traversals cannot be requested by a browser or
CLI client.

## Implementation Boundaries

The backend parses typed query terms, compiles validated terms using blueprint
metadata, and invokes a shared candidate-ID resolver. Main record search and
relationship-tree facet child/count paths use that resolver so their text-query
semantics agree.

No migration is required. Only `*:` global traversal is resource-bounded: all
such terms in one request share caps for matching scalar rows, discovered
records, and relationship edges. Traversal is an ordered, three-level BFS;
exhausting any cap fails the whole request with a typed `422` response (never a
partial page). Its scalar, edge, and selected-ID reads run in one transaction
with a 250 ms PostgreSQL `statement_timeout`. Counters, histograms, and warning
logs record global-search outcomes and the consumed dimensions. Bare and
qualified selectors retain their existing semantics and are not subject to
these global caps.

## Coverage

Integration tests cover direct matches, the fact that bare terms do not
traverse relationships, `*:` global traversal, explicit relationship matches,
deleted records, inactive relationships, structured selectors (including
selected-blueprint code and user-specified-name aliases in both `blueprint:term`
and `blueprint.attribute:term` forms), wildcard matching, numeric equality,
comparisons, and ranges on number and integer leaves (directly, through one
hop, and through a multi-target hop), exact boolean leaves, string leaves'
unchanged substring matching, invalid or ambiguous
syntax/selectors, AND intersections, pagination, and facet counts/filtering. Frontend coverage should verify URL parsing/submission and forwarding structured
query text.
