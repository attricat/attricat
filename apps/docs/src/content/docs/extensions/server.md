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

Every import is always available, but some only work in the right place. The operation interfaces (`artifacts`, `attricat-data`, `catalog`, `transfer`, `selection`) return an error outside an operation run. Inside a run, the typed `read` and `write` functions and the `attricat.read.v1` and `attricat.command.v1` calls return an error; use the run's own catalog interfaces instead.

Later 1.x versions only add to 1.0. A component built for an earlier 1.x keeps working on newer hosts without a rebuild.

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

A debounced event waits until its record has been quiet for `debounce_ms` (up to one hour). Events that a newer event of the same record supersedes are skipped. `max_wait_ms` (default ten times `debounce_ms`, at most 24 hours) caps how long an event can wait while new ones keep arriving.

### Typed outcomes

A 1.1 `event-handler` exports `handle-event(event, delivery)`, which returns `result<_, failure>`. `delivery` carries the event's `sequence`, the record's `record-revision` for record content events, and the `attempt` number, starting at 1.

`failure`, from the `outcomes` interface, is one of:

| Case | What happens |
| --- | --- |
| `rejected { code, message, record-id?, connection? }` | The failure is recorded and never retried; the delivery completes. With a `record-id`, the record's [export status](#export-mappings) becomes `rejected` with the code and message. |
| `retry { message, after-ms? }` | Only this handler is retried, after `after-ms` or a backoff from 5 seconds doubling to one hour, up to 10 attempts. After that the delivery is recorded as `exhausted`. |
| `fault(message)` | The installation is quarantined. |

Traps, running out of fuel or memory, and timeouts count as faults. Outcomes are kept per event and handler, so when one handler of an event is retried, handlers that already completed are not called again. Rejected and exhausted deliveries stay visible for 30 days on the extension's management page.

