---
title: Core concepts
description: The ideas behind Attricat's data model, explained with one running example.
---

This page follows one example, a clothing retailer selling in Poland and Germany, through each of Attricat's concepts.

## Workspace

A workspace is one catalog. It has its own members, roles, blueprints, records, contexts, extensions, and audit log. Nothing is shared between workspaces.

People sign in to a workspace with its **sign-in identifier**, such as `retailer.example`, plus their email and password.

## Blueprints

A blueprint describes one kind of record. The retailer has blueprints for `product`, `category`, `brand`, and `material`.

A blueprint is written in TOML and lists **attributes**, each with a type:

```toml
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "price"
value_type = "number"

[[attributes]]
code = "categories"
value_type = "relationship"
target_blueprint = "category"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
```

A blueprint can also define validation schemas, page layouts, data-quality rules, and connector jobs.

**Mixins** are blueprints that only exist to be included in others. The retailer keeps SEO fields in an `seo` mixin used by both `product` and `category`.

## Revisions

Blueprints change over time. Each change is a new **revision**: 1, 2, 3. A revision starts as a draft and becomes immutable once published.

When the retailer adds `care_instructions` in revision 2, the products created under revision 1 keep revision 1. They are marked outdated and can be **migrated** when someone is ready. Old values never get reinterpreted by new rules.

## Records

A record is one catalog item: one product, one category. It is created from a blueprint's current revision and stays pinned to that revision until migrated.

In blueprint TOML, the API, and the CLI, records are called entities, as in `kind = "entity"` and `entity_schema`.

Besides its attributes, every record has **system tags** and **system metadata** for automation, such as a `needs-review` tag set by a workflow.

## Contexts

A context is a place where values can differ. The retailer's contexts are:

```text
default
├── PL
│   ├── PL-web
│   └── PL-marketplace
└── DE
    └── DE-web
```

A product's `title` is written in `default` in English, overridden in `PL` in Polish and in `DE` in German. `PL-web` has no title of its own, so it **inherits** the Polish one from `PL`.

Each attribute decides whether it inherits (`context_fallback`) and whether it can be overridden outside `default` (`context_editable`). A SKU is set only in `default`; a promotional banner in `PL-web` does not leak into other channels.

## Relationships

A relationship attribute links a record to others. `product.categories` links to `category` records; `category.parent` links a category to its parent, forming a tree.

Relationships can be single-select (`cardinality = "one"`) or multi-select, and can differ by context like any other value.

Classifications such as categories, brands, and materials are records, not strings. That gives each one an identity, a translatable name, and a place in a hierarchy.

## Publication

The retailer enables `PL-web`, `PL-marketplace`, and `DE-web` as **publication channels**. A merchandiser reviews a product and **publishes** it to `PL-web`. The CSV export for `PL-web` includes only published products.

If anyone edits the product afterwards, the publication is withdrawn until it is reviewed and published again.

## Validation

Every write is validated on the server:

- the attribute type (a number must be a number);
- the attribute's `value_schema` (a price must not be negative);
- the blueprint's `entity_schema` (a product on sale must have a sale price).

Validation runs for every context the change affects. A change that would break any context is rejected as a whole.

## History and audit

Every change is recorded with who made it, when, and through what: the web app, an API token, a workflow, an extension, or an agent. Previous values are kept, 90 days by default, and can be restored.

## Automation

- **Rules** check records and report findings, such as "product has no title". They never change data.
- **Workflows** respond to events by tagging a record or writing a value.
- **Extensions** run code in a sandbox to integrate other systems, add UI, compute values, and import or export data.
- **Agents** answer questions about the catalog and propose changes, each of which waits for a person to approve it.

## Access

People get **roles** (`owner`, `admin`, `editor`, `viewer`, or custom roles). A role can be granted for the whole workspace or limited to one blueprint, one record, or one branch of the context tree. The retailer's German team is an editor on the `DE` subtree only.
