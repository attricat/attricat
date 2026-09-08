---
title: Contexts
description: Model values that vary by market, channel, location, or hierarchy.
---

A context is a node in your workspace's hierarchy. Use contexts when an entity needs a value that differs by market, channel, store, or another scope.

## Inheritance

Every entity has values in the default context. A non-default context can either define its own value or inherit the default value, depending on the attribute's blueprint configuration.

For example, a product can have a default description and a translated description in a market context. When no local description exists, Attricat displays the configured inherited value.

## Editing values

Select a context while viewing or editing an entity. Attributes configured for default-only editing are read-only outside the default context. Other attributes can be overridden where your permissions allow it.

Create and organize contexts under **Manage → Contexts**. Choose a clear hierarchy before adding many entity overrides: inheritance follows that hierarchy.
