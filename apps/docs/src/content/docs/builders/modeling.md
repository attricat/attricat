---
title: Model your catalog
description: Decide what becomes a blueprint, an attribute, a relationship, or a context, with patterns for taxonomies, tags, hierarchies, and translations.
---

Most modeling questions in Attricat come down to four building blocks. Pick the right one early; changing it later means migrating data.

| Use | For | Example |
| --- | --- | --- |
| An **entity blueprint** | A thing with its own identity, name, or lifecycle | Product, category, brand, supplier |
| A **scalar attribute** | A fact about one entity | SKU, title, weight, launch date |
| A **relationship** | A link between entities | Product → categories, product → brand |
| A **context** | A scope in which values differ | Market, language, sales channel, store |

## Classifications are entities

Tags, labels, categories, colors, materials, and certifications should be entities linked by relationships. Do not model them as strings or lists of strings.

A classification usually needs things a string cannot give it: a stable identity when it is renamed, a translated name, a parent in a hierarchy, a description, an owner. An entity has all of these, and a relationship records which classifications apply.

```toml
format_version = 1
code = "material"
name = "Material"
kind = "entity"

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "code"
value_type = "string"
context_editable = "default"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
```

```toml
# On product
[[attributes]]
code = "materials"
value_type = "relationship"
target_blueprint = "material"
```

Some guidelines:

- **One blueprint per vocabulary.** A color is not a category just because both appear on a product. Keep them apart unless they share both meaning and lifecycle.
- **Name relationships for their meaning.** `materials` and `certifications` read better than a catch-all `tags`. Keep a generic `labels` relationship for annotations that cut across domains.
- **Keep a stable code** attribute when integrations need one. Relationships themselves always use the entity UUID.
- **Use a scalar** for values that are intrinsic and not shared: a SKU, a free-text note.

### Single-select and exclusive links

`cardinality = "one"` makes a relationship single-select. Many products can still share one brand:

```toml
[[attributes]]
code = "brand"
value_type = "relationship"
target_blueprint = "brand"
cardinality = "one"
```

Add `target_cardinality = "one"` only when each target may be claimed once, such as a product and its unique barcode record.

## Hierarchies

Give the classification blueprint a relationship to itself:

```toml
# On category
[[attributes]]
code = "parent"
value_type = "relationship"
target_blueprint = "category"
cardinality = "one"
```

The Explorer detects self-referencing relationships and turns the facet for `product.categories` into a tree with roll-up counts. Selecting *Shirts* also matches products assigned to its children. See [Explore entities](/guides/explore/#relationship-facets).

To show the path on a product page, use the [`catalog.relationship_hierarchy`](/builders/views/#hierarchies) component.

## Contexts

A context tree describes where values differ. Plan it before you add many overrides, because inheritance follows the tree.

```text
default
├── PL            (language: pl)
│   ├── PL-web
│   └── PL-marketplace
└── DE            (language: de)
    └── DE-web
```

With this tree, a product description written in `PL` is used by `PL-web` and `PL-marketplace` unless they set their own.

- **Contexts are not automatically languages.** If your tree represents locales, say so in the context `data` (for example `{"language": "pl"}`) and in your team's conventions.
- **Mix dimensions carefully.** A single tree can combine market and channel, as above, but every combination you need must exist as a node.
- **Decide per attribute.** `context_fallback = "none"` stops inheritance for values that must never leak into a child, such as a channel-specific promotion. `context_editable = "default"` locks values that are global, such as a SKU.
- **Translate classification names in place.** A material's `name` can have overrides in language contexts. Relationship pickers and table columns resolve the label in the selected context.
- **Relationships are contextual too.** A product can have a different set of categories in one market.

Contexts also act as [publication channels](/guides/publishing/) when you enable them for export.

## Reuse attributes

There are two ways to share attribute definitions.

**Mixins** share attributes between blueprints. A mixin is a blueprint with `kind = "mixin"`; other blueprints include one exact revision and select its attributes with `from`. Use it for groups of fields many blueprints need in the same shape, such as SEO or dimensions. See [Author a blueprint](/builders/blueprints/#step-9-share-attributes-with-a-mixin).

**Reusable attributes** are a workspace registry of single attribute definitions, managed under **Manage → Reusable attributes**. Editors can attach a published reusable attribute, or a group of them, to an individual entity. Use them for occasional fields that only some entities need and that do not belong in the blueprint. A definition is TOML:

```toml
code = "country_of_origin"
name = "Country of origin"
value_type = "string"
searchable = true
```

It accepts the same attribute keys as a blueprint (`value_type`, `value_schema`, `default_value`, `tags`, `context_fallback`, `context_editable`, `readonly`, and the relationship keys with `target_blueprint_code`), plus `searchable`, which includes its values in Explorer search and filters. Each change creates a new revision; publish a revision before it can be attached. The workspace supplies the namespace, so a reusable attribute's full code is `<namespace>:<code>`.

Creating and publishing reusable attributes needs `blueprints.write`. Attaching one to an entity needs `entities.write` on that entity.

## System tags and metadata

Every entity also has `system_tags` (a set of strings) and `system_metadata` (a JSON object up to 64 KiB). They sit outside the blueprint, are not versioned, and are not shown in views. They exist for automation: marking a batch for processing, recording an import source, or flagging a record for review. Workflows and rules can read and write them. Search can filter by system tags.

Use attributes for anything a person should see or edit.
