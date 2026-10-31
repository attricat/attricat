---
title: Views and layouts
description: Control how record pages, forms, and Explorer tables are laid out, using declarative blocks and registered components.
---

Views are part of a blueprint revision. They describe layout only: which attributes appear, in what order, grouped how. They cannot run code. The web app renders the blocks it knows and the components registered with it.

Views are optional. Without them, the web app shows attributes in declaration order, respecting [visibility tags](/reference/blueprint/#visibility-tags).

## The views a blueprint can define

| View | Where it appears |
| --- | --- |
| `dropdown_option` | The label of a record in pickers, filter pills, and search. Required for record blueprints. |
| `page_title` | The record page's browser tab title. Falls back to the `dropdown_option` label. |
| `detail` | The record page, its preview, and the create form. Fields are shown and edited in this layout. |
| `table` | The Explorer's result columns. |
| `explorer` | Explorer defaults, such as the field the **Browse by** panel lists. |
| `extension_layout` | Order and visibility of extension panels and actions on this blueprint's record pages. |

There is no separate edit view: records are edited in place on the detail layout. A blueprint that defines `views.edit` is rejected.

## Building a detail layout

A layout is a tree. Containers hold other blocks; leaves show a field or static content.

| Containers | Leaves |
| --- | --- |
| `stack` (vertical), `grid` (responsive columns), `section`, `tabs`, `accordion` | `field`, `relationship_list`, `incoming_relationship_list`, `heading`, `text`, `divider` |

YAML gives you two ways to write the same tree. Block style, one key per line, is verbose but easy to diff:

```yaml
views:
  detail:
    type: stack
    children:
      - type: field
        field: title
      - type: grid
        children:
          - type: field
            field: price
          - type: field
            field: stock_on_hand
```

Flow style, with `{ }` and `[ ]`, is compact:

```yaml
views:
  detail:
    type: stack
    children:
      - {type: field, field: title}
      - {type: grid, children: [{type: field, field: price}, {type: field, field: stock_on_hand}]}
```

Tabs and accordions take a list of labeled groups:

```yaml
views:
  detail:
    type: tabs
    tabs:
      - label: Overview
        children:
          - {type: field, field: title}
      - label: Logistics
        children:
          - type: accordion
            sections:
              - label: Stock
                children:
                  - {type: field, field: stock_on_hand}
```

`field` accepts scalar and file attributes. `relationship_list` accepts relationship attributes. Referencing an attribute the blueprint does not have, or the wrong kind, fails validation.

The detail layout drives both display and editing. On the record page, every field the user may change is an editable control in its place in the layout; readonly fields, fields locked by the record's status, and fields managed in the default context show their value. Editable attributes the layout leaves out appear after it under **Other attributes**, so a required attribute is always reachable. The create form uses the same layout.

## Record heading

To give the detail page a proper title, wrap fields in a `stack` with the `attricat.record_heading` component. Its first child must be a scalar field and becomes the page heading. Later children, text or scalar fields, form the subtitle.

```yaml
views:
  detail:
    children:
      - type: stack
        component: {id: attricat.record_heading, version: 1}
        children:
          - {type: field, field: title}
          - {type: field, field: sku}
```

If the title field is empty, the record ID is shown. The heading is removed from the page body so it does not appear twice. The heading only displays values, so its editable fields are offered as editors first, above the rest of the layout.

## Showing what links here

`incoming_relationship_list` shows records that point at the current one, such as the products in a category. It is a link that opens the Explorer in a new tab, filtered to those records. The Explorer searches one blueprint at a time, so a list with several `relationships` shows one link per entry, named after its source blueprint.

```yaml
- type: incoming_relationship_list
  label: Products in this category
  page_size: 10
  relationships:
    - {source_blueprint: product, field: categories}
```

List several `relationships` to combine sources. A record that matches more than one appears once.

## Hierarchies

`attricat.relationship_hierarchy` shows ancestor chains as breadcrumbs, resolved in the selected context.

On a self-referencing relationship, such as `category.parent`, it shows the current record's own ancestry:

```yaml
- type: relationship_list
  field: parent
  component: {id: attricat.relationship_hierarchy, version: 1}
```

On a relationship to another blueprint, set `parent_field` to the target's self-referencing field. On a product, this shows the full path of every linked category, such as *Apparel › Shirts › Linen*:

```yaml
- type: relationship_list
  field: categories
  component:
    id: attricat.relationship_hierarchy
    version: 1
    props: {parent_field: parent}
```

## Explorer table

```yaml
views:
  table:
    type: table
    columns:
      - {field: title, label: Product}
      - {field: family.product_type.name, label: Product type}
```

A column is a local scalar attribute, or a path through up to three relationships that ends in a scalar attribute. Each hop is resolved with the linked record's own blueprint revision. If an older linked record does not have the attribute, the cell is empty. A many-valued path can show several values in one cell.

Columns can be sorted when they resolve to a scalar and every relationship in the path has `cardinality: one`.

### Image thumbnails

`attricat.table_image` shows a thumbnail for a single-image attribute. The attribute must have `value_type: file`, `cardinality: one` (the default), and `image_only: true`.

```yaml
views:
  table:
    columns:
      - field: main_photo
        label: Image
        renderer: {id: attricat.table_image, version: 1}
```

### Extension cell renderers

An enabled extension can declare table cells, such as an amount with a fixed number of decimals and a unit. Reference one by ID and version, and pass its options in `props`:

```yaml
views:
  table:
    columns:
      - field: price
        renderer:
          id: example.amount
          version: 1
          props: {precision: 2, unit: USD}
```

The cell must be declared by an enabled extension for the column's value type, and each prop must match the type and bounds it declares. Attricat draws the cell itself; see [Table cells](/extensions/manifest/#table-cells).

### Extension attribute editors

On the record page, a field in the `detail` view can use an input control from an enabled extension instead of the default one, such as a formula input that suggests attribute names. Name the editor by ID and version, and pass its options in `props`:

```yaml
views:
  detail:
    type: stack
    children:
      - type: field
        field: discount_formula
        editor: {id: formula, version: 1, props: {rows: 2}}
```

The editor must be declared by an enabled extension for the field's value type, and each prop must match the type and bounds it declares. The field keeps its label, help, validation and saving: it still saves when you leave it or press Enter, and Escape restores the saved value. While the extension is disabled or not working, the field shows its default control. Editors apply only on the record page, never in Explorer tables.

## Explorer browse panel

The **Browse by** panel beside the Explorer results lists the targets of one relationship. A blueprint with its own [hierarchy](#hierarchies) lists its own tree instead, under the blueprint's name. By default the panel opens for that tree, or else for the first relationship whose target blueprint has a hierarchy. To choose the field, name it in `views.explorer`:

```yaml
views:
  explorer:
    type: explorer
    browse_by: brand
```

The field must be a relationship with a single target blueprint. A field whose target has no hierarchy is listed flat. Set `browse_by: false` to keep the panel closed until someone opens it. People can still pick another field or hide the panel; their choice takes precedence in their browser.

To browse by a blueprint that is a few relationships away, name a path of up to three relationships, separated by dots. On a vehicle linked to a model, which is linked to a brand, this lists brands and counts each brand's vehicles:

```yaml
views:
  explorer:
    type: explorer
    browse_by: model.brand
```

Every relationship on a path must have a single target blueprint and `cardinality: one`. When the last target has a hierarchy, the panel shows its tree. The panel's **Browse by** list also offers such paths after the direct relationships, labelled like "Model → Brand".

## Field controls

A `string` attribute is plain text by default. To show it as a color, email address, URL, phone number, or Markdown, name a display component on its detail field (or table column). Where the field is editable, the web app uses the paired edit component, such as `attricat.url_edit` for `attricat.url_display`:

```yaml
views:
  detail:
    type: stack
    children:
      - type: field
        field: website
        component: {id: attricat.url_display, version: 1}
      - type: field
        field: description
        component: {id: attricat.markdown_display, version: 1}
  table:
    columns:
      - field: website
        renderer: {id: attricat.url_display, version: 1}
```

| Control | Components | Behavior |
| --- | --- | --- |
| Color | `attricat.color_display`, `attricat.color_edit` | Six-digit hex (`#RRGGBB`), chosen with a color picker; the edit button beside it opens a field for typing an exact value. Shown as a swatch next to the text, which copies the value when clicked. |
| Email | `attricat.email_display`, `attricat.email_edit` | One plain ASCII address, such as `name+tag@example.com`. Shown as a `mailto:` link. |
| URL | `attricat.url_display`, `attricat.url_edit` | Absolute `http://` or `https://` URLs only. Links open in a new tab. |
| Phone | `attricat.phone_display`, `attricat.phone_edit` | Stored as typed. Numbers starting with `+` and a country code become `tel:` links; extensions can use `ext.` or `x`. |
| Markdown | `attricat.markdown_display`, `attricat.markdown_edit` | CommonMark with **Write** and **Preview** tabs. On the record page a set value shows formatted until you choose to edit it. Raw HTML is ignored, images show their alt text, and links are limited to HTTP(S), `mailto:`, relative paths, and fragments. Text is stored exactly as typed, including whitespace. Not available for table columns. |

Values that do not fit the control, such as older data, are still shown, as plain text without a link or swatch. Fields without a component use the standard editor for their value type.

The edit controls validate only in the web app. To reject bad values from the API, CLI, and imports too, add a `value_schema` or `record_schema`; see [Validation](/builders/validation/).

## Components

Every block can name a component with `component = { id, version, props }`. The ID and version must match a registered component, the component must support the block and value type, and `props` may contain only the options it declares. The built-in components are listed in the [blueprint reference](/reference/blueprint/#component-references).

## Extension panels on record pages

Workspace administrators set the default order of extension contributions for the whole workspace. A blueprint can override three record page outlets for its own records:

```yaml
views:
  extension_layout:
    type: extension_layout
    version: 1
    outlets:
      record_tab:
        order: ['acme.inventory:summary', 'acme.pricing:margin']
        hidden: ['acme.legacy:panel']
```

The outlets you can set here are `record_tab` and `record_action`. The layout never grants permissions or enables an extension. Entries for extensions that are missing or disabled are kept and ignored, so a layout survives an extension being turned off and on again.
