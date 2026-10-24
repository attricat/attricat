---
title: Export connectors
description: Keep a marketplace or storefront in sync with the catalog, with connections, export mappings, change-triggered runs, rejections, withdrawals, and resyncs.
---

An export connector pushes catalog records to an external destination, such as a Shopify shop, an Amazon seller account, or a marketplace, and keeps them in sync. Attricat gives connectors the parts that are hard to get right on your own:

- **Connections** hold the destination's credentials and run its OAuth flow. Your component never sees a secret or a token.
- **Export mappings** remember, per record and connection, the external ID and what was last pushed. Their status is the record's export status that people see in Attricat.
- **Connector jobs** decide when to export: on a schedule, when records change, or when an operator asks for a resync or a retry.
- **Typed outcomes** let a component reject one record or wait for the destination without failing the whole run.

Everything on this page needs host API 1.1 (`"host_api": "^1.1"`).

## What to declare

```json
{
  "attricat": { "id": "acme.shopify", "host_api": "^1.1" },
  "permissions": ["attricat.read", "exports.mappings", "connections.use", "network.request"],
  "host_permissions": [
    { "id": "shopify-token", "matches": ["https://*.myshopify.com/admin/oauth/*"], "methods": ["POST"] },
    { "id": "shopify-api", "matches": ["https://*.myshopify.com/admin/api/*"], "methods": ["POST"],
      "rate_limit": { "requests": 2, "per_seconds": 1, "burst": 40 } }
  ],
  "artifacts": [{ "id": "server", "kind": "server_wasm", "path": "dist/server.wasm" }],
  "server": {
    "operations": [{ "id": "export", "handler": "export", "request_schema": { "type": "object" } }],
    "connections": [{
      "id": "shop", "title": "Shopify shop", "grant_types": ["client_credentials"],
      "host_permission": "shopify-token", "api_host_permissions": ["shopify-api"],
      "token_header": { "name": "x-shopify-access-token" },
      "parameters_schema": { "type": "object", "required": ["shop_domain"] }
    }],
    "exports": [{ "id": "shop", "title": "Shopify", "connection": "shop", "resync_operation": "export" }]
  }
}
```

