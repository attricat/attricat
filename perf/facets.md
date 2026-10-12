# Relationship tree facet latency

`apps/api/tests/facet_latency.rs` seeds 100,000 products linked to a
four-level tree of 4,110 categories (every tenth product overrides its
category in a child context), then times the Browse rail's children pages and
searches that select tree nodes through the HTTP handlers:

```sh
set -a; source .env; set +a
OTEL_EXPORTER_OTLP_TRACES_ENDPOINT= RUST_LOG=warn \
  cargo test --release -p api --test facet_latency -- --ignored --nocapture
```

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

## Remaining cost

The first Browse rail page for a tree, and the first after its counts expire,
still resolves every edge of the relationship (about 200 ms at this size).
With a search term, counts are exact and uncached, and resolving the term
dominates each page. Caching those per term would need the cache to bound
entries by size rather than count.
