---
title: API reference
description: How to authenticate against the Attricat HTTP API, its conventions, and its routes grouped by area.
---

Attricat's web app is built on the same HTTP API you can call from scripts and integrations. The [CLI](/reference/cli/) wraps nearly all of it.

## Base URL

When the API serves the web app (as the container image does), API routes are available both at their own paths and below `/api`. For example, `GET https://catalog.example.com/api/blueprints` and `GET https://catalog.example.com/blueprints` are the same route. Integrations should use the `/api` prefix.

## Authentication

**Personal API token**: send `Authorization: Bearer cat_pat_…`. The token decides the workspace. Tokens are the recommended way to integrate.

**Browser session**: `POST /auth/login` sets an HttpOnly `catalog_session` cookie and a readable `catalog_csrf` cookie. Every unsafe request (anything but `GET`, `HEAD`, `OPTIONS`) made with the cookie must also send `X-Catalog-Csrf` with the CSRF cookie's value.

Clients never send a workspace ID; it always comes from the token or session.

Public routes that need no credentials: `/health`, `/health/live`, `/health/ready`, `POST /auth/discover`, `POST /auth/login`, `POST /auth/password-reset`, `POST /auth/password-reset/confirm`, and `POST /onboarding/complete`.

## Conventions

- Requests and responses are JSON unless a route says otherwise. Successful empty responses are `204`.
- Errors return the HTTP status and a body like `{"error": {"code": "entity_schema_mismatch", "message": "…"}}`. Match on `code`, not on the message.
- `401` means no valid credential; `403` means the credential lacks the permission or scope.
- `422` means the request was understood but is invalid, such as a blueprint that does not compile or a value that fails its schema.
- `409` means a conflict with current state, such as a relationship cardinality limit.
- Each response carries an `x-request-id`. You can send your own UUID in that header; it appears in the audit log.
- Every response has a `Server-Timing` header with the total server time.

## Common error codes

| Code | Status | Meaning |
| --- | --- | --- |
| `invalid_blueprint_definition` | 422 | The blueprint TOML does not compile. The message says why. |
| `attribute_value_schema_mismatch` | 422 | A value fails its attribute's `value_schema`. |
| `entity_schema_mismatch` | 422 | The entity fails its `entity_schema` in some context. |
| `relationship_cardinality_conflict` | 409 | A relationship write exceeds `cardinality` or `target_cardinality`. |
| `relationship_path_sort_requires_single_result_version` | 422 | Sorting by a related value across several blueprint revisions. |
| `file_processing` | 409 | The file is not ready to download yet. |
| `approval_already_decided` | 409 | An agent tool call was already approved or rejected. |
| `service_unavailable` | 503 | For agent routes: no AI provider is configured. |

## Routes

