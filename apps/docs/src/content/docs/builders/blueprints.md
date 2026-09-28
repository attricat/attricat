---
title: Blueprints
description: Define versioned schemas for the entities in your catalog.
---

A blueprint defines an entity type or a reusable mixin. It describes attributes, validation, relationships, files, and views. Each revision is immutable once published.

## Create a blueprint

Open **Manage → Blueprints** and create a new draft. Give it a stable code and name, then define its attributes in TOML.

```toml
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[[attributes]]
code = "name"
value_type = "string"
```

An entity blueprint can create entities. A mixin contributes reusable attributes to other blueprints.

## Publish intentionally

Publishing makes a blueprint revision available for new entities. Existing entities remain on the revision they were created with; this preserves the meaning and validation rules that applied at the time.

When you publish a newer revision, review migration candidates before upgrading existing entities. Do not reuse codes for an incompatible meaning.

## Publication reapproval

By default, every edit to a published entity withdraws its channel publication and requires a new approval. A blueprint revision can designate trusted workspace roles whose edits retain the entity's existing publication:

```toml
[publication]
retain_on_edit_roles = ["catalog_manager", "product_owner"]
```

These are workspace role codes. The roles must exist when you publish the blueprint revision. This setting does not grant editing or publishing permission; users still need the normal workspace permissions. It applies to value, relationship, metadata, file, and entity-upgrade changes. Context changes always withdraw channel publication because they can alter resolved output for many entities.

## Attribute behavior

Attributes can be scalar values, relationships, or files. Blueprint configuration also controls validation, defaults, contextual inheritance, and whether browser users can edit a field.

For the full attribute syntax and validation rules, see [Blueprint authoring in the repository](https://github.com/attricat/attricat/blob/main/docs/blueprints.md).
