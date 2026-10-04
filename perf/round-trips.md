# Database round trips

`apps/api/tests/round_trip_baseline.rs` measures database round trips (one per
completed SQL statement, including `BEGIN`/`COMMIT`) for hot request and worker
paths using the `catalog_db_round_trips_*` metrics:

```sh
set -a; source .env; set +a
OTEL_EXPORTER_OTLP_TRACES_ENDPOINT= RUST_LOG=warn \
  ROUND_TRIP_EXTENSION_ARCHIVE=../../../attricat-extension-example/dist/attricat-extension-example-0.2.0.tar.zst \
  cargo test -p api --test round_trip_baseline -- --ignored --nocapture
```

Without `ROUND_TRIP_EXTENSION_ARCHIVE` the harness installs the unified test
component, whose event handler is a no-op.

In a running server, `catalog_db_round_trips_per_operation{scope}` (per
request route, background tick, or task kind) and
`catalog_db_round_trips_total{scope}` are exported on `/metrics`; set
`RUST_LOG=catalog_repository::round_trips=debug` for a log line per operation.

## Baseline (before optimization)

| Scenario | Round trips |
| --- | ---: |
| Explorer search, 3 filters (cold / warm) | 17 / 16 |
| Entity update, 10 values (cold / warm) | 83 / 85 |
| Extension event task, unified no-op handler | 9 |
| Extension event task, example extension | 23 / 22 |
| Idle round trips per second, 1 workspace | 261 |

Idle breakdown (round trips per second): extension coordinator 64.8, task
claim 48.0, each of three event-dispatcher handlers 34.2, rule schedule 22.8,
workflow schedule 22.8, task metrics 0.4.