### Blueprints

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/blueprints` | Published entity blueprints. |
| `GET` | `/blueprints/catalogue` | All blueprint families and revisions. |
| `POST` | `/blueprints` | Create a blueprint (first draft) from `{"definition": "<toml>"}`. |
| `GET`, `POST` | `/blueprints/{id}/versions` | List revisions, or create the next draft. |
| `GET` | `/blueprints/{id}` | Current published revision. |
| `GET` | `/blueprints/{id}/versions/{version}` | Exact revision, including drafts. |
| `POST` | `/blueprints/{id}/versions/{version}/publish` | Publish a draft. |
| `GET` | `/blueprints/by-code/{code}` and `/blueprints/by-code/{code}/versions/{version}` | Look up by code. |
| `GET` | `/blueprints/{id}/migration-batches` | Background migration batches. |
| `GET` | `/blueprints/{id}/connector-jobs` | Connector jobs. |
| `POST` | `/blueprint-connector-jobs/{id}/run` | Run a connector job, with `{"idempotency_key": "…"}`. |
| `GET`, `POST` | `/reusable-attributes`; `POST /reusable-attributes/{id}/versions`; `POST /reusable-attribute-revisions/{id}/publish`; `GET`, `POST /reusable-attribute-groups` | Reusable attributes. |

### Entities

| Method | Path | Description |
| --- | --- | --- |
| `POST` | `/v1/entities` | Create an entity with values and optional `system_tags` and `system_metadata`. |
| `GET`, `PUT` | `/v1/entities/{id}` | Read or update an entity's form: values, relationships, removals, annotations. |
| `GET`, `DELETE` | `/entities/{id}` | Read or delete an entity. |
| `POST` | `/v1/entities/search` | Search. See below. |
| `POST` | `/v1/entities/facets/relationship-tree/children` | One page of a relationship facet's children, with counts. |
| `GET` | `/entities/{id}/preview` | Values per context, with related entities inline. |
| `GET` | `/entities/{id}/resolved-preview?context_id=…` | Values resolved in one context, with the context each came from. |
| `GET` | `/entities/{id}/hierarchy` | Ancestors along a relationship. |
| `POST` | `/v1/entities/{id}/incoming-relationships` | Entities linking to this one. |
| `GET` | `/entities` | Browse relationship targets. |
| `GET` | `/entities/{id}/values/current` | Current direct values and links. |
| `POST` | `/entities/{id}/values` | Append values. |
| `POST` | `/entities/{id}/relationships/replace`, `/remove` | Replace or remove relationship targets. |
| `GET` | `/entities/{id}/changes` | Change history. Add `limit` (1 to 50) and `offset` for pages. |
| `GET` | `/entities/{id}/values/history` | Value history. Same paging. |
| `POST` | `/entities/{id}/values/history/{history_id}/restore` | Restore an earlier value. |
| `POST` | `/v1/entities/{id}/blueprint-migration/preview` | Check migration to the current revision. |
| `POST` | `/v1/entities/{id}/blueprint-migration` | Migrate. |
| `POST` | `/v1/entities/{id}/reusable-attributes`, `/v1/entities/{id}/reusable-attribute-groups/{group_id}` | Attach a reusable attribute or group. |

### Search

```json
POST /api/v1/entities/search
{
  "blueprint": { "code": "product", "version": 3 },
  "query": "colors.name:red linen",
  "filters": [{ "field": "price", "operator": "lt", "value": 50 }],
  "relationship_tree_facets": [{
    "source_relationship_field": "categories",
    "hierarchy_field": "parent",
    "context_id": "00000000-0000-4000-8000-000000000001",
    "selected_target_ids": ["e8b7a8d3-c954-4c0f-b658-0f686ba466a3"]
  }],
  "system_tags": ["needs-review"],
  "sort": { "field": "title", "direction": "asc" },
  "page": { "size": 50, "cursor": null }
}
```

- Omit `blueprint.version` to search all published revisions.
- `query` uses the [search syntax](/guides/search-syntax/).
- Filter operators are `eq`, `contains`, `starts_with`, `gt`, `gte`, `lt`, and `lte`. `field` can be a relationship path of up to three hops.
- `sort.field` must be a scalar column in the blueprint's table view, `blueprint_version`, or `publication_status` (with `context_code` naming a channel).
- Responses contain `items`, `next_cursor`, `result_version_scope`, and, for each item, `table_values` and `match_explanations`. Pass `next_cursor` back as `page.cursor` with the same sort.
- `include_total` returns a first-page total, capped at 500.

### Files

| Method | Path | Description |
| --- | --- | --- |
| `POST` | `/entities/{entity_id}/file-attributes/{attribute_code}/uploads` | Upload with `multipart/form-data`: one or more `files` parts and an optional `context_id` part. Returns `201`. |
| `GET` | `/files/{file_id}` | Metadata and processing status. |
| `GET` | `/files/{file_id}/download` | Original file. Supports one `Range`. |
| `GET` | `/files/{file_id}/variants/{kind}/download` | `thumbnail` or `display` variant. |

Downloads return `409 file_processing` until the file is `ready`.

### Contexts and publication

| Method | Path | Description |
| --- | --- | --- |
| `GET`, `POST` | `/contexts` | List or create contexts. |
| `GET` | `/contexts/{code}` | Read by code. |
| `PUT`, `DELETE` | `/contexts/id/{id}` | Update or delete. |
| `GET` | `/publication-channels` | Channel contexts. |
| `PUT` | `/publication-channels/{context_id}` | `{"enabled": true}` to make a context a channel. |
| `GET`, `POST` | `/v1/entities/{id}/publications` | Publication status, or publish with `{"context_id": "…"}`. |
| `POST` | `/v1/entities/{id}/publications/unpublish` | Unpublish from one channel. |
| `POST` | `/v1/entities/{id}/publications/publish-all` | Publish to every channel. |
| `POST` | `/blueprints/{id}/versions/{version}/entity-publications`, `…/publish-all` | Publish all entities of a revision. |

### Saved searches

| Method | Path | Description |
| --- | --- | --- |
| `GET`, `POST` | `/saved-views` | List (optional `q`) or create named searches. |
| `GET`, `PUT`, `DELETE` | `/saved-views/{id}` | Read, update, or delete your own. |
| `POST`, `GET` | `/view-state-links`, `/view-state-links/{id}` | Create or read a shareable snapshot. |

State is at most 32 KiB and uses the Explorer URL keys: `blueprint`, `version`, `allVersions`, `query`, `context`, `locked`, `sort`, `attributeFilters`, and `relationshipFacets`.

### Translations

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/lexicon/entries` | List entries, optionally for one `language` (`entities.read`). |
| `PUT` | `/lexicon/entries` | Create or replace an entry: `key`, optional `context`, `language`, optional `plural_category` (default `other`), and `text` (`blueprints.write`). |
| `DELETE` | `/lexicon/entries` | Delete the entry identified by the `key`, `context`, `language`, and `plural_category` query parameters (`blueprints.write`). |
| `GET` | `/lexicon/export` | Export one `language` as an import file (`blueprints.read`). |
| `POST` | `/lexicon/import` | Import a file for one language; `mode=replace` also deletes entries missing from it (`blueprints.write`). |
| `GET` | `/lexicon/report` | Untranslated references, missing plural categories, and orphaned entries for comma-separated `languages` (`blueprints.read`). |

