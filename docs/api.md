# API Reference

The API is JSON over HTTP. Successful responses are JSON; failures use an
`error` object with a machine-readable code, message, and HTTP status. The
[CLI](cli.md) is the preferred interface for shell automation.

## Authorization

`GET /health` and `POST /auth/login` are public. Browser requests authenticate
with the opaque HttpOnly `catalog_session` cookie created by login; missing,
expired, rotated, or revoked sessions return `401`. Unsafe cookie-authenticated
requests must also supply `X-Catalog-Csrf` with the readable `catalog_csrf`
synchronizer token or receive `403`. An authenticated principal without the
required permission or scope receives `403` without revealing whether a target
exists.

Permissions are evaluated from active workspace membership role grants:
blueprint reads/writes/publishing require `blueprints.read`, `blueprints.write`,
and `blueprints.publish`; entity operations require `entities.read`,
`entities.write`, or `entities.delete`; context operations require
`contexts.read` or `contexts.write`; data-health and metrics require
`data_health.read`. A context-subtree grant applies to its root and descendants,
never its ancestors or siblings.

Catalog data remains scoped to the trusted server-selected
`CATALOG_WORKSPACE_ID` RLS boundary. The workspace header must equal that value;
a deployment therefore serves one workspace rather than using a request header
to route a shared database pool. See [Configuration](configuration.md#request-authorization).

`POST /auth/renew` atomically rotates the browser session, `POST /auth/logout`
revokes it, and `GET /auth/session` returns the active user identity. The local
password, cookie, CSRF, expiry, and revocation contract is documented in
[Browser Authentication](authentication.md).

## Routes

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/health` | Confirm the migrated API is ready. |
| `GET` | `/metrics` | Scrape Prometheus service metrics. |
| `GET` | `/blueprints` | List published entity blueprints. |
| `GET` | `/blueprints/catalogue` | List blueprint families and revisions. |
| `POST` | `/blueprints` | Create the first draft revision from TOML. |
| `GET`, `POST` | `/blueprints/{id}/versions` | List revisions or create the next draft. |
| `GET` | `/blueprints/{id}` | Read the highest published revision. |
| `GET` | `/blueprints/{id}/versions/{version}` | Read an exact revision, including drafts. |
| `POST` | `/blueprints/{id}/versions/{version}/publish` | Publish a draft revision. |
| `GET` | `/blueprints/by-code/{code}` | Read the highest published revision by code. |
| `GET` | `/blueprints/by-code/{code}/versions/{version}` | Read an exact revision by code. |
| `GET`, `POST` | `/contexts` | List or create contexts. |
| `GET` | `/contexts/{code}` | Read a context. |
| `PUT`, `DELETE` | `/contexts/id/{id}` | Update or delete a context. |
| `GET` | `/entities` | Browse relationship targets. |
| `GET`, `DELETE` | `/entities/{id}` | Read or soft-delete an entity. |
| `GET` | `/entities/{id}/preview` | Read direct contextual preview values. |
| `GET` | `/entities/{id}/resolved-preview` | Resolve values through a requested context's ancestors. |
| `POST` | `/entities/{id}/values` | Append value history. |
| `GET` | `/entities/{id}/values/current` | Read current direct values and edges. |
| `POST` | `/entities/{id}/relationships/replace` | Replace relationship target sets. |
| `POST` | `/entities/{id}/relationships/remove` | Remove relationship targets. |
| `POST` | `/v1/entities/search` | Search entities in one blueprint revision. |
| `POST` | `/v1/entities` | Create an entity atomically with form values. |
| `GET`, `PUT` | `/v1/entities/{id}` | Read or update an entity form atomically. |
| `POST` | `/v1/entities/{id}/blueprint-migration/preview` | Assess migration to the highest published revision. |
| `POST` | `/v1/entities/{id}/blueprint-migration` | Migrate an entity to that revision. |
| `GET` | `/data-health/summary` | Read aggregate data-health metrics. |
| `GET` | `/data-health/blueprints` | Read blueprint health metrics. |
| `GET` | `/data-health/freshness` | Read value freshness metrics. |
| `GET` | `/data-health/completeness` | Read completeness metrics. |
| `GET` | `/data-health/contexts` | Read context metrics. |
| `GET` | `/data-health/relationships` | Read relationship metrics. |
| `GET` | `/data-health/storage` | Read storage metrics. |
| `POST` | `/data-health/refresh` | Clear cached data-health responses. |

Blueprint creation and revision routes create drafts. Only published revisions
can create entities or serve as migration targets. See [Blueprint Publication](database.md#blueprint-publication).

`POST /v1/entities/search` optionally accepts a relationship tree facet. See
[Relationship Tree Facets](search-facets.md) for its request and response
contract.

## Request performance

Every API response includes a standard `Server-Timing` header. Chrome DevTools
shows these values in **Network → Timing**, so a developer can inspect the
server work for an individual request without any extra tooling.

- `app;dur=<milliseconds>` is the total time spent handling the API request.
- Data-health responses also include `cache;desc=HIT`, `MISS`, or `BYPASS`.
  `BYPASS` means `DATA_HEALTH_CACHE_TTL_SECONDS` is zero and caching is
  disabled.

The header intentionally contains aggregate timings only; it never exposes SQL,
request bodies, identifiers, or other request data. The Vite development proxy
makes API calls same-origin. Deployments that call the API from another origin
must configure `Timing-Allow-Origin` separately if browser Resource Timing API
access is required.

## Metrics and traces

`GET /metrics` serves Prometheus text exposition. It provides
`catalog_http_requests_total` and `catalog_http_request_duration_seconds`, both
labeled only by method, matched route template, and response status. Data-health
cache decisions are exposed as `catalog_data_health_cache_total` with a bounded
`status` label. Scrape this endpoint from the private monitoring network rather
than exposing it publicly.

The API emits structured `tracing` events for startup, database migrations, and
each HTTP request. Request spans include the method, matched route template,
response status, and duration; 5xx responses are emitted at error level. Set
`RUST_LOG` (for example, `RUST_LOG=api=debug`) to control output verbosity.
