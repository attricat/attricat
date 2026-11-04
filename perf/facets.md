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

## Filter and text scenarios

`FACET_BENCH_SCENARIOS=text,search:,filter,reference` runs the attribute filter,
relationship filter, text query and selected facet searches, and two searches
without any. Run them with `jit = off` on the server, because the benchmark's pool
doesn't go through `disable_jit`.

Filters, text terms and selected facets are conditions on each statement's
records. Only `*:` terms load their matches, under their own budgets. A page takes
one of two paths:

- **Broad:** it checks its first 100 records one by one. If at least 5% match, its
  statements run the conditions per record (`OFFSET 0`, explicit joins), so a page
  and its capped total stop within a few thousand records.
- **Narrow:** otherwise it loads up to 20,000 matches (`MAX_LOADED_FILTER_MATCHES`)
  in one scan, planned afresh each time. Those IDs answer the version scope and
  total and drive the page. A search that matches more than that runs per record
  like a broad one.

A sorted page reads every match, so it uses a semi-join, except a relationship
sort, which walks its indexed leaves in order. A semi-join needs all its conditions
in the subquery's `WHERE`, or PostgreSQL runs it once per record. A relationship
text term walks back from its matching values, because PostgreSQL can't estimate
a pattern.

JIT compilation was the largest single cost of a broad search: hundreds of
milliseconds to compile a plan that reads 500 rows, because the estimate can't
reflect stopping early. API connections turn it off.

Medians on a local PostgreSQL (Linux, release build), with JIT off on both sides.
The test process's peak memory fell from 194 MB to 83 MB at 100,000 products, and
from 271 MB to 96 MB at 120,000.

| Scenario | Before, 100k | Now, 100k | Before, 120k | Now, 120k |
| --- | ---: | ---: | ---: | ---: |
| No filter | 33 ms | 10 ms | 42 ms | 11 ms |
| Selective scalar | 64 ms | 8 ms | 73 ms | 8 ms |
| Twenty selective scalars | 1,091 ms | 103 ms | 1,342 ms | 98 ms |
| Relationship filter | 643 ms | 10 ms | 815 ms | 11 ms |
| One hop | 703 ms | 27 ms | 861 ms | 28 ms |
| One hop, child context | 961 ms | 27 ms | 1,203 ms | 26 ms |
| Three hops | 2,799 ms | 583 ms | 3,303 ms | 470 ms |
| Reusable attribute | 9 ms | 10 ms | 8 ms | 10 ms |
| Broad scalar | 693 ms | 12 ms | 844 ms | 13 ms |
| Twenty broad scalars | 11,835 ms | 81 ms | 14,488 ms | 82 ms |
| Broad, sorted through a relationship | 4,319 ms | 16 ms | 6,408 ms | 19 ms |
| Broad, sorted by a column | 1,081 ms | 955 ms | 1,341 ms | 649 ms |
| Contains, rare value | 416 ms | 86 ms | 465 ms | 100 ms |
| Absence, few matches | 403 ms | 324 ms | 517 ms | 349 ms |
| Absence, no matches | 426 ms | 302 ms | 515 ms | 331 ms |
| Text: rare value | 69 ms | 69 ms | 79 ms | 79 ms |
| Text: rare word | 556 ms | 69 ms | 900 ms | 79 ms |
| Text: short number | 100 ms | 81 ms | 125 ms | 100 ms |
| Text: common number | 369 ms | 21 ms | 419 ms | 17 ms |
| Text: every record | 686 ms | 15 ms | 974 ms | 14 ms |
| Text: two broad terms | 768 ms | 30 ms | 1,057 ms | 29 ms |
| Text: every record, sorted | 922 ms | 1,031 ms | 1,149 ms | 379 ms |
| Text: one attribute | 62 ms | 52 ms | 76 ms | 56 ms |
| Text: relationship path | 70 ms | 15 ms | 79 ms | 15 ms |
| Text: relationship, every record | 943 ms | 22 ms | 1,161 ms | 22 ms |
| Text and broad scalar | 544 ms | 105 ms | 838 ms | 133 ms |
| Root selected | 148 ms | 95 ms | 172 ms | 111 ms |
| Root selected and a filter | 786 ms | 202 ms | 996 ms | 244 ms |
| Root selected and a text term | 205 ms | 214 ms | 239 ms | 254 ms |
| Level 2 selected | 15 ms | 15 ms | 14 ms | 14 ms |
| Root selected through a large hop | 288 ms | 181 ms | 351 ms | 245 ms |

Global `*:` terms didn't change: repeated runs took about 220 ms on both, though a
single run can differ widely.

With `plan_cache_mode=force_generic_plan`, no search timed out. Broad searches
stayed far ahead (a broad scalar took 5 ms against 11,470 ms). Selected roots took
about 2.2 s on both, and a selective scalar took 647 ms against 345 ms.

## Browse rail and agent counts

`FACET_BENCH_SCENARIOS=browse,agent` runs the Browse rail's pages and
`count_records`. With a search term, a page samples like a search page. A narrow
term loads up to 20,000 source IDs and follows the hops in turn. A broad term, and a
reload of the cached counts, count in one statement that hash joins each hop's whole
relationship. PostgreSQL underestimates a resolved edge list as one row, and the
nested loops it then picks take minutes, so these statements turn them off with
`SET LOCAL enable_nestloop = off`.

Medians at 100,000 products with JIT off on both sides. The test process's peak
memory across the Browse scenarios fell from 192 MB to 68 MB.

| Scenario | Before | Now |
| --- | ---: | ---: |
| Root level, cached | 19 ms | 18 ms |
| Root level, first request | 458 ms | 516 ms |
| Two-hop path, first request | 291 ms | 631 ms |
| Large middle hop, first request | 544 ms | 832 ms |
| Three-hop path, first request | 468 ms | 992 ms |
| Query "Product 1" | 1,151 ms | 449 ms |
| Query "12" | 112 ms | 148 ms |
| Large middle hop, query "Product 1" | 1,164 ms | 705 ms |
| Large middle hop, query "7" | 711 ms | 610 ms |
| Large middle hop, query "12" | 136 ms | 147 ms |
| Three-hop path, query "12" | 141 ms | 153 ms |
| `count_records`, all | 33 ms | 3 ms |
| `count_records`, broad filter | 643 ms | 127 ms |
| `count_records`, relationship filter | 722 ms | 6 ms |
| `count_records`, per category | 455 ms | 324 ms |
| `count_records`, per line | 285 ms | 315 ms |
| `count_records`, per line, broad filter | 905 ms | 451 ms |

A first request reloads the cached counts, which then serve the workspace for 30
seconds. Counting in the database costs more there than reading the edges into the
API, in exchange for not holding them in memory.

## Remaining cost

The first Browse rail page for a tree, and the first after its counts expire,
still resolves every edge of the relationship (about 200 ms at this size).
With a search term, counts are exact and uncached, and resolving the term
dominates each page. Caching those per term would need the cache to bound
entries by size rather than count.
