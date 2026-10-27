# Rules

Rules are Attricat-owned, versioned blueprint checks. They are deliberately separate from workflows: a workflow mutates one trigger record, while a rule evaluates a restricted predicate over a bounded blueprint candidate page and manages a finding lifecycle.

## Definition contract

A rule is a standalone TOML document (`contracts/rule-definition-v1.schema.json`), created through `/rules` for a blueprint and optionally a context. Blueprints do not contain rules: a blueprint definition with `[[rules]]` tables is rejected when saved, and revisions stored before that still parse, with their tables ignored. A rule has `format_version = 1`, a severity, one to eight triggers, and exactly one predicate. Unknown fields and arbitrary code/selectors are rejected.

```toml
format_version = 1
code = "product-title-required"
name = "Published products have a title"
severity = "error"

[[triggers]]
type = "schedule"
cron = "0 0 * * * *"
timezone = "UTC"

[[triggers]]
type = "manual"

[predicate]
type = "required"
attribute_code = "title"
```

Schedules are six-field UTC cron expressions. Event triggers accept only catalog record and value event types; post-import is a reserved declarative trigger shape.

Triggers decide when a rule checks its blueprint's records as a whole.
Changing a record (`record.created.v1`, `record.updated.v1`,
`record.migrated.v1`, `attribute_value.changed.v1`,
`attribute_value.restored.v1`, `relationship.changed.v1`) always re-checks that
record for every enabled rule of its blueprint, whatever the rule's triggers, so
a record's findings follow its saved state. Dependent records (below) re-run
only for rules with a matching event trigger.

## Blueprint revisions

A rule belongs to a blueprint, not to one of its revisions, and checks the
records of every revision it fits. It fits a revision when every attribute its
predicate and guarded transitions name exists there with a type the predicate
can check, and each guarded transition names a status attribute and codes of
it (`attricat_rules::applicability`). The check runs in memory against the
revision's attributes, so it costs no query per record:

- Saving or validating a rule checks it against the blueprint's latest
  published revision and rejects it with `422 invalid_rule_definition` when it
  does not fit. A blueprint without a published revision returns
  `422 blueprint_not_published`.
- Runs skip records on a revision the rule does not fit: no finding is opened,
  an open one is resolved, and the run counts the record in
  `candidates_skipped`.
- Enforcement (`checks::enabled_rules`) and the cached enabled-rule sets file
  each rule only under the revisions it fits, so a skipped revision's records
  are never blocked by it.
- `GET /rules` and `GET /rules/{rule_id}` list, as `not_applicable`, each
  revision among the latest published one and those with live records that the
  rule does not fit, with a `reason` (`RuleInapplicability`: `attribute_missing`,
  `attribute_type_changed`, `status_changed`) and a message.

Publishing a blueprint revision is never refused because of rules. The publish
response lists the enabled rules the new revision does not fit as
`rule_warnings` (`rule_id`, `code`, `name`, `reason`, `message`), and the same
transaction notifies every member who holds `rules.manage` for the workspace,
except the publisher, with a `rule.not_applicable` notification about the rule
(see [Notifications](notifications.md)).

## Revisions and lifecycle

A rule has one current revision and an enabled switch (`rule_lifecycles`:
`current_version`, `enabled`). Saving a changed definition with
`PUT /rules/{rule_id}` stores a new revision and makes it current in the same
transaction; there are no drafts and no publication step. Every revision stays
in `rules` as the rule's changelog, with `created_by_user_id` and `created_at`.
The rule code, blueprint and context never change. Saving a definition
identical to the current one returns `409 rule_revision_unchanged`; returning
to an earlier definition is a change.

Saving a new revision also:

- resolves the open findings of earlier definitions (finding identity includes
  the definition hash), so a finding always belongs to the current definition;
- cancels the rule's unfinished non-dry runs, which would report against the
  earlier definition;
- for an enabled rule, replaces its schedule cursors with the new revision's
  and queues a full manual run (`revision:<version>`) that re-checks every
  record.

A new rule starts disabled with revision 1. `POST /rules/{rule_id}/enable` and
`POST /rules/{rule_id}/disable` switch the rule; enabling records the
activation boundary (only later events fan out) and starts schedule cursors.

## Predicates

