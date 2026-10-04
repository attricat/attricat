---
title: Validation
description: Constrain single values and whole entities with JSON Schema, and understand how validation interacts with contexts.
---

Attricat validates every write on the server before anything is saved. Validation comes from four places:

1. **The attribute type.** A `number` attribute rejects `"abc"`; a `date` rejects `2026-13-01`.
2. **`value_schema`** on an attribute: a JSON Schema for one value.
3. **`entity_schema`** on a blueprint: a JSON Schema for the whole entity.
4. **`unique_keys`** on a blueprint: business identifiers no two entities may share. See [Unique keys](#unique-keys).

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

A status can also restrict who may make each transition, lock finalized records, and bind approvals to reviewed content. See [Control a record's lifecycle](/builders/blueprints/#step-10-control-a-records-lifecycle).

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

## Unique keys

A schema checks one entity at a time, so it cannot stop two entities from getting the same part number. Declare a unique key instead:

```toml
[[unique_keys]]
code = "part_number"
attributes = ["part_number"]

[[unique_keys]]
code = "document_revision"
attributes = ["document", "revision_label"]
```

The second key is composite: a document can have only one revision `B`, but every document can have its own. Keys can combine up to eight scalar attributes or single-target relationships.

- **Comparison.** Text is trimmed, runs of whitespace become one space, and case is ignored, so `ABC-1` and ` abc-1 ` collide. Set `case_sensitive = true` to compare text exactly. Numbers compare by value and relationships by the linked entity.
- **Missing values.** An entity without a value for one of the key's attributes is not checked against that key. Require the attributes in `entity_schema` if every entity must have one.
- **Contexts.** By default a key compares default-context values. With `scope = "context"`, it compares the values each context shows, including inherited ones, so a slug can be unique per market.
- **Concurrent saves.** The database checks the key inside the save. If two people save the same part number at the same moment, one save succeeds and the other gets `409 unique_key_conflict` with the conflicting entity in `error.details.conflicting_entity_id`.
- **Adding a key later.** Publishing a revision that adds a key first checks existing entities. Duplicates make publication fail with `409 unique_key_duplicates`, listing the entities that share each value. The key then covers every entity of the blueprint, including those still on older revisions.

See [Unique keys](/reference/blueprint/#unique-keys) for every option.

## Validation and contexts

An entity has a resolved value set in every context, built from its own values plus whatever it inherits. After each change, Attricat validates the entity in **every** context, not only the one you edited.

For example, say `title` is required and inherits from the default context. Clearing the default title would leave every child context without one, so the write is rejected even though you were editing the default context.

Moving a context to a new parent is checked the same way: every active entity is validated against the new inheritance chain before the move is saved.

## What is not validated

- Visibility tags and `readonly` are presentation hints. They do not stop the API from writing a value.
- `system_tags` and `system_metadata` on entities are outside the blueprint and are not schema-checked.
- A newer blueprint revision's schema does not apply to entities still on an older revision until they are migrated.
