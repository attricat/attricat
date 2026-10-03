# Workflows

Workflows are workspace-scoped, versioned TOML definitions. Published revisions are immutable; enabling records an outbox-sequence high-water boundary, so only later events fan out. Each run snapshots its exact enabled compiled revision and never changes to a newer definition.

Local actions remain deliberately narrow: `system_tags_add`, `system_tags_remove`, `system_metadata_merge`, `system_metadata_delete`, and `attribute_write`. They only affect one existing entity, with normal schema, readonly and status-transition validation. The only cross-entity action is the bounded `referencing_entities_update` described below. There are no scripts, templates, loops, queries, SQL, or arbitrary target selectors.

Execution lives in `crates/repository/src/repository/workflow_actions.rs`. Every workflow entity write, for the trigger entity or a referencing entity, goes through `apply_workflow_actions`: it stages all actions for one locked entity, then runs `validate_entity_schema` (schemas, readonly, status transitions), rebuilds the preview, writes audit evidence and enqueues one `entity.updated.v1` in the caller's transaction. New write checks (for example locks on finalized records or per-transition permissions) must be enforced in that shared path, or in the normal entity write validation it calls, so that workflow writes cannot bypass them.

## Changed-attribute filters

An event trigger may declare `attributes = ["code", ...]` (1-100 unique codes). Fan-out then creates a run only if `catalog_workflow::changed_attributes_match` finds a payload fact whose `attribute_code` is listed; it is ANDed with `envelope` and `facts`. The field is optional and omitted from the compiled plan when absent, so stored revisions and their hashes are unchanged. It is rejected on `entity.migrated.v1`, whose payload has no facts.

Verification for this contract: `entity.created.v1`, `entity.updated.v1`, `attribute_value.changed.v1`, `attribute_value.restored.v1` and `relationship.changed.v1` carry `facts[]` built from the audit before/after diff (`audit_changes`), so a fact exists only for a value that actually changed and always names its `attribute_code`. Relationship facts are one per added/removed target (`relationship_add`/`relationship_remove`). File attributes are excluded from the audit snapshot and the file-reference write paths (`files.rs`) enqueue no domain event, so a file attribute can never satisfy a filter. Emitting file-reference facts is a separate eventing contract change.

## Referencing entity updates

```toml
[[actions]]
type = "referencing_entities_update"
relationship_attribute = "license"   # attribute on the referencing entities
max_targets = 100                    # default 100, hard maximum 500
[[actions.actions]]
type = "attribute_write"
attribute_code = "status"
fixed = "in_review"
```

Nested actions are 1-20 local actions with fixed values only (no `event_field`, no nesting). Targets are live entities with an active relationship value, in any context, whose attribute with that code belongs to the entity's current blueprint revision or the entity itself, excluding the trigger entity. They are selected in ID order with `LIMIT max_targets + 1`; exceeding the limit fails the attempt before any further target is written.

Each target is a separate transaction: lifecycle lock, run lock and task fence (as for any action), then the target entity row lock, a reference recheck under that lock, a `workflow_run_action_targets (run_id, action_index, entity_id)` marker and the entity write. A missing or no-longer-referencing target is marked `skipped`. A failing target rolls back and is recorded as `failed` (with a bounded `last_error` and attempt count) in its own task-fenced transaction; other targets continue. If any target failed, the action returns an error and the run retries normally; a retry revisits only `failed` rows and targets not yet reached, so completed targets are never written twice. The action's `workflow_run_actions` marker is written only after every target has settled. A lost task lease aborts immediately. `GET /workflow-runs/{run_id}/targets` exposes the rows. Target writes use the `workflow:<id>` source, so they do not fan out to workflows.

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
