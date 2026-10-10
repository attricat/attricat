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
scalar values only; relationship constraints belong in the record schema.

## Record Schemas

`record_schema` validates the complete resolved record document. It supports
cross-field JSON Schema rules such as `required`, `if`/`then`/`else`, and
dependencies.

```toml
record_schema = '''
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
relationships as sorted arrays of target record UUID strings and files as
ordered arrays of `{"id", "sha256"}` references (an explicitly emptied file
value is `[]` and is not inherited). Missing values are
omitted. It does not use preview labels or relationship display objects.

## Declarative Checks

JSON Schema cannot compare two attributes or look at linked records. The
record schema's `x-attricat-checks` array adds named checks that use the
declarative [predicate](#predicates) engine shared with status transition
conditions, rules and publication channel gates. A check *holds* when the
data is acceptable; a write that leaves any check failing is rejected.

```toml
record_schema = '''
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
- Only synchronous-safe predicates are allowed: `stale`, `unique` and
  `acyclic` are rules-only and rejected here.

### Predicates

A predicate *holds* when the data is acceptable. Predicates are tagged by
`type`, unknown fields are rejected, and every predicate is type-checked
against the blueprint revision's attributes when the blueprint or rule is
saved (`422 invalid_blueprint_definition` or `invalid_rule_definition`).

| `type` | Fields | Holds when |
| --- | --- | --- |
| `required` | `attribute_code` | The attribute has a value; a relationship has at least one target. |
| `stale` | `attribute_code`, `max_age_seconds` (1–31536000) | The current value changed within the age limit. Rules only. |
| `has_tag` / `missing_tag` | `tag` | The record has / does not have the system tag. |
| `compare` | `attribute_code`, `op`, exactly one of `other_attribute_code`, `subject_attribute_code`, `value` | The comparison is true. |
| `one_of` | `attribute_code`, `values` (1–100) | The value is one of the listed values, such as status codes. Not for relationships or files. |
| `relative_date` | `attribute_code`, `op` (`lt`, `lte`, `gt`, `gte`), `offset_days` (−36500–36500, default 0) | A date/datetime compares with now + `offset_days`. |
| `unique` | `attribute_codes` (1–4) | No other live record of the same blueprint family (any revision, attributes matched by code) has the same values in the evaluated context. Values compare as [unique keys](database.md#structural-constraints) do: strings trimmed, whitespace collapsed and case-insensitive; numbers by value. String, number, integer, boolean, date and datetime only. Rules only. |
| `linked` | `relationship_code`, `quantifier` (`all` default, `any`, `none`), `predicate` | `all`: every linked record satisfies the predicate (holds with no links); `any`: at least one does; `none`: none does. |
| `referenced_by` | `blueprint_code`, `relationship_code`, optional `predicate`, `min` and/or `max` (≤ 1000) | The number of `blueprint_code` records whose `relationship_code` targets this record and that match `predicate` is within the bounds. |
| `acyclic` | `relationship_code` | Following the relationship never returns to the record. Rules only. |
| `all_of` / `any_of` | `predicates` (1–16) | Every / at least one nested predicate holds. |

- `compare` operators are `eq`, `ne`, `lt`, `lte`, `gt`, `gte` and `disjoint`.
  Ordering applies only to `number`, `integer`, `date` and `datetime`; strings
  and booleans support `eq`/`ne`. Relationships compare target sets: `eq`/`ne`
  (same set) and `disjoint` (no common target). Files cannot be compared, and
  both sides must have compatible types. Write dates as `"2026-01-31"`.
- A comparison with a missing operand (absent, null, empty string or empty
  relationship) holds, and so do `one_of` and `relative_date` on an empty
  attribute. Use the JSON Schema `required` keyword or a `required` predicate
  when a value must exist.
- `subject_attribute_code` is available only inside `linked` or
  `referenced_by` and names an attribute of the record being checked.
- Predicates nest at most 4 deep with at most 32 parts.
- `stale`, `unique` and `acyclic` are *rules only*: they are not safe for
  synchronous evaluation and cannot be used by enforcing rules, record checks or
  transition conditions.
- User or team assignment values are plain strings to predicates: `required`,
  `compare` `eq`/`ne` and `one_of` work against a literal `user:<uuid>` or
  `team:<uuid>`. No predicate can refer to the acting user.

### Linked checks

`linked` evaluates the records reached through one relationship attribute:
`quantifier` is `all` (default; holds with no links), `any` or `none`. Inside
it, `attribute_code` names an attribute of the linked record and
`subject_attribute_code` names an attribute of the record being saved.
`referenced_by` counts live records of `blueprint_code` whose
`relationship_code` targets the record in the same context and that match the
optional nested `predicate`; set `min`, `max` or both (at most 1000), for
example `max = 0` for "no open corrective actions".

- Links are followed one hop. Nested predicates cannot use `linked`,
  `referenced_by`, `unique`, `acyclic` or `stale`.
- Attributes that a linked record does not declare are treated as missing.
- More than 200 linked records per relationship, or more than 1000 referencing
  records, fail the check. `acyclic` stops after 1000 visited records.
- Changes to the linked or referencing record are **not** rejected because of
  another record's checks. Event-triggered rules with the same predicate report
  affected dependents as findings, and the dependent's next save is rejected
  until the check holds again.

## Contexts And Errors

After a mutation, the API validates every resolved context for the record.
Reparenting a context validates all active records before the context change is
committed. This prevents an inherited value from making a descendant context
invalid.

Schema violations return `422` with either
`attribute_value_schema_mismatch` or `record_schema_mismatch`. No value or
preview change is committed on failure.

### Evaluation order

Every write path (API create/update/value writes, workflow writes,
migrations, history restoration and context reparenting) runs, on the
transaction's final state:

1. JSON Schema (`value_schema`, then `record_schema`).
2. `x-attricat-checks` in every context, with inherited values resolved:
   `422 record_check_failed`.
3. Status transition conditions for each changed status: `conditions` on the
   taken edge of an `x-attricat-status` `transitions` entry, at most 16 checks
   of the same shape as `x-attricat-checks`
   ([status attributes](status-control.md#transition-conditions)):
   `422 transition_conditions_unmet`.
4. Enforcing rules: enabled rules with `[rules.enforcement]` (`on_save = true`
   and/or up to 16 guarded status `transitions`), severity `error` or
   `critical`, and a synchronous-safe predicate ([rules](rules.md#enforcement)):
   `422 rule_violation`.

Only the first failing group is returned, and the whole write rolls back. Each
group reports every failing item, not just the first.

### Error details

These errors and `publication_checks_failed` include `error.details`:

```json
{
  "error": {
    "code": "record_check_failed",
    "message": "record checks failed: Valid until must not be before valid from (valid-range)",
    "details": {
      "violations": [
        {
          "source": "record_check",
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

- `source`: `record_check`, `transition_condition`, `rule` or `record_schema`
  (publication gates only, with `code = "record_schema"`).
- `code`: the check, condition or rule code. `message`: the custom message or a
  generated one.
- `contexts`: codes of the contexts in which it failed. One violation is
  reported per declaration.
- `attributes`: attributes of the saved record involved, for highlighting form
  fields.
- `severity`: present for rules. `transition`: `{attribute_code, from, to}` for
  conditions and transition-guarding rules.
- `evidence`: predicate-specific data, such as `failing_record_ids` for
  `linked` or `count` and `matching_record_ids` for `referenced_by`.
- At most 50 violations are reported. Publication errors add
  `details.context`, the channel's context code.

To recover, fix the listed `attributes` (or the linked or referencing records
named in `evidence`) in the listed contexts and retry.

Schemas are persisted with blueprint and attribute revisions and returned by
existing blueprint API responses as `blueprint.record_schema` and
`attributes[].value_schema`.
