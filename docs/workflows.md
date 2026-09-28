# Workflows

Workflows are workspace-scoped, versioned TOML definitions. Published revisions are immutable; enabling records an outbox-sequence high-water boundary, so only later events fan out. Each run snapshots its exact enabled compiled revision and never changes to a newer definition.

Local actions remain deliberately narrow: `system_tags_add`, `system_tags_remove`, `system_metadata_merge`, `system_metadata_delete`, and `attribute_write`. They only affect one existing entity, with normal schema/readonly validation. There are no scripts, templates, loops, queries, SQL, target selectors, or cross-entity writes.

## Trigger contracts

`format_version = 1` remains event-only and accepts only the documented catalog entity event types. `format_version = 2` additionally has these closed trigger types:

```toml
format_version = 2
code = "retag-product"
name = "Retag product"
[[triggers]]
type = "manual"
[[actions]]
type = "system_tags_add"
tags = ["reviewed"]
```

A manual run is available only to `workflows.manage`, only against the currently enabled published revision, and requires one existing `entity_id`. It accepts no arbitrary payload. `POST /workflows/{workflow_id}/run-now` returns a durable run identifier; diagnostics show source and safe status, never its private trigger snapshot.

Schedules are revisioned TOML, not mutable database jobs. They use a six-field cron and **must** explicitly declare `timezone = "UTC"`; local/DST time zones are rejected to eliminate DST duplicate/skipped ambiguity. `target_entity_id` is the single existing entity action target. Schedule actions require fixed attribute values (there are no event facts).

```toml
format_version = 2
code = "hourly-retag"
name = "Hourly retag"
[[triggers]]
type = "schedule"
cron = "0 0 * * * *"
timezone = "UTC"
target_entity_id = "00000000-0000-0000-0000-000000000001"
[[actions]]
type = "system_tags_add"
tags = ["hourly"]
```

The Rust scheduler owns a durable cursor per revision/trigger. It skips downtime misfires older than five minutes, records them, and skips an occurrence while that workflow has a pending or leased run (no overlap). Occurrence keys are `(revision, trigger index, UTC due timestamp)`, so duplicate ticks/restarts cannot create duplicate runs. Disable cancels queued/leased runs and removes the lifecycle eligibility on the next scheduler transaction. Runs retain normal bounded exponential retry (five attempts) and dead-letter semantics.

`extension_event` is an explicit v2 parser contract containing `provider`, a versioned `plugin.*.v1` event type, and `contract_version`. It is **not delivered yet**: the event dispatcher currently has a static core event subscription and cannot safely recheck an extension provider/release/export grant at delivery. No extension event can trigger a workflow until a dynamic, tenant-scoped consumer can atomically pin and recheck those grants; client diagnostics do not expose extension payloads.

## External effects

Webhooks, network delivery, secrets, and external effects remain deferred **for workflows**. Extensions have mediated network and secrets APIs, but workflows do not have an outbound delivery adapter or secret references; they must not create raw clients or read secrets. A future outbound capability must be a permissioned mediated delivery queue with encrypted secret references, execution-time grant checks, bounded payloads, idempotency keys, retries and dead letters; it is not implemented by this slice.

## Operations

The `catalog.workflows` outbox consumer only creates durable event runs. Workers lease runs, retry with bounded exponential delay, and dead-letter after five attempts. Each action inserts its `(run, action index)` idempotency key in the same transaction as entity locking, mutation, audit evidence, and outgoing outbox event. Disabling cancels queued and leased runs; every action rechecks this execution fence. `GET /workflow-runs` needs `workflows.read`; manual runs, replay, and lifecycle changes need the narrow `workflows.manage` permission.
