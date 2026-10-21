# Retention sweep batches

The maintenance worker's retention sweep
(`crates/workers/src/maintenance.rs`, batches in
`crates/repository/src/repository/retention.rs`) deletes at most
`RETENTION_BATCH_ROWS` rows per transaction, under `SET LOCAL
statement_timeout = '2s'` and `lock_timeout = '1s'`. Each family's expiry scan
starts from a partial index added in `20261109035944_retention_sweep.sql`, so a
batch reads about as many rows as it deletes, however large the table is.

## Measurement

Measured on 2026-10-21 against a scratch database in the local PostgreSQL 18
container (not the development database), with 1,000-row batches. Seeded rows:

- `tasks`: 3M. 1.6M are expired `rule_run.v1` envelopes, 200k are queued, and 1M
  are `event_delivery.v1` envelopes still linked to their deliveries.
- `rule_runs`: 2M across 200 rules, 90% record-scoped, spread over 115 days.
- `domain_events`: 1M over 115 days, each with one completed delivery. The
  consumer's watermark trails the newest event by 1,000.

| Batch | Time per 1,000 rows | Plan |
| --- | ---: | --- |
| `purge_tasks_batch` | 45 ms | `tasks_terminal_retention_idx` range scan, anti-join on `event_deliveries_task_id_unique_idx`. 12 ms is the `event_deliveries.task_id` FK check. |
| `purge_rule_runs_batch` (select, envelope delete, run delete) | 32 ms | `rule_runs_terminal_retention_idx` range scan. Probes `rule_runs_version_created_idx` ("is there a newer run?"), `rule_runs_enable_gate_idx` and the tasks unique key once per row. |
| `purge_domain_events_batch` (select, delivery, envelope and event deletes) | 90 ms | `domain_events_occurred_at_idx` range scan. Per-workspace max via `domain_events_workspace_sequence_idx`, unfinished deliveries via `event_deliveries_poll_idx`. |

With the default 10-second budget, one sweep drains roughly 100k–300k rows per
family when it is the only family with a backlog. Families share the budget
round-robin, one batch each in turn.

An earlier draft of the event batch checked unfinished deliveries with
`d.status <> 'completed'`. The planner ran that as a sequential scan of all
1M deliveries, taking 48 ms on its own. `d.status IN ('pending', 'leased',
'dead_letter')` uses the status index and takes 4 ms.

## Reproduce

```sh
set -a; source .env; set +a
docker exec <postgres-container> createdb -U postgres retention_perf
sqlx migrate run --source apps/api/migrations --database-url "${DATABASE_URL%/*}/retention_perf"
```

Seed with `generate_series` inserts like the counts above. Drop the
`rule_runs` foreign keys to `rules` and `records` in the scratch database so
runs can be seeded without definitions. Then run each batch's statements inside
`BEGIN; EXPLAIN (ANALYZE, BUFFERS) ...; ROLLBACK;`.
