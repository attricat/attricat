# View Configuration

Blueprint `views` define declarative layouts for catalog screens. The platform
renders known blocks and registered components; definitions never run arbitrary
frontend code. Views are optional, with schema-order fallbacks for blueprints
that do not define them.

## View Types

- `detail`: read-only entity preview.
- `edit`: create and edit form layout.
- `table`: configured columns in the entity explorer.

`detail` and `edit` use recursive layout roots. A table column may reference a
local scalar field or a scalar field one hop through a `one_to_one`
relationship.

```toml
[views.table]
type = "table"

[[views.table.columns]]
field = "title"
label = "Product"

[[views.table.columns]]
field = "category.name"
label = "Category"
```

`columns` must be non-empty and has unique `field` paths. A relationship path
requires an existing relationship, an existing scalar target field, and
`cardinality = "one_to_one"` on the relationship. `label` is optional.
`fields = ["title", "price", "available"]` remains a legacy shorthand for
local scalar columns; do not combine it with `columns`.

Explorer users can sort a configured scalar column in ascending or descending
order. The table uses the API's keyset cursor for the selected field, including
for a configured relationship path. Columns that resolve to non-scalar values
are not sortable.

## Blocks

Container blocks are `stack`, `grid`, `section`, `tabs`, and `accordion`.
`stack`, `grid`, and `section` contain `children`; tabs and accordion sections
contain named `children` collections. Leaf blocks are `field`,
`relationship_list`, `heading`, `text`, and `divider`.

```toml
[views.detail]
type = "stack"

[[views.detail.children]]
type = "tabs"

[[views.detail.children.tabs]]
label = "Overview"

[[views.detail.children.tabs.children]]
type = "grid"

[[views.detail.children.tabs.children.children]]
type = "field"
field = "title"

[[views.detail.children.tabs.children.children]]
type = "field"
field = "price"
```

Every referenced field must be an effective attribute. `field` accepts scalar
attributes; `relationship_list` accepts relationships only.

## Entity Heading

The preview heading reuses a normal `stack` with the
`catalog.entity_heading` component override. Its first child is the scalar
field rendered as the page `h1`; subsequent children are subtitle or metadata
content.

```toml
[[views.detail.children]]
type = "stack"
component = { id = "catalog.entity_heading", version = 1 }

[[views.detail.children.children]]
type = "field"
field = "title"

[[views.detail.children.children]]
type = "text"
text = "Current catalog entry"
```

The entity ID is the fallback when the first field is unset or its renderer
fails. The heading block is removed from the normal detail body so it is not
rendered twice. This component is detail-only; its first child must be a scalar
field and later children may be text or scalar fields.

## Components

Data blocks select registered components with a versioned reference:

```toml
[[views.edit.children]]
type = "field"
field = "price"
component = { id = "catalog.field_edit", version = 1 }
```

Component IDs use lowercase dotted namespaces. The frontend registry lives at
`apps/catalog-web/src/features/views/components/registry.ts`; each registered
component has its own module in that directory. A module exports its typed
definition and, when it has one, its React renderer. `EntityView` resolves a
blueprint reference through this registry and isolates field renderers with an
error boundary.

The Rust blueprint compiler reads
`contracts/view-components.json` to validate a
component's version, props, placement, value type, and required `display` or
`edit` capability. Keep this contract synchronized with the TypeScript
definition. `registry.test.ts` verifies that their metadata is identical.

See [Component Authoring](component-authoring.md) for the implementation and
verification workflow.
