# Workflows

Workflows are workspace-scoped, versioned TOML definitions. Published revisions are immutable; enabling records an outbox-sequence high-water boundary, so only events committed after enablement fan out. A run stores its exact compiled revision and trigger snapshot and is never switched to a newer definition.

v1 executes only local actions on the triggering, non-deleted entity: `system_tags_add`, `system_tags_remove`, `system_metadata_merge`, `system_metadata_delete`, and `attribute_write`. Attribute values are scalar fixed values or `facts.<index>.<field>` scalar inputs. Normal current blueprint/schema, readonly, default-context and annotation validation applies. Actions are ordered and derive tag/metadata changes from the entity row locked at execution time; event payloads are deliberately minimal and unordered.

Triggers are exact matches for entity events only: `entity.created.v1`, `entity.updated.v1`, `entity.migrated.v1`, `attribute_value.changed.v1`, `attribute_value.restored.v1`, and `relationship.changed.v1`. `envelope` permits only `event_type`, `aggregate_kind`, `source_kind`, `source_name`, or explicit `metadata.<field>` paths. `facts` and dynamic attribute input are restricted to `facts.<index>.<field>` paths and all values are scalar. There is no template, query, script, target selector, loop, cross-entity write, schedule, webhook, or external effect.

```toml
format_version = 1
code = "tag-new-products"
name = "Tag new products"
[[triggers]]
event_type = "entity.created.v1"
[triggers.envelope]
source_name = "catalog_api"
[[actions]]
type = "system_tags_add"
tags = ["new"]
```

## Management UI

The **Manage → Workflows** page is available to members with `workflows.read`. It lists workflow families, lifecycle state, current revision, and safe run/dead-letter indicators. Members with `workflows.manage` can validate TOML with the server compiler, save drafts, publish, enable or disable revisions, and replay terminal dead letters. Revision source and comparisons are read-only; diagnostics expose only safe run state, timestamps, attempts, and outcome/error evidence—never internal domain-event payloads.

## Delivery and operations

The internal `catalog.workflows` outbox consumer only creates durable runs. Dispatcher redelivery is expected: `(workspace, workflow revision, trigger event)` is unique. A worker leases runs, retries with bounded exponential delay, and dead-letters after five attempts. Each action inserts its `(run, action index)` idempotency key in **the same transaction** as its entity row lock, mutation, audit evidence, and outgoing outbox event. A crash after commit but before a run acknowledgement therefore reclaims the run without duplicate catalog effects or audits.

Disabling a workflow cancels queued and leased runs; every action rechecks that state while holding the run lock. Dead letters can be repaired and replayed via the tenant-scoped diagnostics endpoints (`GET /workflow-runs`, `workflows.read`; `POST /workflow-runs/{run_id}/replay`, `workflows.manage`). Diagnostics never expose internal domain-event payloads.

Workflow events retain correlation and direct causation and include workflow/revision/run/action audit provenance plus a persisted root trigger and bounded causal depth (8). Workflow-originated sources are excluded from default intake, preventing feedback. Operators should investigate a dead letter, fix the schema/state condition, and replay; do not rely on chronological delivery or exactly-once worker attempts.
