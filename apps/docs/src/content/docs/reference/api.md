---
title: API reference
description: How to authenticate against the Attricat HTTP API, its conventions, and its routes grouped by area.
---

Attricat's web app is built on the same HTTP API you can call from scripts and integrations. The [CLI](/reference/cli/) wraps nearly all of it.

## Base URL

API routes are served below `/api`, for example `GET https://attricat.example.com/api/blueprints`; the paths on this page are relative to it. Every other path belongs to the web app. The health probes (`/health`, `/health/live`, `/health/ready`) also answer at the root for load balancers and container checks.

## Authentication

**Personal API token**: send `Authorization: Bearer cat_pat_…`. The token decides the workspace. Tokens are the recommended way to integrate.

**Browser session**: `POST /auth/login` sets an HttpOnly `attricat_session` cookie and a readable `attricat_csrf` cookie. Every unsafe request (anything but `GET`, `HEAD`, `OPTIONS`) made with the cookie must also send `X-Attricat-Csrf` with the CSRF cookie's value.

Clients never send a workspace ID; it always comes from the token or session.

Public routes that need no credentials: `/health`, `/health/live`, `/health/ready`, `POST /auth/discover`, `POST /auth/login`, `POST /auth/password-reset`, `POST /auth/password-reset/confirm`, `POST /onboarding/complete`, and `GET /public/extension-files/{token}`, where the token in the link is the credential.

## Conventions

- Requests and responses are JSON unless a route says otherwise. Successful empty responses are `204`.
- Errors return the HTTP status and a body like `{"error": {"code": "record_schema_mismatch", "message": "…"}}`. Match on `code`, not on the message. Some errors add an `error.details` object with data you can act on, such as the record that already holds a unique key or the checks that failed; see below.
- `401` means no valid credential; `403` means the credential lacks the permission or scope.
- `422` means the request was understood but is invalid, such as a blueprint that does not compile or a value that fails its schema.
- `409` means a conflict with current state, such as a relationship cardinality limit.
- Each response carries an `x-request-id`. You can send your own UUID in that header; it appears in the audit log.
- Every response has a `Server-Timing` header with the total server time.

## Common error codes

