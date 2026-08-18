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

## Contexts And Errors

After a mutation, the API validates every resolved context for the entity.
Reparenting a context validates all active entities before the context change is
committed. This prevents an inherited value from making a descendant context
invalid.

Schema violations return `422` with either
`attribute_value_schema_mismatch` or `entity_schema_mismatch`. No value or
preview change is committed on failure.

Schemas are persisted with blueprint and attribute revisions and returned by
existing blueprint API responses as `blueprint.entity_schema` and
`attributes[].value_schema`.
