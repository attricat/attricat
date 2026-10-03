# Rules

Rules are Catalog-owned, versioned blueprint checks. They are deliberately separate from workflows: a workflow mutates one trigger entity, while a rule evaluates a restricted predicate over a bounded blueprint candidate page and manages a finding lifecycle.

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

The initial predicates are `required`, `stale`, `has_tag`, and `missing_tag`. Schedules are six-field UTC cron expressions. Event triggers accept only catalog entity and value event types; post-import is a reserved declarative trigger shape. Rules can be attached to a published blueprint revision and optionally to a context.

## Runtime guarantees

- A schedule has one durable cursor per rule revision/trigger; Catalog never creates per-entity timers.
- Each run leases one bounded page (500 entities), has a durable UUID cursor, and stops after 10,000 candidates.
- Schedule occurrence and event IDs are idempotency keys. A rule cannot have overlapping pending or leased scheduled runs.
- Workers lease, retry with bounded exponential delay, dead-letter after five attempts, and recheck the enabled lifecycle on claim.
- Event intake only creates a durable entity-scoped run after the lifecycle high-water boundary. It does not evaluate on an outbox lease.
- Finding identity is rule/entity/context/definition hash. Re-evaluation updates one finding, reopens it if necessary, and resolves it when the predicate passes. Dry runs persist no findings and report the predicted failing count on the run.
- Rules use normal workspace authorization and request audit attribution. They contain no SQL, scripts, extension invocation, network access, or host mutation actions.

## API

`rules.manage` is required for validation, creation, publication, enable/disable, and manual runs; `rules.read` is required for reads.

- `POST /rules/validate`
- `GET|POST /rules`
- `GET /rules/{rule_id}`
- `POST /rules/{rule_id}/versions/{version}/publish`
- `POST /rules/{rule_id}/versions/{version}/enable`
- `POST /rules/{rule_id}/disable`
- `POST /rules/{rule_id}/run-now` with `{ "entity_id": "optional UUID", "dry_run": false, "idempotency_key": "..." }`
- `GET /rule-runs`, `POST /rule-runs/{run_id}/replay`, `GET /rule-findings?entity_id=...`
- `POST /rule-findings/{finding_id}/acknowledge`

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
