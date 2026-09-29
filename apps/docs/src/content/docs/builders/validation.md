---
title: Validation
description: Constrain single values and whole entities with JSON Schema, and understand how validation interacts with contexts.
---

Attricat validates every write on the server before anything is saved. Validation comes from three places:

1. **The attribute type.** A `number` attribute rejects `"abc"`; a `date` rejects `2026-13-01`.
2. **`value_schema`** on an attribute: a JSON Schema for one value.
3. **`entity_schema`** on a blueprint: a JSON Schema for the whole entity.

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

## Validation and contexts

An entity has a resolved value set in every context, built from its own values plus whatever it inherits. After each change, Attricat validates the entity in **every** context, not only the one you edited.

For example, say `title` is required and inherits from the default context. Clearing the default title would leave every child context without one, so the write is rejected even though you were editing the default context.

Moving a context to a new parent is checked the same way: every active entity is validated against the new inheritance chain before the move is saved.

## What is not validated

- Visibility tags and `readonly` are presentation hints. They do not stop the API from writing a value.
- `system_tags` and `system_metadata` on entities are outside the blueprint and are not schema-checked.
- A newer blueprint revision's schema does not apply to entities still on an older revision until they are migrated.
