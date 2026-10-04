# Extension manifest and lifecycle (v1)

Extension v1 separates a package's **manifest syntax** (`manifest_version: 1`),
its SemVer **release** (`version`), and the SemVer range for callable Catalog
host functions (`catalog.host_api`). Event, capability, configuration, and UI
contribution versions are independent contracts. A host never coerces or
downgrades any of these contracts.

Extension registries and extension repositories are separate external inputs.
Every workspace has the built-in Attricat GitHub registry plus zero or more
workspace-managed GitHub `owner/repository` registries. Each registry exposes a
raw root `registry.json` with `registry_version: 1` and an `extensions` list;
each entry supplies a stable `id`, `name`, `description`, optional `icon`, and a
GitHub extension `repository`. Sources may be submitted as
`github:owner/repository`, `owner/repository`, or the canonical GitHub URL.
Catalog canonicalizes them and rejects non-GitHub origins, credentials,
query/fragment suffixes, and ambiguous paths.

Discovery loads trusted registry indexes only; an index is the sole authority
for which extension repositories Catalog may resolve. The v1 index shape is:

```json
{
  "registry_version": 1,
  "extensions": [{
    "id": "acme.example",
    "name": "Acme Example",
    "description": "Example extension",
    "icon": "icon.svg",
    "repository": "acme/catalog-extension"
  }]
}
```

Opening a listed entry loads that extension repository's README and non-draft,
non-prerelease GitHub Releases, returning its `.tar.zst` assets only when their
download path belongs to that exact extension repository. Catalog stores no
global copy of indexes, READMEs, releases, or archives.

A user with `extensions.manage` may alternatively install a local archive from
**Extensions → Upload archive** (`/manage/extensions/sideload`). The archive is
sent by Catalog's web client to `POST /extensions/sideload` as an
`application/zstd` request body and must be a `.tar.zst` extension package no
larger than 32 MiB. The server enforces the request media type and validates
the archive contents. Side-loaded
packages are treated as untrusted input: they receive the same compressed and
unpacked size limits, safe entry-path checks, strict `manifest.json`
validation, artifact staging, disabled initial state, lifecycle records, and
audit events as registry installs. Their recorded source is `sideload`; the raw
archive is never retained. Users must review configuration and explicitly grant
required permissions before enabling a side-loaded extension.

### Local extension integration testing

The sibling [`attricat-extension-example`](../../attricat-extension-example)
checkout is the host's end-to-end test extension. From an Attricat worktree,
it is available two levels above the repository root:

```sh
example_extension="$(cd ../../attricat-extension-example && pwd)"
(cd "$example_extension" && just check && just pack)
```

