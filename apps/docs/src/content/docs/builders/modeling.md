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
- **Keep a stable code** attribute when integrations need one, and declare it as a [unique key](/builders/validation/#unique-keys) so it cannot be reused. Relationships themselves always use the entity UUID.
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

### Links to several kinds of entity

A relationship can target any blueprint, one blueprint, or a list of them. Use a list when a link has a clear meaning but more than one kind of target, such as the subject of a compliance assessment:

```toml
[[attributes]]
code = "subject"
value_type = "relationship"
target_blueprints = ["product", "product_revision", "material", "part"]
```

Links to other blueprints are rejected, the entity picker lets editors choose which of the listed blueprints to search, and each listed blueprint can show the links with an `incoming_relationship_list`. Prefer one relationship per meaning over one unrestricted relationship.

### Business keys

Part numbers, document numbers, and accession numbers identify an entity to people and other systems. Declare them as [unique keys](/builders/validation/#unique-keys); combine attributes when the identifier is only unique within something else, such as a revision label within its document or a part number within its manufacturer.

## Hierarchies

Give the classification blueprint a relationship to itself:

```toml
# On category
[[attributes]]
code = "parent"
value_type = "relationship"
target_blueprint = "category"
tree = true
context_editable = "default"
```

`tree = true` makes the relationship single-select and rejects any link that would make a category its own ancestor (`409 relationship_cycle`). Use `acyclic = true` instead for structures where an entity may have several targets but must never loop back, such as dependencies or predecessor chains. Both require `context_editable = "default"`, so the hierarchy is the same in every context. Publishing either setting on existing data reports any cycles that already exist.

The Explorer detects self-referencing relationships and turns the facet for `product.categories` into a tree with roll-up counts. Selecting *Shirts* also matches products assigned to its children. See [Explore entities](/guides/explore/#relationship-facets).

To show the path on a product page, use the [`catalog.relationship_hierarchy`](/builders/views/#hierarchies) component. To flag a category that ends up as its own ancestor, add a rule with the [`acyclic`](/builders/rules/#predicates) predicate.

## Constraints across relationships

Some constraints span two records: a certificate's facilities must belong to the certificate's supplier, or a nonconformance cannot close while a corrective action that refers to it is still open. Express them on the record that owns the constraint, with a [`linked` or `referenced_by` check](/builders/validation/#check-linked-records).

- **One hop.** A check reads the records an entity links to, or the records that link to it, and stops there. If a constraint needs two hops, add a relationship or attribute that brings the value one hop closer.
- **Bounded.** A check reads at most 200 linked records per relationship and 1,000 referring records. Keep relationships that need checking small; checks on larger sets fail.
- **Checked on the owner's save.** Changing the linked record is not rejected. Pair the check with an event-triggered [rule](/builders/rules/#changes-to-linked-records) so affected records are reported as findings.
- **Per context.** Linked records are read in the same context as the entity, so a relationship that differs by market is checked per market.

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

## Assign responsibility

Store who is responsible for a record, such as an assignee, owner, or reviewer, in a **user or team attribute** rather than free text. Free text cannot check that the person exists or still works in the workspace, and it is not reliable for filtering. A user or team attribute is a `string` attribute with an `x-attricat-principal` annotation that lists what it accepts:

```toml
[[attributes]]
code = "reviewer"
name = "Reviewer"
value_type = "string"
value_schema = '''{
  "type": "string",
  "x-attricat-principal": { "version": 1, "kinds": ["user", "team"] }
}'''
```

- The form shows a picker of active workspace members and teams, and the record shows the chosen name. Use `"kinds": ["user"]` when the responsibility must belong to one person.
- Saving checks that a new value is an active member or an existing team. A record assigned to someone who has since left keeps the assignment and shows it as no longer active until someone changes it.
- In the Explorer, filter the attribute by a person or team, or choose **Assigned to me (or my teams)**. Saved searches with that filter work for whoever opens them.
- [Teams](/operate/workspaces/#teams) are managed under **Manage → Workspace management → Teams**. An assignment stores the team, so changing who belongs to it never touches records.
- An assignment does not grant or restrict access. Status [transition requirements](/builders/validation/#who-may-make-a-transition) check the person saving, not the assignee, and rule predicates can only check that a record is assigned or compare the value with a fixed user or team. "Only the assignee may close this" cannot be declared; use roles or [separation of duties](/builders/validation/#who-may-make-a-transition) instead.

See [User or team assignments](/reference/blueprint/#user-or-team-assignments) for the exact rules.

## System tags and metadata

Every entity also has `system_tags` (a set of strings) and `system_metadata` (a JSON object up to 64 KiB). They sit outside the blueprint, are not versioned, and are not shown in views. They exist for automation: marking a batch for processing, recording an import source, or flagging a record for review. Workflows and rules can read and write them. Search can filter by system tags.

Use attributes for anything a person should see or edit.
