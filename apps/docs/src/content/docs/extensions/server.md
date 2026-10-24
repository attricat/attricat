---
title: Server runtime
description: Run extension code on the server as a WebAssembly component, react to events, read and write the catalog, call external services, and publish events.
---

Server-side extension code is a WebAssembly **component** declared as a `server_wasm` artifact. It runs in a sandbox with no WASI context: no file system, environment variables, sockets, or pre-opened files, and no clock other than the host's `clock` interface. Everything it does goes through calls to the host, and the host checks the extension's grants on every call.

## Host API version

The host interface is the WIT package `attricat:host@1.1.0`, in `crates/extension-runtime/wit-host/attricat-extension.wit` in the Attricat repository. Set `attricat.host_api` in your manifest to a range that accepts it, such as `">=1.0.0, <2.0.0"` or `"^1.1"`. Every release with such a range can use every feature: event handlers, commands, typed reads and writes, scoped configuration, every kind of [operation](/extensions/operations/), and all client outlets. If your component uses anything added in 1.1, declare `"^1.1"`, so a host that only supports 1.0 refuses the package instead of failing at run time.

Build your component against one of these worlds:

| World | Exports |
| --- | --- |
| `attricat-extension` | `handler` and `operations` |
| `handler-extension` | `handler` (event handlers and commands) |
| `operation-extension` | `operations` |
| `outcome-extension` (1.1) | `event-handler`, `command-handler`, and `deferrable-operations` |

