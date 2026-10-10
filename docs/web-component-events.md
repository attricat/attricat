# Web-component event bridge

Attricat custom elements communicate with their host through versioned DOM events.
This bridge is a convenience API, not an authorization boundary: the host always
validates a request and components re-fetch authorized current data. It must not
forward a transactional-outbox envelope, event metadata, payload, snapshots, or
other server-internal facts to a component.

Contracts and helpers live in
`apps/web/src/features/web-components/attricatEvents.ts`. The host
listener is `installAttricatEventBridge` in `attricatEventBridge.ts`.

## Host to component

Dispatch these on the intended component element. They do not bubble or cross a
shadow boundary (`bubbles: false`, `composed: false`). Their details contain
only client-safe identifiers, coarse change hints, and an optional correlation
ID. The helpers validate the strict schemas before dispatching, so invalid
outbound details throw instead of being emitted.

| Event                        | Detail                                         |
| ---------------------------- | ---------------------------------------------- |
| `attricat:record-updated.v1`  | `{ record_id, change_hints, correlation_id? }` |
| `attricat:context-changed.v1` | `{ context_id, correlation_id? }`              |

`record_id`, `context_id`, and `correlation_id` are UUIDs. `change_hints` is a
bounded list of `record`, `attribute_values`, `relationships`, or `blueprint`.
It is deliberately not a list of outbox facts or values.

## Component to host

A component dispatches these bubbling, composed events so an embedding host can
listen across a shadow boundary. The host bridge ignores invalid details,
including details with extra fields, and calls `preventDefault()` after passing
a valid detail to its configured callback. Dispatchers should not treat that as
an authorization result: the event is not cancelable by default. Once
validated, the embedding page supplies the behavior:

| Event                       | Detail                                          | Host behavior                                                               |
| --------------------------- | ----------------------------------------------- | --------------------------------------------------------------------------- |
| `attricat:refresh-record.v1` | `{ record_id, change_hints?, correlation_id? }` | Re-fetch or invalidate authorized record data.                              |
| `attricat:navigate.v1`       | `{ record_id, correlation_id? }`                | Navigate using the host-owned record route. Components cannot supply a URL. |
| `attricat:notify.v1`         | `{ message, severity?, correlation_id? }`       | Display a bounded user-facing notification.                                 |

`severity` is `success`, `info`, `warning`, or `error`; `message` is trimmed
and limited to 512 characters. The host owns routing, data fetching, and UI
presentation.

## Security model and non-goals

This is a local UI convenience protocol, not a trust boundary. Any component
can dispatch an event, so the host must validate again before acting and every
resulting API request is still authenticated and authorized by Attricat. A
component cannot provide a URL, HTTP method, token, permission, workspace, or
server-side mutation payload through this bridge. Treat IDs and correlation IDs
as untrusted input; re-fetch current authorized data rather than trusting a
component's view.

The bridge is intentionally **not** an outbox subscription, webhook, server
push protocol, audit stream, or generic inter-plugin RPC mechanism. It never
forwards domain-event envelopes, event metadata/payloads, record snapshots,
values, credentials, or authorization decisions. It also does not provide
ordering, delivery, persistence, retries, or DOM-event cancellation semantics.
