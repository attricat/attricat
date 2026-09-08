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

## Attribute behavior

Attributes can be scalar values, relationships, or files. Blueprint configuration also controls validation, defaults, contextual inheritance, and whether browser users can edit a field.

The blueprint reference will grow with additional attribute examples and validation details as the public documentation expands.
