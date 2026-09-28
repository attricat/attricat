---
title: Contexts
description: Model values that vary by market, channel, location, or hierarchy.
---

A context is a node in your workspace's hierarchy. Use contexts when an entity needs a value that differs by market, channel, store, or another scope.

## Inheritance

The workspace has a default root context; an entity can store values there or in descendant contexts. For an attribute configured to inherit, a missing local value resolves from the nearest ancestor with a value, up to the default context. An attribute with `context_fallback = "none"` has no inherited value.

For example, a product can have a default description and a translated description in a market context. A child channel context with no local description uses the market description when inheritance is enabled.

## Editing values

Select a context while viewing or editing an entity. Attributes configured for default-only editing are read-only outside the default context. Other attributes can be overridden where your permissions allow it.

Create and organize contexts under **Manage → Contexts**. Choose a clear hierarchy before adding many entity overrides: inheritance follows that hierarchy.
