# Explorer performance validation

`apps/api/tests/devtools_timing.rs` is the representative, isolated seed: it
creates a table blueprint, a scalar value, and runs a sorted Explorer search
with development timings enabled. Related-table hydration and related sorting
remain covered by `apps/api/tests/entity_search_related_projections.rs`.

For a local run, seed these fixtures through their API tests, then use `just
sql` to run `EXPLAIN (ANALYZE, BUFFERS)` for both the scalar-sort and
relationship-sort selection queries. Do not put fixture values, UUIDs, or SQL
text into `Server-Timing` or the browser inspector.

The local scalar-sort run for the 40-row development seed used
`entities_blueprint_version_idx` for the page candidates and
`attribute_values_scalar_history_idx` for current scalar values. Its measured
execution time was 0.261 ms with shared-buffer hits only. The corresponding
related-path integration fixture verifies the relationship join/hydration and
stable related-column sort; rerun its query plan after index changes.

The end-to-end devtools test asserts the emitted header contains only
`candidate`, `page`, `related`, and `serialize` durations. Browser tests cover
the 50-entry bounded buffer and extension-cell frame load/fallback records.
