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

The component implements `prepare`, `start`, `process-batch`, `checkpoint`, `finish`, and `cancel`. Each call receives an `operation-request` with the run ID, the operation's `handler` as `operation-id`, the configuration snapshot, the validated input, the last checkpoint, and a **batch key**. The configuration, input, and checkpoint are JSON strings.

Attricat runs one batch at a time:

1. On the first batch, it calls `prepare`, then `start`. If the worker stops before the first batch completes, they are called again, so keep them free of side effects you cannot repeat.
2. It calls `process-batch`. Do a bounded amount of work and return `{checkpoint, progress, done}`. `checkpoint` and `progress` must be JSON objects; the checkpoint is at most 64 KiB, so store cursors and counters in it, not data.
3. It saves the checkpoint and calls `checkpoint` with it.
4. When `done` is true, it calls `finish`. Otherwise the next batch gets the saved checkpoint and a new batch key.

The checkpoint starts as `{}`. When someone cancels a running operation, the next batch calls `cancel` instead of `process-batch`, and the run ends.

## Defer and fail batches (1.1)

A component built for the `outcome-extension` [world](/extensions/server/#host-api-version) exports `deferrable-operations` instead of `operations`. Its `process-batch` returns `result<batch-outcome, failure>`.

`batch-outcome` adds `resume-after-ms` to the 1.0 result. Attricat saves the checkpoint, releases the run, and queues it again after the delay (at most 24 hours) instead of at once. Use it to wait for a destination to process a feed, to poll a long-running job, or to back off after throttling. The run list shows `resume_at` while the run waits.

`failure` is one of:

- `rejected { code, message, record-id?, connection? }`: when it names a record and the extension has `exports.mappings`, the record's export status becomes `rejected` and the batch runs again a second later. The mapping now reads as settled, so the component moves past the record and the rest of the run continues. Each newly rejected record is progress, so a batch may reject any number of records; repeating a rejection, or rejecting a record another worker holds, counts like a retry delay, at most 100 times in a row. A record another worker holds is replayed once that worker's lease ends. A rejection that cannot be recorded, for example for an unknown connection or a record outside the run, and any other rejection make the run a dead letter with code `rejected`, without retries.
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
- A run that keeps failing becomes a dead letter, and records it leased but did not settle become `retrying` with an error. An operator can replay it; the replayed run starts with a fresh failure budget and 100 new deferrals.

## Start, watch, and download

```sh
acli extension operation start acme.export --operation-id export \
  --input '{"profile":{"version":1}}' --idempotency-key export-2026-03-01
acli extension operation list
acli extension operation get <run-id>
acli extension operation artifacts <run-id>
acli extension operation download <run-id> <artifact-id> --output products.csv
acli extension operation cancel <run-id>
acli extension operation replay <run-id>
```

`--input-file-id` attaches a ready workspace file as the operation's input. The component opens it as `open-input("source")`; it never sees file IDs or storage keys.

Run lists never include inputs, configuration, or checkpoints. Values in configuration and diagnostics whose names suggest secrets, credentials, passwords, tokens, keys, or authorization are replaced with `[redacted]`.

## Outputs

With `artifacts.write`, a component builds output files in pieces:

- `artifacts.append-output(name, media-type, batch-key, bytes)` adds 1 to 65,536 bytes for the current batch. Repeating the same bytes for the same batch key is ignored; different bytes are rejected.
- `artifacts.finalize-output(name)` assembles the pieces into one checksummed file.

Limits are 1 GiB per output, 2 GiB per run, and 8 GiB per workspace. Outputs can be downloaded once the run completes and are kept for 30 days.

## Interactive operations

Add `interactive` to an operation to let signed-in users start it for a record or a selection, from the [action dialog](/extensions/client/#action-dialog) one of your actions opens, or directly from an [operation action](/extensions/client/#actions) with fixed input:

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

## Blueprint operations

An import starts from a blueprint, not from records. Set `scope` to `blueprint` to let a toolbar action start the operation for one blueprint revision:

```json
{"id": "import", "handler": "import", "request_schema": {"type": "object"}, "interactive": {"version": 1, "scope": "blueprint"}}
```

Starting one needs a workspace-wide `records.write` grant, because the run can create records. The run has no selection, and is limited to less than a person filling in the same values:

- `attricat-data.batch` accepts only `create` and `upsert` intents for the run's blueprint revision, without system tags or metadata, checked against the initiator's current permissions. Other intents make the call fail.
- Record reads are refused. `attricat.schema` works for the run's own revision, so you can map codes to attribute IDs.
- `selection.describe()` returns `scope: "blueprint"`, a `count` of 0, the blueprint revision and `context_id`, and `default_context_id` for when the action was started in the default context. Name a context on each value you write.

## File input

Add `accepts` to an operation [action](/extensions/client/#actions) to make Attricat ask for a file when the user chooses it:

```json
{ "id": "import-csv", "version": 1, "kind": "action", "outlet": "explorer_action", "title": "Import CSV…",
  "accepts": ["text/csv"], "invoke": { "operation": { "id": "import", "input": {} } } }
```

It needs `client.files.upload` and `artifacts.read`, and the person needs `records.write`. Attricat uploads the file and starts the run. Your component opens it with `artifacts.open-input("source")`, and `describe-input` gives its size, media type, and checksum. Parse it in batches and keep your position in the checkpoint.

Attricat decides the media type from the file itself, and refuses a file `accepts` doesn't list. It recognizes PNG, JPEG, GIF, WebP, PDF, DOCX, XLSX, and PPTX by their contents. Text is named by its file extension: `text/csv`, `text/tab-separated-values`, `application/json`, `application/xml`, or `text/plain` for any other. Text must be valid UTF-8. The usual upload size limit applies.

A file can only be used by the person who uploaded it, for your extension, within an hour of uploading it. Attricat deletes it some time after the run ends, so copy what you need to keep. It isn't shown anywhere in Attricat and isn't searchable.

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

A delivery attempt is recorded before any network traffic. After a timeout or crash the result is `uncertain` and it is never resent automatically. `acli extension operation deliveries <run-id>` shows the delivery history.

Host API 1.1 adds versions that fit marketplace and cloud storage uploads:

- `transfer.fetch-input-v2` and `transfer.deliver-output-v2` take the v1 fields plus `headers` (headers you control, such as the exact `Content-Type` a presigned URL was signed for) and `connection` (adds the [connection's](/extensions/exports/#connections) token). They accept query strings, such as presigned S3 or GCS URLs, when the rule sets `allow_query`. When a fetch with a `connection` gets a 401, the token is discarded and the call answers `{"outcome": "failed", "status": 401}`, so you can retry with a fresh token. A `deliver-output-v2` whose connection or TLS handshake fails answers `{"outcome": "unreachable"}`: nothing was sent, and the same delivery key may be sent again.
- A v2 delivery sends `Idempotency-Key` only when you pass `idempotency_key`. A `PUT` to a presigned URL is idempotent by itself. A `POST` still needs `idempotent_delivery: true` on its rule.
- `multipart: {"fields": [{"name", "value"}], "file_field", "filename"?}` sends a `multipart/form-data` form with the artifact as its file part, for form-style staged uploads.
- A v2 delivery can upload a completed input artifact of the run as well as an output.
- `transfer.open-record-file({"record_id", "file_id", "variant"?})` copies a file a live record references, up to 16 MiB, into an input artifact of the run and returns its artifact ID, so you can upload it directly. `variant` picks an image variant instead of the original. Records that share a file get the same artifact. It needs `attricat.read`, `attricat.files.read`, and `artifacts.read`. Connector runs can only open files of the records they export and of the live records those link to (the related records `page-v2` expands), and interactive runs only of their selection.

Every request a transfer makes counts against its rule's [request budget](/extensions/manifest/#request-budgets).

## Attricat access in operations

Operations can call `attricat-data.read` and `attricat-data.batch` (the same JSON as `attricat.read.v1` and `attricat.command.v1`), plus connector-shaped `schema`, `page`, and `upsert-batch` calls. Pages hold up to 100 records and batches up to 100 intents. Batches must carry the current batch key, and each intent key is recorded, so a replayed batch returns `already_applied` instead of writing twice.

Page cursors resolve values as of the first page, using value history, and expire after 30 days. This is not a database snapshot: records created, deleted, or migrated during a long export can still change which records appear. Freeze the source if you need an exact export.

In a connector run, `attricat-data.batch` also accepts annotation-only batches on records of the job's blueprint, so an exporter can [annotate](#record-annotations) records with external IDs or sync state where checks and the Explorer can see them. Export runs still cannot write values.

Export runs read records with the 1.1 `attricat.page-v2` and `attricat.withdrawn` calls instead. See [Export connectors](/extensions/exports/#read-records-to-export).

## Schedules

```sh
acli extension schedule create acme.export --operation-id export \
  --input '{"profile":{"version":1}}' --interval-seconds 86400
acli extension schedule update <schedule-id> --enabled false --interval-seconds 86400
```

A schedule runs an operation every 60 seconds to 30 days. Missed intervals and overlapping runs are skipped. A schedule pauses while its release is disabled, quarantined, upgraded, or missing a grant.

## Blueprint connector jobs

A connector job ties an operation to a blueprint. It is declared in the blueprint's YAML, so it is versioned and reviewed with the data model:

```yaml
connector_jobs:
  - code: csv_export
    direction: export
    extension_id: attricat-connector-csv
    operation_id: export
    interval_seconds: 3600
    input:
      profile:
        version: 1
        columns:
          - {header: SKU, attribute: sku, kind: string}
          - {header: Title, attribute: title, kind: string}

  - code: csv_import
    direction: import
    extension_id: attricat-connector-csv
    operation_id: import
    context: default
    input:
      profile:
        version: 1
        business_key: sku
        columns:
          - {header: SKU, attribute: sku, kind: string}
```

- Jobs are validated against the enabled extension when the blueprint revision is published. An invalid job blocks publication.
- An **export** job creates one run per enabled [publication channel](/guides/publishing/), or only for the channel it names as `context`. Each run reads values in that channel's context and includes only records published to it.
- An **import** job creates one run that writes to its `context`.
- A later revision matches jobs by `code`. A job left out of the new revision is disabled; its history stays.
- Set `enabled: false` to pause a job.
- An export job can name the `connection` it pushes to. Without one, it uses the extension's default destination.
- For the CSV connector, Attricat fills the profile's blueprint ID, revision, and context in for each run, so the YAML only needs the columns (and `business_key` for imports).

Run a job by hand:

```sh
acli extension connector-job list <blueprint-id>
acli extension connector-job run <job-id> --idempotency-key manual-2026-03-01
```

Managing connector jobs needs `extensions.manage`.

Export jobs can also run when records change, push to a named connection, and be resynced. See [Export connectors](/extensions/exports/).
