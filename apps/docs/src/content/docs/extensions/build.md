---
title: Build an extension
description: The parts of an Attricat extension, how they are packaged, and how to test one against a local workspace.
---

An extension is a `.tar.zst` archive with a `manifest.json` at its root and the files the manifest declares. It can contain:

- **Server components**: WebAssembly components that handle catalog events, answer commands from the extension's UI, and run long operations such as imports and exports. See [Server runtime](/extensions/server/).
- **Client components**: JavaScript modules that render inside sandboxed frames at fixed places in the web app, or as full pages. See [Client contributions](/extensions/client/).
- **Declarations** the host acts on without running your code: attribute types, table cell renderers, configuration schemas, and event contracts.

A reference extension, `attricat-extension-example`, shows all of these working together: it computes numeric attributes from formulas, with a workbench page, an entity action, a table cell, and a server event handler.

## The security model

Extension code never runs in Attricat's own process or page.

- Server components run in a WebAssembly sandbox with no file system, environment, clock, or sockets. Everything they do goes through host calls, and each call is checked against the permissions the administrator granted.
- Client components run in `<iframe sandbox="allow-scripts">` frames with an opaque origin and a Content Security Policy that blocks network access. They talk to Attricat through a message channel that exposes only the operations they were granted.
- Catalog changes made by an extension go through the same validation, audit log, and event stream as a person's edits.

Design with that in mind: your extension asks for capabilities; an administrator decides which to grant.

## A minimal manifest

```json
{
  "manifest_version": 1,
  "name": "Inventory panel",
  "version": "1.0.0",
  "description": "Shows warehouse stock on product pages.",
  "icons": { "48": "assets/icon-48.svg" },
  "catalog": {
    "id": "acme.inventory",
    "host_api": ">=1.0.0, <2.0.0"
  },
  "permissions": ["catalog.read"],
  "artifacts": [
    { "id": "panel", "kind": "client_component", "path": "dist/panel.js" }
  ],
  "ui": [
    {
      "id": "summary",
      "version": 1,
      "kind": "embedded",
      "artifact": "panel",
      "outlet": "entity_preview_panel"
    }
  ]
}
```

```js
// dist/panel.js
export const mount = async (root, catalog) => {
  const form = await catalog.request(`/api/v1/entities/${catalog.context.entity_id}`);
  root.textContent = `${form.blueprint.blueprint.name} v${form.entity.blueprint_version}`;
  return () => root.replaceChildren();
};
```

The full manifest format is in the [manifest reference](/extensions/manifest/).

## Versions

Three version numbers are independent:

| Field | Meaning |
| --- | --- |
| `manifest_version` | The manifest format. Currently `1`. |
| `version` | Your release, as SemVer. |
| `catalog.host_api` | The SemVer range of Attricat host APIs your code works with, such as `>=1.0.0, <2.0.0`. |

Attricat never installs a release whose `host_api` range does not include the running host API.

## Package

Build your artifacts, then create a zstd-compressed tar archive whose root contains `manifest.json`, every declared artifact path, and your icons:

```text
manifest.json
assets/icon-48.svg
dist/server.wasm
dist/panel.js
```

```sh
tar -cf - manifest.json assets dist | zstd -19 -o dist/acme.inventory-1.0.0.tar.zst
```

Packages are limited to 32 MiB compressed. Paths must be relative, without `..`, links, or devices.

## Test locally

1. Run a local Attricat workspace.
2. Upload the archive under **Manage → Extensions → Upload archive**, or with `acli extension sideload --file dist/acme.inventory-1.0.0.tar.zst`.
3. Grant every permission you need and enable the extension.
4. Exercise each contribution: open the pages where your UI appears, trigger the events your handlers listen to, run your commands and operations.

Every upload is a new release, so grants and enablement must be repeated after each one.

Check that your code handles:

- **Duplicate and out-of-order events.** Delivery is at least once. Use the event ID as an idempotency key.
- **Its own events.** A write from your event handler produces a new event. Ignore events whose source is your own extension, or you will loop.
- **Missing permissions.** Optional permissions may not be granted; degrade gracefully.
- **Light and dark mode.** Read `catalog.theme` in client components.

## Publish

To distribute through a registry, publish the archive as an asset of a GitHub Release (not a draft or prerelease) in your extension's repository, and list the repository in a registry's `registry.json`:

```json
{
  "registry_version": 1,
  "extensions": [{
    "id": "acme.inventory",
    "name": "Inventory panel",
    "description": "Shows warehouse stock on product pages.",
    "icon": "icon.svg",
    "repository": "acme/catalog-inventory"
  }]
}
```

Workspace administrators add your registry with `acli extension-registry add --source acme/catalog-extensions`. The official registry is `attricat/attricat-extensions`.
