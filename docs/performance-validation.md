# Performance validation

## Rust compile-time benchmark

Use the isolated API compile benchmark to compare crate-boundary changes without
invalidating the normal development target directory:

```sh
scripts/benchmark-api-compile.sh all
# Reuse retained artifacts for a warm-only sample:
scripts/benchmark-api-compile.sh warm
```

It measures clean and warm `cargo check -p api --all-targets` runs plus
representative domain, repository, HTTP, and extension-runtime edit/recheck
samples. Record the Rust version, host hardware, git revision, and emitted real
timings when comparing results.

## Explorer performance validation

Explorer performance is measured locally against the deterministic ByteForge Components demo catalog. The generator writes all catalog data through the public API; the load test measures API reads only.

## Prepare a dataset

Use a dedicated local database for performance work. Start a release-mode API against it, then create one of the documented [demo catalog profiles](../examples/generate.md):

```sh
CATALOG_TOKEN=cat_pat_... just generate medium
# Full benchmark dataset (1,000,000 entities; this can take substantial time):
CATALOG_TOKEN=cat_pat_... just generate large
```

The generator prints throughput, retries, and estimated ready time. It persists a resumable checkpoint under `.catalog-generator/`; inspect or resume it with:

```sh
node examples/generate.mjs --size large --status
CATALOG_TOKEN=cat_pat_... just generate-resume large
```

Do not run a benchmark until its checkpoint reports `benchmark_ready: true`.

## Run the local workload

Install [k6](https://grafana.com/docs/k6/latest/set-up/install-k6/) and use a personal access token with `entities.read`:

```sh
CATALOG_TOKEN=cat_pat_... just perf smoke
CATALOG_TOKEN=cat_pat_... just perf baseline
CATALOG_TOKEN=cat_pat_... just perf load
```

`perf/explorer.js` covers scalar text/price sorting, related-manufacturer sorting, keyset pagination, and relationship-facet root expansion. It fails on request errors or missing `Server-Timing`; latency measurements are reported locally rather than treated as a gate until a stable hardware baseline is established.

Save the k6 JSON summary, the generator checkpoint, API git revision, PostgreSQL version/settings, host CPU/RAM, and the selected profile when comparing runs. Keep cold-start and warmed runs separate.

## Query plans

For index changes or unexpected latency, capture `EXPLAIN (ANALYZE, BUFFERS)` for the equivalent scalar-sort and relationship-sort selection queries using `just sql`. Relationship sorts use fixed one-, two-, or three-hop SQL: indexed non-null leaf values traverse incoming edges back to sources, while missing paths use a separate source-ID/null phase. Compare index scans, buffers, rows removed, and warm/cold timings before adding another declarative index. Do not put query text, fixture values, UUIDs, or SQL text into `Server-Timing` or the browser inspector.

When development tools are enabled, Explorer responses emit aggregate `app`,
`sql`, allowlisted `sql-*` query-count, and phase (`candidate`, `page`,
`related`, and `serialize`) timings. Browser tests continue to cover the
bounded timing buffer and extension-cell frame load/fallback records.