Rules share one declarative predicate engine with record-schema checks
(`x-attricat-checks`), status transition conditions and publication channel
gates. A predicate *holds* when the data is acceptable; a rule reports a
finding when it does not. Every predicate type, field, limit and comparison
rule, including the rules-only `stale`, `unique` and `acyclic`, is defined in
[JSON Schema Validation](json-schema-validation.md#predicates). `unique`
compares the record with every other live record of the blueprint family
(any revision), in the evaluated context. `referenced_by` counts only live
records in the same context.

Expiry, with a schedule trigger so findings open as dates pass:

```toml
[predicate]
type = "relative_date"
attribute_code = "certificate_expires_on"
op = "gt"
offset_days = 30
```

No open corrective actions reference this nonconformance:

```toml
[predicate]
type = "referenced_by"
blueprint_code = "corrective_action"
relationship_code = "nonconformance"
max = 0
predicate = { type = "one_of", attribute_code = "status", values = ["open", "in_progress"] }
```

## Contexts

A rule without a context evaluates the record in every context and fails if
any context fails; the finding evidence lists the failing context codes as
`contexts`. A rule with a context evaluates only that context, with values
resolved through its parent chain.

## Dependent records

Event-triggered rules whose predicate uses `linked` or `referenced_by` also
re-run for dependents when a linked or referencing record changes: records of
the rule's blueprint that link to the changed record, and the records a changed
record of the `referenced_by` blueprint points to or, according to the event's
facts, stopped pointing to (a removed or re-pointed relationship).
`record.migrated.v1` carries no facts; a migration that drops or re-points a
relationship lists the released targets in the payload's
`released_relationships` (at most 100 per relationship and 1,000 per event)
instead.
At most 100
dependents run per rule and event. Changing a linked record is never rejected
because of another record's checks; the dependent receives a finding, and a
record check or enforcing rule blocks the dependent's next save.

## Enforcement

By default a rule only reports findings. `[enforcement]` also rejects
writes while the rule is violated:

```toml
[enforcement]
on_save = true                       # reject any write that leaves the record violating the rule

[[enforcement.transitions]]          # and/or guard status transitions
attribute_code = "status"
from = "review"                      # optional; omit to guard every change into `to`
to = "released"
```

- Enforcement requires `error` or `critical` severity, `on_save` or at least one
  transition (at most 16), and a synchronous-safe predicate. Saving the rule
  checks that each transition attribute of the latest published revision is a
  status attribute and that `from`/`to` are its codes; a revision where they are
  not is skipped (`status_changed`).
- Enforcing rules run after record checks and transition conditions on every
  write path, on the transaction's final state. Transition guards evaluate the
  state the transition produces. A context-scoped rule enforces only in its
  context; otherwise every context is checked.
- A violation returns `422 rule_violation` with `error.details.violations`
  (`source = "rule"`, `severity`, and `transition` for a guarded transition).
- A record that already violates an `on_save` rule cannot be saved until the
  same save fixes the violation.

### Dry run gate

An enforcing definition reaches live records in two ways, and both need a
completed full dry run (no `record_id`) of that exact definition when the
rule's blueprint has live records:

- enabling a rule whose current definition enforces, and
- saving an enforcing definition for a rule that is already enabled.

Runs record the `definition_hash` they evaluated. A dry run may check an
unsaved definition: `POST /rules/{rule_id}/run-now` with
`{"dry_run": true, "idempotency_key": "...", "definition": "<TOML>"}` compiles
it, checks that its code is the rule's and that it fits the latest published
revision, and stores the compiled plan on the run (only dry runs may carry
one). The gate reads the latest completed full dry run with the definition's
hash:

1. Dry-run the change (or the current definition before enabling).
2. `PUT /rules/{rule_id}` with the definition, or
   `POST /rules/{rule_id}/enable`.

Without that run, the request returns `409 rule_dry_run_required`. A dry run
stops after 10,000 candidates; when more remained, the run is recorded as
truncated (`truncated: true` on the run in `GET /rule-runs`) and the gate also
returns `409 rule_dry_run_required` unless the request sets
`accept_existing_violations`, because the dry run cannot prove the remaining
records pass. That error has its own message and
`error.details = {"truncated": true, "existing_violations": n}`, where `n`
counts the violations among the candidates the run did check. If the latest
completed dry run found violations, it returns
`409 rule_has_existing_violations` with `error.details.existing_violations`
(the count). Fix those records and dry-run again, or send
`{"accept_existing_violations": true}`; the violating records then cannot be
saved until fixed. Definitions without enforcement need no dry run.

## Publication channels

A publication channel can require rules by code
(`PUT /publication-channels/{context_id}` with `required_rule_codes`). Required
rules are the enabled rules of the record's blueprint with that code that fit
the record's revision, excluding rules scoped to another context; any predicate
is allowed, including rules-only ones. They are evaluated live in the channel
context when publishing and fail with `422 publication_checks_failed`. See
[API](api.md#errors).

## Runtime guarantees

- A schedule has one durable cursor per rule revision/trigger; Attricat never creates per-record timers.
- Each run leases one bounded page (500 records of the rule's blueprint, any revision), has a durable UUID cursor, and stops after 10,000 candidates.
- Schedule occurrence and event IDs are idempotency keys. A rule cannot have overlapping pending or leased scheduled runs.
- Workers lease, retry with bounded exponential delay, dead-letter after five attempts, and recheck the enabled lifecycle on claim.
- Event intake only creates a durable record-scoped run after the lifecycle high-water boundary. It does not evaluate on an outbox lease.
- Finding identity is rule/record/context/definition hash. Re-evaluation updates one finding, reopens it if necessary, and resolves it when the predicate passes or the record's revision is skipped. Dry runs persist no findings and report the predicted failing count on the run.
- Rules use normal workspace authorization and request audit attribution. They contain no SQL, scripts, extension invocation, network access, or host mutation actions.

## API

`rules.manage` is required for validation, creation, changes, enable/disable, and manual runs; `rules.read` is required for reads.

- `POST /rules/validate` runs the same checks as `POST /rules` (definition, the blueprint's latest published revision, attribute types, context) and returns the compiled rule without storing it. The web rule editor uses it as you type, with `contracts/rule-definition-v1.schema.json` for completions.
- `GET /rules?blueprint_id=...` and `GET /rules/{rule_id}` return each rule's current revision with `enabled` and `not_applicable`.
- `POST /rules` with `{ "blueprint_id": "UUID", "context_id": null, "definition": "..." }` creates a disabled rule with revision 1.
- `PUT /rules/{rule_id}` with `{ "definition": "...", "accept_existing_violations": false }` saves a new current revision.
- `GET /rules/{rule_id}/revisions` returns the changelog, newest first; `GET /rules/{rule_id}/revisions/{version}` returns one revision.
- `POST /rules/{rule_id}/enable`, with an optional body `{ "accept_existing_violations": true }` for enforcing rules, and `POST /rules/{rule_id}/disable`.
- `POST /rules/{rule_id}/run-now` with `{ "record_id": "optional UUID", "dry_run": false, "idempotency_key": "...", "definition": "optional TOML" }` runs the current revision; a normal run needs the rule enabled, and only a dry run may send `definition`.
- `GET /rule-runs?record_id=...` (runs include `definition_hash` and `candidates_skipped`), `POST /rule-runs/{run_id}/replay`, `GET /rule-findings?record_id=...` (add `unresolved=true` to skip resolved findings; also narrows by `rule_id`, `blueprint_id`, `context_id`, `severity` and `state`), `GET /rule-findings/count?record_id=...`
- `POST /rule-findings/recheck` with `{ "record_id": "UUID", "idempotency_key": "..." }` re-checks one record against every enabled rule of its blueprint and returns `202 { "run_ids": [...] }`, one record-scoped manual run per rule. Repeating the key returns the same runs.
- `POST /rule-findings/{finding_id}/acknowledge`
- `POST /v1/records/search` with `rules.read` returns each record's unresolved
  findings as `data_quality` and accepts the `data_quality` filter. Core keeps
  the `attricat.needs_attention` tag on records with open findings, which
  search sorts by with `system_tag:attricat.needs_attention`; see
  [API](api.md#search-table-sorting).

Errors: `422 invalid_rule_definition`, `422 blueprint_not_published`,
`409 rule_revision_unchanged`, `422 rule_not_enabled` (a normal `run-now` of a
disabled rule), `409 rule_dry_run_required`,
`409 rule_has_existing_violations`. Writes rejected by an enforcing rule return
`422 rule_violation`.

## Rules installed by solution packs

A [solution pack](solution-packs.md#rules-workflows-and-saved-searches) can
declare standalone rules for its own blueprints. Applying the pack creates each
rule through the same path as `POST /rules`: the definition is compiled with the
same parser, checked against the plan's created or mapped blueprint, and stored
for that blueprint (and optional created or mapped context). A rule the pack
declares as enabled is then enabled exactly as `POST /rules/{rule_id}/enable`
would, with the same dry-run gate, recording the lifecycle activation boundary
and schedule cursors in the same transaction. Other rules stay disabled.

The pack's rule code becomes `<prefix>_<code>`, so a plan reports an existing
rule code as a conflict instead of creating a second rule. A pack blueprint
with `[[rules]]` tables is invalid. After installation, seeded rules are
ordinary rules; a pack never updates, re-enables, or removes them.
