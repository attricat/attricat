---
title: Author a blueprint
description: Build a blueprint from an empty file to a published revision with relationships, views, validation, and a mixin.
---

A blueprint is a TOML document that describes one kind of catalog record: its attributes, how they behave across contexts, how they are validated, and how the web app lays them out. This guide builds a small product catalog one step at a time. Every key used here is listed in the [Blueprint TOML reference](/reference/blueprint/).

## Where to write blueprints

You can write blueprints in two places:

- **Manage → Blueprints → New blueprint** in the web app. The editor validates as you type and shows a preview of the resulting form.
- Any text editor, then upload with the CLI:

  ```sh
  acli blueprint create --file product.toml
  ```

Both paths store the TOML exactly as written. Keeping blueprint files in version control alongside your integration code works well; the CLI sends them unchanged.

You need `blueprints.write` to create drafts and `blueprints.publish` to publish them.

## Step 1: the smallest valid blueprint

```toml
format_version = 1
code = "category"
name = "Category"
kind = "entity"

[[attributes]]
code = "name"
value_type = "string"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
```

- `code` identifies the blueprint for its whole life. Pick it carefully; it cannot change in later revisions.
- `kind = "entity"` means people can create entities from it.
- Every entity blueprint needs `views.dropdown_option`. It tells Attricat how to label a category wherever one is shown in a list: relationship pickers, filter pills, search results.

Save this as a draft. A draft can be edited freely and cannot hold entities yet.

## Step 2: publish

Publishing freezes the revision. From then on it never changes, and entities can be created from it.

In the web app, open the blueprint and choose **Publish**. With the CLI:

```sh
acli blueprint publish <blueprint-id> 1
```

To change a published blueprint, create a new revision. The next section explains why that matters.

## Step 3: a product with typed attributes

```toml
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "sku"
value_type = "string"
context_editable = "default"

[[attributes]]
code = "price"
value_type = "number"
value_schema = '{"type":"number","minimum":0}'

[[attributes]]
code = "stock_on_hand"
value_type = "integer"
default_value = 0

[[attributes]]
code = "available"
value_type = "boolean"

[[attributes]]
code = "available_on"
value_type = "date"

[[attributes]]
code = "order_cutoff"
value_type = "time"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title", "sku"]
separator = " / "
```

A few choices in this file:

- `sku` has `context_editable = "default"`. A SKU is the same everywhere, so it can only be edited in the default context. In any other context it shows as read-only.
- `price` has a `value_schema` that rejects negative numbers. The schema is JSON inside a TOML string. See [Validation](/builders/validation/).
- `stock_on_hand` starts at `0` on new entities because of `default_value`.
- `order_cutoff` is a `time`: a wall-clock time plus an IANA time zone, such as 09:30 in `Europe/Warsaw`.

## Step 4: relationships

Link each product to its categories and a single brand:

```toml
[[attributes]]
code = "categories"
value_type = "relationship"
target_blueprint = "category"

[[attributes]]
code = "brand"
value_type = "relationship"
target_blueprint = "brand"
cardinality = "one"
```

`target_blueprint` makes Attricat reject a link to anything that is not a `category` (or `brand`). `categories` allows many targets; `brand` allows one per product. Many products can still share a brand. If a target must belong to exactly one source, also set `target_cardinality = "one"`.

The target blueprints must exist before you publish the product blueprint.

Relationships are how you model tags, labels, and taxonomies in Attricat. [Model your catalog](/builders/modeling/) explains why a category should be an entity and not a string.

## Step 5: context behavior

[Contexts](/guides/contexts/) let a value differ by market, language, or channel. Each attribute decides two things:

- `context_fallback`: what a context without its own value shows. The default, `"default"`, inherits from the nearest parent context that has a value. `"none"` shows nothing.
- `context_editable`: where the attribute can be written. The default, `"all"`, allows every context. `"default"` allows only the root.

```toml
[[attributes]]
code = "description"
value_type = "string"
# Inherit from the parent market when a channel has no description.
context_fallback = "default"

[[attributes]]
code = "promo_banner"
value_type = "string"
# A banner shown in one channel must not leak into its children.
context_fallback = "none"
```

## Step 6: files

```toml
[[attributes]]
code = "main_photo"
value_type = "file"
allowed_mime_groups = ["image"]
image_only = true
max_bytes = 10485760

[[attributes]]
code = "manuals"
value_type = "file"
cardinality = "many"
allowed_extensions = ["pdf"]
```

