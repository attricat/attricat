---
title: Validation
description: Constrain single values and whole entities with JSON Schema, and understand how validation interacts with contexts.
---

Attricat validates every write on the server before anything is saved. Validation comes from these places:

1. **The attribute type.** A `number` attribute rejects `"abc"`; a `date` rejects `2026-13-01`.
2. **`value_schema`** on an attribute: a JSON Schema for one value.
3. **`entity_schema`** on a blueprint: a JSON Schema for the whole entity.
4. **Checks** in the entity schema's `x-attricat-checks`: comparisons between attributes and checks on linked records.
5. **Conditions** on status transitions, and **enforcing rules**.

Both schemas use JSON Schema Draft 2020-12. The web app uses the same schemas to warn you while you type, but the server's answer is the one that counts.

## Constrain one value

Write the schema as JSON inside a TOML string. Single quotes avoid escaping.

```toml
[[attributes]]
code = "price"
value_type = "number"
value_schema = '{"type":"number","minimum":0}'

[[attributes]]
code = "sku"
value_type = "string"
value_schema = '{"type":"string","pattern":"^[A-Z]{3}-[0-9]{4}$"}'

[[attributes]]
code = "size"
value_type = "string"
value_schema = '{"enum":["XS","S","M","L","XL"]}'
```

`value_schema` works on scalar attributes: strings, numbers, integers, booleans, dates, datetimes, and times. Relationships and files cannot have one. Constrain relationships with `entity_schema`.

A value that fails returns `422 attribute_value_schema_mismatch`.

## Statuses

A status is a `string` attribute whose `value_schema` has an `enum` of stable codes plus an `x-attricat-status` annotation. The annotation gives each code a label, an optional color tone, and optionally the transitions allowed between them:

```toml
[[attributes]]
code = "status"
value_type = "string"
value_schema = '''{
  "type": "string",
  "enum": ["draft", "live", "retired"],
  "x-attricat-status": {
    "version": 1,
    "options": [
      { "code": "draft", "label": "Draft" },
      { "code": "live", "label": "Live", "tone": "success" },
      { "code": "retired", "label": "Retired" }
    ],
    "transitions": [
      { "from": null, "to": "draft" },
      { "from": "draft", "to": "live" },
      { "from": "live", "to": "retired" }
    ]
  }
}'''
```

- Every `enum` code needs exactly one option, listed in display order. Codes use letters, digits, `_`, and `-`; labels are plain text.
- `tone` is `default`, `success`, `warning`, `error`, or `info`. The label is always shown, so color is never the only signal.
- Omit `transitions` to allow any change. With `transitions`, only the listed changes are allowed; an empty array allows none. `null` means "no value": an edge from `null` allows setting the first value (including defaults), and an edge to `null` allows clearing it. Keeping the same value is always allowed.

The web app shows a status as a labeled chip and edits it with a select that disables forbidden choices. Transitions are checked by the server for every writer, including the API, CLI, workflows, history restores, and migrations. They compare effective values, so a value inherited from a parent context counts as the starting point. A forbidden change returns `422 attribute_value_schema_mismatch`.

### Conditions on transitions

An allowed transition can still depend on the data. Add `conditions` to an edge to require something before the change is made, such as a recorded root cause before a nonconformance is closed:

```toml
[[attributes]]
code = "status"
value_type = "string"
value_schema = '''{
  "type": "string",
  "enum": ["open", "closed"],
  "x-attricat-status": {
    "version": 1,
    "options": [
      { "code": "open", "label": "Open" },
      { "code": "closed", "label": "Closed", "tone": "success" }
    ],
    "transitions": [
      { "from": null, "to": "open" },
      { "from": "closed", "to": "open" },
      { "from": "open", "to": "closed", "conditions": [
        { "code": "root-cause", "message": "Record the root cause before closing",
          "predicate": { "type": "required", "attribute_code": "root_cause" } },
        { "code": "actions-closed", "message": "Close every corrective action first",
          "predicate": { "type": "referenced_by", "blueprint_code": "corrective_action",
            "relationship_code": "nonconformance", "max": 0,
            "predicate": { "type": "one_of", "attribute_code": "state", "values": ["open"] } } }
      ] }
    ]
  }
}'''
```

