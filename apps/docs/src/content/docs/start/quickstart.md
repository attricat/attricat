---
title: Quickstart
description: Build a small catalog with two linked blueprints, a context, and a published product.
---

This walkthrough takes about fifteen minutes. You will create a category and a product blueprint, add a product in two languages, and publish it to a channel.

You need a workspace where you are an owner or admin. To try Attricat on your own machine, run `curl -fsSL https://docs.attricat.com/install.sh | sh` and sign in with the credentials it prints. See [Try it on your machine](/operate/deployment/#try-it-on-your-machine) for the details.

## 1. Create a category blueprint

Open **Manage → Blueprints → New blueprint** and paste:

```toml
format_version = 1
code = "category"
name = "Category"
kind = "record"

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "parent"
value_type = "relationship"
target_blueprint = "category"
cardinality = "one"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
```

Save it, then **Publish** revision 1.

`parent` points at another category, so categories can form a tree. `kind = "record"` makes this a record blueprint.

## 2. Create a product blueprint

Create a second blueprint:

```toml
format_version = 1
code = "product"
name = "Product"
kind = "record"
record_schema = '{"type":"object","required":["sku","title"]}'

[[attributes]]
code = "sku"
value_type = "string"
context_editable = "default"

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "price"
value_type = "number"
value_schema = '{"type":"number","minimum":0}'

[[attributes]]
code = "categories"
value_type = "relationship"
target_blueprint = "category"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title", "sku"]
separator = " / "

[views.table]
type = "table"
columns = [
  { field = "sku", label = "SKU" },
  { field = "title" },
  { field = "price" },
]
```

Publish it.

`record_schema` holds the record schema. It sits above the first `[[attributes]]` on purpose. In TOML, every key after a `[table]` header belongs to that table, so a top-level key placed further down would become part of the last attribute.

## 3. Add categories

Open the **Record explorer**, choose **Category**, and **Create record** twice:

1. `name` = *Apparel*
2. `name` = *Shirts*, `parent` = *Apparel*

## 4. Add a product

Choose **Product** in the Explorer and create one:

- `sku` = *SH-0001*
- `title` = *Linen shirt*
- `price` = *49*
- `categories` = *Shirts*

Try saving without a title: the record schema rejects it. Try a price of *-1*: the value schema rejects it.

## 5. Add a context

Open **Manage → Contexts → Create context**. Create `PL` under the root, with metadata `{"language": "pl"}`.

Open the product and switch **Context** to `PL`. Every field shows *Inherited from Default context*. Set `title` to *Lniana koszula* and leave the field to save it. `sku` is read-only here because it is managed in the default context.

Switch the Explorer's **Context** selector between `default` and `PL` to see the title change.

## 6. Publish to a channel

Open **Manage → Exports** and turn on **Export channel** for `PL`.

Back on the product, the **Publication** section shows `PL` as *Not published*. Choose **Publish**.

Now change the product's price and leave the field. The publication is withdrawn: it shows *Not published* again, because the approved state is no longer the current one. **Republish** to approve the change.

## 7. Try the Explorer

With a few more products and categories:

- search `categories.name:shirts`;
- open the **categories** facet and pick *Apparel* to include everything under it;
- **Add filter** for `price` less than 50;
- **Save search** to keep the result.

## Next steps

- [Author a blueprint](/builders/blueprints/) covers files, layouts, mixins, and more.
- [Model your catalog](/builders/modeling/) helps decide what should be a record, attribute, or context.
- [Workspace administration](/operate/workspaces/) explains how to invite your team.
