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
local scalar field or a scalar leaf through at most three relationship hops.

## Extension layout

Entity blueprint revisions may override workspace extension placement for their
owned surfaces (`entity_preview_panel`, `entity_attribute_decoration`, and
`entity_action`). This is versioned declarative data, published with the normal
blueprint revision flow; it neither grants permissions nor runs extension code.
Use stable contribution keys (`<extension-id>:<contribution-id>`). Missing or
disabled contributions are ignored at runtime while their entries remain in the
published layout for restoration. Publication rejects a currently enabled
contribution if its key is assigned to a different outlet; unknown keys remain
valid so removed or disabled contributions can later be restored:

```toml
[views.extension_layout]
type = "extension_layout"
version = 1

[views.extension_layout.outlets.entity_preview_panel]
order = ["acme.inventory:summary"]
hidden = ["acme.legacy:panel"]
```

Each declared entity-owned outlet replaces only that outlet's workspace default;
unspecified entity outlets and all global outlets continue to use the workspace
layout. Global outlets, including navigation and explorer surfaces, are
workspace-only.

```toml
[views.table]
type = "table"

[[views.table.columns]]
field = "title"
label = "Product"

[[views.table.columns]]
field = "family.product_type.name"
label = "Product type"
```

`columns` must be non-empty and have unique `field` paths. Every intermediate
segment must be a relationship and relationship-path leaves must be scalar.
Direct columns may also be file attributes when a compatible renderer is used.
Paths are resolved with each linked entity's pinned blueprint revision; incompatible
historical revisions yield an empty cell. `label` is optional.

Use `catalog.table_image@1` to render a direct, image-only single-file attribute
as a thumbnail:

```toml
[[views.table.columns]]
field = "main_photo"
label = "Image"
renderer = { id = "catalog.table_image", version = 1 }
```

The renderer is valid only when the attribute has `value_type = "file"`,
`cardinality = "one"`, and `image_only = true`. It cannot be used on a
relationship path or a multi-file/non-image attribute. Empty values display the
normal unset state; files still processing or without a usable thumbnail show
the thumbnail status UI.
`fields = ["title", "price", "available"]` remains a legacy shorthand for
local scalar columns; do not combine it with `columns`.

Explorer's version-scope selector defaults to the current published revision,
can select an older published revision, and provides an explicit **All
versions** option. Current and historical single-version scopes can sort a
configured scalar column in ascending or descending order. In an all-version
scope, relationship sorting is enabled only when the complete matching result
set contains one source revision. The built-in **Schema** column is sortable
without any blueprint table-column configuration. Select **All versions** and
click **Schema** for ascending version order (oldest/outdated first); click
again for descending order (newest first). In a single-version scope every
entity has the same schema version, so this sort only changes the ID tie order.
The built-in **Publication** column is sortable when the selected context is an
enabled publication channel. Click once for unpublished entities first and
again for published entities first. Changing the selected context sorts by
that channel instead; the agent `search_entities` tool can request the same
order with `sort.field = "publication_status"` and `sort.context_code`.

The table uses the API's version-bound keyset cursor for the selected field.
Relationship-path sorting requires `cardinality = "one"` on every hop and uses
fixed-depth, leaf-first traversal for paths of up to three hops. Many-valued
paths are displayable but are not sortable, and columns that resolve to
non-scalar values are rejected.

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

Component IDs use dot-delimited lowercase, underscore-separated segments. The frontend registry lives at
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

### Colors

Colors remain string attributes. Opt in per view with `catalog.color_display@1`
for detail fields or table columns and `catalog.color_edit@1` for edit fields.
Both accept only string attributes and have no props:

```toml
[views.detail]
type = "stack"
children = [{ type = "field", field = "hex", component = { id = "catalog.color_display", version = 1 } }]

[views.edit]
type = "stack"
children = [{ type = "field", field = "hex", component = { id = "catalog.color_edit", version = 1 } }]

[views.table]
type = "table"
columns = [{ field = "hex", renderer = { id = "catalog.color_display", version = 1 } }]
```

The editor accepts opaque, six-digit hex (`#RRGGBB`, case-insensitive) through
text or a native color picker. Clear the text to unset an optional value.
Shorthand, alpha, named colors, and CSS expressions are not supported. The
read-only renderer shows a swatch alongside the stored text; invalid legacy
values remain visible as text without a swatch. Neither component changes
unconfigured string fields or enforces color syntax on API/CLI writes. For
API-wide enforcement, add an appropriate pattern to the existing blueprint
`entity_schema` (and `required` if the value must be present).

### Email fields

Use `catalog.email_display` version 1 for a string field in a detail view or
as a table column's `renderer`; use `catalog.email_edit` version 1 for a field
in an edit view. Neither component accepts props. Store the address as a string,
not a `mailto:` URL, and set the attribute's
`value_schema = '{"type":"string","format":"email"}'` to validate API writes too.
Choosing an input component alone does not impose a server-side data constraint.

The input supports a single ASCII dot-atom address (including plus tags),
preserves case, and uses the normal form trimming, required-field, context and
draft behavior. Internationalized addresses, quoted local parts, display names
and recipient lists are not supported by this control. JSON Schema email format
validation on the server may accept a broader set of addresses. Unsupported or
malformed stored values remain visible as plain text; supported values link to
the user's mail client. The component never sends mail or checks deliverability.

See [Component Authoring](component-authoring.md) for the implementation and
verification workflow.
