---
title: Author a blueprint
description: Build a blueprint from an empty file to a published revision with relationships, views, validation, and a mixin.
---

A blueprint is a YAML document that describes one kind of catalog record: its attributes, how they behave across contexts, how they are validated, and how the web app lays them out. This guide builds a small product catalog one step at a time. Every key used here is listed in the [Blueprint YAML reference](/reference/blueprint/).

## Where to write blueprints

You can write blueprints in two places:

- **Manage → Blueprints → New blueprint** in the web app. See [The web editor](#the-web-editor).
- Any text editor, then upload with the CLI:

  ```sh
  acli blueprint validate --file product.yaml
  acli blueprint create --file product.yaml
  ```

  `validate` runs every check that saving runs without storing anything. Add `--blueprint-id <id>` to check a file as that blueprint's next revision.

Both paths store the YAML exactly as written. Keeping blueprint files in version control alongside your integration code works well; the CLI sends them unchanged.

You need `blueprints.write` to create drafts and `blueprints.publish` to publish them.

### The web editor

The blueprint editor and the [reusable attribute](/builders/modeling/#reuse-attributes) editor work the same way:

- **Examples** starts a new definition from a valid template.
- **Insert** adds a ready-made piece, such as a status attribute, a table view, or a unique key, with placeholders to fill in. Press Tab to move between them. Pieces that would duplicate something already defined are greyed out.
- Typing suggests keys and values, and hovering a key explains it. Mistakes against the format are underlined as you type.
- A moment after you stop typing, the editor runs every check that saving runs, without saving. The status above the editor says **Ready to save**, or names the line with the problem, which is also underlined.
- **Preview** shows the last version that passed the checks. For a blueprint it shows each view and an editable sample record; for a reusable attribute, its field. Sample values are checked against the attribute's `value_schema` and the blueprint's `record_schema`, and are never saved.
- In a blueprint's preview, a field's details (the ⓘ button beside it) offer **Show in definition**, which selects the attribute's code in the editor; the button beside the view tabs does the same for the view shown. Moving the cursor the other way, into an attribute or a view, makes the preview show and highlight it.
- **Go to…** (Ctrl+Shift+O, or Command+Shift+O) lists the definition's sections, attributes and views; type part of a name and press Enter to jump there.
- With the cursor on an attribute code or include alias, every use of it is highlighted. Shift+F12 lists the uses, and F2 renames it everywhere at once. A new revision can't rename codes its records already store values under; add a new attribute instead.
- Drag the divider between the editor and **Preview** to resize them, or choose **Hide preview** to give the editor the full width. Every definition editor remembers the layout.
- Press Ctrl+S (Command+S on a Mac) to save.

## Step 1: the smallest valid blueprint

```yaml
format_version: 1
code: category
name: Category
kind: record

attributes:
  - code: name
    value_type: string

views:
  dropdown_option:
    type: dropdown_option
    fields: [name]
```

- `code` identifies the blueprint for its whole life. Pick it carefully; it cannot change in later revisions.
- `kind: record` means people can create records from it.
- Every record blueprint needs `views.dropdown_option`. It tells Attricat how to label a category wherever one is shown in a list: relationship pickers, filter pills, search results.

Save this as a draft. A draft can be edited freely and cannot hold records yet.

## Step 2: publish

Publishing freezes the revision. From then on it never changes, and records can be created from it.

In the web app, open the blueprint and choose **Publish**. With the CLI:

```sh
acli blueprint publish <blueprint-id> 1
```

To change a published blueprint, create a new revision. The next section explains why that matters.

## Step 3: a product with typed attributes

```yaml
format_version: 1
code: product
name: Product
kind: record

attributes:
  - code: title
    value_type: string

  - code: sku
    value_type: string
    context_editable: default

  - code: price
    value_type: number
    value_schema: '{"type":"number","minimum":0}'

  - code: stock_on_hand
    value_type: integer
    default_value: 0

  - code: available
    value_type: boolean

  - code: available_on
    value_type: date

  - code: order_cutoff
    value_type: time

views:
  dropdown_option:
    type: dropdown_option
    fields: [title, sku]
    separator: ' / '
```

A few choices in this file:

- `sku` has `context_editable: default`. A SKU is the same everywhere, so it can only be edited in the default context. In any other context it shows as read-only.
- `price` has a `value_schema` that rejects negative numbers. The schema is JSON inside a YAML string. See [Validation](/builders/validation/).
- `stock_on_hand` starts at `0` on new records because of `default_value`.
- `order_cutoff` is a `time`: a wall-clock time plus an IANA time zone, such as 09:30 in `Europe/Warsaw`.

## Step 4: relationships

Link each product to its categories and a single brand:

```yaml
attributes:
  - code: categories
    value_type: relationship
    target_blueprint: category

  - code: brand
    value_type: relationship
    target_blueprint: brand
    cardinality: one
```

`target_blueprint` makes Attricat reject a link to anything that is not a `category` (or `brand`). `categories` allows many targets; `brand` allows one per product. Many products can still share a brand. If a target must belong to exactly one source, also set `target_cardinality: one`.

The target blueprints must exist before you publish the product blueprint.

Relationships are how you model tags, labels, and taxonomies in Attricat. [Model your catalog](/builders/modeling/) explains why a category should be a record and not a string.

## Step 5: context behavior

[Contexts](/guides/contexts/) let a value differ by market, language, or channel. Each attribute decides two things:

- `context_fallback`: what a context without its own value shows. The default, `"default"`, inherits from the nearest parent context that has a value. `"none"` shows nothing.
- `context_editable`: where the attribute can be written. The default, `"all"`, allows every context. `"default"` allows only the root.

```yaml
attributes:
  - code: description
    value_type: string
    # Inherit from the parent market when a channel has no description.
    context_fallback: default

  - code: promo_banner
    value_type: string
    # A banner shown in one channel must not leak into its children.
    context_fallback: none
```

## Step 6: files

```yaml
attributes:
  - code: main_photo
    value_type: file
    allowed_mime_groups: [image]
    image_only: true
    max_bytes: 10485760

  - code: manuals
    value_type: file
    cardinality: many
    allowed_extensions: [pdf]
```

A file attribute holds one file by default; `cardinality: many` makes it an ordered list. The policy is checked on every upload: the file's content signature, MIME type, extension, and size. Images get `thumbnail` and `display` WebP variants generated in the background.

## Step 7: layout

Without views, the web app lists attributes in the order you declared them. Add views when you want tabs, grids, or a curated Explorer table.

```yaml
views:
  detail:
    type: stack
    children:
      - type: stack
        component: {id: attricat.record_heading, version: 1}
        children:
          - {type: field, field: title}
          - {type: field, field: sku}
      - type: tabs
        tabs:
          - label: Overview
            children:
              - type: grid
                children:
                  - {type: field, field: price}
                  - {type: field, field: stock_on_hand}
              - {type: relationship_list, field: categories}
          - label: Media
            children:
              - {type: field, field: main_photo}
  table:
    type: table
    columns:
      - field: main_photo
        label: Image
        renderer: {id: attricat.table_image, version: 1}
      - {field: title}
      - {field: brand.name, label: Brand}
      - {field: price}
```

The `attricat.record_heading` stack turns its first field into the page title and the rest into a subtitle. The table column `brand.name` follows the `brand` relationship and shows the brand's `name`. Because `brand` has `cardinality: one`, that column can also be sorted.

[Views and layouts](/builders/views/) covers every block and component.

## Step 8: validation across fields

A `value_schema` checks one value. An `record_schema` checks the whole record, so it can express rules like "a product on sale needs a sale price". This example assumes the blueprint also has an `on_sale` boolean and a `sale_price` number:

```yaml
record_schema: |
  {
    "type": "object",
    "required": ["title", "sku"],
    "if": { "properties": { "on_sale": { "const": true } }, "required": ["on_sale"] },
    "then": { "required": ["sale_price"] }
  }
```

Put `record_schema` with the other top-level keys, at the same indentation as `attributes`. Indented under an attribute, it would belong to that attribute instead.

Attricat checks the schema in every context after every change. A write that would leave any context invalid is rejected with `422 record_schema_mismatch` and nothing is saved.

JSON Schema cannot compare two attributes. For rules like "a sale price must be lower than the price", add a named check to the same schema under `x-attricat-checks`:

```yaml
record_schema: |
  {
    "type": "object",
    "required": ["title", "sku"],
    "x-attricat-checks": [
      { "code": "sale-below-price", "message": "The sale price must be lower than the price",
        "predicate": { "type": "compare", "attribute_code": "sale_price", "op": "lt", "other_attribute_code": "price" } }
    ]
  }
```

A failing check is rejected with `422 record_check_failed`. Checks can also look at linked records. See [Validation](/builders/validation/#compare-attributes-with-checks).

## Step 9: share attributes with a mixin

When several blueprints need the same fields, such as SEO metadata, put them in a mixin:

```yaml
format_version: 1
code: seo
name: SEO fields
kind: mixin

attributes:
  - code: meta_title
    value_type: string

  - code: meta_description
    value_type: string
```

Publish it, then include it in `product` and select the attributes you want:

```yaml
includes:
  - alias: seo
    code: seo
    version: 1

attributes:
  - code: meta_title
    from: seo.meta_title

  - code: meta_description
    from: seo.meta_description
```

An include pins an exact mixin revision. Publishing `seo` version 2 does not change `product` until you publish a new `product` revision that includes version 2.

## Step 10: control a record's lifecycle

A [status](/builders/validation/#statuses) can do more than restrict which transitions exist. For controlled documents, inspections, or assessments it can also decide who may make a transition, freeze the record once it is final, bind an approval to the exact content that was reviewed, and keep released files for a retention period:

```yaml
attributes:
  - code: status
    value_type: string
    value_schema: |-
      {
        "type": "string",
        "enum": ["draft", "review", "approved", "released"],
        "x-attricat-status": {
          "version": 1,
          "options": [
            { "code": "draft", "label": "Draft" },
            { "code": "review", "label": "In review" },
            { "code": "approved", "label": "Approved",
              "approval": { "covers": ["title", "procedure"], "void_to": "review" } },
            { "code": "released", "label": "Released", "tone": "success",
              "lock": "all", "retention_days": 3650 }
          ],
          "transitions": [
            { "from": null, "to": "draft" },
            { "from": "draft", "to": "review", "code": "submit" },
            { "from": "review", "to": "approved", "code": "approve",
              "roles": ["reviewer"], "separate_from": ["submit"] },
            { "from": "approved", "to": "released", "code": "release",
              "permission": "records.publish" },
            { "from": "released", "to": "draft", "code": "correct",
              "roles": ["owner", "admin"] }
          ]
        }
      }
```

Here:

- Whoever submitted a document for review cannot also approve it, and only `reviewer`s can approve. Releasing needs `records.publish`.
- Editing the title or procedure of an approved document voids the approval and sends it back to review.
- A released document is read-only, cannot be deleted, and keeps its files for ten years. Owners and admins correct it with the `correct` transition, then edit it.

See [Control a record's lifecycle](/builders/validation/#control-a-records-lifecycle) for how each control behaves, and [Statuses](/reference/blueprint/#statuses) for every key.

## Revisions and existing records

Every record remembers the exact blueprint revision it was created with. When you publish revision 2 of `product`, existing products stay on revision 1: their values and validation keep meaning what they meant when they were written.

Attricat then marks them as outdated and offers to migrate them. Revisions that only add optional attributes can be migrated in bulk. Changes that remove or retype attributes, or add required ones, need a decision per record. See [Revisions and migration](/builders/revisions/).

Two practices save trouble later:

- Never reuse an attribute code for a different meaning in a later revision. Add a new code instead.
- Prefer adding attributes to changing them. Additive revisions migrate without anyone's help.

## Beyond the basics

A blueprint can also carry:

- [Rules](/builders/rules/) that flag data-quality problems, such as a missing title, and can [enforce](/builders/rules/#enforce-a-rule) them on save or on status changes.
- [Status attributes](/builders/validation/#statuses) with allowed transitions and [translated labels](/builders/translations/#status-and-choice-labels).
- A [publication policy](/guides/publishing/#keep-publication-after-trusted-edits) that lets trusted roles edit without withdrawing channel approvals.
- [Connector jobs](/reference/blueprint/#connector-jobs) that import or export records through a connector extension.
- Attributes whose type comes from an [extension](/reference/blueprint/#extension-attribute-types).
- A [layout for extension panels and actions](/reference/blueprint/#extension_layout) on its record pages.