See [Translate labels](/builders/translations/).

### Rules and workflows

| Method | Path | Description |
| --- | --- | --- |
| `POST` | `/rules/validate` | Validate rule TOML. |
| `GET`, `POST` | `/rules` | List or create rules. |
| `GET` | `/rules/{id}` | Read a rule. |
| `POST` | `/rules/{id}/versions/{version}/publish`, `/enable`; `/rules/{id}/disable` | Lifecycle. |
| `POST` | `/rules/{id}/run-now` | `{"entity_id": null, "dry_run": false, "idempotency_key": "…"}` |
| `GET` | `/rule-runs`, `/rule-findings` | Runs and findings. |
| `POST` | `/rule-runs/{id}/replay`, `/rule-findings/{id}/acknowledge` | Replay a dead letter; acknowledge a finding. |
| `POST` | `/workflows/validate` | Validate workflow TOML. |
| `GET`, `POST` | `/workflows`, `/workflows/{id}/versions` | List or create workflows and revisions. |
| `POST` | `/workflows/{id}/versions/{version}/publish`, `/enable`; `/workflows/{id}/disable` | Lifecycle. |
| `POST` | `/workflows/{id}/run-now` | Manual run. |
| `GET` | `/workflow-runs` | Run history. |
| `POST` | `/workflow-runs/{id}/replay` | Replay a dead letter. |

### Agents

| Method | Path | Description |
| --- | --- | --- |
| `GET`, `POST` | `/agent/conversations` | List or create conversations. |
| `GET`, `PATCH`, `DELETE` | `/agent/conversations/{id}` | Read, rename, or archive. |
| `GET`, `POST` | `/agent/conversations/{id}/messages` | Messages. Posting starts a run and returns `202`. |
| `POST` | `/agent/conversations/{id}/uploads` | Upload an attachment. |
| `GET` | `/agent/conversations/{id}/runs` | Runs. |
| `GET` | `/agent/runs/{run_id}/events` | Server-sent events. Reconnect with `Last-Event-ID`. |
| `GET` | `/agent/approvals` | Pending approvals. |
| `POST` | `/agent/tool-calls/{id}/approve`, `/reject` | Decide. |

### Workspace and access

