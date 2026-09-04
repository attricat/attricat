# Extension manifest and lifecycle (v1)

Extension v1 separates a package's **manifest syntax** (`manifest_version: 1`),
its SemVer **release** (`version`), and the SemVer range for callable Catalog
host functions (`catalog.host_api`). Event, capability, configuration, and UI
contribution versions are independent contracts. A host never coerces or
downgrades any of these contracts.

Extension repositories and `.tar.zst` release-archive discovery are external to
Catalog. Every workspace has the built-in Attricat official GitHub registry plus
zero or more workspace-managed GitHub `owner/repository` sources. Sources may be
submitted as `github:owner/repository`, `owner/repository`, or the canonical
`https://github.com/owner/repository` URL; Catalog canonicalizes them and rejects
non-GitHub origins, credentials, query/fragment suffixes, and ambiguous paths.
Discovery queries GitHub Releases directly, ignores drafts and prereleases, and
returns only `.tar.zst` assets whose download path belongs to that exact source.
Catalog stores no global copy of that catalogue. When a user installs one release, Catalog validates the
selected `.tar.zst` archive with bounded decompression and entry-path checks,
then reads and validates `manifest.json` before uploading declared extracted
artifacts to Catalog S3 storage. Artifact integrity verification is deliberately
deferred: selected release archives are trusted registry inputs in v1. Catalog
retains only that installed release's immutable manifest and source identity
alongside its workspace-owned installation; raw archives are not retained.

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

Registry source APIs expose `GET/POST /extension-registries`,
`DELETE /extension-registries/{id}`, and `GET /extension-registries/discover`.
`extensions.read` authorizes discovery while `extensions.manage` authorizes
source changes; the built-in official source cannot be removed. The deployment
may set `EXTENSION_OFFICIAL_REGISTRY` to a validated GitHub owner/repository
instead of the default `attricat/catalog-extensions`.

No web UI, client runtime,
or WASM execution is part of this issue; those are #145–#149 follow-on
boundaries.
