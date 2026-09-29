---
title: Data quality rules
description: Define versioned checks that flag entities with missing, stale, or wrongly tagged data, and manage the findings.
---

A rule checks entities of one blueprint against a simple condition and records a **finding** for each entity that fails. Findings resolve themselves when the entity is fixed. Rules only read data; they never change an entity.

Use rules for questions like "which published products have no title?" or "which prices haven't been touched in a year?". To change data automatically, use a [workflow](/builders/workflows/).

## Define a rule

A rule has a code, a name, a severity, one to eight triggers, and exactly one predicate.

```toml
format_version = 1
code = "title-required"
name = "Products have a title"
severity = "error"

[[triggers]]
type = "schedule"
cron = "0 0 6 * * *"
timezone = "UTC"

[[triggers]]
type = "event"
event_type = "entity.updated.v1"

[[triggers]]
type = "manual"

[predicate]
type = "required"
attribute_code = "title"
```

| Key | Description |
| --- | --- |
| `format_version` | `1`. Omit it when the rule is embedded in a blueprint. |
| `code` | Unique [code](/reference/blueprint/#codes). |
| `name` | Display name. |
| `severity` | `info`, `warning`, `error`, or `critical`. |
| `triggers` | 1 to 8 triggers. See below. |
| `predicate` | The condition an entity must satisfy. See below. |

### Triggers

| `type` | Keys | Runs |
| --- | --- | --- |
| `manual` | | When someone chooses **Run now**. |
| `schedule` | `cron`, `timezone = "UTC"` | On a six-field cron schedule (seconds first), in UTC. `0 0 6 * * *` is 06:00 every day. |
| `event` | `event_type` | For the changed entity, after one of: `entity.created.v1`, `entity.updated.v1`, `entity.migrated.v1`, `attribute_value.changed.v1`, `attribute_value.restored.v1`, `relationship.changed.v1`. |
| `post_import` | | Reserved for future import integration. |

### Predicates

An entity **fails** when the predicate is not satisfied.

| `type` | Keys | Satisfied when |
| --- | --- | --- |
| `required` | `attribute_code` | The attribute has a value. |
| `stale` | `attribute_code`, `max_age_seconds` (1 to 31536000) | The attribute was updated within the last `max_age_seconds`. |
| `has_tag` | `tag` | The entity has this system tag. |
| `missing_tag` | `tag` | The entity does not have this system tag. |

## Where rules live

Rules can be written two ways.

**Inside a blueprint**, as `[[rules]]` tables without `format_version`. They are versioned and published with the blueprint:

```toml
[[rules]]
code = "price-fresh"
name = "Prices reviewed in the last 90 days"
severity = "warning"
triggers = [{ type = "schedule", cron = "0 0 3 * * 1", timezone = "UTC" }]
predicate = { type = "stale", attribute_code = "price", max_age_seconds = 7776000 }
```

**As standalone rules** under **Manage → Data quality rules**, attached to one published blueprint revision and optionally to one context:

```sh
acli rule create --blueprint-id <uuid> --blueprint-version 3 --file title-required.toml
acli rule publish <rule-id> 1
acli rule enable <rule-id> 1
```

Standalone rules have their own drafts and revisions. A rule revision must be published and then enabled before it runs. Only one revision of a rule is enabled at a time; `acli rule disable` stops it.

## Run a rule

Enabled rules run on their triggers. You can also run one by hand from the rule's page or the CLI:

```sh
acli rule run-now <rule-id> --idempotency-key 2026-03-01-audit
acli rule run-now <rule-id> --idempotency-key check-one --entity-id <uuid>
acli rule run-now <rule-id> --idempotency-key preview --dry-run
```

A dry run reports how many entities would fail and saves no findings. Reusing an idempotency key returns the original run instead of starting a new one.

Each run processes candidates in pages of 500 and stops after 10,000. A schedule never overlaps itself: if the previous run is still pending, the next occurrence is skipped.

## Work with findings

**Manage → Data quality rules → Findings** lists findings with their rule, entity, context, and status. There is one finding per rule, entity, and context. When a later evaluation passes, the finding is resolved. If the problem comes back, the same finding reopens.

**Acknowledge** a finding to record that someone has seen it. It stays until the entity passes.

```sh
acli rule findings --entity-id <uuid>
acli rule acknowledge <finding-id>
```

The **Runs** tab shows run history and failures. A run that fails five times becomes a dead letter; fix the cause and replay it with `acli rule run-replay <run-id>`.

## Permissions

`rules.read` lets someone see rules, runs, and findings. `rules.manage` lets them create, publish, enable, disable, and run rules, and acknowledge findings. Both are granted to the owner and admin roles by default.
