---
title: Workflows
description: Automate small, auditable changes to an entity, or to the records that reference it, in response to catalog events, a schedule, or a manual trigger.
---

A workflow reacts to a trigger by applying a short list of actions to one entity: add or remove system tags, update system metadata, or write an attribute value. Workflows are versioned TOML, like blueprints, and every change they make goes through normal validation and appears in the audit log.

Workflows have no scripts, loops, queries, or network calls. They change the entity that triggered them and, with one bounded action, the records that link to it through a named relationship. For anything bigger, write an [extension](/extensions/build/).

## A first workflow

Tag a product for review whenever its title changes:

```toml
format_version = 1
code = "review-title-change"
name = "Review title changes"

[[triggers]]
event_type = "attribute_value.changed.v1"
attributes = ["title"]

[[actions]]
type = "system_tags_add"
tags = ["needs-review"]
```

Create, publish, and enable it under **Manage → Workflows**, or with the CLI:

```sh
acli workflow validate --file review-title-change.toml
acli workflow create --file review-title-change.toml
acli workflow publish <workflow-id> 1
acli workflow enable <workflow-id> 1
```

Enabling records the current point in the event stream. Only events after that point start runs; the workflow does not process history.

## Definition

| Key | Description |
| --- | --- |
| `format_version` | `1` for event triggers only. `2` adds manual and schedule triggers. |
| `code` | Unique [code](/reference/blueprint/#codes). |
| `name` | Display name. |
| `triggers` | One or more triggers. |
| `actions` | One or more actions, applied in order. |

## Event triggers

An event trigger names one entity event and optionally narrows it:

```toml
[[triggers]]
event_type = "entity.updated.v1"

[triggers.envelope]
source_kind = "api"

[triggers.facts]
"facts.0.attribute_code" = "price"
"facts.0.context_code" = "PL"
```

`event_type` is one of `entity.created.v1`, `entity.updated.v1`, `entity.migrated.v1`, `attribute_value.changed.v1`, `attribute_value.restored.v1`, or `relationship.changed.v1`.

`envelope` matches event properties exactly: `event_type`, `aggregate_kind`, `source_kind`, `source_name`, or `metadata.<key>`.

`facts` matches the facts carried by the event. Each fact describes one changed attribute. Its fields are `attribute_id`, `attribute_code`, `context_id`, `context_code`, `relationship_target_entity_id`, `change_kind`, `before_value`, and `after_value`. Address them as `facts.<index>.<field>`, where index `0` is the first fact.

The actions of an event-triggered workflow apply to the entity the event is about.

### React only to specific attributes

`attributes` lists 1 to 100 attribute codes. The trigger then starts a run only when at least one of those attributes changed in the event, wherever it appears among the event's facts:

```toml
[[triggers]]
event_type = "attribute_value.changed.v1"
attributes = ["body", "revision_notes", "license"]

[[triggers]]
event_type = "relationship.changed.v1"
attributes = ["license"]
```

- A change is a fact in the event. Saving a value that is identical to the current one records no fact, so it does not start a run.
- A single save can produce `attribute_value.changed.v1`, `relationship.changed.v1`, or `entity.updated.v1`, depending on what it contains. List every event type the attributes can change through, as above.
- `entity.created.v1` counts every value the new entity starts with, including defaults, as changed.
- **Relationship attributes:** adding or removing a target is a change to the relationship attribute. Each added or removed target is its own fact, with `change_kind` `relationship_add` or `relationship_remove`. Changes to the linked record itself are events on that record, not on this one.
- **File attributes:** uploading, linking, reordering, or removing files is recorded in the audit log but does not record an event, so a file attribute in `attributes` never starts a run. Track file changes with a scalar attribute that changes alongside them, such as a revision number.
- `attributes` can't be used with `entity.migrated.v1`, which carries no facts. It combines with `envelope` and `facts`: all of them must match.

## Manual and schedule triggers

With `format_version = 2`, a workflow can also be started by hand or on a schedule.

```toml
format_version = 2
code = "nightly-flag"
name = "Nightly flag"

[[triggers]]
type = "manual"

[[triggers]]
type = "schedule"
cron = "0 0 2 * * *"
timezone = "UTC"
target_entity_id = "00000000-0000-0000-0000-000000000001"

[[actions]]
type = "system_metadata_merge"
values = { last_nightly_check = "done" }
```

- **Manual**: start a run against one entity from the workflow page, or with `acli workflow run-now <workflow-id> --entity-id <uuid> --idempotency-key <key>`. It always uses the enabled revision.
- **Schedule**: a six-field cron in UTC. `timezone` must be `"UTC"`, which avoids repeated or skipped runs at daylight-saving changes. `target_entity_id` is the entity the actions apply to. If the server was down when a run was due, occurrences more than five minutes late are skipped and recorded. An occurrence is also skipped while a previous run is still pending.

Manual and scheduled runs have no event, so their actions must use fixed values.

## Actions

| `type` | Keys | Effect |
| --- | --- | --- |
| `system_tags_add` | `tags` (1 to 100, each up to 128 bytes) | Adds system tags. |
| `system_tags_remove` | `tags` | Removes system tags. |
| `system_metadata_merge` | `values` (table of scalar values) | Sets keys in system metadata. Keys may be dotted paths. |
| `system_metadata_delete` | `keys` | Removes keys from system metadata. |
| `attribute_write` | `attribute_code` and exactly one of `fixed` or `event_field` | Writes a scalar attribute value. `fixed` is a literal; `event_field` copies a value from the event, such as `facts.0.after_value`. |
| `referencing_entities_update` | `relationship_attribute`, optional `max_targets`, and nested `actions` | Applies the nested actions to every record that links to the trigger entity. See below. |

`attribute_write` obeys the blueprint: type checks, schemas, and [status transitions](/builders/validation/#statuses) all apply.

### Update records that reference the trigger entity

When a license, specification, or composition changes, the records that depend on it often need to go back to review. `referencing_entities_update` finds every live record whose `relationship_attribute` currently links to the trigger entity, in any context, and applies its nested actions to each of them:

```toml
format_version = 2
code = "license-changed"
name = "Send licensed products back to review"

[[triggers]]
event_type = "attribute_value.changed.v1"
attributes = ["terms"]

[[actions]]
type = "referencing_entities_update"
relationship_attribute = "license"
max_targets = 200

[[actions.actions]]
type = "attribute_write"
attribute_code = "status"
fixed = "in_review"

[[actions.actions]]
type = "system_tags_add"
tags = ["needs-review"]
```

- `relationship_attribute` is the code of the relationship attribute on the **referencing** records, not on the trigger entity.
- An action has 1 to 20 nested actions. They can set statuses and other attributes with fixed values, and add or remove system tags and metadata. They can't use `event_field`, and they can't contain another `referencing_entities_update`.
- `max_targets` defaults to 100 and can be at most 500. If more records link to the trigger entity, the action fails before changing any further record. Raise the limit or narrow the relationship.
- Each record is updated in its own save, with the same checks as any other edit: schemas, checks, unique keys, enforcing rules, and status transitions, including their conditions, [locks, and permission or role requirements](/builders/validation/#control-a-records-lifecycle). Transition requirements are checked against the person whose change started the workflow. A record that fails any of these, such as one whose status is locked or doesn't allow the transition, fails on its own; the other records are still updated.
- If any record fails, the run is retried. A retry only revisits records that failed or weren't reached; records already updated are never changed twice. A record that no longer exists or no longer links to the trigger entity by then is skipped.
- See the outcome for each record, including its latest error, with `acli workflow run-targets <run-id>` or `GET /workflow-runs/{id}/targets`. The run list shows the run's overall error once it becomes a dead letter.
- The records' own changes don't start workflows, like any other change made by a workflow.

## How runs behave

- Each run uses the revision that was enabled when it started, even if a newer one is enabled later.
- Delivery is at least once. Every action is recorded with its run so a retry never applies it twice. A `referencing_entities_update` action also records each record it updates.
- A failed run is retried with increasing delays and becomes a dead letter after five attempts. Replay it from the workflow's run list or with `acli workflow run-replay <run-id>`.
- Disabling a workflow cancels its queued and running runs.
- Changes made by a workflow do not trigger workflows again by default, and chains are capped at a depth of eight.

## Not available yet

- `extension_event` triggers are accepted in `format_version = 2` definitions but no extension event is delivered to workflows yet.
- Workflows cannot call webhooks, send email, or read secrets.

## Permissions

`workflows.read` lets someone see workflows and run history. `workflows.manage` lets them create, publish, enable, disable, run, and replay. Both are granted to the owner and admin roles by default.
