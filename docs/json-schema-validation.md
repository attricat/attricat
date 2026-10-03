# JSON Schema Validation

Blueprints can define JSON Schema Draft 2020-12 contracts for persisted values.
Rust validates these contracts before committing direct or computed values; the
frontend uses the returned schemas for early form feedback only.

## Attribute Schemas

`value_schema` constrains one scalar attribute after its value has been parsed
into the attribute's native type. Write it as a JSON string in the TOML
blueprint definition so every JSON Schema construct remains available.

```toml
[[attributes]]
code = "price"
value_type = "number"
value_schema = '{"type":"number","minimum":0}'
```

Attribute schemas apply to direct and computed writes. They currently apply to
scalar values only; relationship constraints belong in the entity schema.

## Entity Schemas

`entity_schema` validates the complete resolved entity document. It supports
cross-field JSON Schema rules such as `required`, `if`/`then`/`else`, and
dependencies.

```toml
entity_schema = '''
{
  "type": "object",
  "required": ["title", "price"],
  "allOf": [{
    "if": {
      "properties": { "on_sale": { "const": true } },
      "required": ["on_sale"]
    },
    "then": { "required": ["sale_price"] }
  }]
}
'''
```

The validation document contains scalar values in their native JSON form and
relationships as arrays of target entity UUID strings. Missing values are
omitted. It does not use preview labels or relationship display objects.

## Declarative Checks

JSON Schema cannot compare two attributes or look at linked records. The
entity schema's `x-attricat-checks` array adds named checks that use the
declarative predicate engine shared with [rules](rules.md#predicates). A
check *holds* when the data is acceptable; a write that leaves any check
failing is rejected.

```toml
entity_schema = '''
{
  "required": ["valid_from"],
  "x-attricat-checks": [
    {
      "code": "valid-range",
      "message": "Valid until must not be before valid from",
      "predicate": {
        "type": "compare",
        "attribute_code": "valid_until",
        "op": "gte",
        "other_attribute_code": "valid_from"
      }
    },
    {
      "code": "facility-of-supplier",
      "message": "The facility must belong to the selected supplier",
      "predicate": {
        "type": "linked",
        "relationship_code": "facility",
        "predicate": {
          "type": "compare",
          "attribute_code": "supplier",
          "op": "eq",
          "subject_attribute_code": "supplier"
        }
      }
    }
  ]
}
'''
```

- Each check has a `code` (unique, ASCII letters, digits, `_`, `-`, at most
  128 characters), an optional `message` (1–500 characters; otherwise a
  generated message) and one `predicate`. At most 32 checks; unknown fields are
  rejected.
- Checks are type-checked against the blueprint's attributes when the
  blueprint is saved; errors return `422 invalid_blueprint_definition`.
- Predicate `type`s: `required`, `has_tag`, `missing_tag`, `compare`, `one_of`,
  `relative_date`, `linked`, `referenced_by`, `all_of` and `any_of` (see
  [Rules](rules.md#predicates) for every field). `stale`, `unique` and
  `acyclic` are rules-only and rejected here.
- `compare` takes `attribute_code`, `op` (`eq`, `ne`, `lt`, `lte`, `gt`, `gte`,
  `disjoint`) and exactly one of `other_attribute_code`, `value` or (inside
  `linked`/`referenced_by` only) `subject_attribute_code`. Ordering is for
  `number`, `integer`, `date` and `datetime`; strings and booleans use
  `eq`/`ne`; relationships compare target sets with `eq`, `ne` and `disjoint`;
  files cannot be compared. Write dates as `"2026-01-31"`.
- A comparison with a missing operand (absent, null, empty string or empty
  relationship) holds. Use the JSON Schema `required` keyword or a `required`
  predicate when a value must exist.

### Linked checks

`linked` evaluates the records reached through one relationship attribute:
`quantifier` is `all` (default; holds with no links), `any` or `none`. Inside
it, `attribute_code` names an attribute of the linked record and
`subject_attribute_code` names an attribute of the entity being saved.
`referenced_by` counts live records of `blueprint_code` whose
`relationship_code` targets the entity in the same context and that match the
optional nested `predicate`; set `min`, `max` or both (at most 1000), for
example `max = 0` for "no open corrective actions".

- Links are followed one hop. Nested predicates cannot use `linked`,
  `referenced_by`, `unique`, `acyclic` or `stale`.
- Attributes that a linked record does not declare are treated as missing.
- More than 200 linked records per relationship, or more than 1000 referencing
  records, fail the check.
- Changes to the linked or referencing record are **not** rejected because of
  another entity's checks. Event-triggered rules with the same predicate report
  affected dependents as findings, and the dependent's next save is rejected
  until the check holds again.

## Contexts And Errors

After a mutation, the API validates every resolved context for the entity.
Reparenting a context validates all active entities before the context change is
committed. This prevents an inherited value from making a descendant context
invalid.

Schema violations return `422` with either
`attribute_value_schema_mismatch` or `entity_schema_mismatch`. No value or
preview change is committed on failure.

### Evaluation order

Every write path (API create/update/value writes, workflow writes,
migrations, history restoration and context reparenting) runs, on the
transaction's final state:

1. JSON Schema (`value_schema`, then `entity_schema`).
2. `x-attricat-checks` in every context, with inherited values resolved:
   `422 entity_check_failed`.
3. Status [transition conditions](status-control.md#transition-conditions) for
   each changed status: `422 transition_conditions_unmet`.
4. Enforcing [rules](rules.md#enforcement): `422 rule_violation`.

Only the first failing group is returned, and the whole write rolls back. Each
group reports every failing item, not just the first.

### Error details

These errors and `publication_checks_failed` include `error.details`:

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
          "contexts": ["default"],
          "attributes": ["valid_until", "valid_from"],
          "evidence": {
            "attribute_code": "valid_until",
            "value": "2026-01-01",
            "compared_with": "2026-02-01"
          }
        }
      ]
    }
  }
}
```

- `source`: `entity_check`, `transition_condition`, `rule` or `entity_schema`
  (publication gates only, with `code = "entity_schema"`).
- `code`: the check, condition or rule code. `message`: the custom message or a
  generated one.
- `contexts`: codes of the contexts in which it failed. One violation is
  reported per declaration.
- `attributes`: attributes of the saved entity involved, for highlighting form
  fields.
- `severity`: present for rules. `transition`: `{attribute_code, from, to}` for
  conditions and transition-guarding rules.
- `evidence`: predicate-specific data, such as `failing_entity_ids` for
  `linked` or `count` and `matching_entity_ids` for `referenced_by`.
- At most 50 violations are reported. Publication errors add
  `details.context`, the channel's context code.

To recover, fix the listed `attributes` (or the linked or referencing records
named in `evidence`) in the listed contexts and retry.

Schemas are persisted with blueprint and attribute revisions and returned by
existing blueprint API responses as `blueprint.entity_schema` and
`attributes[].value_schema`.
