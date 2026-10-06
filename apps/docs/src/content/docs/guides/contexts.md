---
title: Contexts
description: Store values that differ by market, language, channel, or location, and control how they inherit.
---

A context is a place in which an entity's values can differ. Markets, languages, sales channels, and stores are typical contexts. Contexts form a tree under a root context called `default`.

## How inheritance works

Every entity can have values in any context. When a context has no value for an attribute, Attricat looks at its parent, then the parent's parent, up to `default`, and shows the first value it finds.

```text
default            description = "Linen shirt"
└── PL             description = "Lniana koszula"
    ├── PL-web     (no value → shows "Lniana koszula")
    └── PL-shop    description = "Koszula z lnu, krój regularny"
```

Each attribute can change this:

- `context_fallback = "none"` turns inheritance off. A context without its own value shows nothing. Use it for values that must not spread to children, such as a promotion for one channel.
- `context_editable = "default"` allows the value to be written only in `default`. Other contexts show it read-only. Use it for global facts, such as a SKU.

Relationship values inherit in the same way.

## Create contexts

Open **Manage → Contexts** and choose **Create context**.

- **Code**: letters, numbers, hyphens, and underscores. The code is how people and integrations refer to the context.
- **Parent context**: where it sits in the tree. Leave it at the root to make a top-level context.
- **Metadata**: an optional JSON object describing the context, for example `{"language": "pl"}`.

With the CLI:

```sh
acli context create --code PL --data '{"language":"pl"}'
acli context create --code PL-web --parent-id <PL-context-id>
```

The `default` context always has the ID `00000000-0000-4000-8000-000000000001`.

## Change the tree

You can move a context to a different parent. Before the move is saved, Attricat checks every entity against its new inheritance chain. If any entity would become invalid, for example by losing a required value, the move is rejected.

A context that is in use cannot be deleted.

## Work in a context

- In the **Explorer**, the **Context** selector shows, filters, and sorts values as they resolve in that context.
- On an entity, the **Context** selector switches both the preview and the edit form.
- In relationship facets, **Tree options** chooses the context used to resolve links.

## Plan the tree first

Inheritance follows the tree, so its shape determines how much you have to type. Put the dimension that shares the most values near the root. A common layout is market, then channel:

```text
default
├── PL
│   ├── PL-web
│   └── PL-marketplace
└── DE
    └── DE-web
```

Contexts are not languages by default. If yours represent locales, record that in their metadata and agree on it as a team. See [Model your catalog](/builders/modeling/#contexts).

## Contexts as publication channels

Any context can be enabled as an export channel under **Manage → Exports**. Entities are then published to it separately. See [Publishing](/guides/publishing/).

## Contexts from solution packs

A [solution pack](/builders/solution-packs/#contexts-and-publication-channels) can create the contexts and publication channels it needs when you apply it, with codes that start with the pack's prefix. When you plan the pack, you can instead choose an existing context with `--map-context`; the pack then uses it as it is and never changes its data, parent, or channel. Contexts created by a pack are ordinary contexts that you can change, move, or delete like any other.

## Permissions

`contexts.read` is needed to see contexts, and `contexts.write` to create, change, or delete them and to enable publication channels. A role grant can be limited to a context subtree: it then applies to that context and everything below it, but not to its parent or siblings.
