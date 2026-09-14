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
blueprint codes contain only ASCII letters, numbers, hyphens, and underscores.

## Attributes

Every attribute declares exactly one of `value_type` or `from`. Supported value
types are `string`, `number`, `integer`, `boolean`, `date`, `datetime`, `time`,
`relationship`, and `file`.

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

Set `readonly = true` to make an attribute preview-only in the Catalog web app.
It is intended for values managed by system actions such as agents, extensions,
rules, the API, or the CLI; those integrations can still write the attribute.
The default is `false`. Read-only attributes remain visible in entity forms but
cannot be changed, cleared, linked, or uploaded through the web UI.

```toml
[[attributes]]
code = "external_id"
value_type = "string"
readonly = true
```

Blank contextual string fields remove their override instead of storing an empty
string. Missing contextual values resolve according to `context_fallback`.

Scalar attributes may set `default_value`. The value is stored in the default
context when an entity is created, unless the create request supplies a value
for that attribute in the default context. Defaults support `string`, `number`,
`integer`, `boolean`, `date`, `datetime`, and `time`; relationships and files do
not support defaults.

```toml
[[attributes]]
code = "status"
value_type = "string"
default_value = "draft"
```

A relationship may restrict its target type and directional cardinality.
`cardinality = "one"` allows at most one active target per source and context;
`target_cardinality = "one"` allows at most one source to claim a target for the
versioned field and context. Each defaults to `"many"`, so `cardinality = "one"`
alone models a reusable single-select relationship.

```toml
[[attributes]]
code = "category"
value_type = "relationship"
target_blueprint = "category"
cardinality = "one"
```

A relationship write that would violate this invariant returns
`409 relationship_cardinality_conflict`. A migration preview reports the same
kind of issue when existing relationship values cannot fit a target revision's
source limit. Set both cardinalities to `"one"` only for exclusive pairing; see
[Tags, labels, and classifications](classifications.md) for the recommended
model for controlled classifications.

File attributes declare their cardinality and upload policy. `many` values are
ordered by default; set `ordered = false` when callers must not rely on their
order. File policy fields are valid only with `value_type = "file"`.

```toml
[[attributes]]
code = "product_images"
value_type = "file"
cardinality = "many"
allowed_mime_groups = ["image"]
allowed_extensions = ["jpg", "png", "webp"]
max_bytes = 10485760
purposes = ["product_image"]
image_only = true
```

`cardinality` is `one` by default. The compiler rejects a file value schema or
relationship target, duplicate/empty policy entries, invalid purpose codes,
zero size limits, and ordered single-file declarations. The persisted policy is
part of the pinned blueprint revision.

## Entity Schema

Entity blueprints may define an `entity_schema` JSON Schema contract. Root
`required`, `properties`, `dependentRequired`, and `dependentSchemas` entries
must name attributes materialized by the blueprint, including selected include
attributes. Nested schema properties describe an attribute's value and are not
treated as blueprint attribute codes.

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

## Dropdown Options

Entity blueprints must define how they appear in relationship dropdowns:

```toml
[views.dropdown_option]
type = "dropdown_option"
fields = ["name", "sku"]
separator = " / "
```

`separator` defaults to `·`. Server-rendered dropdown labels respect each attribute's
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

[[views.table.columns]]
field = "title"
label = "Product"

[[views.table.columns]]
field = "category.name"
label = "Category"
```

`columns` is the current table syntax. Each column names either a local scalar
field, a renderer-compatible direct file field, or a scalar field through at
most three relationship hops. Column paths
must be unique; each relationship and the scalar leaf must exist.
`label` is optional. The legacy `fields = ["title", "stock_on_hand"]`
shorthand remains supported for local scalar fields, but cannot be combined
with `columns`.

A column can use a built-in `catalog.*` renderer or an installed extension cell
renderer. Built-in `catalog.table_image@1` renders a direct image-only,
single-file attribute as a thumbnail:

```toml
[[views.table.columns]]
field = "main_photo"
label = "Image"
renderer = { id = "catalog.table_image", version = 1 }
```

It is not valid for relationship paths, multi-file attributes, or file
attributes that are not `image_only = true`.

An extension renderer ID and positive version must match an enabled extension
declaration for the resolved scalar value type, and `props` must be an object:

```toml
[[views.table.columns]]
field = "price"
label = "Price"
renderer = { id = "example.currency", version = 1, props = { currency = "USD" } }
```

See [Extensions](extensions.md#client-extension-runtime-v1) for the renderer
manifest and sandbox contract.

`stack`, `grid`, `section`, `tabs`, and `accordion` are recursive layout
blocks. `heading`, `text`, and `divider` are static blocks. `field` renders a
typed attribute and `relationship_list` renders a relationship attribute.
Table views currently support scalar fields only.

### Incoming Relationships

`incoming_relationship_list` displays entities that reference the current
entity through configured relationship fields. It opens a modal and does not
request linked entities until the user opens it. Results are cursor-paginated;
`page_size` controls each requested page and is bounded by the API's
`INCOMING_RELATIONSHIP_MAX_PAGE_SIZE` setting.

```toml
[views.detail]
type = "stack"
children = [
  { type = "field", field = "name" },
  {
    type = "incoming_relationship_list",
    label = "Products in this category",
    page_size = 10,
    relationships = [
      { source_blueprint = "product", field = "categories" },
    ],
    component = { id = "catalog.incoming_relationship_list_display", version = 1 },
  },
]
```

Each selector names a source blueprint and one of its relationship fields. A
source entity matched by multiple selectors appears once. Selecting an item
opens that source entity.

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
`contracts/view-components.json`; it validates the
version, allowed props, placement, value type, and requested view capability.
The frontend registry test enforces matching metadata between the two.

See [View Configuration](views.md) for the complete block grammar, heading
configuration, and component contracts. See
[Component Authoring](component-authoring.md) for the implementation workflow.

## JSON Schema Validation

Blueprint attributes can define scalar `value_schema` contracts and entity
blueprints can define an `entity_schema` for cross-field validation. Both use
JSON Schema Draft 2020-12 and are enforced by the API before values are stored.
See [JSON Schema Validation](json-schema-validation.md) for authoring syntax,
context behavior, and error handling.

## Validation

Unknown keys, invalid selectors or policy values, missing mixins, and malformed
definitions return `422 invalid_blueprint_definition`.
