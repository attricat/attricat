# Web-component event bridge

Catalog custom elements communicate with their host through versioned DOM events.
This bridge is a convenience API, not an authorization boundary: the host always
validates a request and components re-fetch authorized current data. It must not
forward a transactional-outbox envelope, event metadata, payload, snapshots, or
other server-internal facts to a component.

Contracts and helpers live in
`apps/catalog-web/src/features/web-components/catalog-events.ts`. The host
listener is `installCatalogEventBridge` in `catalog-event-bridge.ts`.

## Host to component

Dispatch these on the intended component element. Their details contain only
client-safe identifiers, coarse change hints, and an optional correlation ID.

| Event                        | Detail                                         |
| ---------------------------- | ---------------------------------------------- |
| `catalog:entity-updated.v1`  | `{ entity_id, change_hints, correlation_id? }` |
| `catalog:context-changed.v1` | `{ context_id, correlation_id? }`              |

`entity_id`, `context_id`, and `correlation_id` are UUIDs. `change_hints` is a
bounded list of `entity`, `attribute_values`, `relationships`, or `blueprint`.
It is deliberately not a list of outbox facts or values.

## Component to host

A component dispatches these bubbling, composed events. The host bridge ignores
invalid details, including details with extra fields. Once validated, the
embedding page supplies the behavior:

| Event                       | Detail                                          | Host behavior                                                               |
| --------------------------- | ----------------------------------------------- | --------------------------------------------------------------------------- |
| `catalog:refresh-entity.v1` | `{ entity_id, change_hints?, correlation_id? }` | Re-fetch or invalidate authorized entity data.                              |
| `catalog:navigate.v1`       | `{ entity_id, correlation_id? }`                | Navigate using the host-owned entity route. Components cannot supply a URL. |
| `catalog:notify.v1`         | `{ message, severity?, correlation_id? }`       | Display a bounded user-facing notification.                                 |

`severity` is `success`, `info`, `warning`, or `error`; `message` is trimmed
and limited to 512 characters. The host owns routing, data fetching, and UI
presentation.
