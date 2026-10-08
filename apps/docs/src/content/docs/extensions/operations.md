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

## How runs are durable

- A run is created once per workspace, release, operation, and idempotency key. Starting it again with the same key returns the same run.
- A run is pinned to the release it started on. Disabling, quarantining, upgrading, or revoking a grant pauses it until that exact release is authorized again; it never switches to new code.
- After each batch, the checkpoint is saved. The batch key changes only once the checkpoint is saved.
- If a worker crashes before the checkpoint, the batch runs again with the **same batch key**. If your component writes to the catalog or an external system, make those writes idempotent on the batch key.
- Cancelling a queued run ends it immediately. Cancelling a running one asks the component to stop at the next batch.
- A run that keeps failing becomes a dead letter. An operator can replay it.

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

It needs `client.operations.start`. Build the component against the `catalog-extension` or `operation-extension` [world](/extensions/server/#host-api-version); its `selection` interface works inside an interactive run.

When a run starts, Catalog checks that the user can read every selected record and freezes the user, release, input, context, and the ordered selection. Then:

- `selection.describe()` returns the count, blueprint revision, and context.
- `selection.page(cursor, limit)` returns 1 to 10 members with their saved values resolved in the run's context, your own annotations, and `read_at`. Members the user can no longer read come back as `unavailable`, deleted ones as `deleted`.
- `catalog-data.read` and the connector `catalog` calls are refused. `catalog-data.batch` accepts `update`, `relationships`, and `annotate` intents for selected records only, checked against the user's current permissions.
- If the user leaves the workspace, the run stops with a safe reason rather than continuing with the extension's own grants.

Capture what you need from each record once and keep it in the checkpoint, so a retried batch renders the same bytes. Report progress as `{"completed": n, "total": n, "outcome": {"succeeded": n, "failed": n, "skipped": n}}`; Catalog shows these counts separately from the run status, so a run can complete with some records failed.

Users see their runs under **Profile → Extension runs**. Only the user who started a run and people with `extensions.manage` can see it. The user who started it can always cancel it, but can open it and download its results only while they can still read every selected record. People with `extensions.manage` can open, cancel, and download any run.

## Record annotations

With `catalog.annotations.write`, add `annotate` intents to a batch to store facts on a record in your own namespace: tags `<extension-id>:<tag>` and the object at `system_metadata[<extension-id>]`. Intents identify the record by `entity_id`.

```json
{"kind": "annotate", "intent_key": "doc-<run>-<entity>", "entity_id": "…",
 "add_tags": ["document-generated"], "set_metadata": {"last_document": {"template_version": 2}},
 "remove_tags": [], "remove_metadata": [], "expected_revision": null}
```

You name local tags and keys only; Catalog adds the namespace. A patch has 1 to 32 operations. Setting a key replaces its value (`null` is allowed). `expected_revision` rejects the write if the namespace changed since you read it; a retried intent key is reported as `already_applied` first. Other writers, including users editing the record, cannot change your namespace, and your writes do not change the record's `updated_at`.

If records already have data under your extension ID, an operator must adopt the namespace before your first write. Do not store signed URLs or secrets in annotations, and do not treat a tag as proof that a file is still downloadable: outputs expire.

## File transfers

With `network.request` and a host permission that sets `max_transfer_bytes`, a component can move large files over HTTPS without passing bytes through JSON:

- `transfer.fetch-input` downloads one byte range, up to 16 MiB, into an input artifact. Follow-up ranges require an ETag so the source cannot change between ranges.
- `transfer.deliver-output` streams a finished output with `PUT`, or with `POST` if the host permission declares `idempotent_delivery: true`. Each delivery carries a stable `Idempotency-Key` header.

A delivery attempt is recorded before any network traffic. After a timeout or crash the result is `uncertain` and it is never resent automatically. `acli extension-operation deliveries <run-id>` shows the delivery history.

## Catalog access in operations

Operations can call `catalog-data.read` and `catalog-data.batch` (the same JSON as `catalog.read.v1` and `catalog.command.v1`), plus connector-shaped `schema`, `page`, and `upsert-batch` calls. Pages hold up to 100 records and batches up to 100 intents. Batches must carry the current batch key, and each intent key is recorded, so a replayed batch returns `already_applied` instead of writing twice.

Page cursors resolve values as of the first page, using value history, and expire after 30 days. This is not a database snapshot: records created, deleted, or migrated during a long export can still change which records appear. Freeze the source if you need an exact export.

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
- For the CSV connector, Attricat fills the profile's blueprint ID, revision, and context in for each run, so the TOML only needs the columns (and `business_key` for imports).

Run a job by hand:

```sh
acli connector-job list <blueprint-id>
acli connector-job run <job-id> --idempotency-key manual-2026-03-01
```

Managing connector jobs needs `extensions.manage`. Jobs cannot be triggered by catalog events yet; run them manually or on an interval.
