---
title: Operations and connectors
description: Long-running, checkpointed extension operations for imports, exports, and file transfers, with schedules and blueprint connector jobs.
---

An **operation** is long-running server work, such as exporting 200,000 products to CSV or importing a supplier feed. Operations run in batches in the background, save a checkpoint after each batch, and survive restarts.

## Declare an operation

```json
"server": {
  "operations": [{
    "id": "export",
    "handler": "export",
    "request_schema": {
      "type": "object",
      "properties": { "profile": { "type": "object" } },
      "required": ["profile"]
    }
  }]
}
```

The component implements `prepare`, `start`, `process-batch`, `checkpoint`, `finish`, and `cancel`. Each call receives the run ID, the configuration snapshot, the validated input, the last checkpoint, and a **batch key**.

## Defer and fail batches (1.1)

A component built for the `outcome-extension` [world](/extensions/server/#host-api-version) exports `deferrable-operations` instead of `operations`. Its `process-batch` returns `result<batch-outcome, failure>`.

`batch-outcome` adds `resume-after-ms` to the 1.0 result. Attricat saves the checkpoint, releases the run, and queues it again after the delay (at most 24 hours) instead of at once. Use it to wait for a destination to process a feed, to poll a long-running job, or to back off after throttling. The run list shows `resume_at` while the run waits.

`failure` is one of:

- `rejected { code, message, record-id?, connection? }`: when it names a record and the extension has `exports.mappings`, the record's export status becomes `rejected` and the batch runs again a second later. The mapping now reads as settled, so the component moves past the record and the rest of the run continues; like a retry delay, this happens at most 100 times in a row. Any other rejection makes the run a dead letter with code `rejected`, without retries.
- `retry { message, after-ms? }`: the batch runs again with the same batch key after the delay. With `after-ms` it waits without spending the run's failure budget, up to 100 times in a row before a batch completes; past that, and without `after-ms`, each retry counts against the budget.
- `fault(message)`: the run fails and the installation is quarantined.

Traps, fuel or memory exhaustion, timeouts, and invalid batch results are faults too. An error returned by `prepare`, `start`, `checkpoint`, `finish`, or `cancel` fails the run without quarantining the installation.

A run whose release is disabled, quarantined, or loses a grant during a batch pauses instead of failing.

## How runs are durable

- A run is created once per workspace, release, operation, and idempotency key. Starting it again with the same key returns the same run.
- A run is pinned to the release it started on. Disabling, quarantining, upgrading, or revoking a grant pauses it until that exact release is authorized again; it never switches to new code.
- After each batch, the checkpoint is saved. The batch key changes only once the checkpoint is saved.
- If a worker crashes before the checkpoint, the batch runs again with the **same batch key**. If your component writes to the catalog or an external system, make those writes idempotent on the batch key.
- Cancelling a queued run ends it immediately. Cancelling a running one asks the component to stop at the next batch.
- A run that keeps failing becomes a dead letter. An operator can replay it; the replayed run starts with a fresh failure budget and 100 new deferrals.

## Start, watch, and download

```sh
acli extension-operation start acme.export --operation-id export \
  --input '{"profile":{"version":1}}' --idempotency-key export-2026-03-01
acli extension-operation list
acli extension-operation show <run-id>
acli extension-operation artifacts <run-id>
acli extension-operation download <run-id> <artifact-id> --output products.csv
acli extension-operation cancel <run-id>
acli extension-operation replay <run-id>
```

`--input-file-id` attaches a ready workspace file as the operation's input. The component opens it as `open-input("source")`; it never sees file IDs or storage keys.

Run lists never include inputs, configuration, or checkpoints. Values in configuration and diagnostics whose names suggest secrets, credentials, passwords, tokens, keys, or authorization are replaced with `[redacted]`.

## Outputs

With `artifacts.write`, a component builds output files in pieces:

- `artifacts.append-output(name, media-type, batch-key, bytes)` adds 1 to 65,536 bytes for the current batch. Repeating the same bytes for the same batch key is ignored; different bytes are rejected.
- `artifacts.finalize-output(name)` assembles the pieces into one checksummed file.

Limits are 1 GiB per output, 2 GiB per run, and 8 GiB per workspace. Outputs can be downloaded once the run completes and are kept for 30 days.

## Interactive operations

Add `interactive` to an operation to let signed-in users start it for a record or a selection from your version 2 actions:

```json
{"id": "generate", "handler": "generate", "request_schema": {"type": "object"}, "interactive": {"version": 1, "max_selection": 50}}
```

It needs `client.operations.start`. Build the component against the `attricat-extension` or `operation-extension` [world](/extensions/server/#host-api-version); its `selection` interface works inside an interactive run.

When a run starts, Attricat checks that the user can read every selected record and freezes the user, release, input, context, and the ordered selection. Then:

- `selection.describe()` returns the count, blueprint revision, and context.
- `selection.page(cursor, limit)` returns 1 to 10 members with their saved values resolved in the run's context, your own annotations, and `read_at`. Members the user can no longer read come back as `unavailable`, deleted ones as `deleted`.
- `attricat-data.read` and the connector `catalog` calls are refused. `attricat-data.batch` accepts `update`, `relationships`, and `annotate` intents for selected records only, checked against the user's current permissions.
- If the user leaves the workspace, the run stops with a safe reason rather than continuing with the extension's own grants.

Capture what you need from each record once and keep it in the checkpoint, so a retried batch renders the same bytes. Report progress as `{"completed": n, "total": n, "outcome": {"succeeded": n, "failed": n, "skipped": n}}`; Attricat shows these counts separately from the run status, so a run can complete with some records failed.

Users see their runs under **Profile → Extension runs**. Only the user who started a run and people with `extensions.manage` can see it. The user who started it can always cancel it, but can open it and download its results only while they can still read every selected record. People with `extensions.manage` can open, cancel, and download any run.

## Record annotations

With `attricat.annotations.write`, add `annotate` intents to a batch to store facts on a record in your own namespace: tags `<extension-id>:<tag>` and the object at `system_metadata[<extension-id>]`. Intents identify the record by `record_id`.

```json
{"kind": "annotate", "intent_key": "doc-<run>-<record>", "record_id": "…",
 "add_tags": ["document-generated"], "set_metadata": {"last_document": {"template_version": 2}},
 "remove_tags": [], "remove_metadata": [], "expected_revision": null}
```

You name local tags and keys only; Attricat adds the namespace. A patch has 1 to 32 operations. Setting a key replaces its value (`null` is allowed). `expected_revision` rejects the write if the namespace changed since you read it; a retried intent key is reported as `already_applied` first. Other writers, including users editing the record, cannot change your namespace, and your writes do not change the record's `updated_at`.

If records already have data under your extension ID, an operator must adopt the namespace before your first write. Do not store signed URLs or secrets in annotations, and do not treat a tag as proof that a file is still downloadable: outputs expire.

## File transfers

With `network.request` and a host permission that sets `max_transfer_bytes`, a component can move large files over HTTPS without passing bytes through JSON:

- `transfer.fetch-input` downloads one byte range, up to 16 MiB, into an input artifact. Follow-up ranges require an ETag so the source cannot change between ranges.
- `transfer.deliver-output` streams a finished output with `PUT`, or with `POST` if the host permission declares `idempotent_delivery: true`. Each delivery carries a stable `Idempotency-Key` header.

A delivery attempt is recorded before any network traffic. After a timeout or crash the result is `uncertain` and it is never resent automatically. `acli extension-operation deliveries <run-id>` shows the delivery history.

Host API 1.1 adds versions that fit marketplace and cloud storage uploads:

- `transfer.fetch-input-v2` and `transfer.deliver-output-v2` take the v1 fields plus `headers` (headers you control, such as the exact `Content-Type` a presigned URL was signed for) and `connection` (adds the [connection's](/extensions/server/#connections) token). They accept query strings, such as presigned S3 or GCS URLs, when the rule sets `allow_query`. When a fetch with a `connection` gets a 401, the token is discarded and the call answers `{"outcome": "failed", "status": 401}`, so you can retry with a fresh token.
- A v2 delivery sends `Idempotency-Key` only when you pass `idempotency_key`. A `PUT` to a presigned URL is idempotent by itself. A `POST` still needs `idempotent_delivery: true` on its rule.
- `multipart: {"fields": [{"name", "value"}], "file_field", "filename"?}` sends a `multipart/form-data` form with the artifact as its file part, for form-style staged uploads.
- A v2 delivery can upload a completed input artifact of the run as well as an output.
- `transfer.open-record-file({"record_id", "file_id", "variant"?})` copies a file a live record references, up to 16 MiB, into an input artifact of the run and returns its artifact ID, so you can upload it directly. `variant` picks an image variant instead of the original. Records that share a file get the same artifact. It needs `attricat.read`, `attricat.files.read`, and `artifacts.read`. Connector runs can only open files of their blueprint's records, and interactive runs only of their selection.

Every request a transfer makes counts against its rule's [request budget](/extensions/manifest/#request-budgets).

## Attricat access in operations

Operations can call `attricat-data.read` and `attricat-data.batch` (the same JSON as `attricat.read.v1` and `attricat.command.v1`), plus connector-shaped `schema`, `page`, and `upsert-batch` calls. Pages hold up to 100 records and batches up to 100 intents. Batches must carry the current batch key, and each intent key is recorded, so a replayed batch returns `already_applied` instead of writing twice.

Page cursors resolve values as of the first page, using value history, and expire after 30 days. This is not a database snapshot: records created, deleted, or migrated during a long export can still change which records appear. Freeze the source if you need an exact export.

In a connector run, `attricat-data.batch` also accepts annotation-only batches on records of the job's blueprint, so an exporter can [annotate](#record-annotations) records with external IDs or sync state where checks and the Explorer can see them. Export runs still cannot write values.

### Export pages and withdrawals

Host API 1.1 adds connector calls for exporters:

- `attricat.page-v2({"cursor", "limit", "expand"?})` reads 1 to 16 rows of the run's blueprint, context, and channel. Each row has `record_id`, `content_revision`, `blueprint_version`, `updated_at`, and:
  - `values`: scalar values resolved through the context's fallback chain, so values inherited from parent contexts are included;
  - `relationships`: relationship targets by attribute code;
  - `files`: file references (`file_id`, `filename`, `mime_type`, `byte_size`, `sha256`) by attribute code, for `open-record-file` and [`files.link.v1`](/extensions/server/#public-file-links);
  - `related`: for up to four relationship codes named in `expand`, the targets' values, relationships, files, revision, and whether each is published in the channel. Use it for variants and parent/child listings. With `expand`, the row's `content_revision` is the newest revision of the record and the records it embeds, so editing or deleting a variant makes its parent's row newer than the revision last committed for it.
- `attricat.withdrawn(cursor, limit)` lists 1 to 100 records of the job's blueprint that were unpublished from the run's channel (and are still unpublished) or deleted, in event order, as `{record_id, reason, record_revision, occurred_at}`, with `next_cursor` and `more`. An empty cursor starts at the oldest change. Save the cursor, for example in extension storage per job and channel, so the next run continues where this one stopped.
- `api.call("connector.scope.v1", "{}")` returns the run's job, blueprint, revision, context, channel, connection, and mode, or `null` outside a connector run.

## Schedules

```sh
acli extension-schedule create acme.export --operation-id export \
  --input '{"profile":{"version":1}}' --interval-seconds 86400
acli extension-schedule update <schedule-id> --enabled false --interval-seconds 86400
```

A schedule runs an operation every 60 seconds to 30 days. Missed intervals and overlapping runs are skipped. A schedule pauses while its release is disabled, quarantined, upgraded, or missing a grant.

## Blueprint connector jobs

A connector job ties an operation to a blueprint. It is declared in the blueprint's TOML, so it is versioned and reviewed with the data model:

```toml
[[connector_jobs]]
code = "csv_export"
direction = "export"
extension_id = "attricat-connector-csv"
operation_id = "export"
interval_seconds = 3600
input = { profile = { version = 1, columns = [
  { header = "SKU", attribute = "sku", kind = "string" },
  { header = "Title", attribute = "title", kind = "string" },
] } }

[[connector_jobs]]
code = "csv_import"
direction = "import"
extension_id = "attricat-connector-csv"
operation_id = "import"
context = "default"
input = { profile = { version = 1, business_key = "sku", columns = [
  { header = "SKU", attribute = "sku", kind = "string" },
] } }
```

- Jobs are validated against the enabled extension when the blueprint revision is published. An invalid job blocks publication.
- An **export** job creates one run per enabled [publication channel](/guides/publishing/). Each run reads values in that channel's context and includes only records published to it.
- An **import** job creates one run that writes to its `context`.
- A later revision matches jobs by `code`. A job left out of the new revision is disabled; its history stays.
- Set `enabled = false` to pause a job.
- An export job can name the `connection` it pushes to. Without one, it uses the extension's default destination.
- For the CSV connector, Attricat fills the profile's blueprint ID, revision, and context in for each run, so the TOML only needs the columns (and `business_key` for imports).

Run a job by hand:

```sh
acli connector-job list <blueprint-id>
acli connector-job run <job-id> --idempotency-key manual-2026-03-01
```

Managing connector jobs needs `extensions.manage`.

### Run exports on change

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

Each run records its mode: `incremental`, `triggered`, or `resync`.

### Resync

A resync exports again every published record of an export job, or only some of them:

```http
POST /api/blueprint-connector-jobs/{id}/resync
{"idempotency_key": "resync-2026-03-01", "status": "rejected"}
```

Pass `record_ids` for given records, or `status` for the records with that export status on the job's connection, but not both. Without either, every published record is exported. A resync reads the same rows as any run, but `begin` leases records settled before the run started, so an exporter should skip records whose stored content hash is unchanged and push the rest. The idempotency key covers the selection: the same key with other `record_ids` or another `status` starts a new resync.
