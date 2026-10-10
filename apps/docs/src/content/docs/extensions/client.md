---
title: Client contributions
description: Add pages, panels, actions, and table cells to the web app from sandboxed frames, and use the catalog client API.
---

A client contribution is a JavaScript module that Attricat loads into a sandboxed frame at a fixed place in the web app. The host owns everything around the frame: layout, loading and error states, focus, and accessibility landmarks. Your module owns what is inside it.

## The sandbox

Every contribution gets its own `<iframe sandbox="allow-scripts">` with an opaque origin and a Content Security Policy that blocks network access. The frame has no access to Attricat's page, cookies, storage, or other extensions' frames. It cannot call `fetch` against the API.

Everything goes through the `attricat` object the host passes to your module. Each call is checked against your grants and the contribution's context.

## Module contract

A client artifact must export `mount(root, attricat)`. It may return a cleanup function, synchronous or async, that runs when the frame is removed.

```js
export const mount = (root, attricat) => {
  const render = () => {
    root.dataset.mode = attricat.theme.color_mode;
    root.textContent = `Record: ${attricat.context.record_id}`;
  };
  root.addEventListener('attricat:context-changed.v1', render);
  root.addEventListener('attricat:theme-changed.v1', render);
  render();
  return () => root.replaceChildren();
};
```

The frame stays mounted when the user switches context or theme. Listen for the change events and update, and throw away any work that belonged to the previous context.

## The `attricat` API

