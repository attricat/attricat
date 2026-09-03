# API Reference

The API is JSON over HTTP. Successful responses are JSON; failures use an
`error` object with a machine-readable code, message, and HTTP status. The
[CLI](cli.md) is the preferred interface for shell automation.

## Authorization

`GET /health`, `POST /auth/discover`, and `POST /auth/login` are public. Browser requests authenticate
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

Browser-session request tenancy is selected from the workspace stored in the
verified session; it is never selected by a client workspace header.

`POST /auth/renew` atomically rotates the browser session, `POST /auth/logout`
revokes it, and login, renew, and `GET /auth/session` return the active user identity,
session-bound `workspace_id`, and human-facing workspace `login_identifier`. The local
password, cookie, CSRF, expiry, and revocation contract is documented in
[Browser Authentication](authentication.md).

## Routes

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/health` | Confirm the migrated API is ready. |
| `POST` | `/auth/discover` | Resolve a normalized workspace identifier and return its sign-in methods; rate-limited and intentionally minimal. |
| `POST` | `/auth/login` | Sign in with a previously resolved workspace identifier, email, and password. |
| `GET` | `/metrics` | Scrape Prometheus service metrics. |
| `GET` | `/workspace/roles` | List fixed and workspace-local roles with permissions (`roles.manage`). |
| `POST` | `/workspace/roles` | Create a workspace-local role. |
| `PUT` | `/workspace/roles/{role_id}` | Update a workspace-local role. Fixed roles are immutable. |
| `POST` | `/workspace/roles/{role_id}/duplicate` | Duplicate a fixed or local role as a custom role. |
| `POST` | `/workspace/roles/{role_id}/retire` | Retire a custom role, optionally replacing its grants. |
| `GET` | `/workspace/permissions` | List permissions available for custom roles (`roles.manage`). |
| `GET` | `/workspace/assignable-roles` | List roles available to member and invitation management (`members.manage`). |
| `GET` | `/workspace/token-permissions` | List the caller's permissions available to personal-token management (`tokens.manage`). |
| `GET` | `/workspace/grant-targets/{scope_type}` | List workspace-owned grant targets for a scope (`members.manage`); ownership is checked again when granting. |
| `GET` | `/workspace/members` | List members and additive grants (`members.manage`). |
| `PUT` | `/workspace/members/{member_id}` | Set member state to `active` or `inactive`. |
| `POST` | `/workspace/members/{member_id}/grants` | Add a role grant at one requested scope. |
| `DELETE` | `/workspace/members/{member_id}/grants/{grant_id}` | Revoke a role grant. |
| `POST` | `/workspace/members/{member_id}/transfer-ownership` | Transfer ownership to an active member (owner only). |
| `GET`, `POST` | `/workspace/invitations` | List or create expiring email invitations. |
| `DELETE` | `/workspace/invitations/{invitation_id}` | Revoke a pending invitation. |
| `POST` | `/workspace/invitations/accept` | Accept `{ "secret": "cat_inv_..." }` as the verified intended account. |
| `GET`, `POST` | `/personal-access-tokens` | List or issue a personal token; creation returns its secret exactly once. |
| `DELETE` | `/personal-access-tokens/{token_id}` | Revoke a personal token. |
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
| `POST` | `/v1/entities/search` | Search entities in one blueprint revision, optionally by system tags. |
| `POST` | `/v1/entities` | Create an entity atomically with form values and optional system annotations. |
| `GET`, `PUT` | `/v1/entities/{id}` | Read or update an entity form atomically, including optional system annotations. |
| `POST` | `/v1/entities/{id}/blueprint-migration/preview` | Assess migration to the highest published revision. |
| `POST` | `/v1/entities/{id}/blueprint-migration` | Migrate an entity to that revision. |
| `POST` | `/entities/{entity_id}/file-attributes/{attribute_code}/uploads` | Stream one or more multipart file parts to a file attribute. |
| `GET` | `/files/{file_id}` | Read safe file metadata and generated variant metadata. |
| `GET` | `/files/{file_id}/download` | Download the original through the API, with one safe byte range. |
| `GET` | `/files/{file_id}/variants/{kind}/download` | Download a ready generated variant through the API. |
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

`POST /v1/entities/search` optionally accepts multiple relationship tree facets. See
[Relationship Tree Facets](search-facets.md) for its request and response
contract.

## Entity system annotations

Entities include `system_tags` (an array of unique, non-empty strings) and
`system_metadata` (a JSON object, up to 64 KiB). These fields are intentionally
outside the versioned blueprint and EAV value model, so automation and operators
can retain workflow markers and diagnostic data without changing an entity's
schema. They are returned with entity reads and form responses, but never added
to projections or views.

`POST /v1/entities` accepts both fields; omitted values default to `[]` and
`{}`. `PUT /v1/entities/{id}` accepts either field independently; omitted fields
are retained, while `[]` or `{}` clears the corresponding value. Search accepts
`system_tags`; returned entities must contain every supplied tag, making it
suitable for finding a marked batch before applying a bulk workflow.

## File uploads and downloads

Upload with `multipart/form-data`. Use `file` or `files` for every binary part
and an optional single `context_id` text part. The API streams parts to a
temporary file rather than buffering the whole request. It validates the
content signature, declared MIME type, extension, request limits, and the
pinned file-attribute policy before persisting metadata and queuing processing.
A successful response is `201` with the attribute, context, and safe file
metadata; originals and storage keys are never returned.

A newly accepted file has `status: "queued"`. Image files are processed into
`thumbnail` and `display` WebP variants; non-image files become `ready` without
variants. `GET /files/{file_id}` returns `200` with safe metadata and its
current status while a file is queued, processing, ready, or failed. The
original and variant download endpoints return `409 file_processing` until the
file is `ready`, including after terminal processing failure. A failed job
records a safe processing error and is retried with bounded exponential backoff;
an operator can requeue a terminal failed job as described in
[Configuration](configuration.md). Ready downloads are authorized, proxied
through the API, private/no-store, and support one `Range: bytes=start-end`
request.

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

`GET /metrics` serves Prometheus text exposition. All labels are bounded: HTTP
metrics use method, matched route template, and status; no file ID, object key,
filename, workspace, or request URL is ever a label. File operation metrics are:

- `catalog_file_uploads_total` (`outcome`),
  `catalog_file_downloads_total` (`outcome`), and
  `catalog_object_store_operations_total` (`operation`, `outcome`);
- `catalog_file_worker_jobs_claimed_total`,
  `catalog_file_worker_jobs_completed_total`, and
  `catalog_file_worker_jobs_failed_total` (`terminal`), plus the queued-job
gauge `catalog_file_worker_jobs_queued`;
- `catalog_file_reconciliation_total` (`outcome`),
  `catalog_file_reconciliation_files_marked_total`, and
  `catalog_file_purge_jobs_queued_total`.

Data-health cache decisions are exposed as `catalog_data_health_cache_total`
with a bounded `status` label. Scrape this endpoint from the private monitoring
network rather than exposing it publicly.

The API emits structured `tracing` events for startup, database migrations, each
HTTP request, file uploads, downloads, and worker jobs. Request spans include
the method, matched route template, response status, and duration; file spans
include only internal IDs and bounded operation values, never object keys or
filenames. 5xx responses are emitted at error level. Set `RUST_LOG` (for
example, `RUST_LOG=api=debug`) to control output verbosity.

## Agents

Agent routes require `agents.run`. `GET`/`POST /agent/conversations` lists or
creates conversations. `GET`/`PATCH`/`DELETE /agent/conversations/{id}` reads,
renames, or archives a thread; it exposes ordered messages at
`/agent/conversations/{id}/messages` and run history at
`/agent/conversations/{id}/runs`. Posting a message creates a durable queued
run and returns `202`; execution is owned by the API worker rather than the
HTTP request.

`GET /agent/runs/{run_id}/events` is an SSE stream of durable status, message,
tool, approval, error, terminal, and schedule events. Each SSE `id` is the
persisted event UUID. Reconnect with `Last-Event-ID` to replay only later
ordered events. `GET /agent/approvals` lists pending tool calls (optionally by
`conversation_id`), and the existing approve/reject routes enqueue the resumed
run after atomically recording the decision. Decisions are accepted only while
a call is pending; a repeated or contradictory decision returns the normal
`approval_already_decided` conflict and never executes the write again.

Conversation titles are limited to 512 bytes and messages to 1–32,768 bytes.
Upload standalone conversation files with multipart `POST`
`/agent/conversations/{id}/uploads`, then include up to 16 returned,
workspace-scoped, unique `attachment_ids` when posting the message. Every
provider request includes each attachment's display filename, MIME type, and
file ID; it excludes object-store keys, download URLs, checksums, and byte
sizes. Only `image/*` attachments whose stored size and retrieved bytes are at
most 5 MiB (5,242,880 bytes) also send their original bytes as a base64 `data:`
image URL. Non-images, oversized images, and unreadable images send no file
content. The same stored conversation history is sent on later runs and
provider tool-call rounds, so this applies each time the message is included.
See [Agent provider attachment forwarding](configuration.md#attachment-forwarding)
for tool-requested file behavior and provider-retention implications. Entity
file uploads remain available through their file-attribute endpoint. A message
or run request returns `503 service_unavailable` when the API has no configured
provider/worker. Runs retain provider/model snapshots and safe error codes, but
never provider credentials or raw provider response bodies. Read tools run
automatically; every mutation is emitted as an approval proposal before it
reaches a repository write.

Schedules are managed by `GET`/`POST /agent/schedules`,
`PUT`/`DELETE /agent/schedules/{id}`, and
`POST /agent/schedules/{id}/run-now` (the temporary `/run` alias is also
accepted). A schedule request contains `conversation_id` and a six-field UTC
`cron_expression`. The creating user is persisted as the scheduled execution
principal; scheduled mutations are re-authorized as that user, while a manual
run uses the user who requested it. Overlapping scheduled occurrences become durable
skipped runs with a `schedule_skipped` event instead of executing concurrently.
