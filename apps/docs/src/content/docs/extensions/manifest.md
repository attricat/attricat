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
| `attricat.host_api` | Yes | SemVer range of supported host APIs. It must accept the current host API, `1.1.0`, for example `>=1.0.0, <2.0.0` or `^1.1`. |
| `artifacts` | Yes | At least one artifact. |
| `permissions` | | Capabilities that must be granted before the extension can be enabled. |
| `optional_permissions` | | Capabilities the administrator may grant. |
| `host_permissions` | | Outbound network rules that must be granted. |
| `optional_host_permissions` | | Outbound network rules the administrator may grant. |
| `configuration` | | Installation settings schema. |
| `scoped_configuration` | | Settings stored per blueprint or attribute. |
| `dependencies` | | Other extensions this one needs. |
| `event_contracts` | | Events this extension publishes or consumes. |
| `server` | | Event handlers, commands, operations, webhooks, connection kinds, and export destinations. |
| `ui` | | Client contributions. |
| `cell_renderers` | | Explorer table cell renderers. |
| `attribute_types` | | Attribute types for blueprints. |
| `record_field_notes` | | The source that describes records' fields, files and actions: `{ "command": "<id>" }` or a client handler, `{ "client": "<artifact>" }`. See [Field notes](/extensions/client/#field-notes). |

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
| `exports.mappings` | Reading and writing the extension's [export mappings](/extensions/exports/#export-mappings). Required to declare `server.exports`. |
| `connections.use` | Reading the extension's [connections](/extensions/exports/#connections) and sending requests with their tokens. Required to declare `server.connections`. |
| `attricat.files.read` | Copying a file a record references into an operation input with `transfer.open-record-file`. |
| `attricat.files.public_links` | Issuing [public links](/extensions/exports/#public-file-links) to files records reference. |
| `webhooks.receive` | Declaring inbound webhooks (not delivered yet). |

### Client interaction

| Capability | Allows |
| --- | --- |
| `client.commands` | Calling the extension's server commands from its UI. |
| `client.navigation` | Navigating the user to a record. |
| `client.notification` | Showing a notification. |
| `client.refresh` | Refreshing the current record after a change. |
| `client.events` | Receiving context-change events in the frame. |
| `client.theme.read` | Accepted for compatibility. The theme is always available. |
| `client.operations.start` | Starting the extension's interactive operations for the frame's selection. |
| `client.operations.read` | Listing the user's runs of this extension, reading them, and downloading their outputs. |
| `client.operations.cancel` | Cancelling the user's runs of this extension. |

The manifest also accepts `client.confirmation`, `client.download`, `client.external_navigation`, `client.files.read`, `client.files.upload`, `client.search`, `client.live_updates`, `client.clipboard`, and `client.locale.read`. These are reserved for frame APIs that don't exist yet: granting them gives a frame nothing, so don't request them.

### Client placement

Each placement capability allows a contribution at one outlet. See [Client contributions](/extensions/client/#outlets).

`client.blueprint_configuration`, `client.record_tab`, `client.record_action`, `client.record_header_action`, `client.record_field_notes`, `client.explorer_row_action`, `client.explorer_table_cell`, `client.explorer_action`, `client.explorer_bulk_action`, `client.blueprint_detail_panel`, `client.blueprint_panel`, `client.blueprint_publish_check`, `client.audit_event_panel`, `client.data_health_card`, `client.action_dialog`, `client.export_mapping`.

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
| `allow_query` | `false` | Lets `network.request.v2` and v2 transfers send query strings, such as presigned URLs, to this rule's destinations. Query values are never logged. |
| `response_headers` | `[]` | Up to 16 lowercase response header names that `network.request.v2` returns in addition to `content-type`, `etag`, and `retry-after`, such as `location` or a request ID. `set-cookie` and authentication headers are not allowed. |
| `rate_limit` | Shared budget | The rule's [request budget](#request-budgets). |

A pattern is `http` or `https`, an exact host or a leading `*.` wildcard, an optional port, and a path prefix ending in `/*`. Queries, fragments, credentials, `localhost`, and private, loopback, or link-local addresses are rejected. At request time only HTTPS is allowed, redirects are not followed, and every DNS answer must be a public address. A query string is allowed at request time only on rules with `allow_query`.

### Request budgets

```json
"rate_limit": { "requests": 5, "per_seconds": 1, "burst": 10 }
```

A budget is a token bucket: `requests` tokens are added every `per_seconds` (1 to 86,400), and up to `burst` (default `requests`) can be spent at once. `requests` and `burst` are 1 to 100,000. Every request the rule matches, including file transfers, takes a token. Declare the destination's published limit, so your extension slows down instead of being throttled.

Rules without a `rate_limit` share 60 requests per minute per release. Budgets are counted per workspace, extension, and rule, in each API process, or across replicas with `CACHE_BACKEND=redis`. An administrator can [lower a budget](/builders/extensions/#request-budgets) but never raise it; a change applies within 10 seconds.

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
| `event_handlers` | `id`, `event_types` (exact versioned types), `handler`, optional `debounce_ms` (1 to 3,600,000) and `max_wait_ms` (at least `debounce_ms`, at most 86,400,000; default ten times `debounce_ms`). See [Debounce](/extensions/server/#debounce). | `events.subscribe` and a `server_wasm` artifact |
| `commands` | `id`, `handler`, `request_schema`, `response_schema`, `max_request_bytes`, `max_response_bytes` (default 64 KiB) | `client.commands` |
| `operations` | `id`, `handler`, `request_schema`, `max_request_bytes`, `max_checkpoint_bytes` (default and maximum 64 KiB), optional `interactive: {"version": 1, "max_selection": 1–50}` | `interactive` needs `client.operations.start` |
| `webhooks` | `id`, `event_type`, `handler`, `methods` (`["POST"]`), `authentication`, `max_body_bytes` | `webhooks.receive`. Declared but not delivered yet. |
| `connections` | See [Connections](#connections). | `connections.use` |
| `exports` | See [Export destinations](#export-destinations). | `exports.mappings` |

`handler` is a name your component sees: commands receive it as `request.handler`, operations as `operation-id`. Event handlers don't receive it; see [Which handler was called](/extensions/server/#which-handler-was-called).

### Connections

A connection kind describes a type of destination account. Administrators create named connections of each kind, such as one per shop or marketplace region.

```json
"connections": [{
  "id": "amazon",
  "title": "Amazon seller account",
  "grant_types": ["refresh_token"],
  "host_permission": "amazon-lwa",
  "api_host_permissions": ["amazon-sp"],
  "token_header": { "name": "x-amz-access-token" },
  "client_authentication": "body",
  "parameters_schema": { "type": "object", "required": ["seller_id"] }
}]
```

| Field | Default | Description |
| --- | --- | --- |
| `id` | Required | Stable kind ID. |
| `title` | Required | Display name, up to 200 bytes. |
| `grant_types` | Required | OAuth grants an administrator can choose: `client_credentials`, `refresh_token`, `device_code`, `authorization_code`. No duplicates. |
| `host_permission` | Required | A declared host permission that allows `POST`. The token and device authorization endpoints must match it. |
| `api_host_permissions` | Required | Declared host permissions whose requests may carry the connection's access token. The token is never added to other requests. |
| `token_header` | `Authorization: Bearer` | `{ "name", "prefix"? }`: the header that carries the access token, with an optional prefix. |
| `client_authentication` | `body` | How the client authenticates to the token endpoint: `body` (`client_id` and `client_secret` as form fields) or `basic` (HTTP Basic). |
| `parameters_schema` | | JSON Schema object for the connection's non-secret parameters. |

### Export destinations

An export destination gives Attricat's export status, preview, retry, and resync screens a name and entry points:

```json
"exports": [{
  "id": "amazon",
  "title": "Amazon",
  "connection": "amazon",
  "preview_command": "preview",
  "resync_operation": "export"
}]
```

| Field | Description |
| --- | --- |
| `id` | Stable destination ID. |
| `title` | Display name, up to 200 bytes. |
| `connection` | Optional. A connection kind declared in `connections`. |
| `preview_command` | Optional. A declared server command that returns the payload for one record without pushing it. It receives `{export_id, record_id, connection}` and conventionally returns `{payload, issues}`. |
| `resync_operation` | Optional. The declared operation a full resync runs. |
| `reconcile_operation` | Optional. A declared operation that compares the destination with Attricat. |

## UI contributions

```json
"ui": [
  { "id": "workbench", "version": 1, "kind": "route", "artifact": "app", "title": "Formula workbench" },
  { "id": "workbench-nav", "version": 1, "kind": "navigation", "route": "workbench", "title": "Formula workbench" },
  { "id": "stock", "version": 1, "kind": "tab", "artifact": "panel", "outlet": "record_tab", "title": "Stock", "icon": "tag", "when": { "blueprints": ["product"] } },
  { "id": "recalc", "version": 1, "kind": "action", "outlet": "explorer_row_action", "title": "Recalculate…", "invoke": { "dialog": "recalc-dialog" } },
  { "id": "recalc-dialog", "version": 1, "kind": "dialog", "artifact": "dialog", "outlet": "action_dialog", "title": "Recalculate" }
]
```

| `kind` | Fields | Description |
| --- | --- | --- |
| `route` | `artifact`, `title` | A full page at `/extensions/<extension-id>/<contribution-id>`. |
| `navigation` | `route`, `title` | A sidebar link to one of this extension's routes. |
| `embedded` | `artifact`, `outlet` | A frame at `navigation`, `blueprint_attribute_configuration`, or `explorer_table_cell`. |
| `action` | `outlet`, `title`, `invoke` | A menu item or button the host draws at `record_action`, `record_header_action`, `explorer_row_action`, `explorer_action`, or `explorer_bulk_action`, or a field-note badge's action at `record_attribute_action`. `invoke` is `{ "command": "<id>" }`, `{ "dialog": "<id>" }` or `{ "operation": { "id", "input" } }`; optional `when`, `confirm`, `icon`. No artifact. See [Actions](/extensions/client/#actions). |
| `panel` | `artifact`, `outlet` | A host-laid-out panel, read-only except `export_mapping`, at `blueprint_detail_panel`, `blueprint_panel`, `blueprint_publish_check`, `audit_event_panel`, `data_health_card`, or `export_mapping`. |
| `dialog` | `artifact`, `outlet`, `title` | A host-managed `action_dialog` opened by this extension's actions. |
| `tab` | `artifact`, `outlet`, `title` | A tab on the record page at `record_tab`, whose panel is the frame; optional `icon`, and `when` limited to `blueprints`. See [Record pages](/extensions/client/#record-pages). |

Each extension can use each frame outlet once, but may add several actions and dialogs and up to two record tabs. Actions, dialogs and tabs use `version` 1. An action's `title` has at most 64 characters; a tab's, in every language, at most 24.

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

`primitive` is `string`, `number`, `integer`, `boolean`, `date`, `datetime`, `time`, or `json`. Blueprints reference the type as `extension_type: acme.commerce:money@^1`. Validation uses only the declared schemas; no extension code runs to validate a value. See [Extension attribute types](/reference/blueprint/#extension-attribute-types).
