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

- `default` is the default and uses the nearest ancestor with a value or set,
  up to the root `default` context.
- `none` leaves the attribute absent when it has no direct value.

`context_editable` controls writes for every value type, including relationships:

- `all` is the default and permits writes in every context.
- `default` permits writes only in the default context. Other contexts render it
  read-only, and the API rejects writes.

Set `readonly = true` to make an attribute preview-only in the Catalog web app.
It is intended for values managed through authorized API or CLI operations,
including agent and extension actions; `readonly` does not restrict those
server-side writes. Rules evaluate findings and do not write attribute values.
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

### Attribute tags

Attribute `tags` are free-form metadata for Catalog extensions and domain
integrations. Catalog reserves the following visibility tags as default UI
hints:

| Tag | Default omission surface |
| --- | --- |
| `hidden` | Every native default surface listed below |
| `hidden:form` | Fallback create and edit forms |
| `hidden:detail` | Fallback entity preview/detail views |
| `hidden:explorer` | Explorer facet and filter candidates |
| `hidden:metadata` | The blueprint Attributes metadata table |

Tags compose: `hidden` applies to every surface, while a scoped tag applies only
to its named surface. They affect automatic/fallback discovery only; an
explicit blueprint view may deliberately render a tagged attribute. Tags are
not access control: attribute metadata and values remain available through the
API, CLI, agents, and extensions according to their existing permissions. Raw
blueprint TOML remains available to blueprint administrators.

```toml
[[attributes]]
code = "price_amount"
value_type = "integer"
tags = ["hidden:form", "hidden:detail"]
```

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
Table views also support direct file fields with a compatible renderer and
scalar leaves reached through relationship paths, as shown above.

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

## Entity migration status

When a published entity blueprint revision is storage-compatible with its
immediately preceding published revision, **Migrate compatible entities** starts
a background migration batch. The batch examines every active entity pinned to
an older version of that blueprint, not only entities on the immediately
preceding version. Each entity is migrated when its individual preview is
`ready`; incompatible entities remain available for review. Open the blueprint
in **Manage → Blueprints** and select the **Migrations** tab to inspect every
batch for that blueprint. While a batch for the current target version is
`queued` or `running`, the migration action is disabled so another batch cannot
be started for that version.

The table shows the target version, current status, processed/total progress,
migrated, needs-review and failed counts, creation/start/completion timestamps,
and batch ID. While any batch is `queued` or `running`, the page refreshes
automatically every two seconds; **Refresh** requests the latest state
immediately. A completed batch remains in the history. The UI reads this state
from `GET /api/blueprints/{blueprint_id}/migration-batches`; migration batch rows
in `blueprint_migration_batches` remain the source of truth.

Batch statuses are:

- `draft`: created but not queued (retained for compatibility with older rows),
- `queued`: persisted and waiting for a worker,
- `running`: claimed by a migration worker,
- `completed`: the worker inspected every eligible entity and finished, and
- `superseded`: replaced by a newer batch.

`completed` describes the batch lifecycle. Individual entities that could not be
migrated remain recorded in `entity_blueprint_migrations` for separate review.

## Validation

Unknown keys, invalid selectors or policy values, missing mixins, and malformed
definitions return `422 invalid_blueprint_definition`.
