---
title: Introduction
description: Start using Attricat to model, manage, and explore your catalog.
---

Attricat is a versioned catalog engine. You define the shape of your data with **blueprints**, create **entities** from those blueprints, and use **contexts** where a value varies by place, channel, or other hierarchy.

## The main ideas

- **Blueprints** are versioned definitions for entity types, attributes, and layouts.
- **Entities** are catalog records. They retain the exact blueprint revision they were created with.
- **Contexts** let an attribute inherit a default value or define an intentional local override.
- **Relationships** connect entities without losing their type or history.

## A typical workflow

1. Create and publish a blueprint, such as `product`.
2. Create entities from the published blueprint.
3. Add contexts when a value needs to differ by market, channel, or location.
4. Find and maintain records in **Entity explorer**.
5. Review changes in the activity log.

> The available actions depend on the permissions granted in your workspace.

Continue with [Explore entities](/guides/explore/) or learn how to [build a blueprint](/builders/blueprints/).