- A condition has a `code`, an optional `message` (1 to 500 characters), and a `predicate`. Predicates are the same as in [rules](/builders/rules/#predicates), except `stale`, `unique`, and `acyclic`. An edge has at most 16 conditions.
- Each `from`/`to` edge can be declared only once.
- Conditions are checked on the state the write produces, so filling in `root_cause` and closing in the same save works.
- They are checked in every context where the status changes, by every writer.

If any condition is unmet, the whole write is rejected with `422 transition_conditions_unmet`, listing every unmet condition. Enforcing rules can guard transitions in the same way; see [Enforce a rule](/builders/rules/#enforce-a-rule).

To find out ahead of time which destinations are available, call `GET /v1/entities/{id}/status-transitions?context_id=<uuid>` (the default context if you leave it out). For each status attribute it returns the current code and each destination with `allowed`, a `reason` (`transition_not_allowed` or `conditions_unmet`), and the `unmet` conditions and enforcing rules, evaluated on the saved entity as if the status had changed.

## Constrain the whole entity

`entity_schema` sees the entity as one JSON object. Use it for rules that involve more than one attribute:

```toml
entity_schema = '''
{
  "type": "object",
  "required": ["title", "price"],
  "allOf": [
    {
      "if": { "properties": { "on_sale": { "const": true } }, "required": ["on_sale"] },
      "then": { "required": ["sale_price"] }
    }
  ],
  "dependentRequired": { "discontinued_on": ["replacement"] }
}
'''
```

The object Attricat validates looks like this:

```json
{
  "title": "Linen shirt",
  "price": 49.0,
  "on_sale": true,
  "sale_price": 39.0,
  "categories": ["e8b7a8d3-c954-4c0f-b658-0f686ba466a3"]
}
```

- Scalar values appear in their JSON form.
- Relationships are arrays of target entity UUIDs. Use `minItems` and `maxItems` to require at least one category or at most three tags.
- Attributes with no value are left out, so `required` means "has a value".

The top-level `required`, `properties`, `dependentRequired`, and `dependentSchemas` may only name attributes the blueprint has, including attributes selected from mixins. A typo there fails compilation.

`entity_schema` is allowed on entity blueprints only. A failing entity returns `422 entity_schema_mismatch`.

## Compare attributes with checks

JSON Schema cannot say "valid until must not be before valid from". Add named checks to the entity schema under `x-attricat-checks` instead:

```toml
entity_schema = '''
{
  "type": "object",
  "required": ["title"],
  "x-attricat-checks": [
    { "code": "valid-range", "message": "Valid until must not be before valid from",
      "predicate": { "type": "compare", "attribute_code": "valid_until", "op": "gte", "other_attribute_code": "valid_from" } },
    { "code": "tolerance", "message": "The lower limit must not exceed the upper limit",
      "predicate": { "type": "compare", "attribute_code": "lower_limit", "op": "lte", "other_attribute_code": "upper_limit" } },
    { "code": "not-self-superseding",
      "predicate": { "type": "compare", "attribute_code": "supersedes", "op": "disjoint", "other_attribute_code": "superseded_by" } }
  ]
}
'''
```

Each check has:

| Key | Description |
| --- | --- |
| `code` | Unique within the list. Reported in errors. |
| `message` | Optional, 1 to 500 characters. Shown when the check fails; otherwise Attricat generates one. |
| `predicate` | Any [rule predicate](/builders/rules/#predicates) except `stale`, `unique`, and `acyclic`, which cannot run during a save. |

A blueprint can have at most 32 checks. They are type-checked when the blueprint is compiled: an unknown attribute, an ordering comparison on a string, or a comparison of a date with a number fails with `422 invalid_blueprint_definition`.

A comparison with a missing value passes, so `valid-range` above only applies once both dates are set. Make the attributes required, or add a `required` predicate, when they must be filled in.

Checks run after the JSON Schema, on every write path: the API, the CLI, the web app, workflows, migrations, history restores, and context moves. Like the schema, they are checked in every context. A failing write returns `422 entity_check_failed`, and nothing is saved.

## Check linked records

A check can look one relationship hop away. `linked` checks the records an entity links to, and `referenced_by` counts the records that link to it.

```toml
entity_schema = '''
{
  "type": "object",
  "x-attricat-checks": [
    { "code": "facility-of-supplier",
      "message": "Every facility must belong to the certificate's supplier",
      "predicate": { "type": "linked", "relationship_code": "facilities",
        "predicate": { "type": "compare", "attribute_code": "supplier", "op": "eq", "subject_attribute_code": "supplier" } } },
    { "code": "approved-facilities",
      "message": "Only approved facilities can be certified",
      "predicate": { "type": "linked", "relationship_code": "facilities",
        "predicate": { "type": "one_of", "attribute_code": "approval_status", "values": ["approved"] } } }
  ]
}
'''
```

- Inside `linked` and `referenced_by`, `attribute_code` names an attribute of the other record. `subject_attribute_code` names an attribute of the entity being saved.
- `quantifier` on `linked` is `all` (the default; satisfied when there are no links), `any`, or `none`.
- Linked records are resolved in the same context as the entity. An attribute the linked record does not have counts as missing. Deleted records are ignored.
- A check fails if a relationship has more than 200 linked records, or more than 1,000 records refer to the entity.

### Changes to linked records are reported, not rejected

A check runs when its own entity is saved. Editing the linked record does not re-check every entity that links to it, and is not rejected. If a facility's approval is withdrawn, that change is saved even though certificates link to the facility.

To catch these cases, add a [rule](/builders/rules/) with the same predicate and an `event` trigger. When a linked or referencing record changes, event-triggered rules re-run for up to 100 entities that depend on it and record findings. The affected certificate itself cannot be saved again until the problem is fixed, because its own check fails on the next save.

## Enforcing rules

A [data quality rule](/builders/rules/#enforce-a-rule) with an `enforcement` table works like a check that can be switched on and off without publishing a new blueprint revision. It rejects violating saves or guarded status transitions with `422 rule_violation`. Use checks for constraints that are part of the model, and enforcing rules for policies a team turns on after a [dry run](/builders/rules/#dry-run-before-enabling).

## Errors and how to fix them

| Code | Status | Meaning |
| --- | --- | --- |
| `entity_check_failed` | 422 | An `x-attricat-checks` check fails in some context. |
| `transition_conditions_unmet` | 422 | A status transition's conditions are not met. |
| `rule_violation` | 422 | The write leaves the entity violating an enforcing rule. |
| `publication_checks_failed` | 422 | A channel's required checks fail. See [Publishing](/guides/publishing/#require-checks-before-publication). |

On a save, checks, conditions, and enforcing rules run in that order, after the JSON Schema, and only the first group that fails is reported. Each error lists up to 50 violations in `error.details.violations`:

```json
{
  "error": {
    "code": "entity_check_failed",
    "message": "entity checks failed: Valid until must not be before valid from (valid-range)",
    "details": {
      "violations": [
        {
          "source": "entity_check",
          "code": "valid-range",
          "message": "Valid until must not be before valid from",
          "contexts": ["default", "PL"],
          "attributes": ["valid_until", "valid_from"],
          "evidence": { "attribute_code": "valid_until", "value": "2026-01-01", "compared_with": "2026-03-01" }
        }
      ]
    }
  }
}
```

| Field | Description |
| --- | --- |
| `source` | `entity_check`, `transition_condition`, `rule`, or, for publication only, `entity_schema`. |
| `code` | The check, condition, or rule code. |
| `message` | The custom message, or a generated one. |
| `contexts` | Codes of the contexts where it failed. |
| `attributes` | Attributes of this entity involved, such as both sides of a comparison or the relationship of a `linked` check. |
| `severity` | For rules: the rule's severity. |
| `transition` | For conditions and guarded transitions: `attribute_code`, `from`, and `to`. |
| `evidence` | Details such as the compared values or the IDs of failing linked records. |

In the web app, the edit form lists the failed checks with the contexts they failed in, and shows each message on the fields named in `attributes`. A field's message clears once you edit that field. API clients get the same information from `attributes`. To fix the problem, change those attributes in the listed contexts, or fix the linked or referring records the message names, and save again. For a status change, check which conditions are unmet with the [status-transitions endpoint](#conditions-on-transitions).

## Validation and contexts

An entity has a resolved value set in every context, built from its own values plus whatever it inherits. After each change, Attricat validates the entity in **every** context, not only the one you edited.

For example, say `title` is required and inherits from the default context. Clearing the default title would leave every child context without one, so the write is rejected even though you were editing the default context.

Moving a context to a new parent is checked the same way: every active entity is validated against the new inheritance chain before the move is saved.

## What is not validated

- Visibility tags and `readonly` are presentation hints. They do not stop the API from writing a value.
- `system_tags` and `system_metadata` on entities are outside the blueprint and are not schema-checked.
- A newer blueprint revision's schema does not apply to entities still on an older revision until they are migrated.
