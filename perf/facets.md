# Relationship tree facet latency

`apps/api/tests/facet_latency.rs` seeds 100,000 products linked to a
four-level tree of 4,110 categories (every tenth product overrides its
category in a child context), then times the Browse rail's children pages and
searches that select tree nodes through the HTTP handlers:

```sh
set -a; source .env; set +a
OTEL_EXPORTER_OTLP_TRACES_ENDPOINT= RUST_LOG=warn \
  cargo test --release -p api --test integration facet_latency:: -- --ignored --nocapture
```

The products also link to 1,000 product lines, each in a leaf category, so
the same scenarios run over the two-hop path `line.category` (every tenth
product and line overrides its hop in the child context). Each product also
has its own listing, in the product's category and line, for paths whose
middle hop is as large as the source: `listing.category` and
`listing.line.category`. Search terms cover three widths: "12" matches 3,970
products, "7" 40,951 and "Product 1" 40,952 (its terms "Product" and "1" are
matched separately).

`FACET_BENCH_PRODUCTS` changes the product count. Each scenario runs once and
then ten more times, which is past the five executions after which PostgreSQL
may switch a pooled prepared statement to a generic plan. The test reports the
first request separately, because Browse scenarios without a query share
cached counts and only the first of them misses the cache. It also prints the
`server-timing` header of each scenario's slowest sample.

## Results

Medians on a local PostgreSQL (Apple silicon, release build). "Exact" is the
first optimization pass; "Cached" adds cached Browse rail counts. Before, the
searches with a selected root also took about 1.5 s on some samples because
of generic plans (see below).

| Scenario | Before | Exact | Cached |
| --- | ---: | ---: | ---: |
| Browse: root level, first request | 1,319 ms | 189 ms | 259 ms |
| Browse: root level, cached | 1,319 ms | 189 ms | 14 ms |
| Browse: root level, child context, first request | 1,157 ms | 222 ms | 220 ms |
| Browse: expand a root | 1,277 ms | 192 ms | 14 ms |
| Browse: expand a level-1 node | 1,270 ms | 177 ms | 14 ms |
| Browse: expand a level-2 node | 1,170 ms | 180 ms | 14 ms |
| Browse: root level, query "Product 1" | 726 ms | 567 ms | 640 ms |
| Search: no facet (reference) | 34 ms | 37 ms | 33 ms |
| Search: root selected | 2,496 ms | 108 ms | 98 ms |
| Search: root selected, child context | 2,460 ms | 103 ms | 106 ms |
| Search: root selected, version 1 | 992 ms | 93 ms | 105 ms |
| Search: level-2 node selected | 960 ms | 11 ms | 9 ms |

## What changed

- **Context resolution.** Relationship edges picked each source's context
  with a per-record `LATERAL … ORDER BY depth LIMIT 1`. The planner estimated
  a few hundred rows for 100,000 and chose nested loops. One `DISTINCT ON`
  pass over the field's values resolves 100,000 edges in about 200 ms instead
  of 830 ms.
- **Search selections.** A search with a tree selection built the whole facet
  (every node's label and count) to keep only the matching IDs. It now walks
  down from the selected nodes and back to the sources through the
  target-side relationship indexes, so its cost follows the selected subtrees.
  A selection without a search term also no longer loads every record ID of
  the blueprint.
- **Browse rail pages.** Each page counted every node of the tree in one
  recursive query (all sources × all ancestors). It now reads the two edge
  sets concurrently and counts only the requested page's subtrees in memory.
- **Generic plans.** The result-version and total-count queries filtered
  `e.id = ANY($n)`. Once a pooled connection caches a generic plan, that scans
  every record of the blueprint and compares each against the whole ID list
  (about 1.4 s for 10,000 IDs). These queries now join from the ID list.

- **Cached counts.** Without a search term, a Browse rail page reads only the
  tree's parent edges and the page's labels (about 14 ms). Every node's count
  is computed once per blueprint, field, hierarchy, context and version, and
  shared by the workspace for 30 seconds, so counts may lag writes by that
  long. The tree's nodes are always read fresh.

## Relationship paths

A facet may follow up to three relationship hops (`source_relationship_field
= "line.category"`). Search selections walk down the selected subtrees and
then back one hop per query through the target-side indexes. Browse rail
counts read each hop's edges with one query per hop and join them in memory:

- Without a search term every source counts, so each hop reaches nearly all
  of its records anyway. The hops' whole relationships are read concurrently,
  and the counts are cached as for a direct field, keyed by the path.
- With a search term matching at most 20,000 sources
  (`TREE_FACET_LISTED_SOURCES`), the first hop reads only the matching
  sources and each later hop only the records the hop before reached, through
  the source-side index. Every hop is single-valued, so no later list is
  longer than the first. A broader term reads every hop's whole relationship
  concurrently and keeps the matching sources, as listing nearly every record
  is slower than reading the relationship.

Medians for 100,000 products (release build). "Before" read every hop's whole
relationship for search terms too:

| Scenario | Before | After |
| --- | ---: | ---: |
| Browse `category`: root level, cached | 15 ms | 14 ms |
| Browse `category`: query "12" | 211 ms | 98 ms |
| Browse `line.category`: root level, cached | 14 ms | 14 ms |
| Browse `line.category`: query "12" | 223 ms | 114 ms |
| Browse `listing.category`: root level, first request | 392 ms | 378 ms |
| Browse `listing.category`: root level, cached | 14 ms | 15 ms |
| Browse `listing.category`: query "12" | 398 ms | 122 ms |
| Browse `listing.category`: query "7" | 519 ms | 506 ms |
| Browse `listing.line.category`: query "12" | 387 ms | 124 ms |
| Search `line.category`: root selected | 105 ms | 104 ms |
| Search `listing.category`: root selected | 203 ms | 206 ms |
| Search `listing.line.category`: root selected | 191 ms | 194 ms |

Queries matching more than 20,000 sources take the unchanged path; their
medians varied by up to 100 ms between runs ("Product 1": 547–660 ms for
`category`, 782–817 ms for `listing.category`), dominated by resolving the
term. In isolation, the listed read of one 100,000-record hop took 84 ms for
11,112 sources against 366 ms for the whole relationship, and 422 ms for all
100,000 sources, which sets the limit.

The first uncached page of a path with a large middle hop still reads that
hop's whole relationship: about 380 ms for `listing.category` against about
240 ms for `line.category`, after which counts come from the cache. Selecting
a root through `listing` reaches about 10,000 listings and products, about
200 ms against 100 ms through the 1,000 product lines.

## Remaining cost

The first Browse rail page for a tree, and the first after its counts expire,
still resolves every edge of the relationship (about 200 ms at this size).
With a search term, counts are exact and uncached, and resolving the term
dominates each page. Caching those per term would need the cache to bound
entries by size rather than count.
