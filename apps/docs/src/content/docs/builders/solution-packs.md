---
title: Solution packs
description: Bootstrap a workspace from a versioned archive of blueprints, navigation, extension layout, assets, and setup checks.
---

A solution pack is a `.tar.zst` archive that sets up a workspace for a use case, such as an e-commerce catalog. It can contain blueprints, Explore navigation shortcuts, extension layout defaults, logos and illustrations, setup guidance, checks, and optional sample data.

Applying a pack is a one-time setup step. Afterwards, everything it created is ordinary workspace data that administrators edit as usual. The pack does not own those resources, keep them in sync, or remove them later.

## What a pack can and cannot do

A pack can:

- create new blueprints (as drafts or published), or reuse matching published blueprints you point it at;
- add entries to Explore navigation and to the workspace extension layout;
- create logos, icons, and illustrations;
- check whether required extensions are installed and configured;
- ship a README, release notes, a setup checklist, and informational checks;
- create synthetic sample entities, if you opt in.

A pack cannot:

- install, configure, grant permissions to, or enable an extension;
- create or change contexts or publication channels;
- update an existing blueprint, or overwrite anything already in the workspace;
- change members, roles, or grants;
- run scripts, SQL, or anything else executable;
- contain secrets.

There is no uninstall. To undo a pack, remove the resources it created one by one.

## Apply a pack

Pack administration uses the CLI and needs `solution_packs.manage`, which the owner and admin roles have by default. You obtain the archive from its publisher; Attricat never downloads packs itself.

### 1. Inspect

```sh
acli solution-pack inspect --file ecommerce-1.2.0.tar.zst
```

The server validates the archive and returns its ID, version, digest, and a summary of what it contains. Nothing is saved.

### 2. Plan

```sh
acli solution-pack plan --file ecommerce-1.2.0.tar.zst \
  --prefix ecom --blueprint-publication publish
```

A plan is a dry run saved on the server. It lists every action the pack would take and whether it can.

- `--prefix` goes in front of new blueprint codes, so the pack's `product` becomes `ecom_product`. It must be 1 to 32 characters: lowercase letters, digits, and underscores, starting with a letter and not ending in an underscore.
- `--blueprint-publication` is `draft` or `publish`. Choose `draft` to review blueprints before anyone can create entities.
- `--include-sample-data` adds the pack's synthetic sample entities. Leave it out unless you want them.

Each action in the plan is one of:

| Action | Meaning |
| --- | --- |
| `create` | Create a new blueprint or asset. |
| `map` | Reuse an existing blueprint or asset you selected. |
| `append` | Add a navigation or extension-layout entry. |
| `satisfied` | The entry already exists exactly; nothing to do. |
| `skip` | An optional item cannot be applied and will be left out. |
| `conflict` | Something in the workspace is in the way, such as a blueprint with the same code. |
| `blocked` | A required dependency is missing, such as a required extension. |

A plan with conflicts or blocked actions cannot be applied. Plans expire after 24 hours.

### 3. Resolve conflicts

If a blueprint code is taken, either choose a different `--prefix`, or tell the planner to reuse an existing published blueprint whose definition matches exactly:

```sh
acli solution-pack plan --file ecommerce-1.2.0.tar.zst --prefix ecom \
  --blueprint-publication publish \
  --map blueprints/product=shared_product \
  --map-asset assets/brand-logo=<existing-asset-uuid>
```

If a required extension is missing, install and configure it through the normal [extension workflow](/builders/extensions/), then plan again.

### 4. Apply

```sh
acli solution-pack apply <plan-id>
```

Apply takes only the plan ID; nothing can change between planning and applying. Before each step, Attricat checks that the workspace still matches the plan. If something changed, the application stops as stale and you plan again. An interrupted application can be run again and continues where it stopped.

### 5. Review

```sh
acli solution-pack applications list
acli solution-pack applications show <application-id>
acli solution-pack checks list <application-id>
acli solution-pack checks rerun <application-id>
```

