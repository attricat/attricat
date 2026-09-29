---
title: Workflows
description: Automate small, auditable changes to one entity in response to catalog events, a schedule, or a manual trigger.
---

A workflow reacts to a trigger by applying a short list of actions to one entity: add or remove system tags, update system metadata, or write an attribute value. Workflows are versioned TOML, like blueprints, and every change they make goes through normal validation and appears in the audit log.

Workflows have no scripts, loops, queries, or network calls, and they cannot change more than the one entity that triggered them. For anything bigger, write an [extension](/extensions/build/).

## A first workflow

Tag a product for review whenever its title changes:

```toml
format_version = 1
code = "review-title-change"
name = "Review title changes"

[[triggers]]
event_type = "attribute_value.changed.v1"

[triggers.facts]
"facts.0.attribute_code" = "title"

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

`attribute_write` obeys the blueprint: type checks, schemas, and `readonly` all apply.

## How runs behave

- Each run uses the revision that was enabled when it started, even if a newer one is enabled later.
- Delivery is at least once. Every action is recorded with its run so a retry never applies it twice.
- A failed run is retried with increasing delays and becomes a dead letter after five attempts. Replay it from the workflow's run list or with `acli workflow run-replay <run-id>`.
- Disabling a workflow cancels its queued and running runs.
- Changes made by a workflow do not trigger workflows again by default, and chains are capped at a depth of eight.

## Not available yet

- `extension_event` triggers are accepted in `format_version = 2` definitions but no extension event is delivered to workflows yet.
- Workflows cannot call webhooks, send email, or read secrets.

## Permissions

`workflows.read` lets someone see workflows and run history. `workflows.manage` lets them create, publish, enable, disable, run, and replay. Both are granted to the owner and admin roles by default.