| Code | Status | Meaning |
| --- | --- | --- |
| `invalid_input` | 422 | The request body has an invalid value, such as a repeated or unknown code. The message says which. |
| `invalid_code` | 422 | A code contains characters other than ASCII letters, digits, hyphens, and underscores. |
| `code_taken` | 409 | The code or name is already in use. `details.resource` is `blueprint`, `context`, `workflow`, `rule`, or `connection`. |
| `default_context_protected` | 422 | The default context cannot be changed or deleted. |
| `default_context_only` | 422 | The attribute can only be written in the default context. |
| `context_cycle` | 422 | A context cannot become its own descendant. |
| `context_in_use` | 422 | A context with descendants, values, or extension runs cannot be deleted. |
| `invalid_blueprint_definition` | 422 | The blueprint TOML does not compile. The message says why. |
| `attribute_value_schema_mismatch` | 422 | A value fails its attribute's `value_schema`. |
| `record_schema_mismatch` | 422 | The record fails its `record_schema` in some context. |
| `relationship_cardinality_conflict` | 409 | A relationship write exceeds `cardinality` or `target_cardinality`, or gives a record in a `tree` a second parent. |
| `relationship_target_type_mismatch` | 422 | The linked record's blueprint is not allowed by `target_blueprint` or `target_blueprints`. |
| `relationship_cycle` | 409 | The link would close a cycle in an `acyclic` or `tree` relationship. `details.path` lists the record IDs around the cycle. |
| `unique_key_conflict` | 409 | Another record already has these values for a unique key. `details` has `key`, `context`, `values`, and `conflicting_record_id`. |
| `unique_key_duplicates` | 409 | Publishing a new unique key, or moving a context to another parent, failed because existing records would share values. `details.duplicates` lists them. |
| `relationship_hierarchy_violations` | 409 | Publishing `acyclic` or `tree` failed because existing links contain cycles or extra parents. `details` lists them. |
| `stale_record` | 409 | `expected_updated_at` no longer matches the record. Reload it and try again. |
| `status_precondition_required` | 428 | A write to a status attribute did not send `expected_updated_at`. Read the record and send its `updated_at`. |
| `status_transition_forbidden` | 403 | The status transition requires a permission or role the caller does not have. |
| `status_separation_of_duties` | 403 | The status transition must be made by someone other than the person who made an earlier transition. Holders of `records.unlock` are exempt. |
| `record_locked` | 409 | The record's status locks the content being changed, or the record cannot be deleted while it is locked. |
| `status_not_locked` | 409 | An unlock named a status that does not lock the record. |
| `record_id_taken` | 409 | A batch `create` chose an `record_id` that already exists. |
| `relationship_path_sort_requires_single_result_version` | 422 | Sorting by a related value across several blueprint revisions. |
| `file_processing` | 409 | The file is not ready to download yet. |
| `approval_already_decided` | 409 | An agent tool call was already approved or rejected. |
| `service_unavailable` | 503 | For agent routes: no AI provider is configured. |
| `record_check_failed` | 422 | An `x-attricat-checks` check fails in some context. |
| `transition_conditions_unmet` | 422 | A status transition's conditions are not met. |
| `rule_violation` | 422 | The write leaves the record violating an enforcing rule. |
| `publication_checks_failed` | 422 | A channel's required checks fail. `details.context` is the channel code. |
| `invalid_rule_definition` | 422 | The rule TOML is invalid or does not fit its blueprint revision. |
| `rule_not_enabled` | 422 | A normal manual run needs an enabled revision. Enable the rule first, or start a dry run. |
| `workflow_not_enabled` | 422 | A manual workflow run needs an enabled revision. Enable the workflow first. |
| `rule_dry_run_required` | 409 | Enabling an enforcing rule needs a completed full dry run of that revision first. If the latest dry run stopped at its record limit, `details` is `{"truncated": true, "existing_violations": …}`. |
| `rule_has_existing_violations` | 409 | The dry run found violations. `details.existing_violations` is the count. |

### Error details

`record_check_failed`, `transition_conditions_unmet`, `rule_violation`, and `publication_checks_failed` list up to 50 violations in `error.details.violations`:

```json
{"error": {"code": "transition_conditions_unmet", "message": "…", "details": {"violations": [
  {"source": "transition_condition", "code": "approver-set", "message": "Set an approver before release",
   "contexts": ["default"], "attributes": ["approved_by"],
   "transition": {"attribute_code": "status", "from": "review", "to": "released"},
   "evidence": {"attribute_code": "approved_by"}}
]}}}
```

| Field | Description |
| --- | --- |
| `source` | `record_check`, `transition_condition`, `rule`, or, for publication only, `record_schema`. |
| `code` | The check, condition, or rule code. |
| `message` | The check's or condition's custom message, the rule's name, or a generated one. |
| `contexts` | Codes of the contexts where it failed. |
| `attributes` | Attributes of this record involved, such as both sides of a comparison or the relationship of a `linked` check. Use them to highlight fields. |
| `severity` | For rules: the rule's severity. |
| `transition` | For conditions and guarded transitions: `attribute_code`, `from`, and `to`. |
| `evidence` | Details such as the compared values or the IDs of failing linked records. Publication bulk failures add `record_id`. |

