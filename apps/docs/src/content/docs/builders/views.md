---
title: Views and layouts
description: Control how entity pages, forms, and Explorer tables are laid out, using declarative blocks and registered components.
---

Views are part of a blueprint revision. They describe layout only: which attributes appear, in what order, grouped how. They cannot run code. The web app renders the blocks it knows and the components registered with it.

Views are optional. Without them, the web app shows attributes in declaration order, respecting [visibility tags](/reference/blueprint/#visibility-tags).

## The views a blueprint can define

| View | Where it appears |
| --- | --- |
| `dropdown_option` | The label of an entity in pickers, filter pills, and search. Required for entity blueprints. |
| `detail` | The entity page and preview. |
| `edit` | The create and edit forms. |
| `table` | The Explorer's result columns. |
| `extension_layout` | Order and visibility of extension panels and actions on this blueprint's entity pages. |

## Building a detail or edit layout

A layout is a tree. Containers hold other blocks; leaves show a field or static content.

| Containers | Leaves |
| --- | --- |
| `stack` (vertical), `grid` (responsive columns), `section`, `tabs`, `accordion` | `field`, `relationship_list`, `incoming_relationship_list`, `heading`, `text`, `divider` |

TOML gives you two ways to write the same tree. Nested arrays of tables are verbose but easy to diff:

```toml
[views.edit]
type = "stack"

[[views.edit.children]]
type = "field"
field = "title"

[[views.edit.children]]
type = "grid"

[[views.edit.children.children]]
type = "field"
field = "price"

[[views.edit.children.children]]
type = "field"
field = "stock_on_hand"
```

Inline tables are compact:

```toml
[views.edit]
type = "stack"
children = [
  { type = "field", field = "title" },
  { type = "grid", children = [
    { type = "field", field = "price" },
    { type = "field", field = "stock_on_hand" },
  ] },
]
```

Tabs and accordions take a list of labeled groups:

```toml
[views.detail]
type = "tabs"
tabs = [
  { label = "Overview", children = [{ type = "field", field = "title" }] },
  { label = "Logistics", children = [
    { type = "accordion", sections = [
      { label = "Stock", children = [{ type = "field", field = "stock_on_hand" }] },
    ] },
  ] },
]
```

`field` accepts scalar and file attributes. `relationship_list` accepts relationship attributes. Referencing an attribute the blueprint does not have, or the wrong kind, fails validation.

## Entity heading

To give the detail page a proper title, wrap fields in a `stack` with the `catalog.entity_heading` component. Its first child must be a scalar field and becomes the page heading. Later children, text or scalar fields, form the subtitle.

```toml
[[views.detail.children]]
type = "stack"
component = { id = "catalog.entity_heading", version = 1 }
children = [
  { type = "field", field = "title" },
  { type = "field", field = "sku" },
]
```

If the title field is empty, the entity ID is shown. The heading is removed from the page body so it does not appear twice.

## Showing what links here

`incoming_relationship_list` shows entities that point at the current one, such as the products in a category. It is a button that opens a paged list, so nothing is loaded until someone asks.

```toml
{ type = "incoming_relationship_list",
  label = "Products in this category",
  page_size = 10,
  relationships = [{ source_blueprint = "product", field = "categories" }] }
```

List several `relationships` to combine sources. An entity that matches more than one appears once.

## Hierarchies

`catalog.relationship_hierarchy` shows ancestor chains as breadcrumbs, resolved in the selected context.

On a self-referencing relationship, such as `category.parent`, it shows the current entity's own ancestry:

```toml
{ type = "relationship_list", field = "parent",
  component = { id = "catalog.relationship_hierarchy", version = 1 } }
```

On a relationship to another blueprint, set `parent_field` to the target's self-referencing field. On a product, this shows the full path of every linked category, such as *Apparel › Shirts › Linen*:

```toml
{ type = "relationship_list", field = "categories",
  component = { id = "catalog.relationship_hierarchy", version = 1, props = { parent_field = "parent" } } }
```

## Explorer table

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

A column is a local scalar attribute, or a path through up to three relationships that ends in a scalar attribute. Each hop is resolved with the linked entity's own blueprint revision. If an older linked entity does not have the attribute, the cell is empty. A many-valued path can show several values in one cell.

Columns can be sorted when they resolve to a scalar and every relationship in the path has `cardinality = "one"`.

### Image thumbnails

`catalog.table_image` shows a thumbnail for a single-image attribute. The attribute must have `value_type = "file"`, `cardinality = "one"` (the default), and `image_only = true`.

```toml
[[views.table.columns]]
field = "main_photo"
label = "Image"
renderer = { id = "catalog.table_image", version = 1 }
```

### Extension cell renderers

An enabled extension can provide cell renderers, such as a currency formatter. Reference one by ID and version, and pass its options in `props`:

```toml
[[views.table.columns]]
field = "price"
renderer = { id = "example.currency", version = 1, props = { currency = "USD" } }
```

The renderer must be declared by an enabled extension for the column's value type. It runs in a sandboxed frame; see [Client contributions](/extensions/client/).

## Components

Every block can name a component with `component = { id, version, props }`. The ID and version must match a registered component, the component must support the block and value type, and `props` may contain only the options it declares. The built-in components are listed in the [blueprint reference](/reference/blueprint/#component-references).

## Extension panels on entity pages

Workspace administrators set the default order of extension contributions for the whole workspace. A blueprint can override three entity outlets for its own entities:

```toml
[views.extension_layout]
type = "extension_layout"
version = 1

[views.extension_layout.outlets.entity_preview_panel]
order = ["acme.inventory:summary", "acme.pricing:margin"]
hidden = ["acme.legacy:panel"]
```

The outlets you can set here are `entity_preview_panel`, `entity_attribute_decoration`, and `entity_action`. The layout never grants permissions or enables an extension. Entries for extensions that are missing or disabled are kept and ignored, so a layout survives an extension being turned off and on again.