| Member | Needs | Description |
| --- | --- | --- |
| `attricat.context` | | Identifiers for the outlet, such as `record_id` and `context_id`. Only the fields documented for the outlet are present. |
| `attricat.theme` | | `{ color_mode: 'light' \| 'dark' }`. The frame's `color-scheme` is set to match before `mount`, so system colors like `Canvas` and `CanvasText` follow it. |
| `attricat.configuration` | `configuration.read` | The installation configuration. |
| `attricat.request(path)` | `attricat.read` | `GET` one of `/api/records`, `/api/v1/records/<uuid>`, or `/api/blueprints/<uuid>/versions/<n>`. Responses are limited to 1 MiB. Revision reads in `blueprint_attribute_configuration` are limited to that outlet's blueprint revision. |
| `attricat.command({ command_id, payload })` | `client.commands` | Calls one of the extension's declared server commands. |
| `attricat.storage.get/set/delete/list(…)` | `storage.extension` | The extension's key-value storage. `set` and `delete` accept `expected_revision`. |
| `attricat.navigate({ record_id })` | `client.navigation` | Opens a record page. |
| `attricat.notify({ message, severity })` | `client.notification` | Shows a host notification. Messages are trimmed to 512 characters. |
| `attricat.refresh({ target: 'current_record' })` | `client.refresh` | Reloads the current record's views after your command changed it. Available in record page outlets. |
| `attricat.dialog.open()` / `attricat.dialog.close()` | `client.action_dialog` | Opens the extension's `action_dialog` from a version 2 selection action, with that action's selection; `close` works inside the dialog. |
| `attricat.operations.start({ operation_id, input, idempotency_key })` | `client.operations.start` | Starts an [interactive operation](/extensions/operations/#interactive-operations) for the frame's selection and resolves to `{ run_id }`. Available in version 2 selection actions and the action dialog. |
| `attricat.operations.list()` / `get({ run_id })` / `download({ run_id, artifact_id })` | `client.operations.read` | Only runs of this extension that the signed-in user started, even when that user is an operator. The host performs downloads. |
| `attricat.operations.cancel({ run_id })` | `client.operations.cancel` | Cancels one of those runs. |

| Event on `root` | Needs | Fired |
| --- | --- | --- |
| `attricat:context-changed.v1` | `client.events` | At start and whenever the outlet's context changes. |
| `attricat:theme-changed.v1` | | Whenever the user switches between light and dark. |

Further mediated operations, each with its own capability, cover confirmation dialogs, downloads, opening allowed HTTPS URLs, reading and uploading files in a file context, catalog search, live updates, clipboard writes, and reading the locale. The host draws the dialogs and progress for these.

Your UI must supply its own translated text and accessible labels.

## Outlets

### Full pages

A `route` contribution is a page at `/extensions/<extension-id>/<contribution-id>`. It can be a whole application: a multi-step import wizard, a workbench with its own list and detail screens. Use an in-memory router inside the frame; the browser URL stays the same and you cannot add host routes.

Add a `navigation` contribution to link to it from the sidebar. Workspace administrators decide whether it appears in the extension group or is promoted into the main navigation.

### Record pages

| Outlet | Kind | Capability | Context |
| --- | --- | --- | --- |
| `record_preview_panel` | `embedded` | | `record_id`, optional `context_id` |
| `record_action` | `embedded` | `client.record_action` | record, attribute, context |
| `record_attribute_decoration` | `embedded` | `client.record_decoration` | record, attribute, blueprint and revision, optional context |
| `record_header_action` | `action` | `client.record_header_action` | `record_id`, `blueprint_id`, `blueprint_version` |
| `record_attribute_panel` | `panel` | `client.record_attribute_panel` | `record_id`, `attribute_id`, `blueprint_id`, `blueprint_version`, `context_id` |
| `file_panel` | `panel` | `client.file_panel` | `file_id`, `record_id`, `attribute_id`, `blueprint_id`, `blueprint_version` |

`record_preview_panel` appears in the record's extension drawer. A version 2 `record_action` receives the [selection context](#selection-context) instead. Action bars show one primary and three secondary actions before an overflow menu. Panels show up to three contributions before overflow.

### Explorer

| Outlet | Kind | Capability | Context |
| --- | --- | --- | --- |
| `explorer_row_action` | `action` | `client.explorer_row_action` | `record_id`, `blueprint_id`, `blueprint_version` |
| `explorer_action` | `action` | `client.explorer_action` | `blueprint_id`, `blueprint_version` |
| `explorer_bulk_action` | `action` | `client.explorer_bulk_action` | `blueprint_id`, `blueprint_version`, selected `record_ids` (1 to 50) |
| `explorer_table_cell` | `embedded` | `client.explorer_table_cell` | The cell value, for a column using your [cell renderer](/extensions/manifest/#cell-renderers). |

Explorer contexts never include the search query, filters, or row values. A selection is a hint about what the user is looking at, not an authorization: commands still check permissions on the server. Version 2 row and bulk actions receive the [selection context](#selection-context).

### Selection context

Contributions to `record_action`, `explorer_row_action`, and `explorer_bulk_action` can declare `"version": 2`. They then receive one shape for every surface:

```json
{
  "context_version": 2,
  "selection_source": "record_preview",
  "blueprint_id": "…",
  "blueprint_version": 3,
  "context_id": null,
  "record_ids": ["…"]
}
```

`selection_source` is `record_preview`, `explorer_row`, or `explorer_selection`. `record_ids` lists 1 to 50 saved records of one blueprint revision in display order. `context_id` is the surface's value-resolution context, or `null` for the default. Version 1 contributions keep the contexts above.

### Action dialog

| Outlet | Kind | Capability | Context |
| --- | --- | --- | --- |
| `action_dialog` | `dialog` | `client.action_dialog` | The opening action's selection context, captured when it opens. |

A `dialog` contribution needs a `title`. Your version 2 actions open it with `attricat.dialog.open()`. The host draws the dialog around your frame, shows how many records it applies to, and warns about unsaved edits. It stays open when the row menu closes or the selection changes, and closes on navigation. Key presses inside your frame do not reach the host, so call `attricat.dialog.close()` when the user presses Escape. Closing it before starting a run does nothing; closing it afterwards does not cancel the run.

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

Every other context object contains `context_version: 1`.

## Ordering and removal

Contributions to the same outlet are ordered by extension ID, then contribution ID, unless a workspace administrator or a blueprint sets a layout. Disabling, quarantining, upgrading, or revoking a grant removes contributions; open frames poll for this and are closed within 15 seconds, and their pending calls are rejected.

If a frame fails to start, the host shows a warning in its place without exposing your code or host details.
