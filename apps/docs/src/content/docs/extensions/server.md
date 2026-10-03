---
title: Server runtime
description: Run extension code on the server as a WebAssembly component, react to events, read and write the catalog, call external services, and publish events.
---

Server-side extension code is a WebAssembly **component** declared as a `server_wasm` artifact. It runs in a sandbox with no WASI context: no file system, environment variables, clock, sockets, or pre-opened files. Everything it does goes through calls to the host, and the host checks the extension's grants on every call.

## Host API versions

The host interface is defined in WIT packages in the Attricat repository under `crates/extension-runtime/`. Each published version is immutable.

| Version | WIT directory | Adds |
| --- | --- | --- |
| `catalog:host@1.0.0` | `wit/` | `api.call` and `api.log` with JSON requests. |
| `catalog:host@1.1.0` | `wit-next/` | Typed `read`, `write`, and scoped-configuration functions. |
| `catalog:host@1.2.0` | `wit-operations/` | Durable [operations](/extensions/operations/). |
| `catalog:host@1.3.0` | `wit-artifacts/` | Operation input and output artifacts. |
| `catalog:host@1.4.0` | `wit-connectors/` | Connector catalog calls and HTTPS file transfer. |
| `catalog:host@1.5.0` | `wit-interactive/` | [Interactive operations](/extensions/operations/#interactive-operations-host-api-15) over a selection. |
| `catalog:host@1.6.0` | `wit-host/` | **Unified ABI**: everything from 1.1 and 1.5 in one package. |

Set `catalog.host_api` in your manifest to the range your component is built for.

### Use the unified ABI for new extensions

Versions up to 1.5 are two separate families. 1.0 and 1.1 handle events and commands. 1.2 to 1.5 run operations. A release on one of them can't use the other family's features, so one release can't have both client commands and interactive operations.

From 1.6 there is one ABI that only grows. Use `"host_api": ">=1.6.0, <2.0.0"` and your component can use event handlers, commands, scoped configuration and every kind of operation together. Build it against one of these worlds in `wit-host/`:

| World | Exports |
| --- | --- |
| `catalog-extension` | `handler` and `operations` |
| `handler-extension` | `handler` (event handlers and commands) |
| `operation-extension` | `operations` |

Every import is always available, but some only work in the right place. The operation interfaces (`artifacts`, `catalog-data`, `catalog`, `transfer`, `selection`) return an error outside an operation run. Inside a run, the typed `read` and `write` functions and the `catalog.read.v1` and `catalog.command.v1` calls return an error; use the run's own catalog interfaces instead.

Later 1.x versions only add to 1.6. A component built for 1.6 keeps working on newer hosts without a rebuild.

Ranges that include 1.5 or earlier, such as `>=1.1.0, <2.0.0`, keep their legacy world.

## Handle catalog events

Declare a handler and subscribe to exact event types:

```json
"permissions": ["events.subscribe", "catalog.read", "catalog.write"],
"server": {
  "event_handlers": [{
    "id": "recalculate",
    "event_types": ["entity.updated.v1"],
    "handler": "handle-event"
  }]
}
```

The component's `handle-event` export receives the event: its ID, type, aggregate kind and ID, correlation and causation IDs, and a JSON payload. Entity events carry the entity ID, its blueprint and revision, and a list of facts describing each changed attribute. See the [event reference](/reference/events/).

Delivery rules:

- **At least once.** The same event can arrive twice, including after your handler finished but before Attricat recorded it. Use the event ID as an idempotency key.
- **No ordering guarantee.** Don't infer causality from arrival order.
- **Current state is yours to read.** The event says what changed; read the entity if you need its full state.
- **Failures quarantine the extension.** A trap, running out of fuel or memory, a timeout, or a returned error quarantines the installation. The delivery is retried, and becomes a dead letter after the configured number of attempts.

Before each delivery, Attricat checks again that the installation is enabled, still on the same release, and still has the grants it needs.

## Read and write the catalog

With `catalog.read`, a component can read an entity, its direct values, or its values resolved in a context. Responses include the entity's pinned blueprint revision.

With `catalog.write`, it can write scalar values in an explicit context. Writes go through the normal path: type checks, schemas, audit, and a new domain event.

A write made while handling an event is attributed to the user or token behind the original change, keeps the event's correlation ID, and is published with source `extension:<extension-id>`. **Ignore events from your own source**, or a handler that writes will trigger itself.

The JSON `catalog.read.v1` and `catalog.command.v1` calls add paged reads, change feeds, single-attribute lookups, and batches of `create`, `update`, `relationships`, and `upsert` intents. An upsert matches on a declared business key attribute, creates only when no entity matches, and fails if more than one does.

## Storage

With `storage.extension`, `storage.get.v1`, `storage.set.v1`, `storage.delete.v1`, and `storage.list.v1` give the extension its own key-value store, scoped to the workspace and release. `set` and `delete` accept an `expected_revision` for optimistic concurrency.

## Configuration

`configuration.get.v1` returns the installation configuration (needs `configuration.read`). Scoped configuration per blueprint or attribute is read and written through the typed functions in host API 1.1 (needs `configuration.write`).

## Commands for your UI

Server commands let an extension's client component ask its server component to do something. Declare them in `server.commands` with request and response schemas. The client calls `catalog.command({ command_id, payload })`. Attricat validates the payload, checks the caller's session, the contribution, the release, and the grants, then invokes your handler.

## Secrets

With `secrets.read`, `secrets.get.v1` with `{"name": "destination-token"}` returns `{"value": "…"}` for the current invocation only. Secret values never appear in configuration, logs, traces, errors, or audit records.

## Call external services

With `network.request` and a granted host permission, `network.request.v1` makes one HTTPS request:

```json
{
  "host_permission_id": "inventory-api",
  "method": "GET",
  "url": "https://api.inventory.example/v2/stock/ABC-1",
  "headers": { "accept": "application/json" },
  "secret_headers": [{ "secret": "inventory-token", "header": "authorization", "prefix": "Bearer " }]
}
```

The host:

- re-checks the release, configuration, and grants on every call;
- requires HTTPS and a URL without a query string that matches the rule;
- resolves DNS, rejects any non-public address, and connects only to the checked address;
- verifies TLS and does not follow redirects;
- enforces the rule's size limits and timeout, and 60 requests per minute per release;
- returns the status, selected safe headers, and the body as base64.

The broker never retries. A timeout or connection failure is an **uncertain outcome**: the other side may have received the request. Retry only if the destination supports an idempotency key and you send a stable one.

## Publish events for other extensions

Extensions communicate through durable events. Declare an export:

```json
"permissions": ["events.emit"],
"event_contracts": {
  "exports": [{
    "id": "inventory.changed",
    "version": "1.0.0",
    "event_type": "plugin.acme.inventory.inventory_changed.v1",
    "schema": { "type": "object", "required": ["sku"] },
    "max_payload_bytes": 4096
  }]
}
```

and publish with `events.emit.v1`:

```json
{ "contract_id": "inventory.changed", "aggregate_kind": "inventory_item",
  "aggregate_id": "<uuid>", "payload": { "sku": "ABC-1" } }
```

The host fills in the event type, source, correlation, and causation IDs. A consumer declares the contract in `event_contracts.consumes` and needs `events.subscribe`.

Administrators must grant `event_publish` for each export (`--grant-id inventory.changed`) and `event_subscribe` for each consumed contract (`--grant-id acme.inventory:inventory.changed`). A consumer can only be enabled while its provider is enabled and compatible, and a provider cannot be disabled, removed, or upgraded in a way that would break an enabled consumer.

There are no request-response calls or shared state between extensions.

## Limits

- JSON passed across the host boundary: 64 KiB per call.
- Log messages: 16 KiB.
- Command requests and responses: 64 KiB by default.
- Operation requests and checkpoints: 64 KiB.
