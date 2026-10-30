---
title: Data quality rules
description: Define versioned checks that flag records with missing, stale, or wrongly tagged data, and manage the findings.
---

A rule checks records of one blueprint against a simple condition and saves a **finding** for each record that fails. Findings resolve themselves when the record is fixed. Rules only read data; they never change a record.

Use rules for questions like "which published products have no title?" or "which prices haven't been touched in a year?". To change data automatically, use a [workflow](/builders/workflows/).

## Define a rule

A rule has a code, a name, a severity, one to eight triggers, and exactly one predicate.

```yaml
format_version: 1
code: title-required
name: Products have a title
description: 'Shoppers search by title, so every product needs one before it goes live.'
severity: error

triggers:
  - type: schedule
    cron: '0 0 6 * * *'
    timezone: UTC

  - type: event
    event_type: record.updated.v1

  - type: manual

predicate:
  type: required
  attribute_code: title
```

| Key | Description |
| --- | --- |
| `format_version` | `1`. |
| `code` | Unique [code](/reference/blueprint/#codes). |
| `name` | Display name. |
| `description` | Optional. What the rule checks and why, in plain words, up to 2,000 characters. It is shown on the rule's page, in the rules list, in the editor's preview and beside the rule's findings on each record, so the people who fix them know what is expected. Line breaks are kept; Markdown is not rendered. |
| `severity` | `info`, `warning`, `error`, or `critical`. |
| `triggers` | 1 to 8 triggers. See below. |
| `predicate` | The condition a record must satisfy. See below. |

### Triggers

| `type` | Keys | Runs |
| --- | --- | --- |
| `manual` | | When someone chooses **Run now**. |
| `schedule` | `cron`, `timezone: UTC` | On a six-field cron schedule (seconds first), in UTC. `0 0 6 * * *` is 06:00 every day. |
| `event` | `event_type` | For the changed record, after one of: `record.created.v1`, `record.updated.v1`, `record.migrated.v1`, `attribute_value.changed.v1`, `attribute_value.restored.v1`, `relationship.changed.v1`. |
| `post_import` | | Reserved for future import integration. |

Triggers decide when a rule checks every record of its blueprint. Editing a record always re-checks that record against every enabled rule of its blueprint, whatever their triggers, so the record's findings match what was saved. An `event` trigger adds the re-runs for [dependent records](#changes-to-linked-records).

### Predicates

A record **fails** when the predicate is not satisfied. Rules use the same predicates as [record checks](/builders/validation/#compare-attributes-with-checks), [status transition conditions](/builders/validation/#conditions-on-transitions), and [publication channel checks](/guides/publishing/#require-checks-before-publication). The [blueprint reference](/reference/blueprint/#predicates) lists every key.

- **Values:** `required`, `compare` (with another attribute or a literal), `one_of`, and `relative_date` (against now plus `offset_days`).
- **Tags:** `has_tag` and `missing_tag` check system tags.
- **Linked records:** `linked` checks the records the checked record links to; `referenced_by` counts the records that link to it.
- **Reporting only:** `stale` (not updated within an age limit), `unique` (no other record of the blueprint has the same values), and `acyclic` (a relationship never leads back to the record).
- **Combinations:** `all_of` and `any_of`.

For an attribute with a fixed list of values, such as a status or a choice, every value `one_of` lists, and the value of an `eq` or `ne` comparison, must be one of them; saving reports a value it does not allow, with the values it does.

A comparison whose value is missing is satisfied, and so are `one_of` and `relative_date` when the attribute is empty. Combine them with `required` when the value must exist:

```yaml
predicate:
  type: all_of
  predicates:
    - {type: required, attribute_code: valid_until}
    - type: relative_date
      attribute_code: valid_until
      op: gt
      offset_days: 30
```

`linked` and `referenced_by` follow one relationship hop. Their nested predicate reads the other record and can compare with the record being checked through `subject_attribute_code`:

```yaml
# Every facility on a supplier certificate belongs to that supplier.
predicate:
  type: linked
  relationship_code: facilities
  predicate:
    type: compare
    attribute_code: supplier
    op: eq
    subject_attribute_code: supplier
```

```yaml
# A nonconformance has no open corrective actions.
predicate:
  type: referenced_by
  blueprint_code: corrective_action
  relationship_code: nonconformance
  max: 0
  predicate:
    type: one_of
    attribute_code: state
    values: [open]
```

### Expiry and other time-based checks

`relative_date` compares with the time the rule runs. Give such a rule a `schedule` trigger: a record that nobody edits can still expire overnight, and only a scheduled run notices.

```yaml
format_version: 1
code: certificate-valid-30-days
name: Supplier certificates are valid for at least 30 more days
severity: warning

triggers:
  - type: schedule
    cron: '0 0 5 * * *'
    timezone: UTC

predicate:
  type: relative_date
  attribute_code: valid_until
  op: gt
  offset_days: 30
```

A negative `offset_days` looks back: `op: gte` with `offset_days: -365` means "within the last year".

### Contexts

A rule attached to a context checks the record's resolved values in that context, including inherited values. A rule without a context checks every context and fails if any of them fails; the finding's evidence lists the failing context codes under `contexts`.

### Changes to linked records

`linked` and `referenced_by` depend on other records. When a linked or referencing record changes, event-triggered rules also re-run for up to 100 records that depend on it, so their findings stay current. That includes a record a referencing record stops pointing to, for example when a corrective action is moved to another nonconformance, as recorded in the event's [facts](/reference/events/#which-changes-produce-facts). A blueprint migration that drops or re-points a relationship records no facts, but its `record.migrated.v1` event lists the released records, so they re-run too. Rules with only schedule or manual triggers notice the change on their next run.

The [blueprint reference](/reference/blueprint/#predicates) has the comparison rules for each type and the limits on nesting and linked records.

## Where rules live

Rules are created under **Manage → Data quality rules**, separately from blueprints. A rule belongs to one blueprint and optionally to one context, not to a blueprint revision. Choose **New rule**, pick the blueprint and, if needed, a context, and write the YAML in the same editor as blueprints: it completes keys and the blueprint's attribute codes, offers ready-made triggers and predicates under **Insert**, and checks the rule against the blueprint's latest published revision as you type. The blueprint needs a published revision. Once you pick it, **Examples** offers rules to start from: a required value, allowed values, a review date, a unique value, an expiry date and linked records. **Save** creates the rule, disabled.

The list shows one row per rule with its blueprint, its severity, how many of its findings are open and acknowledged (each count links to those findings), its current revision and when it changed, an **Enabled** switch, and **Dry run** and **Run now**. Each rule has its own page with a **Definition** tab, showing what it checks, spelled out part by part with attribute and value labels as in the editor's preview, its severity, its finding counts, its current revision, the definition, and its recent runs with a link to all of them in **Run history**, and a **Changelog** tab. **Dry run**, **Run now**, **Enable** and **Disable** are on the rule's page too. **Run history** can be narrowed to one rule.

The CLI does the same:

```sh
acli rule create --blueprint-id <uuid> --file title-required.yaml
acli rule enable <rule-id>
```

A new rule starts disabled and runs only once enabled, with the switch or the CLI. `acli rule disable` stops it. Disabling a rule resolves its findings, so they no longer count against records or tag them **Needs attention**; enabling it again checks every record of its blueprint straight away.

### Change a rule

A rule has one current definition. **Edit** on the rule's page opens the editor. While you have unsaved changes, it shows them compared with the current revision and offers **Dry run this change**. Saving a changed definition adds a new revision that becomes current at once; there are no drafts and nothing to publish. The rule's code cannot change.

```sh
acli rule update <rule-id> --file title-required.yaml
```

When a new revision is saved, the findings of earlier definitions are resolved. If the rule is enabled, its schedules move to the new revision and it re-checks every record of its blueprint straight away. Saving a definition identical to the current one fails with `409 rule_revision_unchanged`.

The **Changelog** tab keeps every earlier revision, with who saved it and when. **Compare with previous** shows a revision next to the one before it, and **From** and **To** compare any two revisions side by side. The page address keeps the pair, for example `?from=2&to=3`, so you can share a comparison. `acli rule revisions <rule-id>` and `acli rule revision <rule-id> <version>` read the same history. Findings and runs name the rule revision that produced them, such as "Products have a title v3", and link to it in the changelog.

### Rules and blueprint revisions

A rule checks the records of every revision of its blueprint that it fits. It fits a revision when every attribute that its predicate and enforcement transitions name exists there with a suitable type and, for guarded transitions, the status attribute and its codes exist. For a revision it does not fit, the rule skips that revision's records: it records no findings for them and does not enforce anything, and an open finding of such a record is resolved. Runs count those records as skipped; the **Runs** tab shows, for example, "3 (1 skipped)".

Saving or validating a rule checks it against the blueprint's latest published revision only. A warning icon in the list, and a warning on the rule's page, mark a rule that skips some revisions of its blueprint and say why.

Rules never block publishing a blueprint revision. If enabled rules do not fit the revision you publish, the blueprint page lists them in a warning, and every other member with the `rules.manage` permission gets an [Inbox](/guides/inbox/) notification that links to the rule. Edit the rule to fit the new revision, or leave it if it should check only older records.

## Run a rule

Enabled rules run on their triggers. You can also run one by hand from the rule's page or the CLI:

```sh
acli rule run-now <rule-id> --idempotency-key 2026-03-01-audit
acli rule run-now <rule-id> --idempotency-key check-one --record-id <uuid>
acli rule run-now <rule-id> --idempotency-key preview --dry-run
```

A dry run reports how many records would fail and names the first 50 of them with what each would report, and saves no findings. **Dry run** in the list opens its result in a dialog; `GET /rule-runs/{id}` returns it as `dry_run_failures`. It works whether or not the rule is enabled; a normal run needs the rule enabled. Reusing an idempotency key returns the original run instead of starting a new one.

Each run processes candidates in pages of 500 and stops after 10,000. A schedule never overlaps itself: if the previous run is still pending, the next occurrence is skipped.

## Enforce a rule

By default a rule only reports findings. An **enforcing** rule also stops writes that would leave a record violating it. Add an `enforcement` mapping:

```yaml
format_version: 1
code: released-documents-approved
name: Released documents have an approver
severity: error

triggers:
  - type: event
    event_type: record.updated.v1

predicate:
  type: required
  attribute_code: approved_by

enforcement:
  on_save: false
  transitions:
    - {attribute_code: status, from: review, to: released}
```

`on_save: true` rejects any write that leaves the record violating the rule. Each entry in `transitions` guards a change of a [status](/builders/validation/#statuses) attribute into `to`, optionally only from `from`. Guarded transitions are checked on the state the change produces, so a value saved in the same write counts.

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `enforcement.on_save` | boolean | `false` | Reject every write that leaves the record violating the rule. |
| `enforcement.transitions` | list of mappings | `[]` | Up to 16 guarded status changes, each with `attribute_code`, optional `from`, and `to`. Without `from`, every change into `to` is guarded. |

An `enforcement` mapping needs `on_save: true` or at least one transition, a severity of `error` or `critical`, and a [predicate](/reference/blueprint/#predicates) without `stale`, `unique`, or `acyclic`, which cannot be checked during a save. Each transition's `attribute_code` must be a status attribute, and `from` and `to` must be codes of its `enum`.

A rule attached to a context enforces in that context only. A rule without a context enforces in every context.

A write that violates an enforcing rule is rejected with `422 rule_violation`, and nothing is saved. The response lists each violated rule, the contexts, and the attributes involved. See [Validation](/builders/validation/#errors-and-how-to-fix-them).

### Dry run before enabling

Enforcing a rule on existing data can block people who did nothing wrong. So when the rule's blueprint already has records, Attricat needs a completed **full dry run** of the rule's current definition before you enable it. Otherwise enabling fails with `409 rule_dry_run_required`.

1. Save the rule.
2. Run a dry run over all records, without `--record-id`, and wait for it to complete on the **Runs** tab. A dry run works on a disabled rule.

   ```sh
   acli rule run-now <rule-id> --idempotency-key enforce-preview --dry-run
   ```

3. Enable the rule.

If the dry run found violations, enabling fails with `409 rule_has_existing_violations`, and `details.existing_violations` gives the count. Either fix those records and dry-run again, or accept them explicitly:

```sh
acli rule enable <rule-id> --accept-existing-violations
```

Through the API, send `POST /rules/{id}/enable` with `{"accept_existing_violations": true}`.

A dry run stops after 10,000 records. When the blueprint has more, the dry run can't vouch for the rest, so enabling still fails with `409 rule_dry_run_required` unless you accept existing violations as above. In that case the error message says the dry run stopped early, and `details` is `{"truncated": true, "existing_violations": …}` with the violations it found among the records it checked. Such a run has `"truncated": true` in `GET /rule-runs`.

Changing an enabled enforcing rule passes the same check for the new definition. A dry run can check a definition before it is saved: in the editor, choose **Dry run this change**, and if it finds violations, **Save anyway** accepts them. From the CLI, pass the file to the dry run and then accept the violations when you save:

```sh
acli rule run-now <rule-id> --idempotency-key change-preview --dry-run --definition-file rule.yaml
acli rule update <rule-id> --accept-existing-violations --file rule.yaml
```

Through the API, `POST /rules/{id}/run-now` accepts `"definition"` with the YAML on a dry run only.

A record that already violates an enforcing rule cannot be saved until a save fixes the violation.

## Work with findings

**Manage → Data quality rules → Findings** lists findings with their rule, record, context, and status. There is one finding per rule, record, and context. When a later evaluation passes, the finding is resolved. If the problem comes back, the same finding reopens.

**Acknowledge** a finding to record that someone has seen it. It stays until the record passes. The findings list shows how many findings match its filters; select open findings and choose **Acknowledge selected**, or **Acknowledge all … open** to acknowledge every open finding the filters match, up to 5,000 at a time. Acknowledging cannot be undone.

A record's own page lists its open and acknowledged findings in its **Data quality** tab, 20 at a time with **Load more findings** for the rest; the data quality chip in the record's header opens the tab. A finding's field name leads to that field; for a finding about other contexts, it switches the record to the first context the finding fails in. After an edit, the list refreshes once the record's checks finish. **Re-run checks** checks the record against every enabled rule of its blueprint straight away, for example after a rule changed.

In the [Explorer](/guides/explore/#filters), the **Data quality** filter narrows results to records with open findings, only acknowledged findings, or none.

```sh
acli rule findings --record-id <uuid>
acli rule finding-count --blueprint-id <uuid> --severity error
acli rule acknowledge <finding-id>
acli rule acknowledge-many --matching --rule-id <uuid>
```

The **Run history** tab shows runs and failures. It lists runs over all records unless **Scope** is set to **One record**, which shows the runs that checked a single record, such as after each edit, or **Any**. **Outcome** gives the findings a run added and resolved, or for a dry run how many records would fail. A run that fails five times becomes a dead letter; fix the cause and replay it with `acli rule run-replay <run-id>`.

## Permissions

`rules.read` lets someone see rules, runs, and findings. `rules.manage` lets them create, change, enable, disable, and run rules, and acknowledge findings. Both are granted to the owner and admin roles by default.

Everyone who can read a record sees its findings: the **Data quality** tab, the chip in its header, and the highlighted fields, so the people who fix the data see what to fix. Without `rules.read` they see the rule's name but not its page. From the CLI, `acli record rule list|findings|finding-count|runs <record-id>` reads a record's rules, findings, and runs without `rules.read`.
