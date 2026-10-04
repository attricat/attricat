---
title: Monitoring
description: Watch catalog data health, background queues, metrics, traces, and failed event deliveries.
---

## Data health

**Manage → Data health** summarizes the state of the catalog:

- **Summary cards**: outdated entities, active entities, stale entities, and relationships pointing at deleted entities.
- **Storage**: size per table and in total.
- **Blueprint health**: per blueprint, how many entities there are, how many are outdated or stale, and the oldest update.
- **Freshness distribution**: how recently values were updated.
- **Default completeness**: how many entities have every field their schema requires in the default context.
- **Context coverage**: how many entities have their own values in each context.
- **Relationship integrity**: links whose target has been deleted.

**Stale after** sets what counts as stale, from 1 to 3650 days (90 by default). Results are cached for five minutes by default (`DATA_HEALTH_CACHE_TTL_SECONDS`); **Refresh** clears the cache.

Extensions can add cards to this page.

## Background processing

**Manage → Background processing** shows the API's task queue per task type: queued, running, failed, expired leases, and how long the oldest due task has waited. It refreshes every 30 seconds.

- **Queued** includes retries scheduled for later.
- **Expired leases** are tasks whose worker stopped responding; another worker picks them up.
- **Failed** are dead letters that need attention.

File processing has its own queue and is not shown here; watch it through metrics.

## Metrics

Both processes expose Prometheus metrics. Keep them on a private network.

| Process | Endpoint | Access |
| --- | --- | --- |
| API | `GET /metrics` | A session or token with `data_health.read`. |
| File worker | `GET /metrics` on the operations listener (port 3001) | `Authorization: Bearer $FILE_WORKER_METRICS_TOKEN`. |

Labels are bounded: routes are reported as templates, and no file name, ID, or URL is ever a label. The only series labelled by workspace is `catalog_event_delivery_queue_depth` (`workspace_id`).

Useful series:

| Series | Watch for |
| --- | --- |
| `catalog_database_ready`, `catalog_object_store_ready` | Zero for two probe intervals. |
| `catalog_task_queue_depth`, `catalog_task_queue_oldest_age_seconds`, `catalog_task_queue_retries` | Growing depth, old tasks, any dead letters. |
| `catalog_file_worker_queue_depth`, `catalog_file_worker_oldest_age_seconds`, `catalog_file_worker_retries` | Same, for file processing. |
| `catalog_file_worker_jobs_failed_total` | Increases. |
| `catalog_event_delivery_queue_depth{workspace_id,consumer,status}` | Growing `pending`, any `dead_letter`. |
| `catalog_event_deliveries_total{outcome}` | `dead_letter` increases. |
| `catalog_extension_operation_runs`, `catalog_extension_operation_oldest_age_seconds` | Stuck extension operations. |
| `catalog_file_uploads_total`, `catalog_file_downloads_total`, `catalog_object_store_operations_total` | Error outcomes. |
| `catalog_value_history_cleanup_total{outcome}` | `failed`. Repeated `budget_exhausted` means each 10-second run ends with old history still left to delete. |
| `catalog_upload_cleanup_total{outcome}` | `failed`. Failed deletions of abandoned uploads are retried. |
| `catalog_query_cache_requests_total{namespace,outcome}` | A falling share of `hit` and `remote_hit` against `miss`. |
| `catalog_query_cache_redis_circuit_opened_total` | Increases: Redis keeps failing and replicas are using memory only. |
| `catalog_db_round_trips_per_operation{scope}` | Rising database round trips per request route, task kind or background loop. |
| `catalog_db_round_trips_total{scope}` | A growing `unscoped` rate, or a background loop whose rate rises while the catalog is idle. |

The event delivery gauge is refreshed every five seconds. For each consumer with deliveries in a workspace, all four statuses are reported, and statuses without deliveries read `0`.

Each request route, task kind and background loop is a round-trip `scope`. To log the count for every operation, set `RUST_LOG=catalog_repository::round_trips=debug`.

Suggested alerts: page when a readiness gauge is zero for two intervals, when any dead-letter or failed count is above zero, or when failure counters increase. Warn when the oldest queued item is older than five minutes for ten minutes, or a queue grows for fifteen minutes. Alert if metrics disappear for two scrape intervals. Tune thresholds to your import and export volume.

## Request timing

Every API response has a `Server-Timing` header with the total handling time (`app;dur=…`). Browser developer tools show it under the request's timing. Data-health responses also say whether the cache was hit.

## Logs and traces

The API and file worker log structured events controlled by `RUST_LOG`. Each request has an `x-request-id`, returned in the response and recorded in the audit log, so you can find the log lines for a change someone reports.

Set `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` to send traces to an OTLP/gRPC collector.

Logs and traces never contain object keys, file names, passwords, or tokens.

## Failed event deliveries

Catalog changes produce internal events that workflows, rules, and extensions consume. A delivery that keeps failing becomes a **dead letter** after five attempts by default.

1. List dead letters (needs `data_health.read`):

   ```sh
   acli event dead-letters
   ```

   Each entry has the consumer and event IDs, the consumer name, event type, attempt count, failure time, and last error.

2. Fix the cause: the extension, the external dependency, or the data.
3. Make sure running the handler again is safe.
4. Replay the delivery (needs `roles.manage`):

   ```sh
   acli event replay <consumer-id> <event-id>
   ```

Replay does not reset the attempt count, so a delivery that fails again goes straight back to dead letter. Don't replay the same delivery repeatedly without fixing the cause.

Rule and workflow runs and extension operations have their own dead letters and replay commands; see [Rules](/builders/rules/), [Workflows](/builders/workflows/), and [Operations](/extensions/operations/).

## Retrying file processing

A file whose processing failed permanently can be queued again:

```sh
docker run --rm --env-file attricat.env ghcr.io/attricat/attricat@sha256:… file-worker --retry <job-uuid>
```
