# API Reference

Most API routes exchange JSON over HTTP; successful empty responses use `204`,
and file, asset, and metrics routes return their documented content types.
Failures return a JSON `error` object with a machine-readable code and message,
alongside the HTTP status; some add an `error.details` object (see
[Errors](#errors)). Use the [CLI](cli.md) for shell automation.

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

Permissions come from role grants on active workspace memberships:

- Blueprint reads, writes, and publishing require `blueprints.read`,
  `blueprints.write`, and `blueprints.publish`, respectively.
- Workflow reads and management require `workflows.read` and `workflows.manage`.
- Entity operations require `entities.read`, `entities.write`, `entities.delete`,
  or `entities.publish`.
- Context operations require `contexts.read` or `contexts.write`.
- Data-health, metrics, and event-delivery dead-letter inspection require
  `data_health.read`.
- Event-delivery replay and role management require `roles.manage`.
- Extension release discovery requires `extensions.read`; trusted registry
  source management requires `extensions.manage`.
- Starting an interactive extension run requires `entities.read` on every
  selected entity, evaluated with the caller's entity-, blueprint- and
  workspace-scoped grants. The run's later catalog reads, annotation writes and
  downloads recheck the initiator's current access; value writes require
  `entities.write` on the entity.
- Solution-pack archive inspection, planning, application, and history require
  `solution_packs.manage`. The fixed owner and administrator roles receive this
  permission during bootstrap.

A context-subtree grant applies to its root and descendants, never its ancestors
or siblings. Browser requests use the workspace stored in the verified session.
A client workspace header cannot select a different workspace.

`POST /auth/renew` atomically rotates the browser session, `POST /auth/logout`
revokes it, and login, renew, and `GET /auth/session` return the active user identity,
preferred `time_zone` (an IANA name, or `null` to follow the client),
session-bound `workspace_id`, and human-facing workspace `login_identifier`. The local
password, cookie, CSRF, expiry, and revocation contract is documented in
[Browser Authentication](authentication.md).

## Routes

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/health`, `/health/live` | Process liveness only; dependencies are deliberately not probed. |
| `GET` | `/health/ready` | Sanitized traffic-readiness check for PostgreSQL and required object storage. |
| `GET` | `/system/health` | Report the running API version and the source branch and commit it was built from (`data_health.read`). |
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
| `POST` | `/rules/{rule_id}/run-now` | Enqueue an idempotent bounded manual or dry run (`rules.manage`). Optional `version` selects a published revision; only dry runs may target a revision that is not enabled. |
| `POST` | `/rules/{rule_id}/versions/{version}/enable` | Enable a published revision (`rules.manage`). Optional body `{ "accept_existing_violations": true }`; enforcing revisions first need a completed full dry run. See [Rules](rules.md#dry-run-before-enabling). |
| `GET` | `/rule-runs`, `/rule-findings` | Read run diagnostics and active/resolved findings (`rules.read`). |
| `POST` | `/extensions/{extension_id}/{contribution_id}/command` | Validate a bounded, manifest-declared client-mediated extension command against the enabled exact release and grants (`entities.write`). |
| `POST` | `/extensions/{extension_id}/{contribution_id}/operations` | Start an interactive extension operation from a selection-aware contribution for the signed-in user. The body is `{release_id, operation_id, input, idempotency_key, selection: {blueprint_id, blueprint_version, context_id, entity_ids}}`; every entity must be saved, belong to the one revision, and be readable by the caller, otherwise the whole request is rejected. A retried identical request returns the same `run_id`; reusing the key with different input or selection returns `409 idempotency_key_reused`. |
| `GET` | `/extension-runs` | List the signed-in user's 50 most recent interactive extension runs, optionally filtered by `extension_id`. Runs with a selected entity the user can no longer read are omitted. Inputs, configuration and checkpoints are never returned. Every run response, here and on the routes below, includes `initiated_by_me`, which is `true` when the signed-in user started the run. |
| `GET`; `POST` | `/extension-runs/{run_id}`; `/extension-runs/{run_id}/cancel` | Read or cancel (`204`) one interactive run. Reading is available to the initiator while they can still read every selected entity, and to `extensions.manage` operators; the initiator can always cancel their own run. Other users receive `404`. With `?scope=own`, which extension frames always send, an operator gets no wider access: the request addresses only runs the caller started. |
| `GET` | `/extension-runs/{run_id}/artifacts/{artifact_id}/download` | Download a finalized output of a completed, unexpired interactive run, with the same access rule as the run, including `?scope=own`. Outputs are attachments named by the extension and expire 30 days after completion. |
| `GET`, `POST` | `/extensions/{extension_id}/annotation-namespace` | Inventory an extension's annotation namespace, or explicitly adopt pre-existing annotations under that name for an installed extension (`extensions.read` / `extensions.manage`). |
| `POST` | `/extensions/{extension_id}/annotation-namespace/entities/{entity_id}` | Operator repair or cleanup of one claimed namespace on an entity using the extension annotation patch shape (`extensions.manage` and `entities.write` on the entity). |
| `POST` | `/auth/discover` | Resolve a normalized workspace identifier and return its sign-in methods; rate-limited and intentionally minimal. |
| `POST` | `/auth/login` | Sign in with a previously resolved workspace identifier, email, and password. |
| `POST` | `/auth/password-reset` | Request a password-reset message for a local account. |
| `POST` | `/auth/password-reset/confirm` | Consume a password-reset secret and set a new password. |
| `PATCH` | `/auth/preferences` | Replace the authenticated user's display preferences, `{ "time_zone": "Europe/Warsaw" \| null }`, and return the updated session payload. Unknown IANA zone names return `422`. Use `acli auth preferences`. |
| `PUT` | `/auth/display-name` | Change the authenticated user's display name, `{ "display_name": "Ada Lovelace" }`, and return the updated session payload. Names are 2–64 letters, digits, and spaces with no leading or trailing space; others return `422`. Use `acli auth display-name`. |
| `PUT`, `DELETE` | `/auth/avatar` | Upload (multipart `file`, PNG or JPEG) or remove the caller's avatar in the active workspace. Upload returns `201` with `{ "file_id", "status" }`; the file worker produces the square `avatar` variant. See [Avatars](authentication.md#avatars). |
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
| `POST` | `/workspace/roles/{role_id}/retire` | Retire a custom role, optionally replacing its grants. `owner` is accepted as the replacement only for an active owner and only when every reassigned grant is workspace-scoped. |
| `GET` | `/workspace/permissions` | List permissions available for custom roles (`roles.manage`). |
| `GET` | `/workspace/assignable-roles` | List roles available to member and invitation management (`members.manage`). |
| `GET` | `/workspace/token-permissions` | List the caller's permissions available to personal-token management (`tokens.manage`). |
| `GET` | `/workspace/grant-targets/{scope_type}` | List workspace-owned grant targets for a scope (`members.manage`); ownership is checked again when granting. |
| `GET` | `/workspace/members` | List members and additive grants (`members.manage`). |
| `PUT` | `/workspace/members/{member_id}` | Set member state to `active` or `inactive`. |
| `POST` | `/workspace/members/{member_id}/grants` | Add a role grant at one requested scope. |
| `DELETE` | `/workspace/members/{member_id}/grants/{grant_id}` | Revoke a role grant. |
| `POST` | `/workspace/members/{member_id}/transfer-ownership` | Transfer ownership to an active member (owner only). |
| `GET` | `/directory` | Users and teams that [assignment attributes](blueprints.md#user-or-team-assignments) can reference (`entities.read`). |
| `GET`, `POST` | `/workspace/teams` | List or create [teams](#teams) (`members.manage`). |
| `PATCH`, `DELETE` | `/workspace/teams/{team_id}` | Rename a team or replace its members; delete it (`members.manage`). |
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
| `PUT`, `DELETE` | `/contexts/id/{id}` | Update or delete a context. Deletion is rejected while the context has values, child contexts, or a pending or running interactive extension run. |
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
| `POST` | `/v1/entities/batch` | Apply create, update, and delete operations to several entities in one transaction; see [Entity batches](#entity-batches). |
| `GET`, `PUT` | `/v1/entities/{id}` | Read or update an entity form atomically, including optional system annotations. |
| `POST` | `/v1/entities/{id}/blueprint-migration/preview` | Assess migration to the highest published revision. |
| `POST` | `/v1/entities/{id}/blueprint-migration` | Migrate an entity to that revision. |
| `GET`, `POST` | `/v1/entities/{id}/publications` | List channel publication status or publish to `{ "context_id": "…" }`. |
| `POST` | `/v1/entities/{id}/publications/unpublish` | Unpublish from `{ "context_id": "…" }`. |
| `POST` | `/v1/entities/{id}/publications/publish-all` | Publish atomically to every enabled channel. |
| `GET` | `/v1/entities/{id}/status-transitions` | Declared status edges from the saved status in `?context_id=`, whether the caller may take each, and `unmet` transition conditions and enforcing rules (`denial_code` `transition_conditions_unmet`). See [status control](status-control.md#controlled-records). |
| `GET` | `/v1/entities/{id}/approvals` | Approval decisions with content digests and void reasons. |
| `GET` | `/v1/entities/{id}/retention-holds` | Retention holds on the entity's files. |
| `GET`, `POST` | `/files/{id}/retention-holds` | List holds, or place an explicit hold (`files.hold`). |
| `POST` | `/files/{id}/retention-holds/{hold_id}/release` | Release an explicit hold early (`files.hold`). |
| `GET` | `/v1/entities/{id}/publications/readiness` | Evaluate each enabled channel's required checks without publishing: `[{ "context_id", "context_code", "ready", "violations" }]`. |
| `GET`, `PUT` | `/publication-channels`, `/publication-channels/{context_id}` | List channel contexts or update one with `{ "enabled": true, "required_rule_codes": ["has-sku"], "require_valid_entity": true }`. The two check fields are optional; omitting one keeps its current value. |
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

### Publication channel checks

A channel can require checks before an entity is published to it:

- `required_rule_codes` (at most 32 unique codes): enabled rules of the
  entity's blueprint revision with those codes, evaluated live in the channel
  context. Rules scoped to another context do not apply; any predicate is
  allowed. Every code must name a rule of the workspace (any revision or
  lifecycle state); an unknown, invalid or repeated code returns
  `422 invalid_input`, so a typo cannot silently disable the gate.
- `require_valid_entity`: re-runs the entity JSON schema (at most 10 errors,
  `source = "entity_schema"`, `code = "entity_schema"`) and `x-attricat-checks`
  in the channel context, which catches inherited or date-dependent failures
  such as expiry.

Publishing to one channel, to all channels, or blueprint bulk publication
returns `422 publication_checks_failed` with `details.context` (the channel's
context code) and `details.violations` (see [Errors](#errors)). A bulk request is rejected as a whole
and each violation's `evidence.entity_id` names the failing entity. Use the
readiness route to check beforehand.

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
summaries, normalized presentation-asset digests, extension requirement
summaries, and a `seeds` summary of prerequisite packs, contexts with their
publication channels, rules and workflows (with their declared enabled state),
and saved searches. `sample_data.file_count` counts bundled sample files.
Inspection never returns resource bytes, archive paths, or private object keys.

`POST /solution-packs/plans` requires `prefix` and
`blueprint_publication=draft|publish` query parameters. Optional
`include_sample_data=true` explicitly selects synthetic sample entities;
omitting it skips them. A `from_application=<uuid>` query parameter selects
one completed application of the same pack in the same workspace, at a lower
SemVer release. It cannot be combined with explicit maps. Catalog does not
search history or suggest a mapping. The planner creates resources for added
keys and reuses unchanged, exactly matching published targets. It blocks changed
definitions and reports conflicts for missing, unpublished, revision-drifted, or
hash-drifted targets. Removed keys are recorded without deleting resources.

Without explicit maps, upload the archive as `application/zstd`, including
when using `from_application`. For explicit reuse, send `multipart/form-data`
with exactly one streamed `archive` part (`application/zstd`) and repeated
`blueprint_map` JSON text parts such as
`{"key":"blueprints/product","code":"shared_product"}`, `asset_map`
parts such as `{"key":"assets/brand-logo","id":"<uuid>"}`, and/or
`context_map` parts such as `{"key":"contexts/poland","code":"PL"}` selecting an
existing context for a pack context. The 32 MiB
compressed archive and structural limits still apply. The archive is not
retained; asset-create actions privately stage normalized bytes before the
plan is ready, and bundled sample files of a sample-selected plan are uploaded
to ordinary file storage under upload intents before the plan is saved.

Plans can also contain `prerequisite`, `context`, `publication_channel`,
`rule`, `workflow`, and `saved_search` actions. Prerequisites are resolved
against completed applications of the required pack in the workspace; they are
never installed automatically. See
[solution-pack operation](solution-packs.md#prerequisite-packs).

The response is at most 1 MiB. It contains safe source/digest metadata,
optional prior-application identity, ordered release-change evidence, mapping
and action summaries, extension requirement outcomes, reasons, preconditions,
readiness, and a fixed 24-hour expiry. A requirement with status `install` also
carries `install`: the official release's `version`, `repository`, `tag_name`,
and the `grants` apply will give it. Planning returns `503` when the official
extension registry is unreachable and `422` when a release in range is not the
required extension or does not accept the pack's configuration template. It never includes blueprint definitions,
normalized resource payloads, configuration templates or installed values,
archive paths, staged bytes, or object keys. See [solution-pack operation](solution-packs.md#apply-and-verify)
for apply-time behavior.

`POST /solution-packs/plans/{plan-id}/apply` starts only a ready, unexpired plan,
or resumes its existing application after plan expiry. It first installs,
configures, grants, and enables the plan's pinned official extension releases,
and returns `409 solution_pack_plan_stale` if a release changed since review. Before each step the
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

For a [user or team assignment](blueprints.md#user-or-team-assignments)
attribute, `{"operator":"eq","value":"@me"}` matches entities assigned to the
caller or to any team the caller belongs to (`@me` is resolved per request, so
saved searches keep it literally). Other values match the stored
`user:<uuid>` / `team:<uuid>` reference exactly.

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

## Lexicon

The workspace lexicon translates `{{key}}` / `{{key|context}}` references in
catalog labels (see [translated labels](blueprints.md#translated-labels)).

| Method | Route | Description |
| --- | --- | --- |
| `GET` | `/lexicon/entries?language=` | List entries, optionally for one language (`entities.read`; the web app loads it into its `lexicon` i18next namespace). |
| `PUT` | `/lexicon/entries` | Upsert `{"key","context"?,"language","plural_category"?,"text"}` (`blueprints.write`). Takes over a solution-pack entry. |
| `DELETE` | `/lexicon/entries?key=&context=&language=&plural_category=` | Delete one entry; `404` when absent (`blueprints.write`). |
| `GET` | `/lexicon/export?language=` | One language as an import file (`blueprints.read`). |
| `POST` | `/lexicon/import?mode=merge\|replace` | Import a file (`contracts/lexicon-v1.schema.json`, up to 10,000 entries); returns `created`, `updated`, `unchanged`, `deleted` (`blueprints.write`). |
| `GET` | `/lexicon/report?languages=pl,de` | Coverage report (`blueprints.read`). |

Keys and contexts are trimmed and whitespace-collapsed, and must not contain
braces, `|`, or control characters. Languages are BCP 47 tags whose primary
language has known CLDR plural rules; tags are canonicalized (`pt-br` →
`pt-BR`). `plural_category` defaults to `other` and must be used by the
language. Entries return `key`, `context` (`null` for none), `language`,
`plural_category`, `text`, `source` (`workspace` or `solution_pack`),
`solution_pack_id`, and `updated_at`.

The report returns `reference_count`, per-language `translated_count`,
`untranslated` references (never for `en`), and `missing_plural_categories`
(blueprint names and any entry with plural forms), plus `orphaned` keys no
non-deleted blueprint revision or reusable attribute name references.

## Teams

Teams are named groups of workspace members that assignment attributes can
reference as `team:<id>`. `POST /workspace/teams` takes
`{"code","name","member_user_ids"?}`; `PATCH /workspace/teams/{id}` takes
`{"name"?,"member_user_ids"?}` and replaces the whole member list when given.
Codes are unique among non-deleted teams, use `A-Z a-z 0-9 _ -` (at most 128
characters) and cannot change; names are 1–200 characters. Members must be
workspace members. A workspace has at most 1,000 teams of up to 1,000 members.
Responses return `id`, `code`, `name`, `member_user_ids`, `created_at` and
`updated_at`. Deleting a team removes its memberships but keeps the row:
existing assignments still resolve its name through `GET /directory`
(`deleted: true`), and new assignments to it are rejected.

`GET /directory` returns `{"users":[{"id","display_name","email","active"}],
"teams":[{"id","code","name","deleted"}]}`. It includes inactive members and
deleted teams so existing assignments render; only active members and
non-deleted teams can be newly assigned.

## Saved views and share links

All routes require an authenticated workspace principal with `entities.read`.
`GET /saved-views` lists up to 100 of the current user's private and
workspace-visible named views. The optional `q` parameter accepts up to 120
characters and matches names or descriptions, ignoring case.

`GET /saved-views/{id}` reads a named view; `POST /saved-views` creates one.
`PUT /saved-views/{id}` and `DELETE /saved-views/{id}` update or delete a view
owned by the caller. Nonexistent or inaccessible views return 404.

`POST /view-state-links` creates or reuses an unnamed link snapshot. `GET /view-state-links/{id}` reads a snapshot for an authorized workspace member. A link is not anonymous access and does not authorize the subsequent entity search.

Creation and update payloads:

```json
{"kind":"explorer_search","name":"My assets","description":"Recently checked","visibility":"workspace","state":{"blueprint":"asset","attributeFilters":[{"field":"status","operator":"eq","value":"active"}]}}
```

Use `visibility: "private"` or `"workspace"` for named views. For a snapshot, omit `name`, `description` and `visibility` when posting to `/view-state-links`; send `kind` and `state`. Responses include `id`, `owner_user_id`, `kind`, `name`, `description`, `visibility`, `state`, `created_at`, `updated_at`. State uses the Explorer URL field names; see [saved views](saved-views.md) for semantics and limits.

## Entity batches

`POST /v1/entities/batch` applies writes, status transitions, and deletions to
several entities atomically: either every operation commits, with its audit
event and domain event, or nothing does.

```json
{
  "operations": [
    {
      "op": "create",
      "entity_id": "5b0b8c55-0c55-4cc5-9a0f-4a4c3d1a2b10",
      "blueprint": { "code": "document_revision" },
      "values": [
        { "kind": "scalar", "attribute_code": "label", "context_id": null, "value": "B" },
        { "kind": "scalar", "attribute_code": "status", "context_id": null, "value": "released" },
        { "kind": "relationship", "attribute_code": "previous", "context_id": null,
          "target_entity_id": "1f7e2d9a-6a3e-4a8a-9d0c-2f8d4f7f9e11" }
      ]
    },
    {
      "op": "update",
      "entity_id": "1f7e2d9a-6a3e-4a8a-9d0c-2f8d4f7f9e11",
      "expected_updated_at": "2026-10-01T09:30:00Z",
      "values": [
        { "kind": "scalar", "attribute_code": "status", "context_id": null, "value": "superseded" }
      ]
    }
  ]
}
```

- `create` takes the `POST /v1/entities` fields plus an optional
  caller-chosen `entity_id`, so later operations can link to the new entity.
  An existing ID returns `409 entity_id_taken`.
- `update` takes the `PUT /v1/entities/{id}` fields (`values`,
  `relationships`, `remove_values`, `system_tags`, `system_metadata`, and
  `expected_updated_at`). Status transitions are ordinary values and keep their
  rules, including the `expected_updated_at` requirement.
- `delete` takes `entity_id` and an optional `expected_updated_at`; a stale
  value returns `409 stale_entity`.
- Operations run in order, each seeing the earlier ones, and each validates as
  its single-entity endpoint does when it runs: types, schemas, statuses,
  relationship targets and cardinality, unique keys, and hierarchies. Order
  operations so each is valid at its turn, for example release a unique value
  before reusing it.
- A batch has 1–50 operations and at most 1,000 values, relationship targets,
  and removals. Each entity appears in at most one operation; combine its
  changes. Violations return `422 invalid_input`.
- Every operation is authorized before anything runs: `entities.write` on the
  entity for updates, `entities.delete` for deletes, and workspace
  `entities.write` for creates. A personal API token needs each of those
  permissions. Any denial returns `403` for the whole batch.

The response is `200` with one result per operation, in order:

```json
{"operations": [{"op": "create", "entity": {}}, {"op": "update", "entity": {}}, {"op": "delete", "entity_id": "…"}]}
```

When an operation fails, the transaction rolls back and the response keeps
that operation's status and error code. The message starts with
`operation <index>:` and `error.details` adds `operation_index` and
`entity_id` to the operation's own details. Each operation's audit event
targets its entity and records `metadata.batch` with `operation_index` and
`operation_count`; all share the request and correlation IDs.

## Errors

Every failure returns `{ "error": { "code", "message", "details"? } }` with the
HTTP status. Switch on `code`; `message` is for people and may change. Codes
and statuses for write failures follow one rule:

- `409` means the current state of other data blocks the write: another entity
  holds the key, the record is locked, a revision changed, or existing data
  violates a constraint you are enabling. Change or inspect that other data.
- `422` means the submitted content itself fails validation or declared
  checks. Change the request.

`403` refuses the actor (permissions, transition roles, separation of duties)
and `428` asks for an optimistic-concurrency precondition.

`error.details` is present only for the codes below. Check failures use
`details.violations[]`; every other code uses a flat object. Agent tool
results report the same `code`, `message` and `details`.

| Status | Code | `error.details` |
| --- | --- | --- |
| `422` | `entity_check_failed`, `transition_conditions_unmet`, `rule_violation` | `violations` (see below) |
| `422` | `publication_checks_failed` | `violations` and `context` (the channel's context code) |
| `422` | `attribute_value_schema_mismatch` | `attribute`, `instance_path` (JSON Pointer of the failing value) |
| `422` | `entity_schema_mismatch` | `context`, `instance_path` |
| `422` | `relationship_target_type_mismatch` | none; the target's blueprint is not in the attribute's `target_blueprint_codes` |
| `428` | `status_precondition_required` | none; resend with `expected_updated_at` |
| `403` | `status_transition_forbidden` | `attribute`, `context`, `from`, `to`, `reason` |
| `403` | `status_separation_of_duties` | `attribute`, `context`, `edge` (the earlier transition made by this user) |
| `409` | `record_locked` | `attribute`, `context`, `status` (the locking status) |
| `409` | `unique_key_conflict` | `key`, `context`, normalized `values`, `conflicting_entity_id` |
| `409` | `unique_key_duplicates` | `duplicates` (up to 20 `{ key, context, values, entity_ids }`) and `total`; returned by blueprint publication |
| `409` | `relationship_cycle` | `attribute`, and `path`: entity IDs from the written entity back to it |
| `409` | `relationship_hierarchy_violations` | `attribute`, `cycles`, `multiple_parents`; returned by blueprint publication |
| `409` | `relationship_cardinality_conflict` | `attribute`, `context_id`, `source_entity_id`, `target_entity_id`, `conflicting_source_entity_id` |
| `409` | `entity_id_taken` | `entity_id` |
| `409` | `annotation_revision_conflict` | `expected`, `actual` revisions |
| `409` | `rule_dry_run_required` | none; run `run-now` with `"dry_run": true` and the `version` first |
| `409` | `rule_has_existing_violations` | `existing_violations` (a count); fix the entities or pass `accept_existing_violations` |

A failed [entity batch](#entity-batches) operation keeps that operation's
status, code and details, and adds `operation_index` and `entity_id`.

The `violations` shape (at most 50 items with `source`, `code`, `message`,
`contexts`, `attributes`, and `severity`, `transition` and `evidence` where
they apply) is defined in
[JSON Schema Validation](json-schema-validation.md#error-details). For a
blocked status change, read the status-transitions route.

Other codes carry no details. Common ones are `invalid_input`, `bad_request`,
`not_found`, `forbidden`, `unauthenticated`, `conflict` (a code or unique value
is already in use), `stale_entity`, `idempotency_key_reused`,
`payload_too_large`, `rate_limited`, `storage_unavailable` and
`internal_error`.

## Entity system annotations

Entities include `system_tags` (an array of unique, non-empty strings) and
`system_metadata` (a JSON object, up to 64 KiB). These fields are
outside the versioned blueprint and EAV value model. Operators and automation
can store workflow markers and diagnostic data there without changing the
entity's schema. They are returned with entity reads and form responses, but never added
to projections or views.

`POST /v1/entities` accepts both fields; omitted values default to `[]` and
`{}`. `PUT /v1/entities/{id}` accepts either field independently; omitted fields
are retained, while `[]` or `{}` clears the corresponding value. Search accepts
`system_tags`; returned entities must contain every supplied tag, making it
suitable for finding a marked batch before applying a bulk workflow.

### Extension annotation namespaces

An extension owns the tags named `<extension-id>:<tag>` and the object at
`system_metadata[<extension-id>]` once its namespace is claimed. A namespace is
claimed by the extension's first annotation write, or by an operator adopting
existing data through `POST /extensions/{extension_id}/annotation-namespace`.
Claims survive disable, upgrade, and removal; annotations are preserved unless
an operator removes them through the repair route.

Every other write path treats claimed namespaces as read-only: entity create,
`PUT /v1/entities/{id}`, duplicate, workflow tag and metadata actions, and the
legacy extension `create`/`upsert` fields. A request that changes a claimed
namespace returns `409 protected_annotation_namespace`; edits that send the
namespace back unchanged, and edits of unrelated tags or keys, still succeed.
Duplicated entities do not copy claimed namespaces. Extension annotation writes
do not change the entity's `updated_at`.

The patch shape used by extensions and the repair route is:

```json
{"add_tags": ["generated"], "remove_tags": [], "set_metadata": {"last": {"run": "…"}}, "remove_metadata": [], "expected_revision": 3}
```

Names are local (1–64 ASCII letters, digits, `.`, `_`, `-`); an added tag must
also fit the 128-byte system-tag limit once qualified as `<extension-id>:<tag>`.
A patch has 1–32
operations, and the same tag or key cannot appear in two operations. Setting a
key replaces its whole value, so JSON `null` is a valid value. The optional
`expected_revision` rejects stale writes with `409 annotation_revision_conflict`;
the namespace revision starts at 0 and increases with every changing patch.
Each change is audited and emits `entity.annotations_changed.v1`. The repair
route may also remove legacy tags and keys that do not follow the local-name
rules, and replaces a namespace value that is not an object; the replaced value
is kept in the audit event.

## File uploads and downloads

Upload with `multipart/form-data`. Use `file` or `files` for every binary part
and an optional single `context_id` text part. The API streams parts to a
temporary file rather than buffering the whole request. It validates the
content signature, declared MIME type, extension, request limits, and the
pinned file-attribute policy before persisting metadata and queuing processing.
A successful response is `201` with the attribute, context, and safe file
metadata; originals and storage keys are never returned.

A newly accepted file has `status: "queued"`. Image files are processed into
`thumbnail` and `display` WebP variants (avatar files get a single square
`avatar` variant instead); non-image files become `ready` without
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
shows these values in **Network → Timing** for each request.

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
- periodic history cleanup: `catalog_value_history_cleanup_total` (bounded
  `outcome` of `success`, `failed`, or `budget_exhausted`),
  `catalog_value_history_entries_purged_total`, and
  `catalog_value_history_cleanup_duration_seconds`;
- abandoned-upload cleanup: `catalog_upload_cleanup_total` (`outcome` of
  `success` or `failed`). Failed deletions retain their durable intent for retry.

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
run and returns `202`. The API worker executes the run independently of the
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
only named targets. `apply_entity_batch` proposes several entity operations
as one approval and applies them through [entity batches](#entity-batches);
each operation is authorized for the initiating user when the approved call
runs. `get_entity_publication_readiness` reports, per channel, whether an
entity passes the channel's required checks. The agent must inspect current values first. These tools
use the initiating user's `blueprints.read` or entity-scoped permissions:
`entities.write` for migration assessment and relationship changes, and
`entities.read` for existing entity inspection. Bounded `get_entity_changes`
and `get_value_history` tools support inspection before approval-gated
`remove_entity_values` and `restore_entity_value`. Read-only operational tools
also provide a data-health summary, paged rule findings (excluding raw evidence),
and paged workflow-run statuses (excluding event payloads and error bodies).
Exact rule/workflow definition reads, paged rule-run summaries, and targeted
workflow-run summaries provide follow-up context without compiled plans,
internal cursors, trigger payloads, or error bodies. They require
`data_health.read`, `rules.read`, and `workflows.read` respectively; no rule or
workflow management action is exposed to the agent. The agent may inspect a
context by ID, then propose an approved parent/data replacement or deletion;
it can also propose approved entity system-tag/metadata updates. Omitted
annotation fields remain unchanged, and context deletion is rejected when the
context is in use. All writes use the same audited mutation services as the API. Read-only
extension-operation and blueprint connector-job tools require
`extensions.manage`; run lists are paged and omit inputs, checkpoints, progress
objects, and internal storage references. No operation start, replay, cancel,
or schedule mutation is exposed to agents.

## Workflow run operations

`GET /workflow-runs` lists workspace-scoped run diagnostics and requires `workflows.read`. `GET /workflow-runs/{run_id}/targets` (also `workflows.read`) lists the per-entity outcomes of a run's `referencing_entities_update` actions: `action_index`, `entity_id`, `status` (`completed`, `failed`, or `skipped`), `attempts`, the latest bounded `last_error`, and timestamps; an unknown run returns `404`. `POST /workflow-runs/{run_id}/replay` requeues only a terminal dead-letter run and requires `workflows.manage`. Neither endpoint exposes internal domain-event payloads.

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