`outcome-extension` exports the 1.1 interfaces, which report [typed outcomes](#typed-outcomes) instead of a plain error. When a component exports a 1.1 interface, the host calls it instead of its 1.0 predecessor. A component that needs only some of the three exports returns a fault from the others.

Every import is always available, but some only work in the right place. The operation interfaces (`artifacts`, `attricat-data`, `attricat`, `transfer`, `selection`) return an error outside an operation run. Inside a run, the typed `read` and `write` functions and the `attricat.read.v1` and `attricat.command.v1` calls return an error; use the run's own catalog interfaces instead.

Later 1.x versions only add to 1.0. A component built for an earlier 1.x keeps working on newer hosts without a rebuild. To build a component, see [Build a server component](/extensions/build/#build-a-server-component).

## Calling the host

The `api` interface has typed functions (`read`, `write`, `scoped-configuration-get`, `scoped-configuration-set`, `log`) and one generic function, `call(operation, request)`. `call` takes an operation name and a JSON request string, and returns a JSON response string or an error message:

```rust
let response = api::call("storage.get.v1", r#"{"key":"last-sync"}"#)?;
```

| Operation | Needs | Described in |
| --- | --- | --- |
| `configuration.get.v1` | `configuration.read` | [Configuration](#configuration) |
| `storage.get.v1`, `storage.set.v1`, `storage.delete.v1`, `storage.list.v1` | `storage.extension` | [Storage](#storage) |
| `secrets.get.v1` | `secrets.read` | [Secrets](#secrets) |
| `attricat.read.v1` | `attricat.read` | [Read and write the catalog](#read-and-write-the-catalog) |
| `attricat.command.v1` | `attricat.write`, or `attricat.annotations.write` for annotation-only batches | [Read and write the catalog](#read-and-write-the-catalog) |
| `events.emit.v1` | `events.emit` | [Publish events](#publish-events-for-other-extensions) |
| `network.request.v1`, `network.request.v2` | `network.request` | [Call external services](#call-external-services) |
| `connections.list.v1`, `connections.get.v1` | `connections.use` | [Export connectors](/extensions/exports/#connections) |
| `mappings.*.v1` | `exports.mappings` | [Export connectors](/extensions/exports/#export-mappings) |
| `files.link.v1` | `attricat.files.public_links` | [Export connectors](/extensions/exports/#public-file-links) |
| `connector.scope.v1` | | [Export connectors](/extensions/exports/#run-scope) |

The host checks the grant on every call, so a grant revoked while your code runs takes effect at its next call. A refused call returns an error such as `missing capability 'storage.extension'`, and an unknown name returns `unknown host operation`. Requests and responses are limited to 64 KiB of JSON.

### Which handler was called

A manifest names a `handler` for each event handler, command, and operation, but a component has one export for each kind of call:

- **Commands** receive the command's `handler` as `request.handler`. Dispatch on it.
- **Operations** receive the operation's `handler` as `request.operation-id`.
- **Event handlers** receive only the event. Dispatch on `event.event-type`. If two event handlers subscribe to the same type, your component is called once for each with the same event.

## Handle catalog events

Declare a handler and subscribe to exact event types:

```json
"permissions": ["events.subscribe", "attricat.read", "attricat.write"],
"server": {
  "event_handlers": [{
    "id": "recalculate",
    "event_types": ["record.updated.v1"],
    "handler": "handle-event"
  }]
}
```

The component's `handle-event` export receives the event: its ID, type, aggregate kind and ID, correlation and causation IDs, and a JSON payload. Record events carry the record ID, its blueprint and revision, and a list of facts describing each changed attribute. See the [event reference](/reference/events/).

Delivery rules:

- **At least once.** The same event can arrive twice, including after your handler finished but before Attricat recorded it. Use the event ID as an idempotency key.
- **No ordering guarantee.** Don't infer causality from arrival order.
- **Current state is yours to read.** The event says what changed; read the record if you need its full state.
- **Failures quarantine the extension.** With the 1.0 `handler` export, a trap, running out of fuel or memory, a timeout, or a returned error quarantines the installation. The delivery is retried, and becomes a dead letter after the configured number of attempts. A 1.1 `event-handler` can report a failure without being quarantined; see [Typed outcomes](#typed-outcomes).

Before each delivery, Attricat checks again that the installation is enabled, still on the same release, and still has the grants it needs.

### Debounce

A burst of edits to one record can produce many events. Add `debounce_ms` to the handler to receive only the latest:

```json
{ "id": "push", "event_types": ["record.updated.v1"], "handler": "handle-event", "debounce_ms": 30000 }
```

A debounced event waits until its record has been quiet for `debounce_ms` (up to one hour). Events that a newer event of the same record supersedes are skipped. An `export.retry_requested.v1` event concerns one connection, so it supersedes only an earlier retry request for the same connection. `max_wait_ms` (default ten times `debounce_ms`, at most 24 hours) caps how long an event can wait while new ones keep arriving.

### Typed outcomes

A 1.1 `event-handler` exports `handle-event(event, delivery)`, which returns `result<_, failure>`. `delivery` carries the event's `sequence`, the record's `record-revision` for record content events, and the `attempt` number, starting at 1.

`failure`, from the `outcomes` interface, is one of:

| Case | What happens |
| --- | --- |
| `rejected { code, message, record-id?, connection? }` | The failure is recorded and never retried; the delivery completes. With a `record-id`, the record's [export status](/extensions/exports/#export-mappings) becomes `rejected` with the code and message. |
| `retry { message, after-ms? }` | Only this handler is retried, after `after-ms` or a backoff from 5 seconds doubling to one hour, up to 10 attempts. After that the delivery is recorded as `exhausted`, and records the handler still leases become `retrying` with error `retries_exhausted`. |
| `fault(message)` | The installation is quarantined, and records the handler leased become `retrying` with error `handler_fault`. |

Traps, running out of fuel or memory, and timeouts count as faults. Outcomes are kept per event and handler, so when one handler of an event is retried, handlers that already completed are not called again. Rejected and exhausted deliveries stay visible for 30 days on the extension's management page.

`command-handler` exports `handle-command` for [commands](#commands-for-your-ui), separately from event handling.

## Clock

`clock.now()` returns the UTC wall-clock time as seconds and nanoseconds since the Unix epoch. Use it for token expiry, poll scheduling, and timestamps a destination needs.

## Read and write the catalog

With `attricat.read`, the typed `api.read` function reads a record (`record`), its direct values (`values`), or its values resolved in a context through the fallback chain (`resolved`). Responses include the record's pinned blueprint revision, so you can map attribute IDs to codes.

With `attricat.write`, `api.write` writes scalar values in an explicit context, naming each attribute by `attribute-id` or `attribute-code`. Writes go through the normal path: type checks, schemas, audit, and a new domain event.

A write made while handling an event is attributed to the user or token behind the original change, keeps the event's correlation ID, and is published with source `extension:<extension-id>`. Your handler receives the event that write causes. The event does not carry its source, so compare before you write: if the value is already what you would write, skip the write, and the loop ends.

`attricat.read.v1` adds paged reads, change feeds, and lookups. The request names an `operation`:

```json
{ "operation": "page", "blueprint_id": "…", "blueprint_version": 3, "context_id": "…", "cursor": null, "limit": 100 }
{ "operation": "changes", "blueprint_id": "…", "blueprint_version": 3, "cursor": null, "limit": 100 }
{ "operation": "lookup", "blueprint_id": "…", "blueprint_version": 3, "attribute_id": "…", "value": "ABC-1" }
```

`page` returns `{records, next_cursor}`, with up to 100 records. Pass `next_cursor` back until it is `null`. `changes` pages through record changes of the blueprint in event order; save its cursor to resume later. `lookup` returns one record or `null`.

`attricat.command.v1` applies a batch of intents:

```json
{
  "operation": "batch",
  "batch": {
    "batch_key": "sync-2026-03-01",
    "dry_run": false,
    "intents": [
      { "kind": "update", "intent_key": "price-ABC-1", "record_id": "…",
        "values": [{ "kind": "scalar", "attribute_code": "price", "context_id": "…", "value": 19.99 }] }
    ]
  }
}
```

Intent kinds are `create`, `update`, `relationships`, `upsert`, and [`annotate`](/extensions/operations/#record-annotations). A batch holds up to 100 intents. The response lists one outcome per intent, `{intent_key, status, record_id, error}`, where `status` is `applied`, `already_applied`, `validated` (with `dry_run: true`), or `rejected`. Keep intent keys stable across retries: an intent key that was already applied is reported as `already_applied` and not written twice.

An upsert matches on a declared business key attribute (`lookup_attribute_id` and `lookup_value`), creates only when no record matches, and fails if more than one does. Its relationship sets apply whether it updates a match or creates the record.

A lookup resolves exactly like an upsert. If the attribute alone is a declared unique key on a string attribute, the lookup uses that key's normalized values across every revision of the blueprint family; otherwise it matches the exact text among records of the requested revision. A value that matches more than one record fails with `lookup matched multiple records` instead of returning one of them.

## Storage

With `storage.extension`, the extension gets its own key-value store in the workspace. Values are any JSON. Only the installed release can reach it, and entries carry over when the extension is upgraded.

| Operation | Request | Response |
| --- | --- | --- |
| `storage.get.v1` | `{key}` | `{value, revision}`, or `null` |
| `storage.set.v1` | `{key, value, expected_revision?}` | `{revision}` |
| `storage.delete.v1` | `{key, expected_revision?}` | `null` |
| `storage.list.v1` | `{prefix?, cursor?, limit?}` | `{entries: [{key, value, revision}], cursor}` |

`expected_revision` makes `set` and `delete` fail if the key changed since you read it. An installation can store up to 65,536 keys and 256 MiB, with values of at most 64 KiB.

## Configuration

`configuration.get.v1` returns the installation configuration (needs `configuration.read`). Scoped configuration per blueprint or attribute is read and written through the typed `api` functions (needs `configuration.write`).

## Commands for your UI

Server commands let an extension's client component ask its server component to do something. Declare them in `server.commands` with request and response schemas, and grant `client.commands`:

```json
"commands": [{
  "id": "recalculate",
  "handler": "recalculate",
  "request_schema": { "type": "object", "required": ["record_id"] },
  "response_schema": { "type": "object" }
}]
```

The client calls `attricat.command({ command_id: 'recalculate', payload: { record_id } })`. Attricat validates the payload against `request_schema`, checks the caller's session, the contribution, the release, and the grants, then calls `handle-command` with `handler: "recalculate"` and the payload as a JSON string. Return the response as a JSON string; it is validated against `response_schema` before the client receives it. An error you return does not quarantine the extension, but the client's promise rejects with a generic `extension command failed` message. Your error text is not passed on, so report problems the user should see, such as invalid input, inside the response.

A command runs with the extension's grants, not the user's permissions. Check in your handler that the action makes sense for the record it names.

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
- enforces the rule's size limits and timeout, and its [request budget](/extensions/manifest/#request-budgets): the rule's own `rate_limit`, or 60 requests per minute shared by the release's rules without one (counted per API process, or shared across replicas when `CACHE_BACKEND=redis`);
- returns the status, selected safe headers, and the body as base64.

The broker never retries. A timeout or connection failure is an **uncertain outcome**: the other side may have received the request. Retry only if the destination supports an idempotency key and you send a stable one. A v1 request over the budget fails with `network request rate limit exceeded`.

### network.request.v2

`network.request.v2` takes the v1 fields plus:

- a query string in `url`, if the matching rule sets `allow_query: true`, for parameters such as `?marketplaceIds=` or presigned URLs. Query values never appear in logs, traces, metrics, errors, or the stored delivery history;
- `body_base64` for a binary body, instead of `body`;
- `secret_body: {"format": "form" | "json", "fields": [{"field", "secret"}]}`, which puts workspace secrets into a form or JSON body. A JSON `field` is a top-level key or a JSON pointer. The component never sees the secret;
- `connection`, which adds the [connection's](/extensions/exports/#connections) access token to the request;
- `idempotency_key`, sent as the `Idempotency-Key` header.

```json
{
  "host_permission_id": "amazon-sp",
  "method": "GET",
  "url": "https://sellingpartnerapi-eu.amazon.com/listings/2021-08-01/items/A1/SKU-1?marketplaceIds=A1PA6795UKMFR9",
  "connection": "amazon-eu"
}
```

The result has an `outcome`:

| `outcome` | Fields | Meaning |
| --- | --- | --- |
| `response` | `status`, `headers`, `body_base64` | The destination answered, with any status. |
| `rate_limited` | `retry_after_ms` | The rule's budget is spent. Nothing was sent. For a rule without a declared budget the wait is the shared minute window. |
| `unreachable` | | The connection or TLS handshake failed. Nothing was sent, so you can send the request again. |
| `uncertain` | | Any other transport failure or a timeout. The destination may have received the request. |
| `connection_unavailable` | `message` | The connection's token could not be refreshed for a temporary reason. |

`headers` holds `content-type`, `etag`, `retry-after`, and the headers the rule lists in `response_headers`. Redirects are never followed: a 3xx response is returned as it is, and you can request its `Location` through a granted rule. A 401 response discards the connection's cached token, so the next request gets a new one.

## Export to marketplaces and storefronts

Connections, export mappings, public file links, and change-triggered export jobs are described in [Export connectors](/extensions/exports/).

## Logging

With `logging.write`, `api.log(level, message)` writes a message to the Attricat server log, tagged with your extension ID. `level` is `trace`, `debug`, `info`, `warn`, or `error`, and messages are limited to 16 KiB. Without the grant, or with another level, the call returns an error. Never log secrets, tokens, or personal data.

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
