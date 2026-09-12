# Workflows (foundation)

Workflows are workspace-scoped, versioned TOML definitions, independent of blueprint TOML. Every saved revision retains its raw TOML, SHA-256 hash, and compiled declarative plan. Revisions start as drafts; publishing makes an immutable revision eligible for enablement. A workflow has one enabled revision or is disabled.

Definitions require `format_version = 1`, a stable `code`, `name`, at least one exact core-event trigger, and ordered actions. Trigger `envelope` and `facts` filters contain scalar exact matches. v1 accepts only registered core v1 event types: `entity.created.v1`, `entity.updated.v1`, `entity.deleted.v1`, `entity.migrated.v1`, `attribute_value.changed.v1`, `attribute_value.restored.v1`, `relationship.changed.v1`, `blueprint.created.v1`, `blueprint.revision_created.v1`, `blueprint.published.v1`, `context.created.v1`, `context.updated.v1`, and `context.deleted.v1`.

Actions are deliberately declarative: `system_tags_add`, `system_tags_remove`, `system_metadata_merge`, `system_metadata_delete`, and `attribute_write`. Attribute writes accept exactly one scalar `fixed` value or a narrowly named `event_field`. Unknown TOML fields, scripts, SQL, HTTP, templates, loops, arrays/objects in filters, and unrecognized events/actions are rejected.

```toml
format_version = 1
code = "tag-new-products"
name = "Tag new products"
[[triggers]]
event_type = "entity.created.v1"
[triggers.facts]
blueprint = "product"
[[actions]]
type = "system_tags_add"
tags = ["new"]
```

The lifecycle records the current domain-event sequence as an internal activation high-water boundary when enabled. The capture is serialized with application outbox appends so it cannot skip an in-flight committed event. It is intentionally not exposed through the management API. **Event dispatch, action execution/workers, and UI are later slices; this foundation never executes a workflow.**

## Durable execution (slice 2)

`catalog.workflows` is a stable internal outbox consumer subscribed only to the listed exact v1 core events. It only fans matching enabled revisions into immutable `workflow_runs`; it never runs actions while holding a shared event-delivery lease. A run is unique for `(workspace, workflow revision, trigger event)`, stores the compiled revision and event reference/snapshot, and is eligible only when the event sequence is **strictly greater** than the enable activation boundary. Thus an event committed before enablement cannot run if delivered later.

Workflow runs use an independent PostgreSQL leased queue. They are at-least-once and unordered, retry with bounded exponential delay, and become `dead_letter` after five attempts. A failing run cannot block other matching workflows. Operators inspect tenant-scoped diagnostics with `GET /workflow-runs` (`workflows.read`) and replay only terminal runs using `POST /workflow-runs/{run_id}/replay` (`workflows.manage`). Internal event payloads are never returned.

Actions execute in definition order against current locked entity state through the normal entity mutation service; event facts are only used for immutable trigger/filter and declared event-field input. Actions are recorded durably by run/action index so completed actions are skipped on a retry. Workflow-originated events use `workflow:<workflow-id>`, retain trigger correlation/causation, and are excluded from default workflow intake to prevent feedback. Operators should investigate dead letters, repair the underlying catalog/schema condition, then replay; do not assume chronological arrival or exactly-once execution.
