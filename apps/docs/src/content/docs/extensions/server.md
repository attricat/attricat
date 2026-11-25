---
title: Server runtime
description: Run extension code on the server as a WebAssembly component, react to events, read and write the catalog, call external services, and publish events.
---

Server-side extension code is a WebAssembly **component** declared as a `server_wasm` artifact. It runs in a sandbox with no WASI context: no file system, environment variables, sockets, or pre-opened files, and no clock other than the host's `clock` interface. Everything it does goes through calls to the host, and the host checks the extension's grants on every call.

## Host API version

The host interface is the WIT package `attricat:host@1.2.0`, published as `wit/attricat-host-1.2.0.wit` in the [attricat-extension-sdk](https://github.com/attricat/attricat-extension-sdk) repository with every other released version. Set `attricat.host_api` in your manifest to a range that accepts it, such as `">=1.0.0, <2.0.0"`, `"^1.1"` or `"^1.2"`. Every release with such a range can use every feature: event handlers, commands, typed reads and writes, scoped configuration, every kind of [operation](/extensions/operations/), [inbound webhooks](#receive-webhooks), and all client outlets. If your component uses anything added in 1.1 or 1.2, declare `"^1.1"` or `"^1.2"`, so an older host refuses the package instead of failing at run time.

Build your component against one of these worlds:

| World | Exports |
| --- | --- |
| `attricat-extension` | `handler` and `operations` |
| `handler-extension` | `handler` (event handlers and commands) |
| `operation-extension` | `operations` |
| `outcome-extension` (1.1) | `event-handler`, `command-handler`, and `deferrable-operations` |
| `webhook-extension` (1.2) | everything `outcome-extension` exports, plus `webhook-receiver` |

`outcome-extension` exports the 1.1 interfaces, which report [typed outcomes](#typed-outcomes) instead of a plain error. When a component exports a 1.1 interface, the host calls it instead of its 1.0 predecessor. A component that needs only some of the exports returns a fault from the others. Build for `webhook-extension` only if the extension [receives webhooks](#receive-webhooks).

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
| `webhooks.receipt.get.v1` | `webhooks.receive` | [Receive webhooks](#receive-webhooks) |

The host checks the grant on every call, so a grant revoked while your code runs takes effect at its next call. A refused call returns an error such as `missing capability 'storage.extension'`, and an unknown name returns `unknown host operation`. JSON you send is limited to 64 KiB, and JSON the host answers with to 1 MiB.

The Rust crate [`attricat-extension`](https://github.com/attricat/attricat-extension-sdk/tree/main/crates/attricat-extension) in the SDK repository types these JSON calls, including storage, catalog batches, network requests, and export mappings.

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
- **Only events from when you subscribe.** A handler receives events that occur while its extension is enabled and subscribed to them. Enabling an extension, granting a capability, or adding an event type doesn't replay earlier events, and events that occur while the installation is disabled or quarantined aren't delivered later. To catch up, read the catalog, for example with the `changes` feed below.
- **Failures quarantine the extension.** With the 1.0 `handler` export, a trap, running out of fuel or memory, a timeout, or a returned error quarantines the installation. The event doesn't reach it again: the delivery's retry finds the installation quarantined and ends, and enabling it later doesn't bring the event back. A 1.1 `event-handler` can report a failure without being quarantined; see [Typed outcomes](#typed-outcomes).

Before each delivery, Attricat checks again that the installation is enabled, still on the same release, and still has the grants it needs.

### Debounce

A burst of edits to one record can produce many events. Add `debounce_ms` to the handler to receive only the latest:

```json
{ "id": "push", "event_types": ["record.updated.v1"], "handler": "handle-event", "debounce_ms": 30000 }
```

`record.updated.v1` alone misses values edited outside a save. See [Events](/reference/events/) for which write emits what, and add `attribute_value.changed.v1` if you need every value change.

A debounced event waits until its record has been quiet for `debounce_ms` (up to one hour). Events that a newer event of the same record supersedes are skipped. An `export.retry_requested.v1` event concerns one connection, so it supersedes only an earlier retry request for the same connection. `max_wait_ms` (default ten times `debounce_ms`, at most 24 hours) caps how long an event can wait while new ones keep arriving.

### Typed outcomes

A 1.1 `event-handler` exports `handle-event(event, delivery)`, which returns `result<_, failure>`. `delivery` carries the event's `sequence`, the record's `record-revision` for record content events, the `attempt` number, starting at 1, and the event's `source-kind` (`api`, `worker`, `plugin`, or `system`) and `source-name`, such as `extension:acme.inventory` or `workflow:<id>`.

`failure`, from the `outcomes` interface, is one of:

| Case | What happens |
| --- | --- |
| `rejected { code, message, record-id?, connection? }` | The failure is recorded and never retried; the delivery completes. With a `record-id`, the record's [export status](/extensions/exports/#export-mappings) becomes `rejected` with the code and message. |
| `retry { message, after-ms? }` | Only this handler is retried, after `after-ms` or a backoff from 5 seconds doubling to one hour, up to 10 attempts. After that the delivery is recorded as `exhausted`, and records the handler still leases become `retrying` with error `retries_exhausted`. |
| `fault(message)` | The installation is quarantined, and records the handler leased become `retrying` with error `handler_fault`. |

Traps, running out of fuel or memory, and timeouts count as faults. Outcomes are kept per event and handler, so when one handler of an event is retried, handlers that already completed are not called again. Rejected and exhausted deliveries stay visible for 30 days on the extension's management page, or until their event expires under `DOMAIN_EVENT_RETENTION_DAYS` if that is sooner.

`command-handler` exports `handle-command` for [commands](#commands-for-your-ui), separately from event handling.

## Clock

`clock.now()` returns the UTC wall-clock time as seconds and nanoseconds since the Unix epoch. Use it for token expiry, poll scheduling, and timestamps a destination needs.

## Read and write the catalog

With `attricat.read`, the typed `api.read` function reads a record (`record`), its direct values (`values`), or its values resolved in a context through the fallback chain (`resolved`). Responses include the record's pinned blueprint revision, so you can map attribute IDs to codes.

With `attricat.write`, `api.write` writes scalar values in an explicit context, naming each attribute by `attribute-id` or `attribute-code`. Writes go through the normal path: type checks, schemas, audit, and a new domain event.

A write made while handling an event is attributed to the user or token behind the original change, keeps the event's correlation ID, and is published with source `extension:<extension-id>`. Your handler receives the event that write causes. A 1.1 `event-handler` can return early when `delivery.source-name` is `extension:<your-extension-id>`. The 1.0 `handler` export gets no source, so compare before you write there: if the value is already what you would write, skip the write, and the loop ends.

`attricat.read.v1` adds paged reads, change feeds, and lookups. The request names an `operation`:

```json
{ "operation": "page", "blueprint_id": "…", "blueprint_version": 3, "context_id": "…", "cursor": null, "limit": 100 }
{ "operation": "changes", "blueprint_id": "…", "blueprint_version": 3, "cursor": null, "limit": 100 }
{ "operation": "lookup", "blueprint_id": "…", "blueprint_version": 3, "attribute_id": "…", "value": "ABC-1" }
{ "operation": "movements", "record_id": "…", "after": 0, "limit": 500 }
{ "operation": "incoming", "record_id": "…", "relationships": [{ "source_blueprint": "order_line", "field": "order" }], "cursor": null, "limit": 100 }
{ "operation": "record", "record_id": "…", "context_id": "…" }
{ "operation": "blueprint", "code": "invoice" }
{ "operation": "context", "code": "default" }
```

`page` returns `{records, next_cursor}`, with up to 100 records. Pass `next_cursor` back until it is `null`. `changes` pages through record changes of the blueprint in event order; save its cursor to resume later. `lookup` returns one record or `null`. `movements` returns a record's movement history after the sequence `after`, as [`GET /v1/records/{id}/movements`](/reference/api/) does, up to 500 entries a page. Every balance change also updates the record and publishes `record.updated.v1`, so a handler of that event can read the history after the last sequence it saw. `incoming` returns `{items, next_cursor}`: the records whose named relationship fields link to the record, such as an order's lines, up to 100 a page. `record` returns one record as `api.read` does, `{record, blueprint, direct_values, resolved_values}`, with `resolved_values` only when you pass `context_id`; operations use it because they cannot call `api.read`. `blueprint` returns the latest published revision of a blueprint, with its ID, version and attributes, and `context` returns a context with its ID; both return `null` for an unknown code. Use them to let configuration name blueprints and contexts by code, and to create records against the current revision.

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

To clear a value, list it in `remove_values` of an `update` or `upsert` intent, as `{"attribute_code": "due_date", "context_id": "…"}`. Writing `null` or `""` as a value does not clear it: `null` fails the type check, and `""` is a value of string attributes only. An upsert that creates a record ignores `remove_values`.

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

`expected_revision` makes `set` and `delete` fail if the key changed since you read it. An installation can store up to 65,536 keys and 256 MiB, with keys of at most 256 bytes and values of at most 64 KiB. `list` returns 1 to 100 entries per page.

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

The broker never retries. A timeout or connection failure is an **uncertain outcome**: the other side may have received the request. Retry only if the destination supports an idempotency key and you send a stable one as `idempotency_key`, which the host sends as the `Idempotency-Key` header. A v1 request over the budget fails with `network request rate limit exceeded`.

### network.request.v2

`network.request.v2` takes the v1 fields plus:

- a query string in `url`, if the matching rule sets `allow_query: true`, for parameters such as `?marketplaceIds=` or presigned URLs. Query values never appear in logs, traces, metrics, errors, or the stored delivery history;
- `body_base64` for a binary body, instead of `body`;
- `secret_body: {"format": "form" | "json", "fields": [{"field", "secret"}]}`, which puts workspace secrets into a form or JSON body. A JSON `field` is a top-level key or a JSON pointer. The component never sees the secret;
- `connection`, which adds the [connection's](/extensions/exports/#connections) access token to the request.

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

## Receive webhooks

Host API 1.2 lets external systems, such as a marketplace sending order notifications, call your extension. Your component answers each request while the sender waits and can keep its payload. A kept payload is stored and later delivered to your own event handlers, where the real work happens.

Declare each webhook in `server.webhooks`, and handle the event it produces:

```json
"attricat": { "id": "acme.orders", "host_api": "^1.2" },
"permissions": ["webhooks.receive", "secrets.read", "events.subscribe", "attricat.write"],
"server": {
  "webhooks": [
    { "id": "orders", "title": "Order notifications", "max_body_bytes": 131072 }
  ],
  "event_handlers": [{
    "id": "orders",
    "event_types": ["webhook.received.v1"],
    "handler": "handle-event"
  }]
}
```

A release can declare up to 16 webhooks. `max_body_bytes` is the largest request body Attricat accepts and the largest payload you can keep: 64 KiB by default, at most 512 KiB. Webhooks need the `webhooks.receive` capability, required or optional, and a component built for the `webhook-extension` world.

A webhook receives nothing until an operator creates an **endpoint** for it in the extension's [Webhooks section](/builders/extensions/#webhooks). Each endpoint has its own secret URL and a name, such as the sending account, which your component receives, so one webhook can serve several accounts with a secret each.

### Answer the request

Attricat finds the endpoint by its URL and answers some requests itself, without calling your component:

| Situation | Response |
| --- | --- |
| Unknown URL | `404` |
| More than 600 requests to the endpoint in a minute | `429` |
| The extension is disabled or quarantined, `webhooks.receive` isn't granted, or the installed release no longer declares the webhook | `503` |
| The body is larger than `max_body_bytes` | `413` |

Otherwise it calls `webhook-receiver.receive` with a `webhook-request`:

| Field | Contents |
| --- | --- |
| `webhook` | The webhook `id` from your manifest. |
| `endpoint` | The operator's endpoint name. |
| `method` | `GET` or `POST`. |
| `query` | The query string without `?`, possibly empty. |
| `headers` | Request headers with lower-case names. `cookie`, `authorization`, `proxy-authorization` and headers whose value isn't UTF-8 are left out. |
| `body` | The raw body bytes. |

Return a `webhook-outcome` with the `reply` the sender gets (`status`, `headers`, `body`) and, optionally, `keep`: a `payload` to store and an optional `dedupe-key`.

```rust
fn receive(request: WebhookRequest) -> Result<WebhookOutcome, String> {
    let secret = call("secrets.get.v1", json!({"name": format!("{}-signing-key", request.endpoint)}))?;
    if !signature_matches(&secret, &request) {
        return Ok(reply(401, "bad signature"));
    }
    let mut outcome = reply(202, "accepted");
    outcome.keep = Some(KeptPayload {
        payload: String::from_utf8(request.body).map_err(|e| e.to_string())?,
        dedupe_key: header(&request, "x-delivery-id").map(str::to_owned),
    });
    Ok(outcome)
}
```

`receive` must be quick and can only read:

- It gets 5 seconds, including waiting for a free invocation slot, and less fuel than other calls.
- It can use `configuration.get.v1`, `secrets.get.v1`, `storage.get.v1`, `storage.list.v1`, scoped configuration reads, `api.log` and `clock`, each with its usual capability. Everything else, including catalog reads and writes, storage writes, publishing events, network requests and `webhooks.receipt.get.v1`, returns an error.
- Any failure, whether an error you return, a trap, a timeout or an invalid outcome, is answered with `500` and logged. It never quarantines the extension, because anyone who knows the URL can send requests.

Attricat checks the outcome before sending it. The status must be between 200 and 599. At most 16 headers are allowed, and you can't set `connection`, `content-length`, `transfer-encoding`, `set-cookie`, `cache-control`, `referrer-policy`, `x-content-type-options`, `strict-transport-security` or `content-security-policy`. The reply body is text of at most 64 KiB. A kept payload is text of at most `max_body_bytes`, so encode a binary body, for example as base64, if you keep it. A dedupe key has 1 to 256 bytes. The reply is sent as you returned it, with `Cache-Control: no-store` and `X-Content-Type-Options: nosniff`, and as `text/plain` unless you set `content-type`.

Some senders check a new URL with a `GET` that expects a value from the query echoed back. Answer it from `receive` like any other request.

### Check the sender

Attricat doesn't authenticate senders beyond the secret URL. Your component decides whether a request is genuine:

- Verify the sender's signature over the raw `body`, with a key from [secrets](#secrets). Answer anything unsigned or wrongly signed with a `4xx` and keep nothing.
- If the sender signs a timestamp, reject requests whose timestamp is too old, so a captured request can't be replayed later.
- Set `dedupe-key` to the sender's delivery ID. An endpoint keeps only the first payload with each key: a repeat gets your reply but isn't stored or delivered again.

### Process kept payloads

A kept payload and its `webhook.received.v1` event are stored in one transaction before the reply is sent. If storing fails, the sender gets an error and can retry, so a request you acknowledged is never lost. Only your extension receives the event; see its [payload](/reference/events/#webhook-receipts).

Your handler reads the payload with `webhooks.receipt.get.v1`:

```json
{ "receipt_id": "<receipt_id from the event>" }
```

It returns `{id, webhook, endpoint, dedupe_key, payload, received_at}`, or `null` once the payload is older than 7 days and has been removed. The event is delivered like any other: at least once, with [typed outcomes](#typed-outcomes), retries and dead letters. Make the handler idempotent. The same event can arrive twice, and a sender may resend a delivery under a new ID, so upsert by the sender's own ID, such as an order number, or record what you've processed in [storage](#storage).

Deleting an endpoint or rotating its URL doesn't affect payloads already kept. Removing the extension deletes its endpoints and payloads.

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

Each call into your component, such as one command, event delivery, or operation batch, gets:

- 1 billion units of fuel, which roughly counts WebAssembly instructions;
- 32 MiB of memory in total, plus at most 8 memories, 64 tables with 100,000 elements between them, and 32 instances;
- 1 MiB of log messages, after which further messages are dropped;
- 10 seconds, not counting time spent waiting on outbound network requests and transfers, which have their own timeouts.

A server component can be at most 32 MiB, and each module in it at most 16,384 types, imports, globals, tables, memories, exports and table elements together. It compiles in a separate process limited to 512 MiB of memory and 2 minutes, and if that fails it isn't tried again for 5 minutes. Each API process runs 16 invocations at once, and 8 from one workspace, so more wait their turn.

Going over a limit on a call counts as a fault and quarantines the installation, so keep each call small. A webhook's `receive` has [tighter limits](#answer-the-request) and never quarantines. Long work belongs in an [operation](/extensions/operations/), which does a bounded amount per batch.

- JSON you send to the host: 64 KiB per call. JSON the host answers with: 1 MiB.
- Log messages: 16 KiB.
- Command requests and responses: 64 KiB by default.
- Operation requests and checkpoints: 64 KiB.
