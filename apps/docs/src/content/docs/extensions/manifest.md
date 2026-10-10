---
title: Manifest reference
description: Every field of an extension's manifest.json, including capabilities, host permissions, contributions, and declarations.
---

`manifest.json` is strict JSON. Unknown fields anywhere in it make the package invalid.

IDs used in the manifest (`attricat.id`, artifact, dependency, permission-rule, handler, and contribution IDs) may contain ASCII letters, digits, `.`, `_`, and `-`. Keep them stable across releases; layouts and grants refer to them.

## Top-level fields

| Field | Required | Description |
| --- | --- | --- |
| `manifest_version` | Yes | `1`. |
| `name` | Yes | Display name. |
| `version` | Yes | Release version, SemVer. |
| `description` | Yes | One-line description. |
| `icons` | Yes | Map of size to icon path, with at least one entry, such as `{ "48": "assets/icon-48.svg" }`. |
| `attricat.id` | Yes | Extension ID, such as `acme.inventory`. |
| `attricat.host_api` | Yes | SemVer range of supported host APIs. It must accept the current host API, `1.0.0`, for example `>=1.0.0, <2.0.0`. |
| `artifacts` | Yes | At least one artifact. |
| `permissions` | | Capabilities that must be granted before the extension can be enabled. |
| `optional_permissions` | | Capabilities the administrator may grant. |
| `host_permissions` | | Outbound network rules that must be granted. |
| `optional_host_permissions` | | Outbound network rules the administrator may grant. |
| `configuration` | | Installation settings schema. |
| `scoped_configuration` | | Settings stored per blueprint or attribute. |
| `dependencies` | | Other extensions this one needs. |
| `event_contracts` | | Events this extension publishes or consumes. |
| `server` | | Event handlers, commands, operations, and webhooks. |
| `ui` | | Client contributions. |
| `cell_renderers` | | Explorer table cell renderers. |
| `attribute_types` | | Attribute types for blueprints. |

## Artifacts

```json
"artifacts": [
  { "id": "server", "kind": "server_wasm", "path": "dist/server.wasm" },
  { "id": "panel", "kind": "client_component", "path": "dist/panel.js" }
]
```

`kind` is `server_wasm` (a WebAssembly component) or `client_component` (a JavaScript module). Paths are relative to the archive root. Only declared artifacts are extracted from the package.

## Capabilities

List capabilities in `permissions` or `optional_permissions`. Each one allows a class of operation; an administrator must grant it.

### Attricat and server

| Capability | Allows |
| --- | --- |
| `attricat.read` | Reading records, values, and blueprints. |
| `attricat.write` | Writing values and running create, update, relationship, and upsert commands. |
| `events.subscribe` | Receiving catalog events and consumed extension events. |
| `events.emit` | Publishing this extension's declared events. |
| `storage.extension` | Reading and writing the extension's own key-value storage. |
| `configuration.read` | Reading the installation configuration. |
| `configuration.write` | Reading and writing scoped configuration. |
| `secrets.read` | Reading named workspace secrets at run time. |
| `logging.write` | Writing log messages. |
| `artifacts.read`, `artifacts.write` | Reading operation inputs and writing operation outputs. |
| `attricat.annotations.write` | Writing this extension's own namespace of record tags and metadata. See [Record annotations](/extensions/operations/#record-annotations). |
| `network.request` | Making outbound HTTPS requests that match a granted host permission. |
| `webhooks.receive` | Declaring inbound webhooks (not delivered yet). |

### Client interaction

