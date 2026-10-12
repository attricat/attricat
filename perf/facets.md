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

`FACET_BENCH_PRODUCTS` changes the product count. Each scenario runs once to
warm up and then ten times, which is past the five executions after which
PostgreSQL may switch a pooled prepared statement to a generic plan. The test
also prints the `server-timing` header of each scenario's slowest sample.

## Results

Medians on a local PostgreSQL (Apple silicon, release build). Before, the
searches with a selected root also took about 1.5 s on some samples because
of generic plans (see below).

| Scenario | Before | After |
| --- | ---: | ---: |
| Browse: root level | 1,319 ms | 189 ms |
| Browse: root level, child context | 1,157 ms | 222 ms |
| Browse: expand a root | 1,277 ms | 192 ms |
| Browse: expand a level-1 node | 1,270 ms | 177 ms |
| Browse: expand a level-2 node | 1,170 ms | 180 ms |
| Browse: root level, query "Product 1" | 726 ms | 567 ms |
| Search: no facet (reference) | 34 ms | 37 ms |
| Search: root selected | 2,496 ms | 108 ms |
| Search: root selected, child context | 2,460 ms | 103 ms |
| Search: root selected, version 1 | 992 ms | 93 ms |
| Search: level-2 node selected | 960 ms | 11 ms |

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

## Remaining cost

A Browse rail page still resolves every edge of the relationship (about
180 ms at this size) because counts include all of a node's descendants. With
a search term, resolving the term dominates. Reusing resolved edges across
pages, or approximate counts, are the next steps.
