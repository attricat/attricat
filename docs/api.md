# API Reference

Most API routes exchange JSON over HTTP; successful empty responses use `204`,
and file, asset, and metrics routes return their documented content types.
Failures return a JSON `error` object with a machine-readable code and message,
alongside the HTTP status. Use the [CLI](cli.md) for shell automation.

## Authorization

`GET /health`, `GET /health/live`, `GET /health/ready`, `POST /auth/discover`, `POST /auth/login`,
`POST /auth/password-reset`, `POST /auth/password-reset/confirm`, and
`POST /onboarding/complete` are public. Browser requests authenticate
with the opaque HttpOnly `catalog_session` cookie created by login; missing,
expired, rotated, or revoked sessions return `401`. Unsafe cookie-authenticated
requests must also supply `X-Catalog-Csrf` with the readable `catalog_csrf`
synchronizer token or receive `403`. An authenticated principal without the
required permission or scope receives `403` without revealing whether a target
exists.

Permissions are evaluated from active workspace membership role grants:
blueprint reads/writes/publishing require `blueprints.read`, `blueprints.write`,
and `blueprints.publish`; workflow reads and management require `workflows.read` and `workflows.manage`; entity operations require `entities.read`,
`entities.write`, `entities.delete`, or `entities.publish`; context operations require
`contexts.read` or `contexts.write`; data-health, metrics, and event-delivery
dead-letter inspection require `data_health.read`; event-delivery replay and
role management require `roles.manage`; extension release discovery requires `extensions.read`, trusted registry source management requires `extensions.manage`, and solution-pack archive inspection, planning, application, and history require `solution_packs.manage`. The solution-pack permission is bootstrapped for the fixed owner and administrator roles. A context-subtree grant applies to its root and descendants,
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
| `GET` | `/health`, `/health/live` | Process liveness only; dependencies are deliberately not probed. |
| `GET` | `/health/ready` | Sanitized traffic-readiness check for PostgreSQL and required object storage. |
| `POST` | `/solution-packs/inspect` | Inspect an uploaded archive without applying it (`solution_packs.manage`); see [inspection](#solution-pack-plan-upload). |
| `GET` | `/presentation-assets` | List immutable private asset metadata (`solution_packs.manage`; `limit` 1–100, `offset` 0–10000). Use IDs for explicit `--map-asset` reuse; direct creation is unavailable. |
| `GET` | `/presentation-assets/{asset-id}` | Return same-workspace metadata only (`solution_packs.manage`); cross-workspace IDs return `404`. Use `acli presentation-asset show <uuid>`. |
| `GET` | `/presentation-assets/{asset-id}/content` | Download integrity-verified normalized bytes with private/no-store caching, a digest ETag, and restrictive content headers (`solution_packs.manage`). No create/update/delete endpoint exists. |
| `POST` | `/solution-packs/plans` | Revalidate an uploaded archive and persist an immutable, workspace-scoped dry-run (`solution_packs.manage`). See [plan request and response](#solution-pack-plan-upload). |
| `GET` | `/solution-packs/plans/{plan-id}` | Read a safe summary of an immutable plan in the authenticated workspace (`solution_packs.manage`). Cross-workspace IDs return `404`. |
| `POST` | `/solution-packs/plans/{plan-id}/apply` | Start or resume one immutable application without a request body or choice flags (`solution_packs.manage`); see [apply semantics](#solution-pack-plan-upload). |
| `GET` | `/solution-packs/applications?limit=25&offset=0` | List compact, bounded, workspace-scoped application provenance, including optional prior application identity, without mapping snapshots, steps, or normalized resource payloads (`solution_packs.manage`; limit 1–100, offset 0–10000). |
| `GET` | `/solution-packs/applications/{application-id}` | Read safe application provenance, its ordered release-change snapshot, and ordered step results (`solution_packs.manage`). Blueprint source, normalized resource payloads, archive bytes, and secrets are never returned. |
| `POST` | `/solution-packs/applications/{application-id}/abandon` | Permanently abandon a resumable sample-selected application and scrub its staged inputs; already committed entities remain ordinary workspace data (`solution_packs.manage`). |
| `GET`, `POST` | `/solution-packs/applications/{application-id}/checks` | List bounded immutable setup-check history, or rerun the application's informational checks against current workspace state (`solution_packs.manage`). Check runs never mutate resources. |
| `GET` | `/solution-packs/applications/{application-id}/checks/{run-id}` | Read one same-workspace immutable check run and its ordered results (`solution_packs.manage`). |
| `GET`, `POST` | `/extension-registries` | List the built-in official source and workspace custom sources, or add a trusted GitHub source (`extensions.manage`). |
| `DELETE` | `/extension-registries/{id}` | Remove one workspace custom source (`extensions.manage`). |
| `GET` | `/extension-registries/discover` | Load extension metadata from configured trusted registry `registry.json` indexes (`extensions.read`). |
| `GET` | `/extension-registries/extensions/{owner}/{repository}` | Resolve a repository listed in a current trusted index, returning its README and non-draft, non-prerelease `.tar.zst` GitHub Release assets (`extensions.read`). |
| `GET`, `POST` | `/extensions` | List installed extensions or install a validated release (`extensions.read` / `extensions.manage`). |
| `POST` | `/extensions/sideload` | Install a local `.tar.zst` archive supplied as an `application/zstd` request body. The 32 MiB archive limit and normal package validation apply; installations start disabled (`extensions.manage`). |
| `GET`, `DELETE` | `/extensions/{extension_id}` | Read or remove an installation (`extensions.read` / `extensions.manage`). |
| `POST` | `/extensions/{extension_id}/upgrade`, `/enable`, `/disable`, `/quarantine` | Change the installed release or lifecycle state (`extensions.manage`). |
| `PUT` | `/extensions/{extension_id}/configure` | Update validated installation configuration (`extensions.manage`). |
| `POST`; `DELETE` | `/extensions/{extension_id}/grants`; `/grants/{grant_kind}/{grant_id}` | Grant or revoke a declared extension permission (`extensions.manage`). |
| `GET` | `/extensions/{extension_id}/{contribution_id}/artifact` | Fetch a validated client artifact for an enabled release. |
| `POST` | `/extensions/{extension_id}/{contribution_id}/storage/{release_id}` | Perform a bounded client-mediated extension storage operation. |
| `PUT` | `/workspace/extensions-mode` | Enable or disable extensions for the current workspace (`extensions.manage`). |
| `GET`, `PUT` | `/workspace/extension-layout` | Read or replace the versioned, host-owned extension outlet layout (`extensions.manage`). Navigation layouts also contain a `promoted` stable-key list; built-ins cannot be referenced. |
| `GET` | `/extensions/runtime` | Return enabled, client-safe extension contributions, their stable keys, host-computed display order, navigation group, and route target for host-owned navigation links (`entities.read`). Pass a published entity `blueprint_id` and `blueprint_version` together to overlay that revision's entity-owned extension outlets while retaining global workspace rules. |
| `GET`, `POST` | `/rules` | List or create versioned blueprint-owned declarative rules (`rules.read` / `rules.manage`). |
| `POST` | `/rules/validate` | Structurally validate a strict rule TOML definition (`rules.manage`). |
| `POST` | `/rules/{rule_id}/run-now` | Enqueue an idempotent bounded manual or dry run (`rules.manage`). |
| `GET` | `/rule-runs`, `/rule-findings` | Read run diagnostics and active/resolved findings (`rules.read`). |
| `POST` | `/extensions/{extension_id}/{contribution_id}/command` | Validate a bounded, manifest-declared client-mediated extension command against the enabled exact release and grants (`entities.write`). |
| `POST` | `/auth/discover` | Resolve a normalized workspace identifier and return its sign-in methods; rate-limited and intentionally minimal. |
| `POST` | `/auth/login` | Sign in with a previously resolved workspace identifier, email, and password. |
| `POST` | `/auth/password-reset` | Request a password-reset message for a local account. |
| `POST` | `/auth/password-reset/confirm` | Consume a password-reset secret and set a new password. |
| `POST` | `/onboarding/complete` | Complete the public onboarding flow with its verified invitation or lifecycle secret. |
| `GET` | `/metrics` | Scrape Prometheus service metrics (`data_health.read`). |
| `GET` | `/audit-events` | List workspace audit evidence (`audit.read`). |
| `GET` | `/event-deliveries/dead-letters` | List terminal event-handler deliveries for the active workspace (`data_health.read`). |
| `POST` | `/event-deliveries/{consumer_id}/{event_id}/replay` | Reactivate one terminal delivery as pending; it preserves the event and attempts (`roles.manage`). |
| `GET` | `/workspace/roles` | List fixed and workspace-local roles with permissions (`roles.manage`). |
| `GET`, `PUT` | `/workspace/navigation` | Read or replace configured workspace navigation (`workspace_navigation.manage`). |
| `GET` | `/workspace/navigation/sidebar` | Read the caller-visible navigation tree. |
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
| `POST` | `/workspace/users` | Create a workspace user and membership (`members.manage`). |
| `DELETE` | `/workspace/invitations/{invitation_id}` | Revoke a pending invitation. |
| `POST` | `/workspace/invitations/accept` | Accept `{ "secret": "cat_inv_..." }` as the verified intended account. |
| `GET`, `POST` | `/personal-access-tokens` | List or issue a personal token; creation returns its secret exactly once. |
| `DELETE` | `/personal-access-tokens/{token_id}` | Revoke a personal token. |
| `POST` | `/workflows/validate` | Strictly validate and compile a workflow TOML definition without persisting it (`workflows.manage`). |
| `GET`, `POST` | `/workflows` | List workflow families/revisions or create the first draft (`workflows.read` / `workflows.manage`). |
| `GET`, `POST` | `/workflows/{id}/versions` | List immutable revisions or create the next draft. |
| `GET` | `/workflows/{id}`, `/workflows/{id}/versions/{version}` | Read the latest or exact workflow revision. |
| `POST` | `/workflows/{id}/versions/{version}/publish`, `/enable`; `/workflows/{id}/disable` | Publish, enable an exact published revision, or disable it. |
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
| `POST` | `/v1/entities/{id}/incoming-relationships` | Browse active incoming relationship edges. |
| `GET` | `/entities/{id}/hierarchy` | Read the configured relationship hierarchy for an entity. |
| `GET` | `/entities/{id}/changes` | Read entity audit changes. |
| `GET` | `/entities/{id}/values/history` | Read retained attribute-value history. |
| `POST` | `/entities/{id}/values/history/{history_id}/restore` | Restore one retained value-history entry. |
| `GET` | `/entities/{id}/resolved-preview` | Resolve values through a requested context's ancestors. |
| `POST` | `/entities/{id}/values` | Append value history. |
| `GET` | `/entities/{id}/values/current` | Read current direct values and edges. |
| `POST` | `/entities/{id}/relationships/replace` | Replace relationship target sets. |
| `POST` | `/entities/{id}/relationships/remove` | Remove relationship targets. |
| `POST` | `/v1/entities/search` | Search a selected blueprint across published revisions by default, or one explicit revision; supports text queries, validated filters, facets, and sorting. |
| `POST` | `/v1/entities` | Create an entity atomically with form values and optional system annotations. |
| `GET`, `PUT` | `/v1/entities/{id}` | Read or update an entity form atomically, including optional system annotations. |
| `POST` | `/v1/entities/{id}/blueprint-migration/preview` | Assess migration to the highest published revision. |
| `POST` | `/v1/entities/{id}/blueprint-migration` | Migrate an entity to that revision. |
| `GET`, `POST` | `/v1/entities/{id}/publications` | List channel publication status or publish to `{ "context_id": "…" }`. |
| `POST` | `/v1/entities/{id}/publications/unpublish` | Unpublish from `{ "context_id": "…" }`. |
| `POST` | `/v1/entities/{id}/publications/publish-all` | Publish atomically to every enabled channel. |
| `GET`, `PUT` | `/publication-channels`, `/publication-channels/{context_id}` | List enabled channel contexts or enable/disable one. |
| `POST` | `/entities/{entity_id}/file-attributes/{attribute_code}/uploads` | Stream one or more multipart file parts to a file attribute. |
| `GET` | `/files/{file_id}` | Read safe file metadata and generated variant metadata. |
| `GET` | `/files/{file_id}/download` | Download the original through the API, with one safe byte range. |
| `GET` | `/files/{file_id}/variants/{kind}/download` | Download a ready generated variant through the API. |
| `GET` | `/data-health/summary` | Read aggregate data-health metrics. |
| `GET` | `/data-health/background-processing` | Read workspace-scoped API task queue counts and lag (`data_health.read`). |
| `GET` | `/data-health/blueprints` | Read blueprint health metrics. |
| `GET` | `/data-health/freshness` | Read value freshness metrics. |
| `GET` | `/data-health/completeness` | Read completeness metrics. |
| `GET` | `/data-health/contexts` | Read context metrics. |
| `GET` | `/data-health/relationships` | Read relationship metrics. |
| `GET` | `/data-health/storage` | Read storage metrics. |
| `POST` | `/data-health/refresh` | Clear cached data-health responses. |

### Entity change and value-history pagination

`GET /entities/{id}/changes` and `GET /entities/{id}/values/history` retain
legacy JSON-array responses when called without pagination parameters. Supply
`limit` (1–50, default 25) or `offset` (0–10000, default 0) to receive
`{ "items": [...], "next_offset": number | null }`. Use `next_offset` for the
next page; the web entity-changes screen requests 25 at a time. Value-history
pages sort by immutable creation time (newest first), rather than the legacy
archive-time ordering. New writes between offset-based requests can shift
pages; refresh from offset zero after an edit. Pagination stops at offset
10000; older records remain available through the legacy response until a
cursor-based history contract is introduced. Both routes require scoped
`entities.read`. Built-in agent tools use bounded pages without calling these
HTTP endpoints.

### Solution-pack plan upload

`POST /solution-packs/inspect` takes an `application/zstd` `.tar.zst` body (at
most 32 MiB compressed). Its response is at most 512 KiB and contains safe
manifest metadata, whole-archive SHA-256, blueprint keys, bounded setting
summaries, normalized presentation-asset digests, and extension requirement
summaries. Inspection never returns resource bytes, archive paths, or private
object keys.

`POST /solution-packs/plans` requires `prefix` and
`blueprint_publication=draft|publish` query parameters. Optional
`include_sample_data=true` explicitly selects synthetic sample entities;
omitting it skips them. A `from_application=<uuid>` query parameter selects
one completed application of the same pack in the same workspace, at a lower
SemVer release. It cannot be combined with explicit maps. Catalog does not
search history or suggest a mapping. Added keys create, unchanged exact
published targets reuse, changed definitions block, and missing, unpublished,
revision-drifted, or hash-drifted targets conflict; removed keys are evidence
only.

Without explicit maps, upload the archive as `application/zstd`, including
when using `from_application`. For explicit reuse, send `multipart/form-data`
with exactly one streamed `archive` part (`application/zstd`) and repeated
`blueprint_map` JSON text parts such as
`{"key":"blueprints/product","code":"shared_product"}` and/or `asset_map`
parts such as `{"key":"assets/brand-logo","id":"<uuid>"}`. The 32 MiB
compressed archive and structural limits still apply. The archive is not
retained; asset-create actions privately stage normalized bytes before the
plan is ready.

The response is at most 1 MiB. It contains safe source/digest metadata,
optional prior-application identity, ordered release-change evidence, mapping
and action summaries, extension requirement outcomes, reasons, preconditions,
readiness, and a fixed 24-hour expiry. It never includes blueprint definitions,
normalized resource payloads, configuration templates or installed values,
archive paths, staged bytes, or object keys. See [solution-pack planning](solution-packs.md#planning-and-application-implemented-v1)
for apply-time behavior.

`POST /solution-packs/plans/{plan-id}/apply` starts only a ready, unexpired plan,
or resumes its existing application after plan expiry. Before each step the
server rechecks target absence or exact mapped-blueprint revision, hash, and
published state. Blocked, stale, or expired-before-start plans return `409`;
inconsistent persisted plan evidence returns `422 invalid_input`. Concurrent or
repeated apply requests converge on one application without duplicating
resources.

### Presentation assets

Only applied packs create presentation assets. The list and detail routes
return metadata but never object keys; the content route returns
integrity-verified bytes with server-owned content type/length, inline
disposition, digest ETag, `Cache-Control: private, no-store`, `nosniff`, and
restrictive CSP. Use `acli presentation-asset download <uuid> --output <path>`
for an authorized download.

Blueprint creation and revision routes create drafts. Only published revisions
can create entities or serve as migration targets. See [Blueprint Publication](database.md#blueprint-publication).

`POST /v1/entities/search` optionally accepts multiple relationship tree facets;
`POST /v1/entities/facets/relationship-tree/children` loads a facet page. See
[Relationship Tree Facets](search-facets.md) for their request and response
contract. Optional first-page totals are capped at 500 and set
`total_count_capped = true` when more results exist; keyset result pagination is
not capped.

### Scalar filters

Structured `filters.field` values may name a local scalar or a scalar leaf
through up to three relationship hops, for example
`family.product_type.name`. Operators are validated against the resolved leaf
type. A many-valued path matches when any reachable scalar satisfies the
criterion. Each hop uses the linked entity's pinned blueprint revision.

### Search table sorting

A search request may include `sort` when its `field` is a scalar column
configured in the selected blueprint's `views.table.columns` and `direction` is
`"asc"` or `"desc"`:

```json
{
  "blueprint": { "code": "product", "version": 3 },
  "sort": { "field": "category.name", "direction": "asc" },
  "page": { "size": 50 }
}
```

The built-in `publication_status` sort orders entities in one enabled
publication channel. Supply its context code as
`{"sort":{"field":"publication_status","direction":"asc","context_code":"web"}}`.
Ascending puts **not published** first (including entities without a channel
publication row); descending puts **published** first. Sorting compares approval
metadata for the specified channel, not an export snapshot. An omitted, unknown,
or disabled context returns `422`. The opaque cursor binds to the context, so
switching channels requires starting at the first page. The agent
`search_entities` tool accepts the same sort object.

The built-in `blueprint_version` field is also sortable without blueprint
configuration: `{"sort":{"field":"blueprint_version","direction":"asc"}}`
orders older revisions first when the blueprint version is omitted (all
versions). Descending orders newer revisions first. Entity ID breaks ties
within a version for stable keyset pagination. An explicit `blueprint.version`
restricts results to that revision before sorting.

The field may be local or a configured path with up to three relationship hops.
Every relationship hop must declare `cardinality = "one"`. A relationship-path
sort normally names an explicit published source version. When `version` is
omitted, it is accepted only if the complete matching result set contains one
source blueprint version; otherwise the API returns
`422 relationship_path_sort_requires_single_result_version`.

Search responses report `result_version_scope` as `empty`, `single` (with its
version), or `multiple`. This summary uses the complete query/filter/facet/tag
candidate set, not the current page. Sorted responses use an opaque keyset
`next_cursor`; return it unchanged as `page.cursor` with the same sort. The
cursor includes the effective source version, so a changed all-version result
scope rejects the stale cursor. An invalid direction, unconfigured field (other
than `blueprint_version`), or non-scalar column also returns `422`.

When an explicit current version is selected, first-page responses also return
`hidden_outdated_count` and `hidden_outdated_count_capped`. This count ignores
the active search predicates and describes older entities across the blueprint
family.

### Search table path projections

For every configured table column, each returned item has
`table_values[field_path]`, an array of scalar values. Relationship edges are
traversed in page-level batches and scalar leaves are read from the target
entities' preview projections. Each hop uses the linked entity's pinned
blueprint revision. A missing or incompatible segment yields an empty array;
many-valued paths may yield multiple values.

Direct `related[relationship]` previews remain available for compatibility with
custom one-hop cell renderers. Blueprint responses expose
`table_path_attributes` with each configured path's scalar `value_type` and
whether every hop is single-valued; the Explorer uses that metadata for filter
controls and sortable headers.

## Saved views and share links

All routes require an authenticated workspace principal with `entities.read`. `GET /saved-views` lists the current user's private and workspace-visible named views (up to 100). `GET /saved-views/{id}` reads a named view; `POST /saved-views` creates one; `PUT /saved-views/{id}` and `DELETE /saved-views/{id}` update/delete a view owned by the caller. Nonexistent or inaccessible views return 404.

`POST /view-state-links` creates or reuses an unnamed link snapshot. `GET /view-state-links/{id}` reads a snapshot for an authorized workspace member. A link is not anonymous access and does not authorize the subsequent entity search.

Creation and update payloads:

```json
{"kind":"explorer_search","name":"My assets","description":"Recently checked","visibility":"workspace","state":{"blueprint":"asset","attributeFilters":[{"field":"status","operator":"eq","value":"active"}]}}
```

Use `visibility: "private"` or `"workspace"` for named views. For a snapshot, omit `name`, `description` and `visibility` when posting to `/view-state-links`; send `kind` and `state`. Responses include `id`, `owner_user_id`, `kind`, `name`, `description`, `visibility`, `state`, `created_at`, `updated_at`. State uses the Explorer URL field names; see [saved views](saved-views.md) for semantics and limits.

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
  `catalog_file_purge_jobs_queued_total`;
- startup history cleanup: `catalog_value_history_cleanup_total` (bounded
  `outcome` of `success` or `failed`),
  `catalog_value_history_entries_purged_total`, and
  `catalog_value_history_cleanup_duration_seconds`.

Data-health cache decisions are exposed as `catalog_data_health_cache_total`
with a bounded `status` label. Scrape this endpoint from the private monitoring
network rather than exposing it publicly.

The API emits structured `tracing` events for startup, database migrations, each
HTTP request, file uploads, downloads, and worker jobs. Request spans include
the method, matched route template, response status, duration, and a validated
`x-request-id` UUID (also returned in the response and used for mutation audits).
File spans include only internal IDs and bounded operation values, never object
keys or filenames. 5xx responses are emitted at error level; repository, storage,
and mail failures also log their server-side cause without exposing it to clients.
Set `RUST_LOG` (for
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
reaches a repository write. Built-in tools include exact blueprint-revision
inspection and read-only entity migration assessment, plus approved replacement
or removal of relationship targets. Replacement sets the complete target list
for each specified attribute/context (an empty list clears it); removal unlinks
only named targets. The agent must inspect current values first. These tools
use the initiating user's `blueprints.read` or entity-scoped permissions:
`entities.write` for migration assessment and relationship changes, and
`entities.read` for existing entity inspection. Bounded `get_entity_changes`
and `get_value_history` tools support inspection before approval-gated
`remove_entity_values` and `restore_entity_value`. Writes use the same audited
mutation services as the API.

## Workflow run operations

`GET /workflow-runs` lists workspace-scoped run diagnostics and requires `workflows.read`. `POST /workflow-runs/{run_id}/replay` requeues only a terminal dead-letter run and requires `workflows.manage`. Neither endpoint exposes internal domain-event payloads.

## Background processing status

`GET /data-health/background-processing` requires `data_health.read` and returns
uncached, payload-free aggregates for the authenticated workspace's API task queue:

```json
[{"kind":"rule_run.v1","queued":2,"running":1,"failed":0,"expired_leases":0,"oldest_due_seconds":12.5}]
```

Only kinds with queued, leased, or dead-letter tasks appear; an empty queue returns
`[]`. `queued` includes future scheduled retries. `running` counts leased tasks;
`expired_leases` is the subset whose lease has expired, which can indicate interrupted
work awaiting recovery. `failed` counts terminal dead-letter tasks. Completed and
cancelled tasks are excluded. `oldest_due_seconds` measures time since the oldest
currently due queued task's `available_at`, or is `null` if none is due. No task
payloads, identifiers, or raw errors are returned. These counts do not establish
worker availability. The separate file-processing queue is not included.

The Manage **Background processing** page displays these metrics and refreshes
every 30 seconds, with a manual refresh option.
