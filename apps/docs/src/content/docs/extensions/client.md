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
| `attricat.dialog.close()` | `client.action_dialog` | Closes the dialog. Available only inside an action dialog; the host opens dialogs when the user chooses an [action](#actions). |
| `attricat.operations.start({ operation_id, input, idempotency_key })` | `client.operations.start` | Starts an [interactive operation](/extensions/operations/#interactive-operations) for the records of the action that opened the dialog and resolves to `{ run_id }`. Available only in an action dialog. |
| `attricat.operations.list()` / `get({ run_id })` / `download({ run_id, artifact_id })` | `client.operations.read` | Only runs of this extension that the signed-in user started, even when that user is an operator. The host performs downloads. |
| `attricat.operations.cancel({ run_id })` | `client.operations.cancel` | Cancels one of those runs. |

| Event on `root` | Needs | Fired |
| --- | --- | --- |
| `attricat:context-changed.v1` | `client.events` | At start and whenever the outlet's context changes. |
| `attricat:theme-changed.v1` | | Whenever the user switches between light and dark. |

The frame has no other members. In particular it has no `fetch` to the API, no access to the browser URL, and no way to call another extension. The manifest accepts some capabilities that are [reserved](/extensions/manifest/#client-interaction) for future frame APIs; granting them adds nothing yet.

Calls return promises. A call your grants don't allow, or that the host refuses, rejects with an `Error` whose message says why.

Your UI must supply its own translated text and accessible labels.

## Outlets

### Full pages

A `route` contribution is a page at `/extensions/<extension-id>/<contribution-id>`. It can be a whole application: a multi-step import wizard, a workbench with its own list and detail screens. Use an in-memory router inside the frame; the browser URL stays the same and you cannot add host routes.

Add a `navigation` contribution to link to it from the sidebar. Workspace administrators decide whether it appears in the extension group or is promoted into the main navigation.

### Record pages

| Outlet | Kind | Capability | Context |
| --- | --- | --- | --- |
| `record_preview_panel` | `embedded` | | `record_id`, optional `context_id` |
| `record_action` | `action` | `client.record_action` | The [action context](#actions), with this record |
| `record_header_action` | `action` | `client.record_header_action` | The [action context](#actions), with this record |
| `record_attribute_action` | `action` | `client.record_field_notes` | The [action context](#actions), with this record and the field's `attribute_code`. Reached only from a [field note](#field-notes) badge. |

`record_preview_panel` appears in the record's **Extensions** tab. Record actions are buttons below the fields, four before an overflow menu; header actions are items of the record's **Actions** menu. Panels show up to three contributions before overflow. Fields and files carry no frames; describe them with [field notes](#field-notes).

### Field notes

Field notes are badges and short notes Attricat shows beside a record's fields and files. Declare a server command that describes the record:

```json
"record_field_notes": { "version": 1, "source": { "command": "describe-fields" } }
```

It needs `client.record_field_notes` and `client.commands`. When a record is shown, Attricat calls the command once with `{ record_id, blueprint_id, blueprint_version, context_id }` (the context the page shows) and expects:

```json
{
  "attributes": {
    "price_gross": {
      "badge": { "label": "Computed", "tone": "info", "tooltip": "price_net * 1.23", "action": "recalculate-field" },
      "note": "price_gross = price_net * 1.23"
    }
  },
  "files": { "<file id>": { "note": "Last document: summary template." } }
}
```

| Field | Limit |
| --- | --- |
| `badge.label` | 1 to 24 characters |
| `badge.tone` | `neutral` (default), `info`, `success`, `warning`, `error` |
| `badge.tooltip` | up to 200 characters; the tooltip also names your extension |
| `badge.action` | an action of yours at `record_attribute_action`; choosing the badge runs it with the field's `attribute_code` in its context |
| `note` | up to 280 characters of plain text |

Each field's badges and note appear in one row below it, and a file's note below the file. Notes for attributes or files the record doesn't have are dropped, as is an answer that breaks these limits. An answer slower than two seconds is left out, and Attricat asks again shortly, so the first call may compile your component without holding up the page. Notes refresh whenever the record's values do.

### Explorer

| Outlet | Kind | Capability | Context |
| --- | --- | --- | --- |
| `explorer_row_action` | `action` | `client.explorer_row_action` | The [action context](#actions), with the row's record |
| `explorer_action` | `action` | `client.explorer_action` | The [action context](#actions), with no records |
| `explorer_bulk_action` | `action` | `client.explorer_bulk_action` | The [action context](#actions), with the selected records (1 to 50) |
| `explorer_table_cell` | `embedded` | `client.explorer_table_cell` | The cell value, for a column using your [cell renderer](/extensions/manifest/#cell-renderers). |

Row actions are items of a row's actions menu, bulk actions items of the selection's **Actions** menu, and toolbar actions sit behind one **Extension actions** button. Explorer contexts never include the search query, filters, or row values. A selection is a hint about what the user is looking at, not an authorization: commands still check permissions on the server.

### Actions

An action is a menu item or button that Attricat draws itself. You declare its title and what choosing it does; no frame loads until the user chooses it.

```json
{ "id": "check", "version": 1, "kind": "action", "outlet": "record_header_action",
  "title": "Check formulas", "invoke": { "command": "check-formulas" } }
{ "id": "generate", "version": 1, "kind": "action", "outlet": "explorer_bulk_action",
  "title": "Generate documents…", "invoke": { "dialog": "documents" } }
```

`invoke.command` calls one of your [server commands](/extensions/manifest/#server) and needs `client.commands`; the action can call only that command. `invoke.dialog` opens one of your `dialog` contributions and needs `client.action_dialog`. You may add several actions to the same outlet. End a title with `…` when the action opens a dialog.

The command receives, and the dialog gets as `attricat.context`, the action context:

```json
{
  "context_version": 1,
  "action_id": "generate",
  "outlet": "explorer_bulk_action",
  "blueprint_id": "…",
  "blueprint_version": 3,
  "context_id": null,
  "record_ids": ["…"]
}
```

`record_ids` lists the saved records of one blueprint revision in display order: one on a record page or row, 1 to 50 for a selection, none for a toolbar action. `context_id` is the surface's value-resolution context, or `null` for the default.

A command can tell the host what to do next. Return `{ "notification": { "message": "…", "severity": "success" }, "refresh": true }` to show a notification and reload the views of the action's records. Other fields of the response are ignored. If the command fails, the host says which action failed.

### Action dialog

| Outlet | Kind | Capability | Context |
| --- | --- | --- | --- |
| `action_dialog` | `dialog` | `client.action_dialog` | The opening action's context, captured when it opens. |

A `dialog` contribution needs a `title`, and an extension may have several. The host draws the dialog around your frame, shows how many records it applies to, and warns about unsaved edits. It stays open when the row menu closes or the selection changes, and closes on navigation. Key presses inside your frame do not reach the host, so call `attricat.dialog.close()` when the user presses Escape. Closing it before starting a run does nothing; closing it afterwards does not cancel the run.

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
| `export_mapping` | `panel` | `client.export_mapping` | `export_id`. Shown in the export status section of your extension's own management page, once per export destination your release declares. Use it to edit large destination mappings kept in extension storage, such as attributes to product-type attributes. Unlike other panels it can call your server commands (with `client.commands`), so a command can validate a mapping before saving it. Only people with `extensions.manage` can call them from this panel. |

Every other context object contains `context_version: 1`.

## Ordering and removal

Contributions to the same outlet are ordered by extension ID, then contribution ID, unless a workspace administrator or a blueprint sets a layout. Disabling, quarantining, upgrading, or revoking a grant removes contributions; open frames poll for this and are closed within 15 seconds, and their pending calls are rejected.

If a frame fails to start, the host shows a warning in its place without exposing your code or host details.