A file attribute holds one file by default; `cardinality = "many"` makes it an ordered list. The policy is checked on every upload: the file's content signature, MIME type, extension, and size. Images get `thumbnail` and `display` WebP variants generated in the background.

## Step 7: layout

Without views, the web app lists attributes in the order you declared them. Add views when you want tabs, grids, or a curated Explorer table.

```toml
[views.detail]
type = "stack"

[[views.detail.children]]
type = "stack"
component = { id = "catalog.entity_heading", version = 1 }

[[views.detail.children.children]]
type = "field"
field = "title"

[[views.detail.children.children]]
type = "field"
field = "sku"

[[views.detail.children]]
type = "tabs"

[[views.detail.children.tabs]]
label = "Overview"

[[views.detail.children.tabs.children]]
type = "grid"
children = [
  { type = "field", field = "price" },
  { type = "field", field = "stock_on_hand" },
]

[[views.detail.children.tabs.children]]
type = "relationship_list"
field = "categories"

[[views.detail.children.tabs]]
label = "Media"

[[views.detail.children.tabs.children]]
type = "field"
field = "main_photo"

[views.table]
type = "table"

[[views.table.columns]]
field = "main_photo"
label = "Image"
renderer = { id = "catalog.table_image", version = 1 }

[[views.table.columns]]
field = "title"

[[views.table.columns]]
field = "brand.name"
label = "Brand"

[[views.table.columns]]
field = "price"
```

The `catalog.entity_heading` stack turns its first field into the page title and the rest into a subtitle. The table column `brand.name` follows the `brand` relationship and shows the brand's `name`. Because `brand` has `cardinality = "one"`, that column can also be sorted.

[Views and layouts](/builders/views/) covers every block and component.

## Step 8: validation across fields

A `value_schema` checks one value. An `entity_schema` checks the whole entity, so it can express rules like "a product on sale needs a sale price". This example assumes the blueprint also has an `on_sale` boolean and a `sale_price` number:

```toml
entity_schema = '''
{
  "type": "object",
  "required": ["title", "sku"],
  "if": { "properties": { "on_sale": { "const": true } }, "required": ["on_sale"] },
  "then": { "required": ["sale_price"] }
}
'''
```

Put `entity_schema` with the other top-level keys, before the first `[[attributes]]`. In TOML, a key written after a table header belongs to that table.

Attricat checks the schema in every context after every change. A write that would leave any context invalid is rejected with `422 entity_schema_mismatch` and nothing is saved.

## Step 9: share attributes with a mixin

When several blueprints need the same fields, such as SEO metadata, put them in a mixin:

```toml
format_version = 1
code = "seo"
name = "SEO fields"
kind = "mixin"

[[attributes]]
code = "meta_title"
value_type = "string"

[[attributes]]
code = "meta_description"
value_type = "string"
```

Publish it, then include it in `product` and select the attributes you want:

```toml
[[includes]]
alias = "seo"
code = "seo"
version = 1

[[attributes]]
code = "meta_title"
from = "seo.meta_title"

[[attributes]]
code = "meta_description"
from = "seo.meta_description"
```

An include pins an exact mixin revision. Publishing `seo` version 2 does not change `product` until you publish a new `product` revision that includes version 2.

## Revisions and existing entities

Every entity remembers the exact blueprint revision it was created with. When you publish revision 2 of `product`, existing products stay on revision 1: their values and validation keep meaning what they meant when they were written.

Attricat then marks them as outdated and offers to migrate them. Revisions that only add optional attributes can be migrated in bulk. Changes that remove or retype attributes, or add required ones, need a decision per entity. See [Revisions and migration](/builders/revisions/).

Two practices save trouble later:

- Never reuse an attribute code for a different meaning in a later revision. Add a new code instead.
- Prefer adding attributes to changing them. Additive revisions migrate without anyone's help.

## Beyond the basics

A blueprint can also carry:

- [Rules](/builders/rules/) that flag data-quality problems, such as a missing title.
- A [publication policy](/guides/publishing/#keep-publication-after-trusted-edits) that lets trusted roles edit without withdrawing channel approvals.
- [Connector jobs](/reference/blueprint/#connector-jobs) that import or export entities through a connector extension.
- Attributes whose type comes from an [extension](/reference/blueprint/#extension-attribute-types).
- A [layout for extension panels and actions](/reference/blueprint/#extension_layout) on its entity pages.