`publication_checks_failed` also has `details.context`, the channel code. For what each error means and how to fix it, see [Validation](/builders/validation/#errors-and-how-to-fix-them).

## Routes

### Blueprints

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/blueprints` | Published record blueprints. |
| `GET` | `/blueprints/catalogue` | All blueprint families and revisions. |
| `POST` | `/blueprints` | Create a blueprint (first draft) from `{"definition": "<toml>"}`. |
| `GET`, `POST` | `/blueprints/{id}/versions` | List revisions, or create the next draft. |
| `POST` | `/blueprints/validate`, `/blueprints/{id}/versions/validate` | Run every check that creating the blueprint, or its next draft, would run, without storing it. Returns the resulting blueprint and attributes, or the same error the save would. |
| `GET` | `/blueprints/{id}` | Current published revision. |
| `GET` | `/blueprints/{id}/versions/{version}` | Exact revision, including drafts. |
| `POST` | `/blueprints/{id}/versions/{version}/publish` | Publish a draft. |
| `GET` | `/blueprints/by-code/{code}` and `/blueprints/by-code/{code}/versions/{version}` | Look up by code. |
| `GET` | `/blueprints/{id}/migration-batches` | Background migration batches. |
| `GET` | `/blueprints/{id}/connector-jobs` | Connector jobs. |
| `POST` | `/blueprint-connector-jobs/{id}/run` | Run a connector job, with `{"idempotency_key": "…"}`. |
| `POST` | `/blueprint-connector-jobs/{id}/resync` | Export again every published record of an export job: `{"idempotency_key": "…"}`, plus either `record_ids` (only those records) or `status` (only records with that export status on the job's connection; more than 10,000 such records is a `422`, so resync the whole job instead). Returns `{"run_ids": [...]}`. |
| `GET`, `POST` | `/reusable-attributes`; `POST /reusable-attributes/{id}/versions`; `POST /reusable-attribute-revisions/{id}/publish`; `GET`, `POST /reusable-attribute-groups` | Reusable attributes. |
| `POST` | `/reusable-attributes/validate`, `/reusable-attributes/{id}/versions/validate` | Run every check that creating the reusable attribute, or its next revision, would run, without storing it. |

### Records

| Method | Path | Description |
| --- | --- | --- |
| `POST` | `/v1/records` | Create a record with values, optional uploaded `files`, and optional `system_tags` and `system_metadata`. See [Files on a new record](#files-on-a-new-record). |
| `POST` | `/v1/records/batch` | Create, update, and delete several records at once: all changes are saved, or none are. See [Batch changes](#batch-changes). |
| `GET`, `PUT` | `/v1/records/{id}` | Read or update a record's form: values, relationships, removals, annotations. |
| `GET`, `DELETE` | `/records/{id}` | Read or delete a record. |
| `POST` | `/v1/records/{id}/duplicate` | Create a copy of a record with its values, relationships, and files. Unique-key values are left out. |
| `POST` | `/v1/records/search` | Search. See below. |
| `POST` | `/v1/records/labels` | Display labels for up to 100 record IDs: `{"record_ids": [...]}`. Returns only live records you can read; other IDs are left out. |
| `POST` | `/v1/records/facets/relationship-tree/children` | One page of a relationship facet's children, with counts. |
| `GET` | `/records/{id}/preview` | Values per context, with related records inline. |
| `GET` | `/records/{id}/resolved-preview?context_id=…` | Values resolved in one context, with the context each came from. |
| `GET` | `/records/{id}/hierarchy` | Ancestors along a relationship. |
| `POST` | `/v1/records/{id}/incoming-relationships` | Records linking to this one. |
| `GET` | `/records` | Browse relationship targets. |
| `GET` | `/records/{id}/values/current` | Current direct values and links. |
| `POST` | `/records/{id}/values` | Append values. |
| `POST` | `/records/{id}/relationships/replace`, `/remove` | Replace or remove relationship targets. |
| `GET` | `/records/{id}/changes` | Change history. Add `limit` (1 to 50) and `offset` for pages. |
| `GET` | `/records/{id}/values/history` | Value history. Same paging. |
| `POST` | `/records/{id}/values/history/{history_id}/restore` | Restore an earlier value. |
| `POST` | `/v1/records/{id}/blueprint-migration/preview` | Check migration to the current revision. |
| `POST` | `/v1/records/{id}/blueprint-migration` | Migrate. |
| `POST` | `/v1/records/{id}/reusable-attributes`, `/v1/records/{id}/reusable-attribute-groups/{group_id}` | Attach a reusable attribute or group. |
| `GET` | `/v1/records/{id}/status-transitions?context_id=…` | Declared transitions from the saved status in one context (default context if omitted): `{"items": [{attribute_code, from, to, code, allowed, denial_code, denial_reason, waives_separation, unmet}]}`. `denial_code` is `status_transition_forbidden`, `status_separation_of_duties` or `transition_conditions_unmet`; `waives_separation` is `true` when the transition is allowed only because the caller holds `records.unlock`; `unmet` lists unmet conditions and enforcing rules as violations. |
| `GET` | `/v1/records/{id}/approvals` | Approvals recorded by status transitions, newest first and 20 per page, with who approved, when, a digest of the covered content, and why an approval was voided. Returns `{"items","has_more"}`; continue with `before_time` (the last item's `approved_at`) and `before_id`. |
| `GET` | `/v1/records/{id}/export-status` | The record's export status at every export extension (`statuses`: connection, status, external ID, error, last sync) and the enabled export destinations (`destinations`). |
| `GET` | `/v1/records/{id}/retention-holds` | Retention holds on the record's files, newest first and 20 per page. Returns `{"items","has_more"}`; continue with `before_time` (the last item's `created_at`) and `before_id`. |

### Batch changes

Some changes only make sense together: releasing a new document revision and marking the previous one superseded, or recording a movement and updating the item's current location. Send them as one batch so a failure cannot leave half of the change saved.

```json
POST /api/v1/records/batch
{
  "operations": [
    {
      "op": "create",
      "record_id": "5b0b8c55-0c55-4cc5-9a0f-4a4c3d1a2b10",
      "blueprint": { "code": "document_revision" },
      "values": [
        { "kind": "scalar", "attribute_code": "label", "context_id": null, "value": "B" },
        { "kind": "scalar", "attribute_code": "status", "context_id": null, "value": "released" },
        { "kind": "relationship", "attribute_code": "previous", "context_id": null,
          "target_record_id": "1f7e2d9a-6a3e-4a8a-9d0c-2f8d4f7f9e11" }
      ]
    },
    {
      "op": "update",
      "record_id": "1f7e2d9a-6a3e-4a8a-9d0c-2f8d4f7f9e11",
      "expected_updated_at": "2026-10-01T09:30:00Z",
      "values": [
        { "kind": "scalar", "attribute_code": "status", "context_id": null, "value": "superseded" }
      ]
    }
  ]
}
```

- `op` is `create`, `update`, or `delete`. `create` takes the same fields as `POST /v1/records`, plus an optional `record_id` you choose so later operations can link to the new record. `update` takes the same fields as `PUT /v1/records/{id}`. `delete` takes `record_id`.
- `expected_updated_at` on `update` and `delete` is a precondition: if the record changed since you read it, the batch fails with `409 stale_record`. Status changes need it, as in a single update.
- Operations run in order, and each is checked like the equivalent single request when it runs: values, schemas, status transitions, relationship rules, and unique keys. Order them so each is valid at its turn.
- A batch has 1 to 50 operations and up to 1,000 values, links, and removals. A record can appear in only one operation.
- Every operation needs its own permission: `records.write` on the record to update it, `records.delete` to delete it, and workspace-wide `records.write` to create. If any is missing, nothing runs and the response is `403`.

A successful batch returns `200` with one result per operation, such as `{"op": "update", "record": {…}}` or `{"op": "delete", "record_id": "…"}`. Each operation is recorded in the audit log and emits its usual event, but only after the whole batch is saved.

If an operation fails, nothing is saved. The response has that operation's status and error code, the message starts with `operation <index>:`, and `error.details` includes `operation_index` and `record_id`. Fix that operation and send the whole batch again. The CLI equivalent is `acli record batch --operations <file>`.

### Search

```json
POST /api/v1/records/search
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
  "export_status": { "extension_id": "acme.shopify", "connection": "shop-eu", "status": "rejected" },
  "context_code": "PL",
  "sort": { "field": "title", "direction": "asc" },
  "page": { "size": 50, "cursor": null }
}
```

- Omit `blueprint.version` to search all published revisions.
- `query` uses the [search syntax](/guides/search-syntax/).
- `source_relationship_field` can be a relationship path of up to three hops, such as `model.brand`. Each hop needs a single target blueprint and `cardinality = "one"`, and resolves in the facet's context with its own context fallback. The children endpoint accepts the same paths.
- A `relationship_tree_facets` item without `source_relationship_field` filters the searched blueprint by its own `hierarchy_field`: a record matches when it is a selected node or one of its descendants. The children endpoint accepts the same omission to list the blueprint's own tree, counting each node and everything below it.
- `export_status` keeps records with that export status at one extension's connection, or its default destination when `connection` is omitted. `status` is `pending`, `synced`, `rejected`, `retrying`, `uncertain`, or `withdrawn`. It combines with the other filters like any of them, with no limit on how many records have the status.
- Filter operators are `eq`, `contains`, `starts_with`, `gt`, `gte`, `lt`, and `lte`. `field` can be a relationship path of up to three hops.
- `context_code` (default `default`) is the context in which filters, sorting, and `table_values` are resolved, inheriting from parent contexts per attribute. `query` matches values in every context.
- `sort.field` must be a scalar column in the blueprint's table view, `blueprint_version`, or `publication_status` (with `sort.context_code` naming a channel).
- Responses contain `items`, `next_cursor`, `result_version_scope`, and, for each item, `table_values` and `match_explanations`. Pass `next_cursor` back as `page.cursor` with the same sort.
- `include_total` returns a first-page total, capped at 500.
- `include_blueprint: false` leaves `blueprint` out of the response, for later pages when you kept the first page's.
- `include_previews: false` leaves out each item's `preview` and `related` records, unless a table column uses an extension renderer. `display` and `table_values` are always returned.

### Files

| Method | Path | Description |
| --- | --- | --- |
| `POST` | `/records/{record_id}/file-attributes/{attribute_code}/uploads` | Upload with `multipart/form-data`: one or more `files` parts and an optional `context_id` part. Returns `201`. |
| `POST` | `/blueprints/{blueprint_id}/file-attributes/{attribute_code}/staged-uploads` | Upload files for a record you are about to create. Same body as above. Returns `201` with `files` and `expires_at`. |
| `GET` | `/files/{file_id}` | Metadata and processing status. |
| `GET` | `/files/{file_id}/download` | Original file. Supports one `Range`. |
| `GET` | `/files/{file_id}/variants/{kind}/download` | `thumbnail` or `display` variant. |
| `GET`, `POST` | `/files/{file_id}/retention-holds` | List a file's retention holds, or place an explicit hold with `{"days": 365, "reason": "…"}` (`files.hold`). |
| `POST` | `/v1/records/{id}/status-unlock` | Change a locking status to any of its options, outside the blueprint's transitions: `{"attribute_code": "…", "context_id": "…", "to": "…", "expected_updated_at": "…"}`, with `context_id` optional (default context). Only the status changes. Requires `records.unlock` (owners and admins). Returns the record. |
| `POST` | `/files/{file_id}/retention-holds/{hold_id}/release` | Release an explicit hold early (`files.hold`). Holds placed by a status cannot be released. |

Downloads return `409 file_processing` until the file is `ready`.

#### Files on a new record

A record whose schema requires a file is created with its files in one request. First upload each file to `/blueprints/{blueprint_id}/file-attributes/{attribute_code}/staged-uploads`, which needs permission to create records. Then pass the returned IDs to `POST /v1/records`:

```json
{
  "blueprint": { "code": "product" },
  "values": [],
  "files": [{ "attribute_code": "photos", "file_ids": ["…"] }]
}
```

Add `context_id` to an entry for a context other than the default. Uploaded files can only be used by you, for the same blueprint, attribute and context, and only once. They are deleted if no record uses them within an hour. Otherwise the request returns `422 invalid_file_references`.

### Contexts and publication

| Method | Path | Description |
| --- | --- | --- |
| `GET`, `POST` | `/contexts` | List or create contexts. |
| `GET` | `/contexts/{code}` | Read by code. |
| `PUT`, `DELETE` | `/contexts/id/{id}` | Update or delete. |
| `GET` | `/publication-channels` | Channel contexts. |
| `PUT` | `/publication-channels/{context_id}` | `{"enabled": true}` to make a context a channel. Optional `required_rule_codes` (up to 32 rule codes) and `require_valid_record` set the checks publication requires; omitted fields keep their current value. Invalid or repeated codes, or codes that name no rule in the workspace, return `422 invalid_input`. |
| `GET`, `POST` | `/v1/records/{id}/publications` | Publication status, or publish with `{"context_id": "…"}`. |
| `POST` | `/v1/records/publications` | Publication statuses for up to 100 record IDs: `{"record_ids": [...]}` returns `{"items": [{"record_id", "publications"}]}`. Returns only live records you can read; other IDs are left out. |
| `POST` | `/v1/records/{id}/publications/unpublish` | Unpublish from one channel. |
| `POST` | `/v1/records/{id}/publications/publish-all` | Publish to every channel. |
| `GET` | `/v1/records/{id}/publications/readiness` | For each enabled channel: `{context_id, context_code, ready, violations}`. |
| `POST` | `/blueprints/{id}/versions/{version}/record-publications`, `…/publish-all` | Publish all records of a revision. |

### Saved searches

| Method | Path | Description |
| --- | --- | --- |
| `GET`, `POST` | `/saved-views` | List (optional `q`) or create named searches. |
| `GET`, `PUT`, `DELETE` | `/saved-views/{id}` | Read, update, or delete your own. |
| `POST`, `GET` | `/view-state-links`, `/view-state-links/{id}` | Create or read a shareable snapshot. |

State is at most 32 KiB and uses the Explorer URL keys: `blueprint`, `version`, `allVersions`, `query`, `context`, `locked`, `sort`, `attributeFilters`, and `relationshipFacets`.

### Notifications

Every route acts on your own [inbox](/guides/inbox/) in the current workspace and needs only an active membership. Another member's notification returns `404`.

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/notifications` | Newest first, 30 per page, with `has_more` and `unread_count`. `unread_only=true` skips read ones; continue with `before_time` and `before_id` from the last item. |
| `GET` | `/notifications/unread-count` | `{"count"}`. |
| `GET`, `DELETE` | `/notifications/{id}` | Read one, or delete it permanently. |
| `PATCH` | `/notifications/{id}` | `{"read": true}` or `{"read": false}`. |
| `POST` | `/notifications/read-all` | Mark every unread notification read; optional `{"up_to": "<RFC 3339>"}` keeps later ones unread. Returns `{"updated"}`. |