With this worktree's local stack running, upload the resulting
`$example_extension/dist/*.tar.zst` archive through **Manage → Extensions →
Upload archive**. Grant every requested permission and enable the extension.
Each side-loaded replacement is a new release and must be granted and enabled
again. Exercise an extension UI contribution and a server event (for example,
update a formula dependency in the example's blueprint TOML) to verify both the
browser and WASM runtime paths. Use this workflow when changing extension
installation, permissions, artifact storage, runtime, or event dispatch; unit
tests alone do not prove host integration.

For a CLI-driven local verification, authenticate with the workspace **login
identifier**, not its slug, name, or UUID. Discover it first and retain the
browser-session file for every subsequent command; the bootstrap workspace is
normally `default.local`:

```sh
cargo run -p cli -- --session-file .acli-session auth discover default.local
printf '%s' "$CATALOG_BOOTSTRAP_OWNER_PASSWORD" | \
  cargo run -p cli -- --session-file .acli-session auth login default.local \
    --email "$CATALOG_BOOTSTRAP_OWNER_EMAIL" --password-stdin
```

When verifying an inter-extension event, use two packaged server components:
the producer should emit a manifest-declared export while handling a real
Catalog event, and the enabled consumer should make an observable, idempotent
host-mediated change (for example an extension-storage write). Confirm the
producer event and completed consumer delivery through the API/CLI first; use
`just sql` only if the CLI has no read endpoint for the resulting host state.
This distinguishes session/workspace routing failures from event
materialization or runtime failures.

Registry installs validate the selected `.tar.zst` archive with bounded
decompression and entry-path checks, then read and validate `manifest.json`
before uploading declared extracted artifacts to Catalog S3 storage. Artifact
integrity verification is deliberately deferred: selected release archives are
trusted registry inputs in v1. Catalog retains only that installed release's
immutable manifest and source identity alongside its workspace-owned
installation; raw archives are not retained.

## Manifest

A manifest is strict JSON. Unknown fields at every v1 manifest object are
invalid. It requires a positive supported `manifest_version`, `name`, SemVer
`version`, `description`, a non-empty `icons` object, `catalog.id`,
`catalog.host_api`, and at least one artifact. `catalog.id`, artifact,
dependency, permission-rule, handler, and contribution identifiers are stable
ASCII IDs (`A-Z`, `a-z`, `0-9`, `.`, `_`, `-`). Artifact paths are relative and
may not contain traversal segments. Catalog safely unpacks the selected trusted
`.tar.zst` archive and writes only declared artifacts to Catalog S3 storage.

`catalog.host_api` and dependency ranges use SemVer ranges. A package is
accepted only when the host API range matches the host's version, every
declared artifact is present in the archive, configuration and dependencies
validate, and every required capability is known and grantable.

## Capabilities and egress

The v1 capability catalogue is: `catalog.read`, `catalog.write`,
`events.subscribe`, `events.emit`, `storage.extension`, `artifacts.read`,
`artifacts.write`, `configuration.read`,
`configuration.write`, `secrets.read`, `logging.write`, `client.commands`,
`client.navigation`, `client.notification`, `client.events`, `client.refresh`,
`client.confirmation`, `client.download`, `client.external_navigation`,
`client.files.read`, `client.files.upload`, `client.search`,
`client.live_updates`, `client.clipboard`, `client.theme.read`,
`client.locale.read`, `client.blueprint_configuration`,
`client.entity_decoration`, `client.entity_action`, `client.explorer_row_action`,
`client.explorer_table_cell`, `client.blueprint_detail_panel`, `client.explorer_action`,
`client.explorer_bulk_action`, `client.entity_header_action`,
`client.entity_attribute_panel`, `client.blueprint_panel`,
`client.blueprint_publish_check`, `client.file_panel`,
`client.audit_event_panel`, `client.data_health_card`, `client.action_dialog`,
`client.operations.start`, `client.operations.read`,
`client.operations.cancel`, `catalog.annotations.write`, `network.request`,
and `webhooks.receive`.

Required `permissions` and `host_permissions` must be granted before an
extension can be enabled. Optional variants are independently grantable and
never become implicitly enabled. `network.request` authorizes only the
operation class: each outbound request must additionally be matched to a
granted host-permission rule. Catalog capabilities remain separate from egress.

A host-permission rule has a stable ID, one or more URL patterns, explicit
methods, and non-zero request/response byte limits and timeout. V1 patterns
allow only `http` or `https`, an exact host or leftmost `*.` subdomain wildcard,
an optional exact numeric port, and a path prefix ending in `/*`. They reject
queries, fragments, userinfo, wildcard schemes/hosts, arbitrary ports,
localhost, and loopback/private/link-local/unspecified IP destinations.
Redirects are disabled; a future redirect contract must re-check its target.
WASM components receive no ambient sockets. The mediated HTTPS API described
below enforces the host policy at each call.

## Webhooks

`webhooks.receive` is server-only and is not an outbound host permission. A
v1 webhook declares a stable ID, a versioned `.vN` internal event type, named
handler, exactly `POST`, bounded body size, and `hmac-sha256` authentication.
The auth declaration supplies signature and timestamp headers, a positive
replay window, and a *name* for a host-managed secret; manifest snapshots and
audit records never contain the secret.

The future ingress runtime owns public routes and validates method, body,
signature, timestamp, replay/deduplication, rate limits, and diagnostics before
durably queuing an at-least-once delivery. Disabled or quarantined installations
must never invoke a handler. Binding routes, delivery execution, retries, and
observability are deliberately deferred to #145.

## Lifecycle and persistence

Discovery and validation are operations, not states. Installation persists a
workspace-scoped installation in `disabled`; the only durable states are
`disabled`, `enabled`, and `quarantined`. Catalog retains immutable snapshots
only for releases that have been installed, not every release discoverable from
an external GitHub registry. Enablement requires valid configuration, enabled
compatible dependencies without cycles, and every required capability and
host-permission grant. Quarantine may occur from any installed state; direct
re-enablement revalidates the installed release and its configuration before
handlers resume. Upgrade snapshots a newly selected installed release, clears grants and
configuration, and leaves a formerly enabled installation disabled. Removal
deletes current state but retains installed-release and append-only lifecycle
history.

Each lifecycle mutation uses an explicit Rust/SQLx transaction to write the
current state, an append-only lifecycle record, and the existing generic audit
event. Safe source, manifest/release identity, prior/new state, actor,
request/correlation IDs, and bounded diagnostic codes may be recorded; secrets,
credentials, raw packages, and payload bodies may not. SQL migrations remain
declarative and contain no behavior.

Catalog provides management APIs and web UI for registry sources, discovery,
installation, configuration, grants, lifecycle actions, and client runtime
descriptors. Server WASM execution, client components, storage, commands, mediated
extension-owned event publication, and mediated network/secrets APIs are
implemented. Webhook delivery remains follow-on work.

## Host ABI versions and evolution

`catalog.host_api` selects the component ABI the host binds for a release.
There are two eras.

**Legacy worlds (1.0–1.5, frozen).** Each of these is a separate, immutable WIT
package. They form two parallel families: the event/command world
(`wit/` 1.0, `wit-next/` 1.1, which exports `handler`) and the operation world
(`wit-operations/` 1.2, `wit-artifacts/` 1.3, `wit-connectors/` 1.4,
`wit-interactive/` 1.5, which export `operations`). A legacy release is bound
by excluding the previous minor (for example "1.5 but not 1.4"). Because the
families never merged, no legacy range can declare both `server.commands` and
interactive operations. The legacy rules are unchanged and existing releases
keep exactly the world they were built for.

**Unified ABI (1.6 and later).** `crates/extension-runtime/wit-host/` is the
single `catalog:host` package that evolves. 1.6 contains every 1.1
`api`/`handler` item and every 1.5 operation interface, unchanged. A range that
matches a unified minor and **no** legacy minor (for example
`">=1.6.0, <2.0.0"`) binds the unified ABI. Such a release may use every server
feature in one component: event handlers, client commands, scoped
configuration, scheduled, connector and interactive operations, connector jobs,
and version 2 selection actions. `is_unified_host_api` in
`crates/extension-manifest` is the only place that makes this decision, and the
manifest validator, event dispatcher, command broker, operation runs and
connector jobs all call it.

A component targets the combined `catalog-extension` world, or the narrower
`handler-extension` or `operation-extension` world. The host links every
import for every invocation and loads only the export it needs. Run-bound
interfaces (`artifacts`, `catalog-data`, `catalog`, `transfer`, `selection`)
return an error outside an operation run. Direct catalog access through `api`
(`read`, `write`, and the `catalog.read.v1`/`catalog.command.v1` calls) returns
an error inside one, so operations, including interactive runs confined to
their selection, reach catalog data only through their run-scoped interfaces.
Other `api` calls (configuration, secrets, storage, events, network, logging)
work in both, still checked against capabilities at each call.

### Host ABI evolution rules

These rules apply to every change to `wit-host/` and to the code that binds it:

1. **Additive only.** A new minor may add functions, interfaces, types, world
   imports and worlds. It must never remove, rename or change the signature of
   anything released. It must never add a case or field to a released variant,
   enum, flags or record, because an older guest cannot decode it. Introduce a
   new type and a new function instead.
2. **Released worlds keep their exports.** Adding an export to a released world
   would make components built for it invalid. Add a new world instead.
3. **One package, one binding.** Do not add another `wit-*` directory or a new
   parallel world family. Bump the `wit-host` package version and
   `SUPPORTED_HOST_API` together, and copy the released file to
   `wit-released/catalog-host-<version>.wit`. Snapshots are never edited.
4. **Behavior is versioned with the ABI.** A released function keeps its
   semantics. Stricter behavior needs a new function or an opt-in field on a
   new type.
5. **No new mutually exclusive ranges.** Feature gates must accept every unified
   ABI (`is_unified_host_api`), never "X but not X-1".

`released_host_abis_are_preserved` (in
`crates/extension-runtime/src/extension_runtime/abi_evolution_tests.rs`)
enforces rules 1–3 against every snapshot. wasmtime resolves imports and
exports with semver-compatible names, so a component built for 1.6 instantiates
unchanged against a host that binds a later 1.x unified package.

## Server WASM runtime

A `server_wasm` artifact is a WebAssembly **component** using the checked-in
`catalog:host@1.0.0` WIT package at `crates/extension-runtime/wit/catalog-extension.wit`.
Components receive no WASI context, filesystem, environment, clock, socket, or
pre-opened descriptor. The only imports are `api.call` and `api.log`.

A server manifest may declare `server.event_handlers`. Each handler has a
stable ID, the v1 `handle-event` component export, and one or more exact
versioned domain event types. It requires the required `events.subscribe` capability and a
`server_wasm` artifact. The durable event dispatcher treats its initial list
as a hint and immediately re-reads a locked snapshot of enabled state, exact
installed release, configuration, and required grants before each invocation.
Deliveries are at-least-once, may be reordered, and handlers must therefore be
idempotent. Component traps, fuel or
memory exhaustion, timeouts, and returned handler failures are traced, metered,
and quarantine the installation before the delivery is retried/dead-lettered.

The component host ABI uses bounded JSON strings (64 KiB) and bounded log
messages (16 KiB). Every operation is capability checked at the point of call.
`configuration.get.v1` is available to components with
`configuration.read`. `storage.get.v1`, `storage.set.v1` (also accepted as
`storage.put.v1`), `storage.delete.v1`, and `storage.list.v1` are available to
components with `storage.extension`. The legacy JSON
`catalog.read.v1` surface additionally provides bounded `page`, `changes`, and
single-attribute `lookup` requests; `page` cursors pin a database-clock snapshot
and `changes` cursors pin a domain-event sequence high-water mark. Cursors are
opaque and filter/workspace-bound. `catalog.command.v1` accepts a bounded,
idempotent batch of typed `create`, `update`, `relationships`, or `upsert`
intents. Each intent runs the ordinary entity create or update path (validation,
checks, audit, publication reconciliation and its domain event). An upsert
serializes its declared blueprint/attribute business key, creates only when it
is absent, and rejects an ambiguous match. When the lookup attribute alone is a
declared unique key, the lookup uses that key's normalized index across every
revision of the blueprint family; otherwise it matches the exact text among
entities of the requested revision. An upsert's declared relationship sets
apply whether it updates a match or creates; on create they are written as the
new entity's relationship values.
`events.emit.v1`
is implemented only for a manifest-declared, per-contract event export as
described in [Inter-extension events](#inter-extension-events).

### Mediated secrets and HTTPS

Workspace operators manage named extension secrets at
`/workspace/extension-secrets`; listing returns names only and values are
write-only. A component with `secrets.read` may call
`secrets.get.v1` with `{"name":"destination-token"}` and receives
`{"value":"..."}` only in that component invocation. There is no list API,
and values are never copied into manifests, configuration, operation snapshots,
audit records, logs, traces, or errors.

A component with `network.request` calls `network.request.v1` with
`host_permission_id`, `method`, `url`, optional UTF-8 `body`, optional safe
`headers`, and optional `secret_headers` entries (`secret`, `header`, optional
`prefix`). The host rechecks the exact enabled release, configuration,
capabilities, grants, and matching host-permission rule on every call. It only
allows query-free HTTPS, validates every DNS answer as public before connecting,
pins the connection to those checked addresses, verifies TLS, disables
redirects, streams bounded request/response bodies, applies each rule's timeout
and byte limits, and limits an extension release to 60 requests per minute.
Only status and selected safe response headers are returned; response bytes are
base64 encoded.

The broker never automatically retries. A transport failure or timeout before a
response is an **uncertain external outcome** because the peer might have
received the request; callers may retry only when they supply a stable
idempotency key/header whose semantics the destination documents. A timeout
after a response has begun is equally uncertain and is never retried. Durable
operations retain their #253 batch key across a pre-checkpoint replay; a
destination extension must map that stable key to its destination idempotency
key when it performs side effects. `network.request.v1` never grants ambient
sockets.

Registry source APIs expose `GET/POST /extension-registries`,
`DELETE /extension-registries/{id}`, `GET /extension-registries/discover`, and
`GET /extension-registries/extensions/{owner}/{repository}`. The final endpoint
rejects a repository unless the current trusted index lists it, then returns its
README and release assets. `extensions.read` authorizes discovery while
`extensions.manage` authorizes source changes and `POST /extensions/sideload`;
the built-in official source cannot be removed. The deployment may set
`EXTENSION_OFFICIAL_REGISTRY` to a validated GitHub owner/repository instead of
the default `attricat/attricat-extensions`.

## Inter-extension events

The generic Host API v1 call surface provides asynchronous, durable,
extension-owned events as the first inter-extension primitive. It does **not** provide service calls, shared
extension storage, direct networking, browser frame messaging, DOM access,
credentials, or ambient state.

A manifest declares stable exports and compatible consumption ranges:

```json
{
  "permissions": ["events.emit"],
  "event_contracts": {
    "exports": [{
      "id": "inventory.changed",
      "version": "1.0.0",
      "event_type": "plugin.acme.inventory.inventory_changed.v1",
      "schema": {"type": "object", "required": ["sku"]},
      "max_payload_bytes": 4096
    }],
    "consumes": []
  }
}
```

Each export has an independent SemVer version, provider-owned
`plugin.<extension-id>.*.vN` type, object JSON schema, and 1--64 KiB payload
bound. A consumer declares `events.subscribe` and a consumption item with
`provider`, `contract`, and a SemVer `version` range. Enabling a consumer
requires its provider to be installed, enabled, and contract-compatible.
Disabling, removing, or upgrading a provider is rejected when it would break
an enabled consumer.

Capabilities are not global permission. Operators must grant `events.emit` and
an `event_publish` grant for every export; consumers require `events.subscribe`
and an `event_subscribe` grant named `<provider-id>:<contract-id>`. The host
rechecks installation state, exact release, grants, schema, and byte limits on
each publish and delivery. Upgrades clear these grants.

A server component calls `api.call("events.emit.v1", json)` with:

```json
{"contract_id":"inventory.changed","aggregate_kind":"inventory_item","aggregate_id":"<uuid>","payload":{"sku":"ABC-1"}}
```

The host derives type, source, correlation ID, and causation ID; extensions
cannot forge them. Events use the existing transactional outbox and
at-least-once dispatcher. They can be reordered or retried, so handlers must
be idempotent. Failed components are quarantined and deliveries retry or dead
letter under dispatcher policy. Consumers only receive declared provider
contracts; core Catalog event subscriptions are unchanged. Handlers should
ignore events whose source is their own extension ID to prevent feedback loops.

Request/response calls, cancellation, and shared state are deliberately out of
scope for this contract and require a separately versioned design.

## Durable server operations (host API 1.2)

A release compatible with `catalog:host@1.2.0` may declare `server.operations`. Its immutable WIT records (`operation-request` and `batch-result`) are the typed, versioned operation ABI; operation IDs, run IDs, batch keys, checkpoint, progress, and completion state are not overloaded into an unversioned host call.
Each operation has a stable ID, component handler selector, object request schema,
and 64 KiB-or-smaller request/checkpoint limits. The immutable v1.2 WIT package is at
`crates/extension-runtime/wit-operations/catalog-extension.wit`. Artifact
streams use the additive immutable v1.3 package at
`crates/extension-runtime/wit-artifacts/catalog-extension.wit`; its request
contains the run ID, handler selector, configuration snapshot, input, checkpoint,
and durable batch key. Its operation world calls `prepare`, `start`,
`process-batch`, `checkpoint`, `finish`, and cooperative `cancel`.

Catalog creates one durable run per workspace, pinned installed release, operation,
and idempotency key. The task queue leases the run with a fresh token; every
checkpoint is committed with that same token, so a stale worker cannot advance
progress after a lease expiry. The batch key changes only after a checkpoint
commits. Consequently a crash before checkpoint replays the same batch key, and
a crash after a component's domain commit but before checkpoint is safe only when
the component treats that key as idempotent. Restarts reclaim the pending run
from its last committed checkpoint. Runs never switch to upgraded code: disable,
quarantine, grant loss, and upgrades pause a release-pinned run until that exact
release is authorized again. Cancelling queued work is terminal immediately;
cancelling leased work invokes cooperative cancellation at the next batch.

`POST /extensions/{extension_id}/operations` starts a run, while operators can
list `GET /extension-operation-runs`, cancel a run, or replay only a dead-lettered
run through its corresponding `cancel` and `replay` endpoints. The v1.3 artifact operation
WIT imports host-managed `artifacts` resources. Releases need explicit
`artifacts.read` and/or `artifacts.write` grants. Components open only run-bound
approved inputs, read or write at most 64 KiB per call, and exchange opaque
resource handles rather than object keys. An operation caller attaches a ready
workspace file by sending `source_reference: {"input_file_id":"<uuid>"}` to
the existing operation creation endpoint; Catalog locks and snapshots its
metadata/key transactionally with the run. The component opens that attached
input as `open-input("source")`, never by a file ID or object key. Output content is staged locally under
the bounded artifact/run/workspace quotas, committed only after its SHA-256
matches the supplied checksum, and then becomes immutable. Completed output is
available only to an authorized workspace operator at
`GET /extension-operation-runs/{run_id}/artifacts/{artifact_id}/download`; failed,
aborted, and abandoned temporary output is not downloadable and is cleaned up. Configuration,
input, source/destination references, and checkpoints are never returned by the
management API or written to audit metadata. Configuration and diagnostics are
redacted before management-visible persistence; secret-, credential-, password-,
token-, key-, and authorization-named fields are replaced with `[redacted]`.

### Connector catalog calls (host API 1.4)

Releases whose host API range includes 1.4 **but excludes 1.3** use the additive
`wit-connectors/catalog-extension.wit` operation world. Earlier releases keep
using the v1.3 world. In addition to artifacts, the new world imports
`catalog-data.read` and `catalog-data.batch`. Both take/return the same JSON
shapes as `catalog.read.v1` and `catalog.command.v1` respectively. Calls are
limited to 64 KiB of JSON; page size is at most 100 and batches at most 100
intents. The host refreshes the release and grant before each call, requires
`catalog.read` or `catalog.write`, and uses the workspace-scoped repository.
Batch requests must carry the current operation request's `batch-key`; that
key is scoped to the run ID and batch number. Intent keys and input hashes are
persisted transactionally with Catalog mutation, audit and outbox, so a replay
of an already applied intent returns `already_applied` without duplicating it.
Callers must keep intent keys stable across retries. The `catalog` interface
also provides connector-shaped `schema`, `page` (1–16 rows), and `upsert-batch`
operations. The upsert body carries `run_id`, the current `batch_key`, blueprint
and context IDs, and up to 100 keyed rows with scalar attribute-code values.
The host rejects mismatched business key values, rechecks grants and invokes
Catalog's validated, audited, idempotent mutation service. Page cursors are
workspace- and filter-bound and include a database-clock high-water mark;
attribute values use retained history to return their as-of values across
batches. Cursors expire after 30 days (before history retention expires).
This is **not** a long-lived MVCC database snapshot: concurrent transactions
committing after the first page and blueprint migrations/publication changes
can affect entity membership. For an immutable externally delivered export,
freeze the source or coordinate updates for the duration of the run.

`artifacts.append-output(name, media-type, batch-key, bytes)` accepts 1–65536
bytes per batch and stores an immutable, SHA-256-checked object segment under
the current run and batch key. Replaying the same bytes is a no-op; a changed
payload is rejected. `finalize-output(name)` assembles bounded segments into a
checksummed, immutable S3-backed output. The host retains segments across
worker restarts and enforces 1 GiB per output, 2 GiB per run and 8 GiB per
workspace. A finalized output is not downloadable until its run completes.
Completed output is retained for 30 days; staging for completed runs is swept
after one hour, cancelled runs after one hour, dead-letter runs after 30 days.
Object deletion is retried from durable tombstones. A dead-letter run cannot be
replayed after its output has expired.

`transfer.fetch-input` accepts bounded JSON control fields
`{host_permission_id,url,transfer_key,offset,max_bytes,etag?,secret_headers?}`;
it fetches one HTTPS byte range (up to 16 MiB) into a run-bound input artifact
and returns its opaque `artifact_id`, ETag, offset and length. Open the ID with
`artifacts.open-input`. Offsets beyond zero require an ETag; the server must
honor the Range and return the same identity. Replaying a transfer key returns
the existing artifact only when the request digest matches. Large sources are
processed as multiple ranges/checkpoints without placing file bytes in JSON.
`transfer.deliver-output` accepts `host_permission_id`, `url`, `method` (PUT or
POST), `artifact_id`, `delivery_key` and optional named `secret_headers`; it
streams a completed output with a stable `Idempotency-Key` header. An uncertain
attempt is recorded **before** network I/O: a timeout, redirect or crash must
not trigger an automatic resend. The response distinguishes `succeeded`,
`failed` with an HTTP status, and `uncertain`. Operators can inspect the
redacted delivery history at `GET /extension-operation-runs/{run_id}/deliveries`.
POST is rejected unless the
permission declares `idempotent_delivery: true`; only declare it for a target
that documents support for that idempotency key. Both calls need
`network.request`, a granted host permission with nonzero `max_transfer_bytes`
(maximum 1 GiB), HTTPS, public DNS/IPs, and no redirects, proxies, URL query,
URL credentials or ambient sockets. Grants and secrets are resolved anew per
call. The legacy `network.request.v1` remains capped at 1 MiB and is not a
file-transfer API.

`POST /extensions/{extension_id}/operation-schedules` creates a pinned interval
schedule with `operation_id`, bounded validated `input`, optional
`source_reference: {"input_file_id":"<uuid>"}`, empty `destination_reference`,
and `interval_seconds` (60–2592000). `GET /extension-operation-schedules`
lists schedule IDs/status without returning inputs/configuration. `PATCH
/extension-operation-schedules/{id}` accepts `{enabled,interval_seconds}`.
Each due tick enqueues an occurrence-specific idempotent run through the normal
task queue. Missed intervals and overlaps are skipped; disabled, quarantined,
upgraded or grant-revoked releases pause occurrence creation. Schedules retain
the release and validated input/configuration snapshot, not credentials, URLs
or request bodies. Output, progress, status and errors use the existing
workspace-scoped run and artifact endpoints.

### Blueprint connector jobs

Blueprint connector jobs are declared **in the entity blueprint TOML**, not
configured through a separate write API. For example (alongside the regular
`format_version`, `code`, `name`, `kind`, views and attributes):

```toml
[[connector_jobs]]
code = "csv_export"
direction = "export"
extension_id = "attricat-connector-csv"
operation_id = "export"
interval_seconds = 3600
input = { profile = { version = 1, blueprint_id = "00000000-0000-4000-8000-000000000001", blueprint_version = 1, context_id = "00000000-0000-4000-8000-000000000001", columns = [{ header = "ID", attribute = "external_id", kind = "string" }] } }

[[connector_jobs]]
code = "csv_import"
direction = "import"
extension_id = "attricat-connector-csv"
operation_id = "import"
context = "en_GB" # workspace context code
input_file_id = "<ready workspace file UUID>" # optional if using HTTPS transfer
input = { profile = { version = 1, blueprint_id = "00000000-0000-4000-8000-000000000001", blueprint_version = 1, context_id = "00000000-0000-4000-8000-000000000001", business_key = "external_id", columns = [{ header = "ID", attribute = "external_id", kind = "string" }] } }
```

Connector jobs are validated against the enabled extension, its declared
operation/schema, input file and import context **at blueprint publication**.
An invalid connector rejects publication atomically. Publishing a new revision
reconciles jobs by stable `code`; removed jobs become disabled while their run
history remains available. Set `enabled = false` in the TOML to pause a job.
For the packaged CSV connector, supply its `profile` and columns in `input`;
Attricat overwrites the profile's blueprint ID, version and context on each
run. Export declarations have no channel field: they cover all enabled
publication channels. Only releases using the `catalog:host@1.4.0` connector
world are accepted. Interval jobs accept 60–2592000 seconds.

```http
GET /blueprints/{blueprint_id}/connector-jobs
POST /blueprint-connector-jobs/{id}/run
{"idempotency_key":"manual-2026-01-01"}
```

These management routes require `extensions.manage` in the workspace. Inputs
are stored for execution but omitted from the job list. Imports can instead
use a separately granted HTTPS transfer capability.

A manual run returns `run_ids`. Each enabled publication channel in the
workspace gets **one run** for the selected blueprint at its latest published
revision, using that channel as the value-resolution context. The host enqueues
all runs via the existing durable extension-operation background task queue;
interval jobs use the existing operation coordinator to produce the same runs,
skipping overlap. For imports there is one run for the configured context.
The host stores job, blueprint/version, context and channel on each run (the
job/channel IDs appear in the run list), and
rejects connector `schema`, `page` and `upsert-batch` calls outside the
run-bound scope. Scoped runs cannot use the generic catalog read/batch API to
bypass that filter. Export pages include only entities **currently published**
for the run's enabled channel. A disabled channel or withdrawn publication is
excluded from subsequent pages; data already delivered externally cannot be
recalled. Retried manual keys return the same run IDs. Run status and artifacts
remain available via `/extension-operation-runs`.

This uses the released 1.4 connector ABI: the host enforces page filters from
run metadata without extending the WIT signature. Existing unscoped operations
and schedules remain available separately. Catalog-event triggers are not
configured for connector jobs yet; run them manually or on an interval.

## Client extension runtime (v1)

Enabled `client_component` artifacts can expose a strict `ui` contribution:

```json
{
  "id": "inventory-panel",
  "version": 1,
  "kind": "embedded",
  "artifact": "client",
  "outlet": "entity_preview_panel"
}
```

A contribution is either `route` (which requires a non-empty `title`),
`navigation` (which requires a non-empty `title` and targets one of that
extension's `route` contribution IDs), `embedded`, explicit `action`, or
explicit read-only `panel`. `action` is
required for `explorer_row_action` (and requires `client.explorer_row_action`);
`panel` is required for `blueprint_detail_panel` (and requires
`client.blueprint_detail_panel`). `embedded` contributions use `navigation`,
`entity_preview_panel`, `blueprint_attribute_configuration`,
`entity_attribute_decoration`, or `entity_action`; the additional mediated
placement capabilities reserve their matching fixed outlets for their explicit
host-owned action or panel layouts. Routes are always namespaced at
`/extensions/:extensionId/:contributionId`; manifests cannot provide a path,
selector, or host component. Each extension can use an outlet once and all
contribution/artifact IDs remain stable across releases. Multiple enabled
extensions may contribute to a surface. The host orders them deterministically
by `extension_id`, then contribution `id`, unless a workspace administrator
sets an outlet layout through `PUT /workspace/extension-layout`. Layouts use
`{"version":1,"outlets":{"entity_preview_panel":{"order":["acme.panel:summary"],"hidden":[]},"navigation":{"order":["acme.app:entry"],"hidden":[],"promoted":["acme.app:entry"]}}}`.
Keys are stable `<extension-id>:<contribution-id>` values; unavailable keys are
retained for restoration. Navigation contributions are kept in a host-owned
extension group unless an administrator lists their key in `promoted`; built-in
navigation is never addressable by the layout. The runtime descriptor contains
the host-computed `display_order` and navigation grouping, so clients never
implement precedence themselves. Entity action bars render one host-selected
primary action and up to three secondary actions; remaining extension actions
are placed in a host-owned overflow popover. Each mounted panel surface has an
explicit host policy and shows at most three contributions before the host-owned
content overflow popover; extension frames never determine capacity.

### Route contributions as extension applications

A `route` contribution is a dedicated host page and may mount an application-
sized extension UI. Once mounted in its iframe, the artifact may render multiple
screens and manage transitions between them with its own client-side router
(for example, an in-memory router). This supports workflows such as a
multi-step import wizard, a domain-specific workbench with a list and detail
screens, or an extension settings/dashboard experience.

This is an internal routing tree, not a host routing tree: the browser's
host-owned URL remains
`/extensions/:extensionId/:contributionId`, and an extension cannot claim
subpaths, add host route definitions, replace Catalog page chrome, or access
Catalog's router. A route contribution must provide its own UI, localized text,
and accessible labels inside the frame; Catalog owns the surrounding page,
loading/error treatment, and navigation outside it.

Today, such an application can use the granted mediated client APIs described
below: read the limited Catalog resources through `catalog.request`, invoke its
declared server commands through `catalog.command`, persist its own
release-scoped state through `catalog.storage`, show host notifications, and
navigate the user to a Catalog entity. It still has no generic browser fetch,
host DOM access, browser credentials, arbitrary URL navigation, or
inter-extension RPC. Required capabilities and the corresponding server command
or artifact declarations remain necessary for each operation.

### Fixed contribution outlets

The remaining contribution outlets are small, host-owned insertion points rather
than independently routed pages. An enabled contribution gets its own sandboxed
frame at the documented placement; it cannot select a DOM node, change the
surrounding layout, or assume a particular ordering relative to other enabled
extensions. Use them for focused controls, summaries, configuration, and
contextual actions—not an application-wide navigation tree.

- **`navigation`** is a host-owned side-navigation link to one of the same
  extension's declared `route` contributions. It has no artifact or outlet:

  ```json
  {
    "id": "formula-workbench-nav",
    "version": 1,
    "kind": "navigation",
    "route": "formula-workbench",
    "title": "Formula workbench"
  }
  ```

  Catalog validates that `route` identifies a route contribution in the same
  installed release, then generates `/extensions/:extensionId/:contributionId`.
  Extensions cannot provide a path, host route, icon, or ordering. Workspace
  layout still controls hiding, ordering, and promotion of this entry.
- **`navigation`** (`embedded`) remains available for a sandboxed compact
  iframe contribution, such as a button or status indicator in Catalog's side
  navigation.
- **`entity_preview_panel`** (`embedded`) appears in the entity extension
  drawer. It is appropriate for an entity-specific summary, diagnostics, or a
  focused mini-workflow. Its context supplies `entity_id` and optional
  `context_id`.
- **`blueprint_attribute_configuration`** (`embedded`, requiring
  `client.blueprint_configuration`) is rendered for an attribute in the
  blueprint editor. Use it to configure extension-owned behavior for that
  blueprint/attribute; scoped configuration is separately declared and requires
  `configuration.write`.
- **`entity_attribute_decoration`** (`embedded`, requiring
  `client.entity_decoration`) is an attribute-level popover on an entity page.
  Use it for a concise indicator, explanation, or contextual detail. Its
  context identifies the entity, attribute, blueprint/version, and optional
  context.
- **`entity_action`** (`embedded`, requiring `client.entity_action`) is a
  host-owned entity-page action area. Use it for a focused entity action or
  status; mutations still go through declared `catalog.command` commands.
- **`explorer_row_action`** (`action`, requiring
  `client.explorer_row_action`) is the Explorer row overflow UI for one entity.
  Its strict context is `entity_id`, `blueprint_id`, `blueprint_version`, and
  `context_version: 1`; it deliberately does not include search state or entity
  values. A version 2 contribution receives the
  [selection context](#selection-aware-actions-and-interactive-operations-host-api-15)
  instead.
- **`explorer_table_cell`** (`embedded`, requiring
  `client.explorer_table_cell`) mounts a sandboxed renderer for a configured
  scalar Explorer table column when the enabled release declares a matching
  cell-renderer contribution.
- **`blueprint_detail_panel`** (`panel`, requiring
  `client.blueprint_detail_panel`) is a read-only region on a blueprint detail
  page. Its strict context is `blueprint_id`, `blueprint_version`, and
  `context_version: 1`, making it suitable for blueprint-level status,
  validation results, or documentation.

- **`blueprint_panel`** (`panel`, requiring `client.blueprint_panel`) is a
  read-only panel in the blueprint page summary, outside the revision metadata
  tab. Its strict v1 context contains `context_version: 1`, `blueprint_id`,
  and `blueprint_version`. Unlike `blueprint_detail_panel`, it stays visible
  when switching between detail tabs.

- **`audit_event_panel`** (`panel`, requiring `client.audit_event_panel`)
  appears only inside an opened audit-event detail drawer. Its strict v1
  context is `{ "context_version": 1, "event_id" }`; audit metadata, actor
  details, and target values are never passed to the frame. The panel is
  read-only and cannot invoke extension commands.

- **`explorer_action`** (`action`, requiring `client.explorer_action`)
  appears between the Explorer result toolbar and table when results are scoped
  to one blueprint revision. Its strict v1 context contains only
  `context_version: 1`, `blueprint_id`, and `blueprint_version`; it does not
  expose search filters, selections, or entity rows. The host shows one
  primary and three secondary actions before overflow. Mutations still need
  separately declared `client.commands`.

- **`explorer_bulk_action`** (`action`, requiring
  `client.explorer_bulk_action`) appears below the Explorer toolbar only while
  1–50 loaded entities are selected and all belong to the displayed blueprint
  revision. Its strict v1 context contains `context_version: 1`,
  `blueprint_id`, `blueprint_version`, and the selected `entity_ids` in display
  order; it contains no row values or search filters. Closing selection mode
  unmounts the frame. Selection is a UI hint, not an authorization grant:
  commands still require `client.commands` and server-side permission checks.
  A version 2 contribution receives the selection context described below.

- **`action_dialog`** (`dialog`, requiring `client.action_dialog`) is a
  host-managed dialog opened only by the same extension's version 2 selection
  actions through `catalog.dialog.open()`. It requires a `title`, which the host
  renders. Its context is the opening action's selection context, captured when
  it opens; later Explorer selection changes do not alter it. The dialog
  outlives the menu, row, or selection toolbar that opened it and closes on
  `catalog.dialog.close()`, its close controls, Escape pressed on the host, or
navigation. Key presses inside the sandboxed frame never reach the host, so a
dialog frame should call `catalog.dialog.close()` on Escape. Closing it
  before starting a run creates nothing; closing it afterwards does not cancel
  the run.

- **`data_health_card`** (`panel`, requiring `client.data_health_card`)
  appears as a host-owned card below the Data Health summary cards, only when
  the summary loads. Its strict v1 context is `{ "context_version": 1 }`:
  private health metrics and filter values are not passed to the frame. At most
  three cards are visible before overflow. The panel is read-only.

- **`blueprint_publish_check`** (`panel`, requiring
  `client.blueprint_publish_check`) appears in the blueprint revision's publish
  confirmation dialog. Its strict v1 context contains `context_version: 1`,
  `blueprint_id`, and `blueprint_version`; the frame receives no draft contents.
  This panel is **informational only**: it cannot block or approve publication,
  call commands, or replace the host's server-side validation. Authoritative
  publish-time checks require a separate server contract.

- **`entity_attribute_panel`** (`panel`, requiring
  `client.entity_attribute_panel`) appears below each rendered field in the
  entity detail view when an enabled extension contributes that outlet. Its
  strict v1 context contains `context_version: 1`, `entity_id`, `attribute_id`,
  `blueprint_id`, `blueprint_version`, and nullable `context_id`. No field
  values are passed automatically. The panel is read-only; it does not appear
  for attributes outside the blueprint detail view.

- **`file_panel`** (`panel`, requiring `client.file_panel`) appears beneath
  each file link rendered by the default file-value renderer in an entity's
  detail view. Its strict v1 context contains `context_version: 1`, `file_id`,
  `entity_id`, `attribute_id`, `blueprint_id`, and `blueprint_version`. It
  receives no filename, file bytes, or field value automatically. It is
  read-only, appears only for actual file values (not empty fields), and does
  not render in custom file-value renderers that do not forward the optional
  file-panel callback. No separate file-detail page is implied.

Every manifest-declared outlet now has a host-owned client mount path;
`explorer_table_cell` uses the Explorer table renderer rather than
`ExtensionOutlet`. The browser schema tests compare the declared Rust outlets
with the client enum and check each outlet's mount path. A placement is still
shown only when its page, scope, contribution kind, and required grants match.

All mounted frames use the same mediated `catalog` API and capability checks
as route contributions. The host re-authorizes every broker call and unmounts
contributions when their runtime access is removed.

Catalog loads runtime descriptors and JavaScript only for installations whose
effective runtime state is enabled. The deployment gate (`EXTENSIONS_MODE`),
targeted `EXTENSION_DENYLIST`, workspace emergency gate, installation state,
and grants are all checked before descriptors, artifacts, broker requests,
server commands, host calls, storage, and event delivery. `PUT
/workspace/extensions-mode` with `{"enabled": false}` lets an
`extensions.manage` operator contain one workspace without changing any
installation or grant; the change is audited and setting it back to `true`
restores only still-enabled, authorized installations. Mounted frames poll the
runtime descriptor and unmount within 15 seconds; broker calls are rejected
immediately after containment. Release IDs and
object-store keys are never client addresses. The host
runs every contribution in a distinct `<iframe sandbox="allow-scripts">` with
an opaque origin and a CSP that denies network access. The host imports the
artifact and calls its required `mount(root, catalog)` export *inside that
frame*, not in Catalog's document. The artifact has no access to host DOM,
cookies, storage, React state, or other extension frames. `mount` may return a
cleanup function, which the frame calls during shutdown. The host revokes the
frame port and removes the frame on unmount or load error.

### Client API

A client artifact must export `mount(root, catalog)`. It runs only after the
artifact has been imported in its opaque frame; it may return a synchronous or
asynchronous cleanup function. For example:

```js
export const mount = (root, catalog) => {
  root.textContent = `Current entity: ${catalog.context.entity_id}`;
  return () => root.replaceChildren();
};
```

The frame provides a versioned `MessageChannel` API as `globalThis.catalog` and
passes that same object to `mount`. It may use only granted operations:

- `catalog.navigate({ entity_id })` requires `client.navigation` and resolves
  only to Catalog's entity route.
- `catalog.notify({ message, severity? })` requires `client.notification`;
  messages are trimmed and limited to 512 characters and are displayed through
  a host-owned accessible notification surface.
- `catalog.request(path)` requires `catalog.read` and is limited to `GET`
  reads of `/api/entities`, `/api/v1/entities/:uuid`, or the exact revision
  route `/api/blueprints/:uuid/versions/:positive-version`. Responses are
  limited to 1 MiB measured as UTF-8 bytes. Revision reads are bound to the
  `blueprint_attribute_configuration` outlet's blueprint ID and revision.
- `catalog.command({ command_id, payload })` requires `client.commands` and
  invokes a declared, bounded server command. The host validates the caller,
  contribution, release, installation state, configuration, and grants.
- `catalog.refresh({ target: 'current_entity' })` requires `client.refresh`
  and an entity action, preview panel, or attribute decoration outlet with a
  current entity. It invalidates the host's entity-scoped views and awaits
  active refetches; it cannot select another entity or mutate server data.
  Call it after a successful command that changes the current entity.
- `catalog.dialog.open()` requires `client.action_dialog` and a version 2
  selection action; `catalog.dialog.close()` is available only inside the
  `action_dialog` frame.
- `catalog.operations.start({ operation_id, input, idempotency_key })` requires
  `client.operations.start` and is available in version 2 selection actions and
  the action dialog. The host supplies the frame's selection; the frame cannot
  name entities, a workspace, a release, or a user. `idempotency_key` is 1–64
  visible ASCII characters. It resolves to `{ run_id }`.
- `catalog.operations.list()`, `catalog.operations.get({ run_id })`, and
  `catalog.operations.download({ run_id, artifact_id })` require
  `client.operations.read`; `catalog.operations.cancel({ run_id })` requires
  `client.operations.cancel`. They see only the signed-in user's runs of the
  calling extension, even when that user is an operator: the host sends these
  requests with `scope=own`, which the server enforces, and checks the run's
  `initiated_by_me` flag. Downloads are started by the host; the frame never
  receives a URL.
- `catalog.storage.get/set/delete/list(...)` requires `storage.extension` and
  provides release-scoped extension storage. Storage requests and values are
  bounded; `set` and `delete` support an optional optimistic
  `expected_revision`.
- `catalog.context` contains only the documented outlet identifiers (the
  entity preview outlet supplies `entity_id` and optional `context_id`).
  `catalog.configuration` is supplied only when `configuration.read` is
  granted. `client.events` dispatches a `catalog:context-changed.v1` event on
  the `root` passed to `mount` at startup and after every host context update.
  The event detail and `catalog.context` are replaced together through the
  versioned private `MessageChannel`; no browser event, route state, or host
  object is exposed. A frame remains mounted across context updates, so
  extensions must discard work scoped to the prior context.
- `catalog.theme` is always supplied, without any capability, and contains `{ color_mode: 'light' | 'dark' }`, the mode Catalog is currently
  rendering (the user's explicit choice, otherwise the system preference). The
  frame document's `color-scheme` is set to match before `mount` runs, so
  native controls and system colors such as `Canvas` and `CanvasText` follow
  it. A `catalog:theme-changed.v1` event with the same detail is dispatched on
  `root` after `mount` and whenever the user switches modes; the frame stays
  mounted, so extensions should restyle rather than reload. For example:

  ```js
  export const mount = (root, catalog) => {
    const apply = () => (root.dataset.mode = catalog.theme?.color_mode ?? 'light');
    root.addEventListener('catalog:theme-changed.v1', apply);
    apply();
  };
  ```

### Mediated interaction contracts

The interaction and placement capabilities above are independent, narrow
permissions; none grants browser privileges. Every request is versioned,
schema-validated, byte-bounded, bound to the contribution's documented outlet
context, and re-authorized by the host at execution time. The host owns focus,
confirmation, notification, download, navigation, upload, error, and
accessibility UI.

`client.refresh` currently supports only `current_entity` in entity outlets;
other targets such as Explorer results require separate host implementations.
`client.confirmation` has bounded title, message, and severity. `client.download` accepts bounded data or a host artifact
reference with a validated filename and media type. `client.external_navigation`
opens only allowlisted HTTPS URLs in a new tab. `client.files.read` and
`client.files.upload` are restricted to file-detail context; upload selection
and progress are host UI. `client.search` is a bounded authorized Catalog
search, `client.live_updates` is limited to typed current-context events, and
`client.clipboard` writes bounded user-initiated text. The color mode is
available to every contribution as `catalog.theme`; `client.theme.read` is
still accepted in manifests for compatibility but is no longer required. Locale
reads return only the locale.

Placement capabilities authorize only their matching fixed outlet; they do not
imply `client.commands`. Disabling, quarantining, removing, upgrading, or
revoking a grant removes a contribution from runtime descriptors. Mounted
outlets refresh at least every 15 seconds, unmount removed frames, close their
ports, reject pending calls, and discard subscriptions; broker calls are
re-authorized immediately.

There is no generic `fetch`, URL navigation, credential/header access, DOM
bridge, event stream, or inter-extension RPC. Every mediated Catalog request
uses the signed-in browser session in the parent and the server still applies
normal authorization. Invalid messages, missing grants, failures, and startup
timeouts are denied; frame startup failures render as host-owned warnings
without exposing extension source or host internals. Runtime-descriptor
fetch failures render host-owned warning alerts while the affected outlet has no
runtime descriptor. Extension UI must provide its own localized
text and accessible labels; the host owns the surrounding landmarks, focus,
loading state, and failure announcements.

## Selection-aware actions and interactive operations (host API 1.5)

An extension can let a signed-in user process one entity from its preview or
Explorer row, or up to 50 selected Explorer entities, as a durable background
run. Core provides the action surfaces, authorization, the run, its artifacts,
and annotations; templates, rendering, result reports and document history stay
in the extension.

### Selection context

Contributions to `entity_action`, `explorer_row_action`, and
`explorer_bulk_action` may declare `"version": 2`, which requires the unified
host ABI (1.6+) or a legacy `catalog.host_api` range compatible with 1.5 but not 1.4. Version 1
contributions keep their released contexts. A version 2 contribution receives:

```json
{
  "context_version": 2,
  "selection_source": "explorer_selection",
  "blueprint_id": "<uuid>",
  "blueprint_version": 3,
  "context_id": "<uuid or null>",
  "entity_ids": ["<uuid>", "<uuid>"]
}
```

Preview and row actions use a one-item `entity_ids`. `context_id` is the
effective value-resolution context of the surface (the preview's selected
context or the Explorer context), or `null` for the workspace default.
Selections contain only saved entities from one blueprint revision in display
order; the host never widens them to hidden rows or to all matching results.
Generation reads saved data only: when a selected entity has unsaved editor
changes in the tab, the host-owned dialog says so and offers the editor.

### Interactive operations

A `server.operations` entry exposes itself to these actions with:

```json
{"id": "generate", "handler": "generate", "request_schema": {"type": "object"}, "interactive": {"version": 1, "max_selection": 50}}
```

`max_selection` is 1–50. The release needs `client.operations.start`. A unified
release (1.6+) runs it in the [unified world](#host-abi-versions-and-evolution);
a legacy release runs it in the `catalog:host@1.5.0` world at
`crates/extension-runtime/wit-interactive/catalog-extension.wit`. That world is
the released 1.4 connector world plus a `selection` interface and the
`annotate` catalog intent; the 1.4 package is unchanged.

Starting a run (through `catalog.operations.start` or
`POST /extensions/{extension_id}/{contribution_id}/operations`) checks the
enabled exact release, the contribution's capability, the declared operation,
its request schema and byte limit, and that the caller can read every selected
entity. One unreadable or mismatched entity rejects the whole selection. The
host then freezes, in one transaction, the initiating user (and token, if any),
release, operation, validated input, configuration snapshot, effective context,
contribution, and ordered membership. A context cannot be deleted while a
pending or running run reads from it. Idempotency keys are scoped to the user;
an identical retry returns the same run and a changed request with the same key
is rejected.

Inside the run:

- `selection.describe()` returns the count, blueprint revision, and context.
- `selection.page(cursor, limit)` returns 1–10 members with saved values
  resolved in the run's context, the caller's own annotations
  (`{tags, metadata, revision}`), `blueprint_id`, `blueprint_version`,
  `updated_at`, and the page's `read_at`. Pages stay under the 64 KiB host
  bound; a member that alone would exceed it is returned as `too_large`.
  Members the initiator can no longer read are `unavailable`; deleted members
  are `deleted`. Membership and access are separate: frozen membership never
  grants access.
- Generic `catalog-data.read` and the connector `catalog` calls are rejected.
  `catalog-data.batch` accepts only `update`, `relationships`, and `annotate`
  intents for members of the selection, and checks the initiator's current
  grants for each intent (`entities.write` for values, `entities.read` for
  annotations, and `entities.read` on every entity a relationship links to,
  selected or not). Writes are audited as `catalog.extensions.operations.write`
  with the initiator as actor and the extension as event source.
- Before every batch the host checks that the initiator is still an active
  workspace member (and that a token-started run's token is still live). If
  not, the run fails closed with `initiator_access_revoked` instead of
  continuing under the installer's grants. An operator can replay the
  dead-lettered run after access is restored, unless its selection context
  has since been deleted (`409`); start a new run instead. A cancellation requested
  before access was lost is still delivered to the extension's `cancel`
  export, whose host calls are checked against the initiator's current
  access; the run then ends `cancelled`, or fails closed if `cancel` fails.

Disable, quarantine, grant loss, and upgrade pause interactive runs exactly as
they pause other release-pinned runs. Existing administrative operations,
schedules, and connector jobs keep their separate contracts.

### Runs, results, and retention

Users follow their runs under **Profile → Extension runs**
(`/profile/extension-runs`) and through `GET /extension-runs`. Execution
status (`queued`, `running`, `cancelling`, `cancelled`, `completed`, `failed`)
is separate from the extension's domain outcome: a completed run can report
per-entity failures. Extensions report progress as a bounded object; the host
displays the optional fields `completed`, `total`, and
`outcome: {succeeded, failed, skipped}`. A fatal runtime failure still fails
the run.

Runs started in a tab are polled only while active and announced with a
notification that links to the run, so users can close the dialog or leave the
page. Only the initiator and `extensions.manage` operators can see a run.
Operators can read, cancel, or download any run, except with `?scope=own` on
`GET /extension-runs/{run_id}`, its `cancel` and artifact `download` routes,
which limits every caller to runs they started. Run responses include
`initiated_by_me`. The initiator can always
cancel their own run (cancelling reveals nothing about the selection), but can
read or download it only while they can still read every selected entity:
because a combined artifact may contain any member, losing access to one
member denies every download of that run. Outputs are downloadable only after the run
completes and for 30 days; history keeps the run but marks outputs expired.
Finalized outputs carry the extension's output name as the download filename.

## Extension-owned entity annotations

With the `catalog.annotations.write` capability an extension can patch its own
annotation namespace on an entity: tags named `<extension-id>:<tag>` and the
object at `system_metadata[<extension-id>]`. Callers supply only local names; the
host derives the namespace from the extension's provenance, so one extension can
never write another's. The intent is available from 1.5 operation batches and
from any `catalog.command.v1` batch (for example a client command) when the
capability is granted; an annotation-only batch does not need `catalog.write`.

```json
{"kind": "annotate", "intent_key": "doc-<run>-<entity>", "entity_id": "<uuid>",
 "add_tags": ["document-generated"], "remove_tags": [],
 "set_metadata": {"last_document": {"template_version": 2}}, "remove_metadata": [],
 "expected_revision": null}
```

Patches are bounded (1–32 operations, local names of 1–64 ASCII letters,
digits, `.`, `_`, `-`), unambiguous (a tag or key appears once), and applied
under the entity row lock against current state, so concurrent writers of
other namespaces never lose changes. Setting a key replaces its whole value.
`expected_revision` makes a write conditional; a retried intent key is
recognized as `already_applied` before the revision is compared. Applied
outcomes include `annotation_revision`. Each change is audited and emits
`entity.annotations_changed.v1`; it does not change the entity's `updated_at`,
so an extension's own bookkeeping never makes its output look stale. A tag
change runs the entity's checks and on-save enforcing rules when any of them
uses `has_tag` or `missing_tag` on the entity, and is rejected like any other
write that would violate them. Do not
store signed URLs or secret inputs in annotations, and do not treat a tag as
proof that a download is available: resolve that through the run and artifact
APIs.

A namespace is claimed on the extension's first annotation write. If entities
already carry data under that name, the write is rejected until an operator
adopts it with `POST /extensions/{extension_id}/annotation-namespace`
(inventory with `GET`); adoption never renames or deletes existing values. The
names `attricat`, `attricat.sample`, `catalog`, `core`, and `system` cannot be
claimed. Claims are kept across disable, upgrade, and removal, and annotations
are preserved. Generic writes (entity create and `PUT`, duplicate, workflow
annotation actions, and legacy extension `create`/`upsert` fields) cannot
change a claimed namespace. Operators repair or clean up a namespace on one
entity with `POST
/extensions/{extension_id}/annotation-namespace/entities/{entity_id}`.

## Context-aware catalog APIs (host API 1.1)

`catalog:host@1.0.0` remains a supported immutable ABI. New components may use
`crates/extension-runtime/wit-next/catalog-extension.wit` (`catalog:host@1.1.0`); manifests use
`catalog.host_api` SemVer ranges and are never silently downgraded. The v1.1
WIT interface has typed `read`, `write`, `scoped-configuration-get`, and
`scoped-configuration-set` functions. Dynamic attribute and configuration
values are JSON strings bounded to 64 KiB; identifiers, scope kinds, response
shapes, and write selectors are typed WIT records and variants. Components have
no WASI, network, filesystem, database, browser credential, or ambient host
access.

Server components granted `catalog.read` can request an entity, direct current
values, or resolved values for a supplied entity/context pair. Every typed read
response includes the workspace-owned entity, its pinned `BlueprintWithAttributes`
metadata, direct values, and resolved values when requested. Resolved reads use
Catalog's existing context/fallback path rather than a copy in the extension.
`catalog.write` permits only validated scalar writes with an explicit context
ID; ordinary attribute/type/schema validation, audit records, and the domain-event
outbox remain in force. A write made while handling an event inherits the
triggering write's user and token audit attribution, keeps that event's
correlation ID, uses its ID as causation, and is published with `source_kind:
plugin` and `source_name: extension:<extension-id>`. Handlers should ignore their own
extension source name to prevent feedback loops.

This mediation is required: an extension host API must never issue catalog-table
SQL or implement its own validation, projection, audit, or event logic. Every
extension-initiated catalog mutation must use `CatalogMutationService` and its
repository-backed transaction so validation, audit evidence, initiating-actor propagation, and
the transactional outbox cannot be bypassed.

A manifest may declare `scoped_configuration` with an object schema, positive
version, and `blueprint` and/or `attribute` scopes. It requires
`configuration.write`. Catalog persists these values separately from blueprint
schemas and verifies the workspace-owned blueprint revision and attribute on
every get/set. Both attribute-scoped and blueprint-scoped values are
operational. Values are release-bound and
inaccessible after disable,
quarantine, grant changes, or upgrade.

A server manifest may declare bounded `server.commands` (stable ID, handler,
object request/response schemas, and byte limits). They require
`client.commands`. The only browser path is the opaque-frame MessageChannel
`catalog.command`; Catalog validates the command and caller session, then
rechecks contribution, enabled state, exact release, configuration, and grants.
No extension receives browser cookies, routes, or arbitrary fetch access.

The additional fixed embedded outlets are
`blueprint_attribute_configuration` (requires `client.blueprint_configuration`),
`entity_attribute_decoration` (requires `client.entity_decoration`), and
`entity_action` (requires `client.entity_action`). Their contexts contain only
the relevant catalog IDs: blueprint/revision/attribute, or
entity/attribute/context. Extensions cannot provide DOM selectors, arbitrary
host routes, React state, or inter-extension RPC.

The `entity_header_action` outlet mounts compact extension actions immediately
before the entity's host-owned toolbar. Its strict v1 context is
`{ "context_version": 1, "entity_id", "blueprint_id", "blueprint_version" }`;
the host shows one primary and three secondary actions before overflow. It
requires `client.entity_header_action`; commands still need `client.commands`.
The `explorer_row_action` outlet is host-controlled overflow UI for one entity;
its strict v1 context is `{ "context_version": 1, "entity_id", "blueprint_id",
"blueprint_version" }`. The `blueprint_detail_panel` outlet is a host-owned,
read-only detail-page region with strict v1 context `{ "context_version": 1,
"blueprint_id", "blueprint_version" }`. These contexts deliberately exclude
search state, arbitrary entity values, and browser page state. Commands from an
action still require `client.commands` and use the existing validated,
authorized command broker.

## Reference extension and connector compatibility suites

When the sibling `../../attricat-connector-csv` checkout is available, the
packaged CSV connector can be exercised against this host from the repository
root:

```sh
(cd ../../attricat-connector-csv && just pack)
ATTRICAT_CONNECTOR_CSV_ARCHIVE="$(pwd)/../../attricat-connector-csv/dist/attricat-connector-csv-0.1.0.tar.zst" \
  cargo test -p api --test extensions packaged_csv_connector_exports_through_the_real_host
```

This test installs the actual packaged component into a migrated PostgreSQL
workspace, grants its capabilities, runs an export, a 17-row multi-batch import and a multi-batch export
through the production Wasmtime task handler, restarts the runtime between
batches, and verifies output/download and 30-day retention. The
`packaged_v14_transfer_import_rejects_ssrf_without_network_io` test packages
a second component and exercises the actual v1.4 transfer WIT denial path.
When public HTTPS is available, set `ATTRICAT_PUBLIC_HTTP_TRANSFER_TEST=1`
and run `packaged_v14_public_http_transfer_and_redirect_policy` to exercise
real bounded HTTPS input, denied redirects and idempotency-keyed PUT delivery
through that packaged component. The test requires `httpbin.org`; it is
opt-in to keep offline CI deterministic. For an
S3-backed deployment, side-load the same archive via `acli extension sideload
--file ...`, grant `catalog.read`, `catalog.write`, `artifacts.read`,
`artifacts.write`, enable the release, then POST an export or import to
`/extensions/attricat-connector-csv/operations` and inspect the run and
`/extension-operation-runs/{run_id}/artifacts` afterward. The run list returns
the most recent 200; `GET /extension-operation-runs/{id}` retrieves an older
run by ID. Run history includes `schedule_id` and `outputs_expired`
without exposing input or secrets.

When available, the sibling checkout at `../../attricat-extension-example`
contains the packaged `attricat-extension-example` formula component using the
released `catalog:host@1.1.0` ABI and sandboxed client contributions. It
responds to `entity.updated.v1` by writing a calculated numeric attribute.

The same checkout's `just pack` also writes `dist/reference-documents.tar.zst`
(`attricat.reference-documents`), which exercises selection-aware actions: grant all of
its permissions and enable it, then use **Generate document** from an entity
preview, an Explorer row, or an Explorer selection. It renders PDFs (separate,
ZIP, or combined) in the 1.5 operation world, writes a `report.json` with
per-entity results, and annotates entities whose output was finalized. An
entity without values fails rendering on purpose, which exercises partial
failures without failing the run.

Run the real-host compatibility suite only against a worktree-local stack after
creating an owner personal API token:

```sh
just setup
just dev # separate terminal
source .worktree
export CATALOG_TOKEN=... # owner token for this worktree
just test-reference-extension-e2e
```

The suite builds and packages the maintained sibling example archive, side-loads
it through the public CLI, grants every manifest-declared capability, enables
it, and downloads a declared client contribution. It creates a fresh formula
blueprint and entity, updates a dependency in the default context, and waits
for the real event handler's calculated write in that same context. It uses no
repository/runtime mocks. The caller's token needs extension management,
blueprint write/publish, entity write/read, and audit-read permissions.
