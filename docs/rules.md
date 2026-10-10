# Rules

Rules are Attricat-owned, versioned blueprint checks. They are deliberately separate from workflows: a workflow mutates one trigger record, while a rule evaluates a restricted predicate over a bounded blueprint candidate page and manages a finding lifecycle.

## Definition contract

Rules are declared as `[[rules]]` tables inside a strict blueprint TOML document. Blueprint ownership supplies rule format version 1; rules have a severity, one to eight triggers, and exactly one predicate. Unknown fields and arbitrary code/selectors are rejected.

```toml
[[rules]]
code = "product-title-required"
name = "Published products have a title"
severity = "error"

[[rules.triggers]]
type = "schedule"
cron = "0 0 * * * *"
timezone = "UTC"

[[rules.triggers]]
type = "manual"

[rules.predicate]
type = "required"
attribute_code = "title"
```

Schedules are six-field UTC cron expressions. Event triggers accept only catalog record and value event types; post-import is a reserved declarative trigger shape. Rules can be attached to a published blueprint revision and optionally to a context.

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
[rules.predicate]
type = "relative_date"
attribute_code = "certificate_expires_on"
op = "gt"
offset_days = 30
```

No open corrective actions reference this nonconformance:

```toml
[rules.predicate]
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
the rule's blueprint revision that link to the changed record, and the records
a changed record of the `referenced_by` blueprint points to or, according to
the event's facts, stopped pointing to (a removed or re-pointed relationship).
`record.migrated.v1` carries no facts; a migration that drops or re-points a
relationship lists the released targets in the payload's
`released_relationships` (at most 100 per relationship and 1,000 per event)
instead.
At most 100
dependents run per rule and event. Changing a linked record is never rejected
because of another record's checks; the dependent receives a finding, and a
record check or enforcing rule blocks the dependent's next save.

## Enforcement

By default a rule only reports findings. `[rules.enforcement]` also rejects
writes while the rule is violated:

```toml
[rules.enforcement]
on_save = true                       # reject any write that leaves the record violating the rule

[[rules.enforcement.transitions]]    # and/or guard status transitions
attribute_code = "status"
from = "review"                      # optional; omit to guard every change into `to`
to = "released"
```

- Enforcement requires `error` or `critical` severity, `on_save` or at least one
  transition (at most 16), and a synchronous-safe predicate. Blueprint
  compilation checks that each transition attribute is a status attribute and
  that `from`/`to` are its codes.
- Enforcing rules run after record checks and transition conditions on every
  write path, on the transaction's final state. Transition guards evaluate the
  state the transition produces. A context-scoped rule enforces only in its
  context; otherwise every context is checked.
- A violation returns `422 rule_violation` with `error.details.violations`
  (`source = "rule"`, `severity`, and `transition` for a guarded transition).
- A record that already violates an `on_save` rule cannot be saved until the
  same save fixes the violation.

### Dry run before enabling

Enabling an enforcing revision whose blueprint revision has live records needs
a completed full dry run (no `record_id`) of that exact revision:

1. Publish the revision.
2. `POST /rules/{rule_id}/run-now` with
   `{"dry_run": true, "idempotency_key": "...", "version": 2}`. Dry runs may
   target a published revision that is not enabled.
3. `POST /rules/{rule_id}/versions/2/enable`.

Without that run, enabling returns `409 rule_dry_run_required`. A dry run
stops after 10,000 candidates; when more remained, the run is recorded as
truncated (`truncated: true` on the run in `GET /rule-runs`) and enabling also
returns `409 rule_dry_run_required` unless the request sets
`accept_existing_violations`, because the dry run cannot prove the remaining
records pass. That error has its own message and
`error.details = {"truncated": true, "existing_violations": n}`, where `n`
counts the violations among the candidates the run did check. If the latest completed dry run found violations, it
returns
`409 rule_has_existing_violations` with `error.details.existing_violations`
(the count). Fix those records and dry-run again, or enable with
`{"accept_existing_violations": true}`; the violating records then cannot be
saved until fixed. Revisions without enforcement enable as before.

## Publication channels

A publication channel can require rules by code
(`PUT /publication-channels/{context_id}` with `required_rule_codes`). Required
rules are the enabled rules of the record's blueprint revision with that code,
excluding rules scoped to another context; any predicate is allowed, including
rules-only ones. They are evaluated live in the channel context when publishing
and fail with `422 publication_checks_failed`. See [API](api.md#errors).

## Runtime guarantees

- A schedule has one durable cursor per rule revision/trigger; Attricat never creates per-record timers.
- Each run leases one bounded page (500 records), has a durable UUID cursor, and stops after 10,000 candidates.
- Schedule occurrence and event IDs are idempotency keys. A rule cannot have overlapping pending or leased scheduled runs.
- Workers lease, retry with bounded exponential delay, dead-letter after five attempts, and recheck the enabled lifecycle on claim.
- Event intake only creates a durable record-scoped run after the lifecycle high-water boundary. It does not evaluate on an outbox lease.
- Finding identity is rule/record/context/definition hash. Re-evaluation updates one finding, reopens it if necessary, and resolves it when the predicate passes. Dry runs persist no findings and report the predicted failing count on the run.
- Rules use normal workspace authorization and request audit attribution. They contain no SQL, scripts, extension invocation, network access, or host mutation actions.

## API

`rules.manage` is required for validation, creation, publication, enable/disable, and manual runs; `rules.read` is required for reads.

- `POST /rules/validate`
- `GET|POST /rules`
- `GET /rules/{rule_id}`
- `POST /rules/{rule_id}/versions/{version}/publish`
- `POST /rules/{rule_id}/versions/{version}/enable`, with an optional body `{ "accept_existing_violations": true }` for enforcing revisions
- `POST /rules/{rule_id}/disable`
- `POST /rules/{rule_id}/run-now` with `{ "record_id": "optional UUID", "dry_run": false, "idempotency_key": "...", "version": 2 }`; `version` is optional. A normal run uses the enabled revision (`version`, if given, must match it). A dry run uses `version`, else the enabled revision, else the latest published revision.
- `GET /rule-runs`, `POST /rule-runs/{run_id}/replay`, `GET /rule-findings?record_id=...`
- `POST /rule-findings/{finding_id}/acknowledge`

Errors: `422 invalid_rule_definition`, `422 rule_not_enabled` (a normal
`run-now` while no revision is enabled), `409 rule_dry_run_required`,
`409 rule_has_existing_violations`. Writes rejected by an enforcing rule return
`422 rule_violation`.

## Rules installed by solution packs

A [solution pack](solution-packs.md#rules-workflows-and-saved-searches) can
declare standalone rules for its own blueprints. Applying the pack creates each
rule through the same path as `POST /rules`: the definition is compiled with the
same parser, attached to the plan's created or mapped blueprint revision (and
optional created or mapped context), and published. A rule the pack declares as
enabled is then enabled exactly as `POST /rules/{rule_id}/versions/1/enable`
would, recording the lifecycle activation boundary and schedule cursors in the
same transaction. Other rules stay published but disabled.

The pack's rule code becomes `<prefix>_<code>`, so a plan reports an existing
rule code as a conflict instead of creating a second family. Rules embedded in a
seeded blueprint's `[[rules]]` behave as for any blueprint: they are published
with it and start disabled. After installation, seeded rules are ordinary rules;
a pack never updates, re-enables, or removes them.
