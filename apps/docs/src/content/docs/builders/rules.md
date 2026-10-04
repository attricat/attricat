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

An entity **fails** when the predicate is not satisfied. Rules use the same predicates as [entity checks](/builders/validation/#compare-attributes-with-checks), [status transition conditions](/builders/validation/#conditions-on-transitions), and [publication channel checks](/guides/publishing/#require-checks-before-publication). The [blueprint reference](/reference/blueprint/#predicates) lists every key.

- **Values:** `required`, `compare` (with another attribute or a literal), `one_of`, and `relative_date` (against now plus `offset_days`).
- **Tags:** `has_tag` and `missing_tag` check system tags.
- **Linked records:** `linked` checks the records an entity links to; `referenced_by` counts the records that link to it.
- **Reporting only:** `stale` (not updated within an age limit), `unique` (no other entity of the blueprint has the same values), and `acyclic` (a relationship never leads back to the entity).
- **Combinations:** `all_of` and `any_of`.

A comparison whose value is missing is satisfied, and so are `one_of` and `relative_date` when the attribute is empty. Combine them with `required` when the value must exist:

```toml
[predicate]
type = "all_of"
predicates = [
  { type = "required", attribute_code = "valid_until" },
  { type = "relative_date", attribute_code = "valid_until", op = "gt", offset_days = 30 },
]
```

`linked` and `referenced_by` follow one relationship hop. Their nested predicate reads the other record and can compare with the entity being checked through `subject_attribute_code`:

```toml
# Every facility on a supplier certificate belongs to that supplier.
[predicate]
type = "linked"
relationship_code = "facilities"
predicate = { type = "compare", attribute_code = "supplier", op = "eq", subject_attribute_code = "supplier" }
```

```toml
# A nonconformance has no open corrective actions.
[predicate]
type = "referenced_by"
blueprint_code = "corrective_action"
relationship_code = "nonconformance"
max = 0
predicate = { type = "one_of", attribute_code = "state", values = ["open"] }
```

### Expiry and other time-based checks

`relative_date` compares with the time the rule runs. Give such a rule a `schedule` trigger: an entity that nobody edits can still expire overnight, and only a scheduled run notices.

```toml
format_version = 1
code = "certificate-valid-30-days"
name = "Supplier certificates are valid for at least 30 more days"
severity = "warning"

[[triggers]]
type = "schedule"
cron = "0 0 5 * * *"
timezone = "UTC"

[predicate]
type = "relative_date"
attribute_code = "valid_until"
op = "gt"
offset_days = 30
```

A negative `offset_days` looks back: `op = "gte"` with `offset_days = -365` means "within the last year".

### Contexts

A rule attached to a context checks the entity's resolved values in that context, including inherited values. A rule without a context checks every context and fails if any of them fails; the finding's evidence lists the failing context codes under `contexts`.

### Changes to linked records

`linked` and `referenced_by` depend on other records. When a linked or referencing record changes, event-triggered rules also re-run for up to 100 entities that depend on it, so their findings stay current. Rules with only schedule or manual triggers notice the change on their next run.

The [blueprint reference](/reference/blueprint/#predicates) has the comparison rules for each type and the limits on nesting and linked records.

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

## Enforce a rule

By default a rule only reports findings. An **enforcing** rule also stops writes that would leave an entity violating it. Add an `enforcement` table:

```toml
format_version = 1
code = "released-documents-approved"
name = "Released documents have an approver"
severity = "error"

[[triggers]]
type = "event"
event_type = "entity.updated.v1"

[predicate]
type = "required"
attribute_code = "approved_by"

[enforcement]
on_save = false

[[enforcement.transitions]]
attribute_code = "status"
from = "review"
to = "released"
```

`on_save = true` rejects any write that leaves the entity violating the rule. Each entry in `transitions` guards a change of a [status](/builders/validation/#statuses) attribute into `to`, optionally only from `from`. Guarded transitions are checked on the state the change produces, so a value saved in the same write counts. Inside a blueprint, write the table as `[rules.enforcement]` and `[[rules.enforcement.transitions]]` after the rule's `[[rules]]` header.

Enforcement needs a severity of `error` or `critical` and a predicate that can be checked during a save. The [blueprint reference](/reference/blueprint/#rules) lists every key and requirement.

A rule attached to a context enforces in that context only. A rule without a context enforces in every context.

A write that violates an enforcing rule is rejected with `422 rule_violation`, and nothing is saved. The response lists each violated rule, the contexts, and the attributes involved. See [Validation](/builders/validation/#errors-and-how-to-fix-them).

### Dry run before enabling

Enforcing a rule on existing data can block people who did nothing wrong. So when the rule's blueprint revision already has entities, Attricat needs a completed **full dry run** of the exact revision you are enabling. Otherwise enabling fails with `409 rule_dry_run_required`.

1. Publish the revision.
2. Run a dry run over all entities, without `--entity-id`, and wait for it to complete on the **Runs** tab. A dry run may target a published revision that is not enabled yet.

   ```sh
   acli rule run-now <rule-id> --idempotency-key enforce-preview --dry-run
   ```

   The CLI dry-runs the enabled revision, or the latest published one when none is enabled. To dry-run a specific revision while another is enabled, call `POST /rules/{id}/run-now` with `{"dry_run": true, "idempotency_key": "…", "version": 2}`.
3. Enable the revision.

If the dry run found violations, enabling fails with `409 rule_has_existing_violations`, and `details.existing_violations` gives the count. Either fix those entities and dry-run again, or accept them explicitly through the API:

```http
POST /rules/{id}/versions/{version}/enable
{"accept_existing_violations": true}
```

A dry run stops after 10,000 entities. When the blueprint revision has more, the dry run can't vouch for the rest, so enabling still fails with `409 rule_dry_run_required` unless you send `accept_existing_violations` as above.

An entity that already violates an enforcing rule cannot be saved until a save fixes the violation.

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
