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
`client.audit_event_panel`, `client.data_health_card`, `network.request`, and
`webhooks.receive`.

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
descriptors. Server WASM execution, client components, storage, commands, and
host API 1.1 are implemented. Mediated extension-owned event publication is
implemented; mediated network/secrets APIs and webhook delivery remain follow-on work.

## Server WASM runtime (#145)

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
components with `storage.extension`. `secrets.get.v1`, `catalog.read.v1`,
`catalog.command.v1`, and `network.request.v1` are recognized and
capability-checked but are not implemented by this deployment. `events.emit.v1`
is implemented only for a manifest-declared, per-contract event export as
described in [Inter-extension events](#inter-extension-events). In particular,
`network.request.v1` never grants ambient sockets. The manifest
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
  values.
- **`explorer_table_cell`** (`embedded`, requiring
  `client.explorer_table_cell`) is reserved for a future sandboxed Explorer
  table-cell renderer. The current web runtime does not mount this outlet.
- **`blueprint_detail_panel`** (`panel`, requiring
  `client.blueprint_detail_panel`) is a read-only region on a blueprint detail
  page. Its strict context is `blueprint_id`, `blueprint_version`, and
  `context_version: 1`, making it suitable for blueprint-level status,
  validation results, or documentation.

All of these frames use the same mediated `catalog` API and capability checks
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

### Mediated interaction contracts

The interaction and placement capabilities above are independent, narrow
permissions; none grants browser privileges. Every request is versioned,
schema-validated, byte-bounded, bound to the contribution's documented outlet
context, and re-authorized by the host at execution time. The host owns focus,
confirmation, notification, download, navigation, upload, error, and
accessibility UI.

`client.refresh` may target only the current entity, Explorer result set, or a
documented dashboard resource. `client.confirmation` has bounded title,
message, and severity. `client.download` accepts bounded data or a host artifact
reference with a validated filename and media type. `client.external_navigation`
opens only allowlisted HTTPS URLs in a new tab. `client.files.read` and
`client.files.upload` are restricted to file-detail context; upload selection
and progress are host UI. `client.search` is a bounded authorized Catalog
search, `client.live_updates` is limited to typed current-context events, and
`client.clipboard` writes bounded user-initiated text. Theme and locale reads
return only safe tokens, color mode, and locale.

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

The `explorer_row_action` outlet is host-controlled overflow UI for one entity;
its strict v1 context is `{ "context_version": 1, "entity_id", "blueprint_id",
"blueprint_version" }`. The `blueprint_detail_panel` outlet is a host-owned,
read-only detail-page region with strict v1 context `{ "context_version": 1,
"blueprint_id", "blueprint_version" }`. These contexts deliberately exclude
search state, arbitrary entity values, and browser page state. Commands from an
action still require `client.commands` and use the existing validated,
authorized command broker.

## Current implementation limitations

Mediated network, secrets, request/response calls, and webhook-delivery
functionality remain deferred as described above.