| Capability | Allows |
| --- | --- |
| `client.commands` | Calling the extension's server commands from its UI. |
| `client.navigation` | Navigating the user to a record. |
| `client.notification` | Showing a notification. |
| `client.refresh` | Refreshing the current record after a change. |
| `client.events` | Receiving context-change events in the frame. |
| `client.confirmation` | Asking the user to confirm an action. |
| `client.download` | Offering a file download. |
| `client.external_navigation` | Opening allowed HTTPS URLs in a new tab. |
| `client.files.read`, `client.files.upload` | Reading and uploading files in a file context. |
| `client.search` | Running a catalog search. |
| `client.live_updates` | Receiving updates about the current context. |
| `client.clipboard` | Writing text to the clipboard after a user action. |
| `client.locale.read` | Reading the user's locale. |
| `client.theme.read` | Accepted for compatibility. The theme is always available. |
| `client.operations.start` | Starting the extension's interactive operations for the frame's selection. |
| `client.operations.read` | Listing the user's runs of this extension, reading them, and downloading their outputs. |
| `client.operations.cancel` | Cancelling the user's runs of this extension. |

### Client placement

Each placement capability allows a contribution at one outlet. See [Client contributions](/extensions/client/#outlets).

`client.blueprint_configuration`, `client.record_decoration`, `client.record_action`, `client.record_header_action`, `client.record_attribute_panel`, `client.explorer_row_action`, `client.explorer_table_cell`, `client.explorer_action`, `client.explorer_bulk_action`, `client.blueprint_detail_panel`, `client.blueprint_panel`, `client.blueprint_publish_check`, `client.file_panel`, `client.audit_event_panel`, `client.data_health_card`, `client.action_dialog`.

## Host permissions

A host permission is a rule for outbound requests. `network.request` alone permits nothing; each request must match a granted rule.

```json
"host_permissions": [{
  "id": "inventory-api",
  "matches": ["https://api.inventory.example/v2/*"],
  "methods": ["GET", "POST"],
  "max_request_bytes": 65536,
  "max_response_bytes": 1048576,
  "timeout_ms": 10000
}]
```

| Field | Default | Description |
| --- | --- | --- |
| `id` | Required | Stable rule ID, used when granting and when making requests. |
| `matches` | Required | URL patterns. |
| `methods` | Required | Allowed HTTP methods. |
| `max_request_bytes` | 65536 | Request body limit. |
| `max_response_bytes` | 1048576 | Response body limit (1 MiB maximum). |
| `timeout_ms` | 10000 | Request timeout. |
| `max_transfer_bytes` | 0 | Enables streamed file transfers for operations, up to 1 GiB. `0` disables them. |
| `idempotent_delivery` | `false` | Allows `POST` for output delivery. Set it only when the destination honors the `Idempotency-Key` header. |

A pattern is `http` or `https`, an exact host or a leading `*.` wildcard, an optional port, and a path prefix ending in `/*`. Queries, fragments, credentials, `localhost`, and private, loopback, or link-local addresses are rejected. At request time only HTTPS is allowed, redirects are not followed, and every DNS answer must be a public address.

## Configuration

```json
"configuration": {
  "version": 1,
  "schema": {
    "type": "object",
    "properties": { "warehouse": { "type": "string" } },
    "required": ["warehouse"],
    "additionalProperties": false
  }
}
```

The administrator's configuration is validated against `schema` before the extension can be enabled. Never put secrets in configuration; use [secrets](/builders/extensions/#secrets).

`scoped_configuration` has the same shape plus `scopes`, a list of `blueprint` and/or `attribute`. It stores values per blueprint revision or attribute, and needs `configuration.write`.

## Dependencies

```json
"dependencies": [{ "id": "acme.core", "version": ">=1.0.0, <2.0.0" }]
```

The extension can only be enabled while each dependency is installed, enabled, and within the range.

## Event contracts

```json
"event_contracts": {
  "exports": [{
    "id": "inventory.changed",
    "version": "1.0.0",
    "event_type": "plugin.acme.inventory.inventory_changed.v1",
    "schema": { "type": "object", "required": ["sku"] },
    "max_payload_bytes": 4096
  }],
  "consumes": [{
    "provider": "acme.pricing",
    "contract": "price.changed",
    "version": "^1.0.0"
  }]
}
```

Exported event types must start with `plugin.<extension-id>.` and end in `.vN`. Payloads are limited to 1 to 64 KiB. See [Server runtime](/extensions/server/#publish-events-for-other-extensions).

## Server

```json
"server": {
  "event_handlers": [
    { "id": "on-update", "event_types": ["record.updated.v1"], "handler": "handle-event" }
  ],
  "commands": [
    { "id": "recalculate", "handler": "recalculate",
      "request_schema": { "type": "object" }, "response_schema": { "type": "object" } }
  ],
  "operations": [
    { "id": "export", "handler": "export", "request_schema": { "type": "object" } }
  ]
}
```

| Section | Fields | Needs |
| --- | --- | --- |
| `event_handlers` | `id`, `event_types` (exact versioned types), `handler` | `events.subscribe` and a `server_wasm` artifact |
| `commands` | `id`, `handler`, `request_schema`, `response_schema`, `max_request_bytes`, `max_response_bytes` (default 64 KiB) | `client.commands` |
| `operations` | `id`, `handler`, `request_schema`, `max_request_bytes`, `max_checkpoint_bytes` (default and maximum 64 KiB), optional `interactive: {"version": 1, "max_selection": 1–50}` | `interactive` needs `client.operations.start` |
| `webhooks` | `id`, `event_type`, `handler`, `methods` (`["POST"]`), `authentication`, `max_body_bytes` | `webhooks.receive`. Declared but not delivered yet. |

## UI contributions

```json
"ui": [
  { "id": "workbench", "version": 1, "kind": "route", "artifact": "app", "title": "Formula workbench" },
  { "id": "workbench-nav", "version": 1, "kind": "navigation", "route": "workbench", "title": "Formula workbench" },
  { "id": "stock", "version": 1, "kind": "embedded", "artifact": "panel", "outlet": "record_preview_panel" },
  { "id": "recalc", "version": 1, "kind": "action", "artifact": "row", "outlet": "explorer_row_action" }
]
```

| `kind` | Fields | Description |
| --- | --- | --- |
| `route` | `artifact`, `title` | A full page at `/extensions/<extension-id>/<contribution-id>`. |
| `navigation` | `route`, `title` | A sidebar link to one of this extension's routes. |
| `embedded` | `artifact`, `outlet` | A frame at `navigation`, `record_preview_panel`, `blueprint_attribute_configuration`, `record_attribute_decoration`, `record_action`, or `explorer_table_cell`. |
| `action` | `artifact`, `outlet` | A host-laid-out action at `explorer_row_action`, `explorer_action`, `explorer_bulk_action`, or `record_header_action`. |
| `panel` | `artifact`, `outlet` | A read-only host-laid-out panel at `blueprint_detail_panel`, `blueprint_panel`, `blueprint_publish_check`, `record_attribute_panel`, `file_panel`, `audit_event_panel`, or `data_health_card`. |
| `dialog` | `artifact`, `outlet`, `title` | The host-managed `action_dialog` opened by this extension's selection actions. |

Each extension can use each outlet once. `record_action`, `explorer_row_action`, and `explorer_bulk_action` accept `version` 1 or 2; version 2 receives the [selection context](/extensions/client/#selection-context).

## Cell renderers

```json
"cell_renderers": [
  { "id": "example.currency", "version": 1, "value_types": ["number", "integer"], "allowed_props": ["currency"] }
]
```

A cell renderer can be referenced from a blueprint's table view. It needs `client.explorer_table_cell` and a matching `explorer_table_cell` UI contribution.

## Attribute types

```json
"attribute_types": [{
  "id": "money",
  "version": "1.0.0",
  "primitive": "number",
  "value_schema": { "type": "number", "minimum": 0 },
  "configuration_schema": {
    "type": "object",
    "properties": { "currency": { "type": "string", "pattern": "^[A-Z]{3}$" } },
    "required": ["currency"]
  }
}]
```

`primitive` is `string`, `number`, `integer`, `boolean`, `date`, `datetime`, `time`, or `json`. Blueprints reference the type as `extension_type = "acme.commerce:money@^1"`. Validation uses only the declared schemas; no extension code runs to validate a value. See [Extension attribute types](/reference/blueprint/#extension-attribute-types).
