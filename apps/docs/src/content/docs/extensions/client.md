---
title: Client contributions
description: Add pages, panels, actions, and table cells to the web app from sandboxed frames, and use the catalog client API.
---

A client contribution is a JavaScript module that Attricat loads into a sandboxed frame at a fixed place in the web app. The host owns everything around the frame: layout, loading and error states, focus, and accessibility landmarks. Your module owns what is inside it.

## The sandbox

Every contribution gets its own `<iframe sandbox="allow-scripts">` with an opaque origin and a Content Security Policy that blocks network access. The frame has no access to Attricat's page, cookies, storage, or other extensions' frames. It cannot call `fetch` against the API.

Everything goes through the `catalog` object the host passes to your module. Each call is checked against your grants and the contribution's context.

## Module contract

A client artifact must export `mount(root, catalog)`. It may return a cleanup function, synchronous or async, that runs when the frame is removed.

```js
export const mount = (root, catalog) => {
  const render = () => {
    root.dataset.mode = catalog.theme.color_mode;
    root.textContent = `Entity: ${catalog.context.entity_id}`;
  };
  root.addEventListener('catalog:context-changed.v1', render);
  root.addEventListener('catalog:theme-changed.v1', render);
  render();
  return () => root.replaceChildren();
};
```

The frame stays mounted when the user switches context or theme. Listen for the change events and update, and throw away any work that belonged to the previous context.

## The `catalog` API

| Member | Needs | Description |
| --- | --- | --- |
| `catalog.context` | | Identifiers for the outlet, such as `entity_id` and `context_id`. Only the fields documented for the outlet are present. |
| `catalog.theme` | | `{ color_mode: 'light' \| 'dark' }`. The frame's `color-scheme` is set to match before `mount`, so system colors like `Canvas` and `CanvasText` follow it. |
| `catalog.configuration` | `configuration.read` | The installation configuration. |
| `catalog.request(path)` | `catalog.read` | `GET` one of `/api/entities`, `/api/v1/entities/<uuid>`, or `/api/blueprints/<uuid>/versions/<n>`. Responses are limited to 1 MiB. Revision reads in `blueprint_attribute_configuration` are limited to that outlet's blueprint revision. |
| `catalog.command({ command_id, payload })` | `client.commands` | Calls one of the extension's declared server commands. |
| `catalog.storage.get/set/delete/list(…)` | `storage.extension` | The extension's key-value storage. `set` and `delete` accept `expected_revision`. |
| `catalog.navigate({ entity_id })` | `client.navigation` | Opens an entity page. |
| `catalog.notify({ message, severity })` | `client.notification` | Shows a host notification. Messages are trimmed to 512 characters. |
| `catalog.refresh({ target: 'current_entity' })` | `client.refresh` | Reloads the current entity's views after your command changed it. Available in entity outlets. |

| Event on `root` | Needs | Fired |
| --- | --- | --- |
| `catalog:context-changed.v1` | `client.events` | At start and whenever the outlet's context changes. |
| `catalog:theme-changed.v1` | | Whenever the user switches between light and dark. |

Further mediated operations, each with its own capability, cover confirmation dialogs, downloads, opening allowed HTTPS URLs, reading and uploading files in a file context, catalog search, live updates, clipboard writes, and reading the locale. The host draws the dialogs and progress for these.

Your UI must supply its own translated text and accessible labels.

## Outlets

### Full pages

A `route` contribution is a page at `/extensions/<extension-id>/<contribution-id>`. It can be a whole application: a multi-step import wizard, a workbench with its own list and detail screens. Use an in-memory router inside the frame; the browser URL stays the same and you cannot add host routes.

Add a `navigation` contribution to link to it from the sidebar. Workspace administrators decide whether it appears in the extension group or is promoted into the main navigation.

### Entity pages

| Outlet | Kind | Capability | Context |
| --- | --- | --- | --- |
| `entity_preview_panel` | `embedded` | | `entity_id`, optional `context_id` |
| `entity_action` | `embedded` | `client.entity_action` | entity, attribute, context |
| `entity_attribute_decoration` | `embedded` | `client.entity_decoration` | entity, attribute, blueprint and revision, optional context |
| `entity_header_action` | `action` | `client.entity_header_action` | `entity_id`, `blueprint_id`, `blueprint_version` |
| `entity_attribute_panel` | `panel` | `client.entity_attribute_panel` | `entity_id`, `attribute_id`, `blueprint_id`, `blueprint_version`, `context_id` |
| `file_panel` | `panel` | `client.file_panel` | `file_id`, `entity_id`, `attribute_id`, `blueprint_id`, `blueprint_version` |

`entity_preview_panel` appears in the entity's extension drawer. Action bars show one primary and three secondary actions before an overflow menu. Panels show up to three contributions before overflow.

### Explorer

| Outlet | Kind | Capability | Context |
| --- | --- | --- | --- |
| `explorer_row_action` | `action` | `client.explorer_row_action` | `entity_id`, `blueprint_id`, `blueprint_version` |
| `explorer_action` | `action` | `client.explorer_action` | `blueprint_id`, `blueprint_version` |
| `explorer_bulk_action` | `action` | `client.explorer_bulk_action` | `blueprint_id`, `blueprint_version`, selected `entity_ids` (1 to 50) |
| `explorer_table_cell` | `embedded` | `client.explorer_table_cell` | The cell value, for a column using your [cell renderer](/extensions/manifest/#cell-renderers). |

Explorer contexts never include the search query, filters, or row values. A selection is a hint about what the user is looking at, not an authorization: commands still check permissions on the server.

### Blueprints

| Outlet | Kind | Capability | Context |
| --- | --- | --- | --- |
| `blueprint_attribute_configuration` | `embedded` | `client.blueprint_configuration` | blueprint, revision, attribute |
| `blueprint_detail_panel` | `panel` | `client.blueprint_detail_panel` | `blueprint_id`, `blueprint_version` |
| `blueprint_panel` | `panel` | `client.blueprint_panel` | `blueprint_id`, `blueprint_version`. Stays visible across detail tabs. |
| `blueprint_publish_check` | `panel` | `client.blueprint_publish_check` | `blueprint_id`, `blueprint_version`. Shown in the publish dialog. Informational only; it cannot block publishing. |

### Elsewhere

| Outlet | Kind | Capability | Context |
| --- | --- | --- | --- |
| `navigation` | `embedded` | | A compact frame in the sidebar. |
| `audit_event_panel` | `panel` | `client.audit_event_panel` | `event_id` of an opened audit event. |
| `data_health_card` | `panel` | `client.data_health_card` | None. A card on the Data health page. |

Every context object also contains `context_version: 1`.

## Ordering and removal

Contributions to the same outlet are ordered by extension ID, then contribution ID, unless a workspace administrator or a blueprint sets a layout. Disabling, quarantining, upgrading, or revoking a grant removes contributions; open frames poll for this and are closed within 15 seconds, and their pending calls are rejected.

If a frame fails to start, the host shows a warning in its place without exposing your code or host details.