The application record keeps what was created or reused, for audit. Checks report setup status, such as "product blueprint is published" or "extension X is enabled". They are informational: a failing check never blocks or undoes anything.

## Upgrade to a newer pack release

To apply a newer release of a pack you applied before, name the earlier application:

```sh
acli solution-pack plan --file ecommerce-1.3.0.tar.zst --prefix ecom \
  --blueprint-publication publish --from-application <application-id>
```

Blueprints that did not change are reused. New blueprints are created. Blueprints the new release changed are blocked with `update_not_supported`, because a pack never updates an existing blueprint. Update those yourself with a new revision. Blueprints the new release removed are reported and left alone.

## Sample data

When you plan with `--include-sample-data`, the pack's synthetic entities are created in the default context and marked as samples so they are easy to find and remove. Creating them runs the same audit and automation as any other new entity, including workflows that listen to `entity.created.v1`.

## Build a pack

A pack is a `.tar.zst` archive with `solution-pack.json` at its root. The manifest lists every file with its SHA-256 digest:

```json
{
  "manifest_version": 1,
  "id": "acme.ecommerce",
  "name": "Ecommerce Catalog",
  "version": "1.2.0",
  "description": "Product and category blueprints.",
  "catalog": { "host_api": ">=1.0.0 <2.0.0" },
  "documentation": {
    "readme": { "path": "README.md", "sha256": "…" },
    "setup_checklist": { "path": "setup/checklist.json", "sha256": "…" }
  },
  "checks": { "path": "checks/checks.json", "sha256": "…" },
  "resources": {
    "blueprints": [
      { "key": "blueprints/product", "path": "blueprints/product.toml", "required": true, "sha256": "…" }
    ],
    "workspace_settings": [
      { "key": "workspace/explore-navigation", "path": "workspace/explore-navigation.json", "required": false, "sha256": "…" }
    ],
    "presentation_assets": [
      { "key": "assets/brand-logo", "path": "assets/brand-logo.svg", "required": true,
        "purpose": "logo", "media_type": "image/svg+xml", "sha256": "…" }
    ]
  },
  "extensions": [
    { "key": "extensions/shopify", "id": "acme.shopify", "version": ">=2.1.0 <3.0.0", "required": false }
  ]
}
```

JSON schemas for the manifest, checks, setup checklist, Explore navigation, extension layout, and sample data files are published in the repository under `contracts/solution-pack-*-v1.schema.json`. Unknown fields are rejected.

Guidelines for pack authors:

- **Use logical keys, not codes or UUIDs.** Pack blueprints refer to each other by key; the planner turns keys into real codes using the administrator's prefix.
- **Keys are permanent.** Renaming a key in a later release looks like one resource removed and another added.
- **Keep packs declarative.** Pack blueprints cannot use `[publication]` role policies or extension cell renderers.
- **No secrets.** Extension configuration templates are public. Keys named like `password`, `secret`, `token`, `api_key`, `private_key`, `credential`, or `authorization` are rejected.
- **Assets**: PNG, WebP, and SVG for any purpose; JPEG for illustrations only. Up to 2 MiB each, 16 MiB total, and 4096×4096 pixels. SVG is limited to 256 KiB and a safe subset with no scripts, styles, fonts, animation, or external references.
- **Documentation**: README up to 64 KiB, release notes up to 32 KiB. Markdown is shown with HTML disabled; only links within the same document are allowed.
- **Checks** use one of `blueprint_published`, `extension_installed`, `extension_enabled`, `extension_configuration_matches`, `explore_navigation_entry_present`, or `workspace_extension_layout_placement_present`.

```json
{
  "format_version": 1,
  "checks": [{
    "key": "checks/product-published",
    "title": "Product is published",
    "predicate": { "type": "blueprint_published", "blueprint": "blueprints/product" }
  }]
}
```

An Explore navigation file pins blueprints and optionally limits them to role codes:

```json
{
  "format_version": 1,
  "kind": "explore_navigation",
  "entries": [{ "blueprint": "blueprints/product", "visible_to_role_codes": ["editor"] }]
}
```
