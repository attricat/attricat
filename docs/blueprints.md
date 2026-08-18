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

`separator` defaults to `·`. Server-rendered labels respect each attribute's
`context_fallback` policy.

## Views

Entity blueprints can optionally define app views. Existing blueprints without
views use the platform's schema-order fallback.

```toml
[views.detail]
type = "stack"

[[views.detail.children]]
type = "tabs"

[[views.detail.children.tabs]]
label = "Overview"

[[views.detail.children.tabs.children]]
type = "field"
field = "title"

[[views.detail.children.tabs]]
label = "Operations"

[[views.detail.children.tabs.children]]
type = "accordion"

[[views.detail.children.tabs.children.sections]]
label = "Stock"

[[views.detail.children.tabs.children.sections.children]]
type = "field"
field = "stock_on_hand"

[views.table]
type = "table"
fields = ["title", "stock_on_hand"]
```

`stack`, `grid`, `section`, `tabs`, and `accordion` are recursive layout
blocks. `heading`, `text`, and `divider` are static blocks. `field` renders a
typed attribute and `relationship_list` renders a relationship attribute.
Table views currently support scalar fields only.

`views.edit` uses the same layout blocks to order entity create/edit controls.
Field, relationship-list, and table blocks may optionally reference a
platform-registered component:

```toml
[[views.edit.children]]
type = "field"
field = "price"
component = { id = "catalog.field_edit", version = 1 }
```

Component IDs use lowercase, underscore-separated dotted namespaces. React
component definitions are assembled by the TypeScript registry at
`apps/catalog-web/src/features/views/components/registry.ts`, with each
definition in its own module. The Rust validation contract is
`apps/catalog-web/src/features/views/component-contract.json`; it validates the
version, allowed props, placement, value type, and requested view capability.
The frontend registry test enforces matching metadata between the two.

See [View Configuration](views.md) for the complete block grammar, heading
configuration, and component contracts.

## Validation

Unknown keys, invalid selectors or policy values, missing mixins, and malformed
definitions return `422 invalid_blueprint_definition`.
