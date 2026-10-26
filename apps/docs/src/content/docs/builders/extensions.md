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

1. **Read the requested permissions.** Each one names a capability, such as `attricat.write` (change records), `events.subscribe` (react to changes), `network.request` (call external services), or `client.record_action` (add a button to record pages). The [manifest reference](/extensions/manifest/#capabilities) lists them all.
2. **Check network access.** `network.request` only allows calls to the URL patterns listed as host permissions. Each pattern shows its hosts, methods, size limits, and timeout, and whether it may send query strings.
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

## Connections

Extensions that export to marketplaces or storefronts reach the destination through **connections**: one per account, such as a Shopify shop or an Amazon seller account in one region. An extension can have several connections of each kind it declares. Manage them in the **Connections** section of the extension's page under **Manage → Extensions**.

Attricat stores client secrets and tokens encrypted, obtains and refreshes access tokens itself, and adds them only to requests to the destination's API. The extension never sees them, and nobody can read them back. The deployment needs the [`extension_credential_key`](/reference/configuration/#secrets) secret before connections can be created.

To add one, choose **Add connection** and fill in:

- **Kind**, from the kinds the extension declares;
- **Name**, a short identifier that [connector jobs](/reference/blueprint/#connector-jobs) use with `connection = "…"`, and a **Display name**;
- **Client ID**, and the **Client secret** if the destination requires one;
- **Authorization**, the OAuth grant the destination uses:
  - **Client credentials** needs the client secret. The connection works at once.
  - **Refresh token** needs a refresh token you obtained from the destination.
  - **Device code** needs the destination's device authorization URL. After creating the connection, choose **Authorize**, enter the code shown at the destination's page and approve, then choose **Check authorization**.
  - **Authorization code**: enter the destination's **Authorization URL**, and register the redirect URI the connection shows with your app at the destination. Then choose **Authorize in browser**, approve the app at the destination, and you are sent back to the extension's page with the connection active. Complete it within ten minutes, in the same browser where you are signed in to Attricat: the authorization only finishes for the session that started it. If the deployment has no [`EXTENSION_OAUTH_CALLBACK_URL`](/reference/configuration/#extensions), copy the code from the destination's redirect instead, enter it with the redirect URI, and choose **Connect**.
- **Token URL** and any **Scope** the destination requires. The token and device authorization URLs must match a host permission the extension declares for this kind.
- **Parameters**: a JSON object of non-secret settings the extension reads, such as a shop domain or marketplace ID.

**Test** gets an access token to check the connection. To rotate a credential, enter a **New client secret** or **New refresh token** and choose **Replace**. A connection whose grant the destination rejects is marked **Failed** until you fix it; temporary failures do not change its status. **Delete** removes the connection with its stored credentials and its export status.

## Request budgets

A host permission can declare how many requests the extension may send, such as five per second. The **Request budgets** section lists each host permission's declared budget. **Lower** sets a smaller one, for example to leave room for other systems that use the same account. You cannot raise a budget above what the extension declares. **Reset** returns to the declared budget. Host permissions without a declared budget share 60 requests per minute.

## Export status

For an extension that declares export destinations, the **Export status** section counts records per connection and status. Select a status to list its records with the destination's error, select records, and **Retry** them, or retry all records in that status. A retry marks the records as pending and asks the extension to export them again. The section also lists event deliveries the extension rejected or gave up on in the last 30 days, and any mapping editor the extension provides for each destination.

Each record's status is also on its [Exports tab](/guides/records/#exports), and the Data health page counts problems across extensions.

## When something goes wrong

- **Disable** an extension to stop it without losing its configuration and grants.
- **Quarantine** marks an installation as unsafe. Attricat quarantines an extension automatically when its server code crashes or runs out of memory or time. Older extensions are also quarantined when they return an error while handling an event; extensions built for host API 1.1 report a rejected record or a temporary failure instead, which is retried or shown in the export status. Re-enabling it checks the release and configuration again.
- **Remove** uninstalls it. Its history stays in the audit log.
- The workspace **extensions mode** switch turns off every extension in the workspace at once, without changing any installation. Turn it back on to restore the ones that were enabled.

Deployment operators have two more controls: `EXTENSIONS_MODE=disabled` stops extensions across the whole deployment, and `EXTENSION_DENYLIST` blocks specific extensions or releases. See the [configuration reference](/reference/configuration/#extensions).

Open UI frames notice these changes within 15 seconds and close.

## CLI

```sh
acli extension registry list
acli extension registry add --source acme/attricat-extensions
acli extension list
acli extension install --owner acme --repository attricat-inventory --release-id <github-release-id>
acli extension sideload --file inventory-1.2.0.tar.zst
acli extension configure acme.inventory --configuration config.json
acli extension grant acme.inventory --grant-kind capability --grant-id attricat.read
acli extension grant acme.inventory --grant-kind host_permission --grant-id inventory-api
acli extension enable acme.inventory
acli extension disable acme.inventory
acli extension workspace-mode --enabled false
```

## Build your own

See [Build an extension](/extensions/build/).