Each notification has `id`, `kind` (for example `record.assigned`), a plain-text `title`, an optional `body`, the actor, an optional `subject` (`{"kind": "record" | "agent_conversation", "id"}`), kind-specific `data`, `read`, `read_at`, and `created_at`. New kinds can appear; show `title` for kinds you do not recognize.

### Translations

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/lexicon/entries` | List entries, optionally for one `language` (`records.read`). |
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
| `POST` | `/rules/{id}/run-now` | `{"record_id": null, "dry_run": false, "idempotency_key": "…"}`. A dry run may add `"version": 2` to target a published revision that is not enabled; it defaults to the enabled revision, or the latest published one. |
| `GET` | `/rule-runs`, `/rule-findings` | Runs and findings. A run's `truncated` is `true` when it stopped at its record limit before checking every record. Add `limit` and `offset` for pages; the `x-next-offset` header gives the next page's offset. `unresolved=true` leaves out resolved findings. Findings also narrow by `record_id`, `rule_id`, `blueprint_id`, `context_id`, `severity` (`info`, `warning`, `error`, `critical`) and `state` (`open`, `acknowledged`, `resolved`). |
| `GET` | `/rule-findings/count` | `{count}` of open and acknowledged findings; pass `record_id` to count one record's. |
| `POST` | `/rule-runs/{id}/replay`, `/rule-findings/{id}/acknowledge` | Replay a dead letter; acknowledge a finding. |
| `POST` | `/rule-findings/recheck` | Re-check one record against every enabled rule of its revision. Body `{record_id, idempotency_key}`; returns `202 {run_ids}`. Pass `record_id` to `GET /rule-runs` or `/rule-findings` to follow one record. |
| `POST` | `/workflows/validate` | Validate workflow TOML. |
| `GET`, `POST` | `/workflows`, `/workflows/{id}/versions` | List or create workflows and revisions. |
| `POST` | `/workflows/{id}/versions/{version}/publish`, `/enable`; `/workflows/{id}/disable` | Lifecycle. |
| `POST` | `/workflows/{id}/run-now` | Manual run. |
| `GET` | `/workflow-runs` | Run history. |
| `GET` | `/workflow-runs/{id}/targets` | Per-record outcomes of a `referencing_records_update` action. |
| `POST` | `/workflow-runs/{id}/replay` | Replay a dead letter. |

### Agents

| Method | Path | Description |
| --- | --- | --- |
| `POST` | `/agent/conversations` | Create a conversation, optionally about a record (`record_id`, `context_id`). |
| `GET` | `/agent/conversations/search` | Pages of conversations, newest first. `q` matches titles and user messages. Lists conversations not about a record unless `scope=all`; `record_id` (and optionally `context_id`) lists the conversations about one record. |
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
| `POST` | `/auth/access` | Which of the named records, blueprints or contexts (`kind`, `ids`, at most 500) the caller holds a `permission` on, counting record, blueprint-family and context-subtree grants. |
| `GET`, `POST`, `DELETE` | `/personal-access-tokens[/{id}]` | Personal API tokens. The secret is returned once. |
| `GET` | `/workspace/members`; `PUT /workspace/members/{id}` | Members and state. |
| `POST`, `DELETE` | `/workspace/members/{id}/grants[/{grant_id}]` | Role grants. |
| `POST` | `/workspace/members/{id}/transfer-ownership` | Transfer ownership. |
| `GET`, `POST`, `PUT` | `/workspace/roles[/{id}]`, `…/duplicate`, `…/retire` | Roles. |
| `GET` | `/workspace/permissions`, `/workspace/assignable-roles`, `/workspace/token-permissions`, `/workspace/grant-targets/{scope}` | Lookups for administration screens. |
| `GET`, `POST`, `DELETE` | `/workspace/invitations[/{id}]`; `POST /workspace/invitations/accept` | Invitations. |
| `POST` | `/workspace/users` | Create a user. |
| `GET`, `PUT` | `/workspace/navigation`; `GET /workspace/navigation/sidebar` | Sidebar shortcuts. |
| `GET`, `PUT` | `/workspace/agent-settings` | How agent bulk edits are approved (`agents.manage`): `{"bulk_edits": "per_proposal" \| "per_record", "bulk_edit_max_records": 1–5000}`. |
| `GET` | `/directory` | Users and teams that user or team attributes can reference (`records.read`). |
| `GET` | `/directory/users/{user_id}` | One workspace member's profile with their teams (`records.read`). |
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
| `GET` | `/extensions/{id}/export-status` | Status counts per connection and status, a page of export mappings (`?connection=&status=&after=&limit=`, up to 100, continue with `next`), and the 20 most recent rejected or exhausted event deliveries (`handler_failures`); counts and failures come only with the first page. It names records across the workspace, so it also needs a workspace-wide `records.read` grant. |
| `POST` | `/extensions/{id}/export-status/retry` | `{"connection"?, "record_ids"?, "status"?, "run"?, "idempotency_key"?}`, with record IDs, a status, or both: set up to 1,000 matching records back to `pending` (records chosen by ID alone that were never exported start as `pending` when one of the extension's export jobs covers their blueprint and connection, or the extension handles retry requests itself; deleted records and withdrawn mappings are never selected, and `"status": "withdrawn"` is refused: publish the record again instead) and send the extension `export.retry_requested.v1` for each. With `"run": true`, also resync just those records on the extension's export jobs for that connection; with `idempotency_key` (1–40 ASCII bytes) the same request sent again retries nothing twice: it finds the records it retried and the runs it started, and starts a run only for a job that could not start before. Returns `202` with `{"record_ids", "run_ids", "job_errors"}`; a job that cannot resync is listed in `job_errors` and does not fail the retry. Needs a workspace-wide `records.read` grant too; an extension that is not installed with `exports.mappings` is a `404`, and a disabled one a `422`. |
| `POST` | `/extensions/{id}/exports/{export_id}/preview` | `{"record_id", "connection"?}`: the payload the destination's preview command builds for one record, conventionally `{"payload", "issues"}`. Nothing is pushed. Needs `records.write`, like other extension commands, and `records.read` across the whole workspace; `connection` must be a connection of the export's kind. |
| `GET`, `POST` | `/extensions/{id}/connections` | List connections, or create one: `{"connection", "kind", "display_name", "grant_type", "token_url", "device_authorization_url"?, "client_id", "scope"?, "parameters"?, "client_secret"?, "refresh_token"?}`. Responses never contain secrets or tokens, only whether they are stored. |
| `PATCH`, `DELETE` | `/extensions/{id}/connections/{connection}` | Change `display_name`, `token_url`, `device_authorization_url`, `scope` (an empty string clears it), or `parameters`, or replace `client_secret` or `refresh_token`; or delete the connection with its credentials. Moving `token_url` or `device_authorization_url` to another host needs `client_secret` again and drops the stored tokens, so the connection waits for authorization unless a `refresh_token` is sent too. Its export mappings stay, so a connection re-created under the same name keeps the destination's external IDs. |
| `POST` | `/extensions/{id}/connections/{connection}/device-authorization`, `…/device-authorization/poll` | Start the device authorization grant (the response holds the user code and verification URL), then complete it once the user has approved. An active connection keeps working while a new authorization is pending. |
| `POST` | `/extensions/{id}/connections/{connection}/authorization-code` | Exchange an authorization code: `{"code", "redirect_uri", "code_verifier"?}`. |
| `POST` | `/extensions/{id}/connections/{connection}/test` | Get an access token to check the connection. |
| `GET` | `/extensions/{id}/host-permission-budgets` | Each host permission's declared and lowered request budget. |
| `PUT`, `DELETE` | `/extensions/{id}/host-permission-budgets/{host_permission_id}` | Lower a declared budget with `{"requests", "per_seconds"}`, never above the declared rate, or reset it. |
| `GET` | `/public/extension-files/{token}` | A file behind a public link an extension issued. No credentials; it works while the link is unexpired and the extension is enabled with `attricat.files.public_links` and `attricat.files.read`. |

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
| `GET` | `/export-status` | Export status counts per extension, connection, and status, with the enabled export destinations (`data_health.read`). |
| `GET` | `/data-health/summary`, `/blueprints`, `/freshness`, `/completeness`, `/contexts`, `/relationships`, `/storage`, `/background-processing` | Data health. |
| `POST` | `/data-health/refresh` | Clear the data-health cache. |
| `GET` | `/event-deliveries/dead-letters` | Failed event deliveries. |
| `POST` | `/event-deliveries/{consumer_id}/{event_id}/replay` | Replay one. |

Required permissions for each area are in the [permissions reference](/reference/permissions/).