| Method | Path | Description |
| --- | --- | --- |
| `POST` | `/auth/discover`, `/auth/login`, `/auth/renew`, `/auth/logout` | Sessions. |
| `GET` | `/auth/session` | Current identity and capabilities. |
| `GET`, `POST`, `DELETE` | `/personal-access-tokens[/{id}]` | Personal API tokens. The secret is returned once. |
| `GET` | `/workspace/members`; `PUT /workspace/members/{id}` | Members and state. |
| `POST`, `DELETE` | `/workspace/members/{id}/grants[/{grant_id}]` | Role grants. |
| `POST` | `/workspace/members/{id}/transfer-ownership` | Transfer ownership. |
| `GET`, `POST`, `PUT` | `/workspace/roles[/{id}]`, `…/duplicate`, `…/retire` | Roles. |
| `GET` | `/workspace/permissions`, `/workspace/assignable-roles`, `/workspace/token-permissions`, `/workspace/grant-targets/{scope}` | Lookups for administration screens. |
| `GET`, `POST`, `DELETE` | `/workspace/invitations[/{id}]`; `POST /workspace/invitations/accept` | Invitations. |
| `POST` | `/workspace/users` | Create a user. |
| `GET`, `PUT` | `/workspace/navigation`; `GET /workspace/navigation/sidebar` | Sidebar shortcuts. |
| `GET` | `/audit-events` | Audit log. |

### Extensions

| Method | Path | Description |
| --- | --- | --- |
| `GET`, `POST`, `DELETE` | `/extension-registries[/{id}]`; `GET /extension-registries/discover`; `GET /extension-registries/extensions/{owner}/{repo}` | Registries and discovery. |
| `GET`, `POST` | `/extensions` | List or install. |
| `POST` | `/extensions/sideload` | Install an uploaded `application/zstd` archive. |
| `GET`, `DELETE` | `/extensions/{id}` | Read or remove. |
| `POST` | `/extensions/{id}/upgrade`, `/enable`, `/disable`, `/quarantine` | Lifecycle. |
| `PUT` | `/extensions/{id}/configure` | Configuration. |
| `POST`, `DELETE` | `/extensions/{id}/grants[/{kind}/{grant_id}]` | Grants. |
| `GET` | `/extensions/runtime` | Enabled client contributions. |
| `PUT` | `/workspace/extensions-mode` | `{"enabled": false}` turns off all extensions in the workspace. |
| `GET`, `PUT` | `/workspace/extension-layout` | Contribution layout. |
| `GET`, `PUT`, `DELETE` | `/workspace/extension-secrets[/{name}]` | Secrets (names only on read). |
| `POST` | `/extensions/{id}/operations` | Start an operation. |
| `GET` | `/extension-operation-runs[/{id}]`, `…/artifacts`, `…/deliveries` | Runs, outputs, deliveries. |
| `GET` | `/extension-operation-runs/{id}/artifacts/{artifact_id}/download` | Download an output. |
| `POST`, `GET`, `PATCH` | `/extensions/{id}/operation-schedules`, `/extension-operation-schedules[/{id}]` | Schedules. |

### Solution packs

| Method | Path | Description |
| --- | --- | --- |
| `POST` | `/solution-packs/inspect` | Inspect an `application/zstd` archive. |
| `POST` | `/solution-packs/plans?prefix=…&blueprint_publication=draft\|publish` | Create a plan. |
| `GET` | `/solution-packs/plans/{id}` | Read a plan. |
| `POST` | `/solution-packs/plans/{id}/apply` | Apply. |
| `GET` | `/solution-packs/applications[/{id}]` | Application history. |
| `GET`, `POST` | `/solution-packs/applications/{id}/checks` | Check runs. |
| `GET` | `/presentation-assets[/{id}[/content]]` | Presentation assets. |

### Health and operations

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/health/live`, `/health/ready` | Probes. |
| `GET` | `/system/health` | Running API version, source branch, and commit (`data_health.read`). |
| `GET` | `/metrics` | Prometheus metrics. |
| `GET` | `/data-health/summary`, `/blueprints`, `/freshness`, `/completeness`, `/contexts`, `/relationships`, `/storage`, `/background-processing` | Data health. |
| `POST` | `/data-health/refresh` | Clear the data-health cache. |
| `GET` | `/event-deliveries/dead-letters` | Failed event deliveries. |
| `POST` | `/event-deliveries/{consumer_id}/{event_id}/replay` | Replay one. |

Required permissions for each area are in the [permissions reference](/reference/permissions/).
