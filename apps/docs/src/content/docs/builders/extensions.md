---
title: Install and manage extensions
description: Find, install, review, grant, enable, upgrade, and contain extensions in a workspace.
---

Extensions add features to Attricat without changing the core product: a CSV import and export, a formula engine, a panel that shows stock levels from another system, a custom table cell. An extension can run code on the server in a WebAssembly sandbox, add UI in sandboxed frames, react to catalog events, and call external services through a controlled network API.

An enabled extension is trusted workspace software. It acts within the permissions you grant it, so grant only what it needs.

Managing extensions needs `extensions.manage`. Browsing the registry needs `extensions.read`.

## Where extensions come from

**Manage → Extensions** has three places to get an extension:

- **Marketplace** lists extensions from trusted registries. Every workspace has the official Attricat registry. Administrators can add more GitHub registries. A registry is a GitHub repository with a `registry.json` index; Attricat only resolves repositories listed in an index it trusts.
- **Upload archive** installs a `.tar.zst` package from your computer, up to 32 MiB. Use it for private extensions and for testing your own.
- The **CLI**: `acli extension install`, `acli extension sideload --file extension.tar.zst`.

Uploaded archives get the same checks as registry installs: size limits, safe file paths, a strict manifest, and a compatible host API version.

## Install, review, and enable

A new installation starts **disabled**. Before enabling it:

1. **Read the requested permissions.** Each one names a capability, such as `catalog.write` (change records), `events.subscribe` (react to changes), `network.request` (call external services), or `client.entity_action` (add a button to record pages). Permission names call records entities. The [manifest reference](/extensions/manifest/#capabilities) lists them all.
2. **Check network access.** `network.request` only allows calls to the URL patterns listed as host permissions. Each pattern shows its hosts, methods, size limits, and timeout.
3. **Configure** the extension if it has settings.
4. **Grant** the required permissions. Optional permissions can be left out; the extension must work without them.
5. **Enable** it.

Enabling fails if a required permission is not granted, the configuration is invalid, or a dependency on another extension is missing or disabled.

## Upgrades

Upgrading switches the installation to a newer release. Because a new release can ask for different permissions, an upgrade clears all grants and configuration and leaves the extension disabled. Review, configure, grant, and enable it again.

Running operations stay on the release they started with. They pause until that exact release is authorized again.

## Arrange extension UI

When several extensions contribute to the same place, such as the record action bar, they appear in a fixed order by extension ID. **Manage → Extensions → Extension layout** lets you change the order, hide contributions, and promote extension pages into the main navigation.

A blueprint can override the layout for its own record pages. See [Views and layouts](/builders/views/#extension-panels-on-record-pages).

## Secrets

Extensions that call external services often need an API key. Store it as a named workspace extension secret through the API:

```http
PUT /workspace/extension-secrets/destination-token
{"value": "…"}
```

`GET /workspace/extension-secrets` lists names only, and `DELETE` removes a secret. Values are write-only: nobody can read them back, and an extension with `secrets.read` receives a value only while it runs.

## When something goes wrong

- **Disable** an extension to stop it without losing its configuration and grants.
- **Quarantine** marks an installation as unsafe. Attricat quarantines an extension automatically when its server code crashes, runs out of memory or time, or returns an error while handling an event. Re-enabling it checks the release and configuration again.
- **Remove** uninstalls it. Its history stays in the audit log.
- The workspace **extensions mode** switch turns off every extension in the workspace at once, without changing any installation. Turn it back on to restore the ones that were enabled.

Deployment operators have two more controls: `EXTENSIONS_MODE=disabled` stops extensions across the whole deployment, and `EXTENSION_DENYLIST` blocks specific extensions or releases. See the [configuration reference](/reference/configuration/#extensions).

Open UI frames notice these changes within 15 seconds and close.

## CLI

```sh
acli extension-registry list
acli extension-registry add --source acme/catalog-extensions
acli extension list
acli extension install --owner acme --repository catalog-inventory --release-id <github-release-id>
acli extension sideload --file inventory-1.2.0.tar.zst
acli extension configure acme.inventory --configuration config.json
acli extension grant acme.inventory --grant-kind capability --grant-id catalog.read
acli extension grant acme.inventory --grant-kind host_permission --grant-id inventory-api
acli extension enable acme.inventory
acli extension disable acme.inventory
acli extension workspace-mode --enabled false
```

## Build your own

See [Build an extension](/extensions/build/).