- A [connection kind](/extensions/manifest/#connections) describes one type of destination account. Operators create named connections of it under **Manage → Extensions**.
- An [export destination](/extensions/manifest/#export-destinations) gives the export status, preview, retry, and resync screens a name and entry points.
- The operation does the pushing. Build the component for the `outcome-extension` [world](/extensions/server/#host-api-version), so batches can [reject records and defer](/extensions/operations/#defer-and-fail-batches-11).

A blueprint then runs the operation as a [connector job](/extensions/operations/#blueprint-connector-jobs) for one connection:

```toml
[[connector_jobs]]
code = "shopify"
direction = "export"
extension_id = "acme.shopify"
operation_id = "export"
connection = "shop-eu"
trigger = { events = ["record.updated.v1", "record.published.v1", "record.unpublished.v1"], debounce_seconds = 30 }
input = {}
```

## The push loop

Each batch of an export run follows the same steps:

1. **Read the scope.** `api.call("connector.scope.v1", "{}")` tells you the run's [connection, channel, and mode](#run-scope).
2. **Read a page.** `attricat.page-v2` returns up to 16 rows of published records, with values resolved in the channel's context, and each record's `content_revision`. See [Read records to export](#read-records-to-export).
3. **Claim each record.** Call `mappings.begin.v1` with the row's `record_id` and `content_revision`. Skip the record on `stale` (nothing new to push) or `busy` (someone else is pushing it). On `proceed`, keep the `lease_token`.
4. **Skip unchanged content.** Hash the payload you would send. If it equals the mapping's `content_hash`, commit `synced` without calling the destination. This matters most in a resync.
5. **Push.** Send the record with `network.request.v2` and the run's `connection`.
6. **Record the result** with `mappings.commit.v1` and the lease token:
   - success: `status: "synced"`, the `external_id`, and the `content_hash`;
   - the destination refuses the record: return a `rejected` [failure](/extensions/server/#typed-outcomes) that names the `record-id`. The record's export status becomes `rejected` with your code and message, and the batch runs again, finding that record settled;
   - throttled or temporarily unavailable: return `retry` with `after-ms`, or finish the batch with `resume-after-ms`;
   - no answer: commit `uncertain`, and [look the item up](#uncertain-pushes) before you push it again.
7. **Withdraw.** After the last page, read [withdrawals](#withdrawals) and remove those items from the destination, committing `withdrawn`.
8. **Checkpoint** the page cursor and the withdrawal cursor, and return `done` when both are exhausted.

Because a batch can run twice with the same batch key, every step must be safe to repeat. `begin` and `commit` handle most of that for you: a record already committed at its revision answers `stale`.

## Run scope

`api.call("connector.scope.v1", "{}")` returns the run's job, blueprint, revision, context, channel, connection, and mode, or `null` outside a connector run. The mode is `incremental` for a scheduled or manual run, `triggered` for a [change-triggered](#run-exports-on-change) run, or `resync`.

A connector run is confined to its scope. It reads only its blueprint's published records, writes only its own connection's mappings, and sends only that connection's token. A run of the default destination, with no `connection` on the job, sends no connection token. Export runs cannot write values. They can write [annotations](/extensions/operations/#record-annotations) on records of the job's blueprint, so external IDs or sync state show up where checks and the Explorer can see them.

## Read records to export

`attricat.page-v2({"cursor", "limit", "expand"?})` reads 1 to 16 rows of the run's blueprint, context, and channel. Each row has `record_id`, `content_revision`, `blueprint_version`, `updated_at`, and:

- `values`: scalar values resolved through the context's fallback chain, so values inherited from parent contexts are included;
- `relationships`: relationship targets by attribute code;
- `files`: file references (`file_id`, `filename`, `mime_type`, `byte_size`, `sha256`) by attribute code, for [`open-record-file`](/extensions/operations/#file-transfers) and [public links](#public-file-links);
- `related`: for up to four relationship codes named in `expand`, the targets' values, relationships, files, revision, and whether each is published in the channel. Use it for variants and parent/child listings.

With `expand`, the row's `content_revision` is the newest revision of the record and the records it embeds. Editing or deleting a variant therefore makes its parent's row newer than the revision last committed for it, and the parent is pushed again.

### Record revisions

Every record has a `content_revision`: the sequence of the latest event that changed its values, relationships, publication, or lifecycle. It only grows. Annotation changes do not advance it, so your own bookkeeping never makes a record look changed. Publication changes count only for the channel being read: a connector page row counts those of the run's channel, and a read without a channel counts none, so publishing to another channel never makes your export look stale. Typed reads, page rows, and `event-handler` deliveries carry it. A delivery of a publish or unpublish event carries the revision without publication changes, like a read without a channel.

## Export mappings

With `exports.mappings`, Attricat keeps one mapping per installation, connection, and record. A mapping holds the external ID, named external IDs, the content hash and record revision last pushed, a status, the last error, and timestamps. Mappings are built for full catalogs and are indexed on the external ID. A mapping is deleted with its record or connection. A soft-deleted record keeps its mapping so you can withdraw the item, and a connection re-created under the same name gets its external IDs back.

The mapping status is the record's export status, shown on the record page, in the Explorer, and on Data health. Statuses are `pending`, `synced`, `rejected`, `retrying`, `uncertain`, and `withdrawn`.

These `api.call` operations work in event handlers and operation runs alike. `connection` is optional everywhere; without it the mapping belongs to the extension's default destination.

| Operation | Request | Result |
| --- | --- | --- |
| `mappings.get.v1` | `{connection?, record_id}` or `{connection?, external_id}` | `{mapping}`, or `{mapping: null}` |
| `mappings.list.v1` | `{connection?, status?, after?, limit?}` (1 to 100) | `{mappings, next}` |
| `mappings.begin.v1` | `{connection?, record_id, record_revision, lease_ms?, force?}` | `{outcome: "proceed", lease_token, mapping}`, `{outcome: "stale", mapping}`, or `{outcome: "busy", lease_until}` |
| `mappings.commit.v1` | `{connection?, record_id, lease_token?, status, record_revision?, external_id?, external_ids?, content_hash?, error?}` | `{mapping}` |
| `mappings.release.v1` | `{connection?, record_id, lease_token}` | `{released}` |
| `mappings.delete.v1` | `{connection?, record_id}` | `{deleted}` |

Inside an operation run, `begin`, `commit`, `release`, and `delete` accept only records the run covers: records of a connector run's blueprint (including deleted or unpublished ones it withdraws), or an interactive run's selected records. A run limited to selected records writes only their mappings.

### Begin and commit

Call `begin` before pushing a record. It protects against reordered and duplicate work:

- It answers `stale` for a revision older than the mapping's, whatever its status, because the commit would be refused after the push. It also answers `stale` for a deleted record, whose mapping only accepts a `withdrawn` commit.
- At the mapping's own revision, it answers `stale` when the mapping is already `synced` or `rejected`, unless an operator asked for a retry or you pass `force`. A `withdrawn` record published again unchanged comes back at the same revision and is pushed again.
- Otherwise it leases the record for `lease_ms` (default two minutes, at most 15), so an event handler and a connector run never push the same record at the same time. It answers `busy` while someone else holds the lease. A retried batch or delivery takes its own earlier lease back.

`commit` stores the result and releases the lease. Without a lease token it fails while another worker holds a live lease, and a `synced` result never replaces a newer synced revision. If a batch fails, release the leases it holds; leases the host finds unsettled when a run dead-letters become `retrying`.

Keep large destination reference data, such as category trees or product-type definitions, in [extension storage](/extensions/server/#storage), not in mappings.

## Connections

A connection is a destination account that an operator sets up for your extension. You declare connection kinds in the [manifest](/extensions/manifest/#connections). Attricat stores the credentials encrypted, runs the OAuth grant, and refreshes tokens.

With `connections.use`:

- `connections.list.v1` with `{}` returns `{"connections": [...]}`.
- `connections.get.v1` with `{"connection": "shop-eu"}` returns one connection.

Each connection is described by `connection` (its name), `kind`, `display_name`, `parameters` (non-secret settings, such as a shop domain or seller ID, validated against the kind's `parameters_schema`), and `status` (`pending_authorization`, `active`, `failed`, or `revoked`).

To call the destination, pass `connection` to [`network.request.v2`](/extensions/server/#networkrequestv2) or a [v2 transfer](/extensions/operations/#file-transfers). The host adds the access token, as `Authorization: Bearer` or the kind's `token_header`, only to requests matched by one of the kind's `api_host_permissions`. A 401 response discards the cached token, so the next request gets a new one. If the token cannot be refreshed for a temporary reason, the request answers `connection_unavailable`; retry the batch later.

## Withdrawals

`attricat.withdrawn(cursor, limit)` lists 1 to 100 records of the job's blueprint that were unpublished from the run's channel (and are still unpublished), in event order. Items come as `{record_id, reason, record_revision, occurred_at}`, with `next_cursor` and `more`.

- Deleting a record unpublishes it from every channel it was live in, so it is reported with reason `deleted`. A record never published in the channel is never reported.
- An empty cursor starts at the oldest change. Save the cursor, for example in extension storage per job and channel, so the next run continues where this one stopped.
- A run limited to selected records gets each selected record's latest withdrawal instead, page by page with `more`. Its cursor keeps the channel's position, so later runs still see every other withdrawal.
- While Attricat is busy writing events, a page can come back with no records but `more: true` and the same cursor. Call again; it is not the end.

Remove each item from the destination and commit its mapping as `withdrawn`.

## Uncertain pushes

A request that times out may still have been applied. With `network.request.v2` this is the `uncertain` outcome. Destinations that accept an `Idempotency-Key` make this easy: send a stable `idempotency_key`, such as the record ID and revision, and push again.

For destinations without idempotency keys, commit the mapping as `uncertain`. Before pushing that record again, look it up on the destination, for example by SKU, and record what you found. `mappings.get.v1` with `external_id` finds the record for an item you see on the destination.

A drift check is an operation, declared as the destination's `reconcile_operation`, that pages through the destination and compares it with `mappings.list.v1`. It can find items edited or deleted on the destination, listings unknown to Attricat, and mappings that point at missing items.

## Public file links

Some destinations fetch images themselves, sometimes hours after a listing is submitted. With `attricat.files.public_links`, `attricat.files.read` and `attricat.read`, `files.link.v1` issues a public, unguessable link to a file a record references:

```json
{ "file_id": "…", "record_id": "…", "expires_in_seconds": 86400 }
```

It returns `{"url": "…", "expires_at": "…"}`. A link lives from 60 seconds to 7 days. Inside an operation run, only records the run covers can be linked: a connector run's selection (and, for a channel export, published records) and the live records those link to, or an interactive run's selected records. Anyone with the link can download the file while it is unexpired and the extension is enabled with all three capabilities. Deleting the record or replacing the file stops the link at once. Only a hash of the token is stored. The deployment must set [`EXTENSION_PUBLIC_FILE_BASE_URL`](/reference/configuration/#extensions); otherwise the call fails.

To upload the bytes yourself instead, copy the file into the run with [`transfer.open-record-file`](/extensions/operations/#file-transfers) and send it with `transfer.deliver-output-v2`.

## Run exports on change

An export job can also run when its records change:

```toml
[[connector_jobs]]
code = "shopify"
direction = "export"
extension_id = "acme.shopify"
operation_id = "export"
connection = "shop-eu"
trigger = { events = ["record.updated.v1", "attribute_value.changed.v1"], debounce_seconds = 30 }
input = {}
```

A run starts once the newest matching event of the blueprint's records is `debounce_seconds` old, so a burst of edits causes one run. Steady edits cannot hold it back forever: a run also starts once the oldest pending change is ten times `debounce_seconds` old. If the run cannot start, for example while the extension is disabled, the changes stay pending and are exported once it can. A matching event of a record that one of the blueprint's records links to through a relationship counts as a change of the linking record, because `page-v2` can embed it. The run only covers the changed records, up to 10,000; with more changes it runs the whole job. See [Connector jobs](/reference/blueprint/#connector-jobs) for the allowed events.

Because export mappings hold one status per record and connection, an export job of an extension with `exports.mappings` must name its channel as `context` when the workspace has several enabled channels.

### Export from an event handler instead

A connector run reads pages of records and suits catalogs of any size. For destinations that want one call per change, an `event-handler` can push the record itself: debounce it with [`debounce_ms`](/extensions/server/#debounce), call `begin` with the delivery's `record-revision`, push, and commit. Both paths share the same mappings and leases, so a job and a handler never push the same record at once.

## Retries

Operators retry records from the extension's **Export status** section or a record's **Exports** tab. A retry sets the records' mappings to `pending`, so `begin` leases them again even at a settled revision, and emits [`export.retry_requested.v1`](/reference/events/) for each record, to your extension only:

```json
{ "record_id": "…", "extension_id": "acme.shopify", "connection": "shop-eu" }
```

Subscribe an event handler to it to push the record at once, or let the next run pick the pending records up. An operator can also ask for the retry to start a resync of just those records on each of your export jobs for that connection.

## Resync

A resync exports again every published record of an export job, or only some of them:

```http
POST /api/blueprint-connector-jobs/{id}/resync
{"idempotency_key": "resync-2026-03-01", "status": "rejected"}
```

Pass `record_ids` for given records, or `status` for the records with that export status on the job's connection, but not both. Without either, every published record is exported. A resync reads the same rows as any run, but `begin` leases records settled before the run started, so an exporter should skip records whose stored content hash is unchanged and push the rest. The idempotency key covers the selection: the same key with other `record_ids` or another `status` starts a new resync.

## Preview and mapping editors

- A destination's `preview_command` names a [server command](/extensions/server/#commands-for-your-ui) that builds the payload for one record without pushing it. It receives `{export_id, record_id, connection}` and conventionally returns `{payload, issues}`, where `issues` lists what the destination would reject, such as missing product-type attributes. The record's **Exports** tab shows it as **Preview**.
- An [`export_mapping`](/extensions/client/#elsewhere) panel appears on your extension's management page once per export destination. Use it to edit mappings that are too large for configuration, such as attributes to a marketplace's product-type attributes. Unlike other panels it can call your server commands, so a command validates the mapping and saves it to extension storage, where your exporter reads it. Report invalid input in the command's response: a command error reaches the panel only as a generic failure.

## Design notes

- **Channels and accounts.** A job exports one publication channel, with values resolved in that channel's context. A destination account is a connection: one channel can feed several connections, with one job each, and one connection can cover several marketplaces of an account, for example Amazon marketplace IDs as connection parameters.
- **Price and stock.** Push what the catalog holds. Price and stock that change often, or live in another system, are better synced by a separate job on the attributes that hold them than merged into the content export. Take the currency from a connection parameter or a catalog value, never from a default in your code.
