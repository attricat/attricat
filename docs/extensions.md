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
sent to `POST /extensions/sideload` as an `application/zstd` request body and
must be a `.tar.zst` extension package no larger than 32 MiB. Side-loaded
packages are treated as untrusted input: they receive the same compressed and
unpacked size limits, safe entry-path checks, strict `manifest.json`
validation, artifact staging, disabled initial state, lifecycle records, and
audit events as registry installs. Their recorded source is `sideload`; the raw
archive is never retained. Users must review configuration and explicitly grant
required permissions before enabling a side-loaded extension.

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
validate, and every required capability is known and grantable. Artifact digest
verification is deferred from the trusted-source MVP.

## Capabilities and egress

The v1 capability catalogue is: `catalog.read`, `catalog.write`,
`events.subscribe`, `events.emit`, `storage.extension`, `configuration.read`,
`secrets.read`, `logging.write`, `client.navigation`, `client.notification`,
`client.events`, `network.request`, and `webhooks.receive`.

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
WASM components receive no ambient sockets. #145 will call these host policy
checks when it implements the mediated HTTP API.

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
host-permission grant. Quarantine may occur from any installed state; a
quarantined installation must be moved to `disabled` before enabling again.
Upgrade snapshots a newly selected installed release, clears grants and
configuration, and leaves a formerly enabled installation disabled. Removal
deletes current state but retains installed-release and append-only lifecycle
history.

Each lifecycle mutation uses an explicit Rust/SQLx transaction to write the
current state, an append-only lifecycle record, and the existing generic audit
event. Safe source, manifest/release identity, prior/new state, actor,
request/correlation IDs, and bounded diagnostic codes may be recorded; secrets,
credentials, raw packages, and payload bodies may not. SQL migrations remain
declarative and contain no behavior.

The shared lifecycle work intentionally provides no management API, web UI,
external GitHub registry discovery, or client runtime. WASM execution begins in
#145; the remaining boundaries are follow-on work.

## Server WASM runtime (#145)

A `server_wasm` artifact is a WebAssembly **component** using the checked-in
`catalog:host@1.0.0` WIT package at `apps/api/wit/catalog-extension.wit`.
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
`configuration.read`; all other v1 operation names are deliberately rejected
until their command/secret/storage/network contracts are made durable. In
particular, `network.request.v1` never grants ambient sockets. The manifest
host-permission validation remains the egress policy contract for its future
mediated implementation.
Registry source APIs expose `GET/POST /extension-registries`,
`DELETE /extension-registries/{id}`, `GET /extension-registries/discover`, and
`GET /extension-registries/extensions/{owner}/{repository}`. The final endpoint
rejects a repository unless the current trusted index lists it, then returns its
README and release assets. `extensions.read` authorizes discovery while
`extensions.manage` authorizes source changes and `POST /extensions/sideload`;
the built-in official source cannot be removed. The deployment may set
`EXTENSION_OFFICIAL_REGISTRY` to a validated GitHub owner/repository instead of
the default `attricat/attricat-extensions`.

## Client extension runtime (v1)

Enabled `client_component` artifacts can expose a strict `ui` contribution:

```json
{
  "id": "inventory-panel",
  "version": 1,
  "kind": "element",
  "artifact": "client",
  "element": "acme-inventory-panel",
  "outlet": "entity_preview_panel"
}
```

A contribution is either `route` (which requires a non-empty `title`) or
`element` (which requires one of the host-owned `navigation` or
`entity_preview_panel` outlets). Routes are always namespaced at
`/extensions/:extensionId/:contributionId`; manifests cannot provide a path,
selector, or host component. Element names must be lowercase custom-element
names. Each extension can use an outlet once and all contribution/artifact IDs
remain stable across releases.

Catalog loads runtime descriptors and JavaScript only for enabled installations.
Disabling, quarantining, or upgrading an installation immediately prevents new
loads; release IDs and object-store keys are never client addresses. The host
runs every contribution in a distinct `<iframe sandbox="allow-scripts">` with
an opaque origin and a CSP that denies network access. Components register
custom elements *inside that frame*, not in Catalog's document, and have no
access to host DOM, cookies, storage, React state, or other extension frames.
The host revokes the frame port and removes the frame on unmount or load error.

### Client API

The frame receives a versioned `MessageChannel` API as `globalThis.catalog`.
It may use only granted operations:

- `catalog.navigate({ entity_id })` requires `client.navigation` and resolves
  only to Catalog's entity route.
- `catalog.notify({ message, severity? })` requires `client.notification`;
  messages are trimmed and limited to 512 characters.
- `catalog.request(path)` requires `catalog.read` and is limited to `GET`
  reads of `/api/entities`, `/api/v1/entities/:uuid`, or the exact revision
  route `/api/blueprints/:uuid/versions/:positive-version`, with a 1 MiB
  response limit. The last form lets a blueprint-configuration contribution
  inspect only the revision identified by its host-provided outlet context.
- `catalog.context` contains only the documented outlet identifiers (the
  entity preview outlet supplies `entity_id` and optional `context_id`).
  Configuration is provided only when `configuration.read` is granted.

There is no generic `fetch`, URL navigation, credential/header access, DOM
bridge, event stream, or inter-extension RPC. Every mediated Catalog request
uses the signed-in browser session in the parent and the server still applies
normal authorization. Invalid messages, missing grants, failures, and startup
timeouts are denied and rendered as a host-owned warning without exposing
extension source or host internals. Extension UI must provide its own localized
text and accessible labels; the host owns the surrounding landmarks, focus,
loading state, and failure announcements.

## Context-aware catalog APIs (host API 1.1)

`catalog:host@1.0.0` remains a supported immutable ABI. New components may use
`apps/api/wit-next/catalog-extension.wit` (`catalog:host@1.1.0`); manifests use
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
outbox remain in force. A write made while handling an event keeps
that event's correlation ID, uses its ID as causation, and is published with
`source_kind: plugin` and `source_name: extension:<extension-id>`. Handlers should ignore their own
extension source name to prevent feedback loops.

A manifest may declare `scoped_configuration` with an object schema, positive
version, and `blueprint` and/or `attribute` scopes. It requires
`configuration.write`. Catalog persists these values separately from blueprint
schemas and verifies the workspace-owned blueprint revision and attribute on
every get/set. Values are release-bound and inaccessible after disable,
quarantine, grant changes, or upgrade.

A server manifest may declare bounded `server.commands` (stable ID, handler,
object request/response schemas, and byte limits). They require
`client.commands`. The only browser path is the opaque-frame MessageChannel
`catalog.command`; Catalog validates the command and caller session, then
rechecks contribution, enabled state, exact release, configuration, and grants.
No extension receives browser cookies, routes, or arbitrary fetch access.

The fixed element outlets are `blueprint_attribute_configuration` (requires
`client.blueprint_configuration`), `entity_attribute_decoration` (requires
`client.entity_decoration`), and `entity_action` (requires
`client.entity_action`). Their contexts contain only the relevant catalog IDs:
blueprint/revision/attribute, or entity/attribute/context. Extensions cannot
provide DOM selectors, arbitrary host routes, React state, or inter-extension
RPC.
