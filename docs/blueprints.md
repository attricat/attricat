# Blueprint Authoring

Blueprints are versioned TOML definitions. Entities remain pinned to the exact
blueprint version used to create them.

```toml
format_version = 1
code = "product"
name = "Product"
kind = "entity"
```

`kind` is either `entity` or `mixin`. Mixins can be included but cannot create
entities.

Blueprint codes, attribute codes, include codes, and relationship target
blueprint codes contain only ASCII letters, numbers, and underscores.

## Attributes

Every attribute declares exactly one of `value_type` or `from`. Supported value
types are `string`, `number`, `integer`, `boolean`, `date`, `datetime`, `time`,
and `relationship`.

```toml
[[attributes]]
code = "stock"
value_type = "integer"
context_fallback = "default"
context_editable = "default"
```

`context_fallback` controls missing values in a non-default context:

- `default` is the default and inherits the default-context value or set.
- `none` leaves the attribute absent.

`context_editable` controls writes for every value type, including relationships:

- `all` is the default and permits writes in every context.
- `default` permits writes only in the default context. Other contexts render it
  read-only, and the API rejects writes.

Blank contextual string fields remove their override instead of storing an empty
string. Missing contextual values resolve according to `context_fallback`.

A relationship may restrict its target type:

```toml
[[attributes]]
code = "categories"
value_type = "relationship"
target_blueprint = "category"
```

## Includes

Includes are exact version-pinned dependencies. Select each mixin attribute that
the consuming blueprint should materialize:

```toml
[[includes]]
alias = "seo"
code = "seo"
version = 2

[[attributes]]
code = "meta_title"
from = "seo.meta_title"
```

## Display Labels

Entity blueprints must define a scalar display label:

```toml
[display.dropdown_option]
fields = ["name", "sku"]
separator = " / "
```

`separator` defaults to ` · `. Server-rendered labels respect each attribute's
`context_fallback` policy.

## Validation

Unknown keys, invalid selectors or policy values, missing mixins, and malformed
definitions return `422 invalid_blueprint_definition`.