`command-handler` exports `handle-command` for [commands](#commands-for-your-ui), separately from event handling.

## Clock

`clock.now()` returns the UTC wall-clock time as seconds and nanoseconds since the Unix epoch. Use it for token expiry, poll scheduling, and timestamps a destination needs.

## Read and write the catalog

With `attricat.read`, a component can read a record, its direct values, or its values resolved in a context. Responses include the record's pinned blueprint revision.

With `attricat.write`, it can write scalar values in an explicit context. Writes go through the normal path: type checks, schemas, audit, and a new domain event.

A write made while handling an event is attributed to the user or token behind the original change, keeps the event's correlation ID, and is published with source `extension:<extension-id>`. **Ignore events from your own source**, or a handler that writes will trigger itself.

The JSON `attricat.read.v1` and `attricat.command.v1` calls add paged reads, change feeds, single-attribute lookups, and batches of `create`, `update`, `relationships`, and `upsert` intents. An upsert matches on a declared business key attribute, creates only when no record matches, and fails if more than one does. Its relationship sets apply whether it updates a match or creates the record.

A lookup resolves exactly like an upsert. If the attribute alone is a declared unique key on a string attribute, the lookup uses that key's normalized values across every revision of the blueprint family; otherwise it matches the exact text among records of the requested revision. A value that matches more than one record fails with `lookup matched multiple records` instead of returning one of them.

## Storage

With `storage.extension`, `storage.get.v1`, `storage.set.v1`, `storage.delete.v1`, and `storage.list.v1` give the extension its own key-value store, scoped to the workspace and release. `set` and `delete` accept an `expected_revision` for optimistic concurrency.

## Configuration

`configuration.get.v1` returns the installation configuration (needs `configuration.read`). Scoped configuration per blueprint or attribute is read and written through the typed `api` functions (needs `configuration.write`).

## Commands for your UI

Server commands let an extension's client component ask its server component to do something. Declare them in `server.commands` with request and response schemas. The client calls `attricat.command({ command_id, payload })`. Attricat validates the payload, checks the caller's session, the contribution, the release, and the grants, then invokes your handler.

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
- `connection`, which adds the [connection's](#connections) access token to the request;
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
| `rate_limited` | `retry_after_ms` | The rule's budget is spent. Nothing was sent. |
| `uncertain` | | Transport failure or timeout. The destination may have received the request. |
| `connection_unavailable` | `message` | The connection's token could not be refreshed for a temporary reason. |

`headers` holds `content-type`, `etag`, `retry-after`, and the headers the rule lists in `response_headers`. Redirects are never followed: a 3xx response is returned as it is, and you can request its `Location` through a granted rule. A 401 response discards the connection's cached token, so the next request gets a new one.

## Connections

A connection is a destination account, such as a Shopify shop or an Amazon seller account, that an operator sets up for your extension. You declare connection kinds in the [manifest](/extensions/manifest/#connections). Attricat stores the credentials encrypted, runs the OAuth grants, and refreshes tokens. Your component never sees a secret or a token.

With `connections.use`:

- `connections.list.v1` with `{}` returns `{"connections": [...]}`.
- `connections.get.v1` with `{"connection": "shop-eu"}` returns one connection.

Each connection is described by `connection` (its name), `kind`, `display_name`, `parameters` (non-secret settings, such as a shop domain or seller ID, validated against the kind's `parameters_schema`), and `status` (`pending_authorization`, `active`, `failed`, or `revoked`).

To call the destination, pass `connection` to `network.request.v2` or a [v2 transfer](/extensions/operations/#file-transfers). The host adds the access token, as `Authorization: Bearer` or the kind's `token_header`, only to requests matched by one of the kind's `api_host_permissions`.

## Export mappings

With `exports.mappings`, Attricat keeps one mapping per installation, connection, and record: the external ID, named external IDs, the content hash and record revision last pushed, a status, the last error, and timestamps. It is built for full catalogs and has an index on the external ID. A mapping is deleted with its record or connection; a soft-deleted record keeps its mapping so you can withdraw the item.

The mapping status is the record's export status that people see on the record page, in the Explorer, and on Data health. Statuses are `pending`, `synced`, `rejected`, `retrying`, `uncertain`, and `withdrawn`.

Every record has a `content_revision`: the sequence of the latest event that changed its values, relationships, publication, or lifecycle. It only grows. Annotation changes do not advance it, so your own bookkeeping never makes a record look changed. Typed reads, [connector page v2](/extensions/operations/#export-pages-and-withdrawals) rows, and `event-handler` deliveries carry it.

These `api.call` operations work in event handlers and operation runs alike. `connection` is optional everywhere; without it the mapping belongs to the extension's default destination.

| Operation | Request | Result |
| --- | --- | --- |
| `mappings.get.v1` | `{connection?, record_id}` or `{connection?, external_id}` | `{mapping}`, or `{mapping: null}` |
| `mappings.list.v1` | `{connection?, status?, after?, limit?}` (1 to 100) | `{mappings, next}` |
| `mappings.begin.v1` | `{connection?, record_id, record_revision, lease_ms?, force?}` | `{outcome: "proceed", lease_token, mapping}`, `{outcome: "stale", mapping}`, or `{outcome: "busy", lease_until}` |
| `mappings.commit.v1` | `{connection?, record_id, lease_token?, status, record_revision?, external_id?, external_ids?, content_hash?, error?}` | `{mapping}` |
| `mappings.release.v1` | `{connection?, record_id, lease_token}` | `{released}` |
| `mappings.delete.v1` | `{connection?, record_id}` | `{deleted}` |

Call `begin` before pushing a record. It protects against reordered and duplicate work:

- It answers `stale` when the mapping is already `synced`, `withdrawn`, or `rejected` at the same or a newer revision, unless an operator asked for a retry or you pass `force`.
- Otherwise it leases the record for `lease_ms` (default two minutes, at most 15), so an event handler and a connector run never push the same record at the same time. It answers `busy` while someone else holds the lease.

`commit` stores the result and releases the lease. Without a lease token it fails while another worker holds a live lease, and a `synced` result never replaces a newer synced revision. If a batch fails, release the leases it holds.

After an `uncertain` push to a destination without idempotency keys, commit the mapping as `uncertain`. Before pushing that record again, look it up on the destination, for example by SKU, and record what you found. `mappings.get.v1` by `external_id` finds the record for an item you see on the destination.

Keep large destination reference data, such as category trees or product-type definitions, in extension storage.

## Public file links

Some destinations fetch images themselves, sometimes hours after a listing is submitted. With `attricat.files.public_links` and `attricat.read`, `files.link.v1` issues a public, unguessable link to a file a record references:

```json
{ "file_id": "…", "record_id": "…", "expires_in_seconds": 86400 }
```

It returns `{"url": "…", "expires_at": "…"}`. A link lives from 60 seconds to 7 days. Anyone with the link can download the file while it is unexpired and the extension is enabled with the capability. Only a hash of the token is stored. The deployment must set [`EXTENSION_PUBLIC_FILE_BASE_URL`](/reference/configuration/#extensions); otherwise the call fails.

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
