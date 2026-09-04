# Extension manifest and lifecycle (v1)

Extension v1 separates a package's **manifest syntax** (`manifest_version: 1`),
its SemVer **release** (`version`), and the SemVer range for callable Catalog
host functions (`catalog.host_api`). Event, capability, configuration, and UI
contribution versions are independent contracts. A host never coerces or
downgrades any of these contracts.

Extension repositories and release ZIP discovery are external to Catalog. The
configured registry list identifies GitHub repositories; a later registry layer
loads their releases to show what is available. Catalog stores no global copy of
that catalogue. When a user installs one release, Catalog validates the
selected release manifest and artifact hashes supplied by the registry flow and
retains only that installed release's immutable manifest, source identity, and
artifact hashes alongside its workspace-owned installation.

## Manifest

A manifest is strict JSON. Unknown fields at every v1 manifest object are
invalid. It requires a positive supported `manifest_version`, `name`, SemVer
`version`, `description`, a non-empty `icons` object, `catalog.id`,
`catalog.host_api`, and at least one hashed artifact. `catalog.id`, artifact,
dependency, permission-rule, handler, and contribution identifiers are stable
ASCII IDs (`A-Z`, `a-z`, `0-9`, `.`, `_`, `-`). Artifact paths are relative and
may not contain traversal segments; their SHA-256 digest is verified against
package bytes before installation.

`catalog.host_api` and dependency ranges use SemVer ranges. A package is
accepted only when the host API range matches the host's version, every
artifact validates, configuration and dependencies validate, and every
required capability is known and grantable.

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

No management API, web UI, external GitHub registry retrieval, client runtime,
or WASM execution is part of this issue; those are #145–#149 follow-on
boundaries.
