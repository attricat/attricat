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

A custom `edit` layout must place (as `field` or `relationship_list`) every
non-readonly attribute in `entity_schema.required`; the compiler rejects it
otherwise. For blueprints stored before that check, `EntityForm` renders
required attributes the view omits after the layout so Save is never blocked
by a hidden field.

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
historical revisions yield an empty cell. `label` is optional; a direct column
defaults to the attribute's `name`, and a column without either uses the
humanized field path.

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

### Table context

Explorer sends its selected context as the search `context_code` (default
`default`). Table cells, relationship-path hops, scalar and relationship
filters, and column sorts read the value of the nearest context on the selected
context's ancestor path, so an untranslated value shows the inherited one. An
attribute with `context_fallback = "none"` reads only the selected context. The
**Display** column uses the nearest context's `display` label. Free-text query
terms still match values in every context.

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

In `edit`, a relationship field opens the entity picker for its allowed target
blueprints. When the attribute lists several `target_blueprints`, the picker
shows a **Target blueprint** selector and searches one of them at a time;
selected entities are labelled with their own blueprint's `dropdown_option`
view. An `incoming_relationship_list` may name such a field on any of its
allowed target blueprints.

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
`apps/catalog-web/src/features/views/components/registry.ts`. Layout and
relationship components have their own modules in that directory; the string
field controls below live together in `apps/catalog-web/src/features/views/controls`.
`EntityView` resolves a blueprint reference through this registry and isolates
field renderers with an error boundary.

The Rust blueprint compiler reads
`contracts/view-components.json` to validate a
component's version, props, placement, value type, and required `display` or
`edit` capability. Keep this contract synchronized with the TypeScript
definition. `registry.test.ts` verifies that their metadata is identical.

### String field controls

These controls change how a `string` attribute is shown and edited. They are
opt-in per view: unconfigured string fields keep the standard text input and
plain-text display. None of them accepts props, and none changes what is
stored.

| Control | Display (detail field, table column) | Edit (edit field) |
| --- | --- | --- |
| Color | `catalog.color_display@1` | `catalog.color_edit@1` |
| Email | `catalog.email_display@1` | `catalog.email_edit@1` |
| URL | `catalog.url_display@1` | `catalog.url_edit@1` |
| Phone | `catalog.phone_display@1` | `catalog.phone_edit@1` |
| Markdown | `catalog.markdown_display@1` (detail field only) | `catalog.markdown_edit@1` |

```toml
[views.detail]
type = "stack"
children = [{ type = "field", field = "website", component = { id = "catalog.url_display", version = 1 } }]

[views.edit]
type = "stack"
children = [{ type = "field", field = "website", component = { id = "catalog.url_edit", version = 1 } }]

[views.table]
type = "table"
columns = [{ field = "website", renderer = { id = "catalog.url_display", version = 1 } }]
```

Shared behavior:

- The color, email and URL editors validate in the web form only, as the user
  types and again on Save. Choosing a component does not constrain API, CLI or
  agent writes; add an attribute `value_schema` or the blueprint
  `entity_schema` for that.
- Values are trimmed before saving, except Markdown, which is stored verbatim.
  Clearing an optional value unsets it; required, readonly, context and draft
  behavior is the same as for other fields.
- Display controls never hide data. A stored value the control cannot use
  (for example a malformed URL) is shown as plain text without a link or
  swatch. Links do not trigger the surrounding table row.

#### Color

The editor accepts opaque six-digit hex (`#RRGGBB`, case-insensitive) as text
or through the native color picker. Shorthand, alpha, named colors and CSS
expressions are rejected. The display shows a swatch next to the stored text.
For API-wide enforcement add a pattern such as `^#[0-9A-Fa-f]{6}$`.

#### Email

The editor accepts a single ASCII dot-atom address (plus tags allowed) and
preserves case. Internationalized addresses, quoted local parts, display names
and recipient lists are not supported. Valid values link to the user's mail
client; nothing is sent or verified. Set
`value_schema = '{"type":"string","format":"email"}'` to validate API writes
too; the server's format check may accept a broader set of addresses.

#### URL

Only absolute HTTP/HTTPS URLs without whitespace, control characters,
backslashes or embedded credentials are accepted and linked. Links open in a
new tab without opener access or a referrer, and no destination is fetched for
a preview. Typed URLs are not rewritten.

#### Phone

Any text can be typed, including national numbers and extensions; the display
preserves it. Only international numbers starting with `+` become `tel:` links:
spaces, parentheses, periods and hyphens are removed from the dial target, and
a numeric extension after `ext.`, `ext` or `x` becomes `;ext=`. No country is
inferred and reachability is not checked.

#### Markdown

The editor has Write and Preview tabs; preview and display render CommonMark.
Raw HTML is ignored, images show their alt text without being fetched, and
links are limited to HTTP(S), `mailto:`, relative paths and fragments. The
source is stored exactly as typed, including indentation and trailing spaces;
whitespace-only input counts as unset. Entity comments use the same renderer
and link policy.

### Status attributes

Status is not a view component: it is configured on the attribute's
`value_schema` and applies wherever the attribute appears. See
[Status attributes](status-control.md).

See [Component Authoring](component-authoring.md) for the implementation and
verification workflow.
