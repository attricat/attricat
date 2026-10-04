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
- Errors return the HTTP status and a body like `{"error": {"code": "entity_schema_mismatch", "message": "…"}}`. Match on `code`, not on the message. Some errors add an `error.details` object with data you can act on, such as the entity that already holds a unique key or the checks that failed; see below.
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
| `relationship_cardinality_conflict` | 409 | A relationship write exceeds `cardinality` or `target_cardinality`, or gives an entity in a `tree` a second parent. |
| `relationship_target_type_mismatch` | 422 | The linked entity's blueprint is not allowed by `target_blueprint` or `target_blueprints`. |
| `relationship_cycle` | 409 | The link would close a cycle in an `acyclic` or `tree` relationship. `details.path` lists the entity IDs around the cycle. |
| `unique_key_conflict` | 409 | Another entity already has these values for a unique key. `details` has `key`, `context`, `values`, and `conflicting_entity_id`. |
| `unique_key_duplicates` | 409 | Publishing a new unique key failed because existing entities share values. `details.duplicates` lists them. |
| `relationship_hierarchy_violations` | 409 | Publishing `acyclic` or `tree` failed because existing links contain cycles or extra parents. `details` lists them. |
| `stale_entity` | 409 | `expected_updated_at` no longer matches the entity. Reload it and try again. |
| `relationship_path_sort_requires_single_result_version` | 422 | Sorting by a related value across several blueprint revisions. |
| `file_processing` | 409 | The file is not ready to download yet. |
| `approval_already_decided` | 409 | An agent tool call was already approved or rejected. |
| `service_unavailable` | 503 | For agent routes: no AI provider is configured. |
| `entity_check_failed` | 422 | An `x-attricat-checks` check fails in some context. |
| `transition_conditions_unmet` | 422 | A status transition's conditions are not met. |
| `rule_violation` | 422 | The write leaves the entity violating an enforcing rule. |
| `publication_checks_failed` | 422 | A channel's required checks fail. `details.context` is the channel code. |
| `invalid_rule_definition` | 422 | The rule TOML is invalid or does not fit its blueprint revision. |
| `rule_dry_run_required` | 409 | Enabling an enforcing rule needs a completed full dry run of that revision first. |
| `rule_has_existing_violations` | 409 | The dry run found violations. `details.existing_violations` is the count. |

The four 422 check errors list up to 50 violations in `error.details.violations`:

```json
{"error": {"code": "transition_conditions_unmet", "message": "…", "details": {"violations": [
  {"source": "transition_condition", "code": "approver-set", "message": "Set an approver before release",
   "contexts": ["default"], "attributes": ["approved_by"],
   "transition": {"attribute_code": "status", "from": "review", "to": "released"},
   "evidence": {"attribute_code": "approved_by"}}
]}}}
```

`source` is `entity_check`, `transition_condition`, `rule`, or (publication only) `entity_schema`. `contexts` lists the context codes where the check failed, and `attributes` the entity's attributes involved, for highlighting fields. Rules add `severity`; publication bulk failures add `evidence.entity_id`. See [Validation](/builders/validation/#errors-and-how-to-fix-them).

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
| `POST` | `/v1/entities/batch` | Create, update, and delete several entities at once: all changes are saved, or none are. See [Batch changes](#batch-changes). |
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
| `GET` | `/v1/entities/{id}/status-transitions?context_id=…` | Declared transitions from the saved status in one context (default context if omitted): `{"items": [{attribute_code, from, to, code, allowed, denial_code, denial_reason, unmet}]}`. `denial_code` is `status_transition_forbidden`, `status_separation_of_duties` or `transition_conditions_unmet`; `unmet` lists unmet conditions and enforcing rules as violations. |

### Batch changes

Some changes only make sense together: releasing a new document revision and marking the previous one superseded, or recording a movement and updating the item's current location. Send them as one batch so a failure cannot leave half of the change saved.

```json
POST /api/v1/entities/batch
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

- `op` is `create`, `update`, or `delete`. `create` takes the same fields as `POST /v1/entities`, plus an optional `entity_id` you choose so later operations can link to the new entity. `update` takes the same fields as `PUT /v1/entities/{id}`. `delete` takes `entity_id`.
- `expected_updated_at` on `update` and `delete` is a precondition: if the entity changed since you read it, the batch fails with `409 stale_entity`. Status changes need it, as in a single update.
- Operations run in order, and each is checked like the equivalent single request when it runs: values, schemas, status transitions, relationship rules, and unique keys. Order them so each is valid at its turn.
- A batch has 1 to 50 operations and up to 1,000 values, links, and removals. An entity can appear in only one operation.
- Every operation needs its own permission: `entities.write` on the entity to update it, `entities.delete` to delete it, and workspace-wide `entities.write` to create. If any is missing, nothing runs and the response is `403`.

A successful batch returns `200` with one result per operation, such as `{"op": "update", "entity": {…}}` or `{"op": "delete", "entity_id": "…"}`. Each operation is recorded in the audit log and emits its usual event, but only after the whole batch is saved.

If an operation fails, nothing is saved. The response has that operation's status and error code, the message starts with `operation <index>:`, and `error.details` includes `operation_index` and `entity_id`. Fix that operation and send the whole batch again. The CLI equivalent is `acli entity batch --operations <file>`.

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
| `PUT` | `/publication-channels/{context_id}` | `{"enabled": true}` to make a context a channel. Optional `required_rule_codes` (up to 32 rule codes) and `require_valid_entity` set the checks publication requires; omitted fields keep their current value. |
| `GET`, `POST` | `/v1/entities/{id}/publications` | Publication status, or publish with `{"context_id": "…"}`. |
| `POST` | `/v1/entities/{id}/publications/unpublish` | Unpublish from one channel. |
| `POST` | `/v1/entities/{id}/publications/publish-all` | Publish to every channel. |
| `GET` | `/v1/entities/{id}/publications/readiness` | For each enabled channel: `{context_id, context_code, ready, violations}`. |
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
| `POST` | `/rules/{id}/versions/{version}/publish`, `/enable`; `/rules/{id}/disable` | Lifecycle. `/enable` accepts an optional `{"accept_existing_violations": true}` for an enforcing rule whose dry run found violations. |
| `POST` | `/rules/{id}/run-now` | `{"entity_id": null, "dry_run": false, "idempotency_key": "…"}`. A dry run may add `"version": 2` to target a published revision that is not enabled; it defaults to the enabled revision, or the latest published one. |
| `GET` | `/rule-runs`, `/rule-findings` | Runs and findings. |
| `POST` | `/rule-runs/{id}/replay`, `/rule-findings/{id}/acknowledge` | Replay a dead letter; acknowledge a finding. |
| `POST` | `/workflows/validate` | Validate workflow TOML. |
| `GET`, `POST` | `/workflows`, `/workflows/{id}/versions` | List or create workflows and revisions. |
| `POST` | `/workflows/{id}/versions/{version}/publish`, `/enable`; `/workflows/{id}/disable` | Lifecycle. |
| `POST` | `/workflows/{id}/run-now` | Manual run. |
| `GET` | `/workflow-runs` | Run history. |
| `GET` | `/workflow-runs/{id}/targets` | Per-record outcomes of a `referencing_entities_update` action. |
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
| `GET` | `/directory` | Users and teams that user or team attributes can reference (`entities.read`). |
| `GET`, `POST`, `PATCH`, `DELETE` | `/workspace/teams[/{id}]` | Teams (`members.manage`). `PATCH` takes `name` and/or `member_user_ids`, which replaces every member. |
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
