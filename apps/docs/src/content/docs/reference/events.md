---
title: Event reference
description: The catalog events Attricat records for every change, their envelope, and delivery guarantees.
---

Every catalog change records an event in the same database transaction as the change and its audit record. If the change is saved, so is the event; if it fails, neither exists.

Events drive [workflows](/builders/workflows/), [rules](/builders/rules/), and [extension event handlers](/extensions/server/#handle-catalog-events). They are not exposed as a public feed.

## Event types

| Type | Recorded when |
| --- | --- |
| `record.created.v1` | A record is created. |
| `record.updated.v1` | A record's values, relationships, or annotations change. |
| `record.deleted.v1` | A record is deleted. |
| `record.migrated.v1` | A record moves to a newer blueprint revision. |
| `record.published.v1` | A record is published to a channel. |
| `record.unpublished.v1` | A record's publication is withdrawn. |
| `attribute_value.changed.v1` | An attribute value is set, replaced, or removed. |
| `attribute_value.restored.v1` | A value is restored from history. |
| `relationship.changed.v1` | Relationship targets are added or removed. |
| `blueprint.created.v1` | A blueprint is created. |
| `blueprint.revision_created.v1` | A new blueprint revision is drafted. |
| `blueprint.published.v1` | A blueprint revision is published. |
| `context.created.v1`, `context.updated.v1`, `context.deleted.v1` | A context changes. |

Extensions publish their own types, named `plugin.<extension-id>.<name>.vN`. The `record`, `attribute_value`, `relationship`, `blueprint`, and `context` namespaces are reserved.

A version suffix never changes meaning. An incompatible payload gets a new `.vN` type, and consumers subscribe to the exact versions they understand.

## Envelope

| Field | Description |
| --- | --- |
| `id` | Event UUID. Use it to deduplicate. |
| `event_type` | Such as `record.updated.v1`. |
| `occurred_at` | When the change was saved. |
| `aggregate_kind`, `aggregate_id` | What changed, such as `record` and its UUID. |
| `correlation_id` | Shared by everything that came from one request or job. |
| `causation_id` | The event that directly caused this one, if any. |
| `source_kind` | `api`, `worker`, `plugin`, or `system`. |
| `source_name` | Which producer, such as a workflow (`workflow:<id>`) or extension (`extension:<extension-id>`). |
| `metadata`, `payload` | JSON objects, each up to 64 KiB. |

## Record payloads

Record and value events describe what changed, not the whole record:

```json
{
  "record_id": "7f1c…",
  "blueprint_id": "a2d4…",
  "blueprint_version": 3,
  "facts": [{
    "attribute_id": "c9e0…",
    "attribute_code": "price",
    "context_id": "00000000-0000-4000-8000-000000000001",
    "context_code": "default",
    "relationship_target_record_id": null,
    "change_kind": "set",
    "before_value": 49.0,
    "after_value": 39.0
  }]
}
```

To act on the record's current state, read it; the payload is only a description of the change.

### Which changes produce facts

`facts` contains one entry per attribute value that actually changed. A save that leaves a value as it was adds no fact for it. [Workflow triggers](/builders/workflows/#react-only-to-specific-attributes) can filter on `attribute_code` with `attributes`.

- **Scalar attributes:** `change_kind` is `set`, `replace`, or `remove`; `restore` on `attribute_value.restored.v1`. `record.created.v1` lists every initial value, including defaults, as `set`.
- **Relationship attributes:** each added or removed target is its own fact, with `change_kind` `relationship_add` or `relationship_remove`, `relationship_target_record_id` set, and the target ID as the value. A save that only changes relationships records `relationship.changed.v1`; one that also changes other values records `record.updated.v1`.
- **File attributes:** uploading, linking, reordering, or removing files is recorded in the audit log but records no event and no fact.
- System tag and system metadata changes are not attribute values and add no fact.
- **Migrations:** `record.migrated.v1` has no `facts`. Its payload names the record, its blueprint, `source_version`, `target_version`, and `migration_id`. When the migration dropped or re-pointed a relationship, `released_relationships` lists the targets the record no longer points to, as `attribute_code` and `target_record_ids` (at most 100 per relationship and 1,000 per event).

## Delivery guarantees

- **At least once.** A consumer can receive the same event twice, for example if it crashes after doing its work but before the delivery is recorded. Make handlers idempotent, keyed on the event ID.
- **Unordered.** Events can arrive in a different order from the one they were recorded in.
- **From now on.** A new consumer starts at the current position and does not receive older events.
- **Retries.** A failed delivery is retried after 1, 2, 4, … seconds, up to 60, and becomes a dead letter after five attempts. These are the defaults; see [Background work](/reference/configuration/#background-work).

See [Monitoring](/operate/monitoring/#failed-event-deliveries) for finding and replaying dead letters.
