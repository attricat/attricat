# Solution Packs

> **Status:** v1 archive validation, administrator inspection, immutable
> create-only planning, and durable application/history for blueprints are
> implemented. Packs may also declare bounded extension
> requirements, non-secret configuration templates, Explore navigation defaults,
> bounded workspace extension layouts, and entity-blueprint extension layouts.
> Administrators upload `.tar.zst` archives; broader resource types remain deferred.
>
> **Scope of this document:** Sections that describe existing-resource
> adoption, updates, workspace settings other than the two bounded defaults,
> assets, sample data, prerequisites, export, or ownership are future target
> design, not implemented v1 behavior. The explicitly marked v1 sections below
> define the current product contract.

A solution pack is a versioned, declarative bundle of catalog structure,
workspace defaults, extension requirements, assets, and setup guidance. Packs
let a workspace begin with a reviewed domain foundation—such as Ecommerce or
Warehouse—without copying another workspace's database state.

Authorized workspace administrators currently inspect packs and create or review
dry-run plans through the Catalog CLI. Solution packs are an administrative
bootstrap and authoring mechanism,
not an end-user feature. Ordinary workspace users never need to discover,
select, configure, inspect, or otherwise interact with a pack; they interact
only with the resulting blueprints, navigation, extensions, and functionality.
The server remains authoritative for validation, authorization, planning, and
execution; the CLI does not mutate the database or unpack resources into a
workspace directly.

A pack is a starting point or template for a set of functionality. It is not a
database export, backup, executable installer, or a way to bypass extension
approval. It contains portable intent expressed through stable logical
identifiers. Catalog resolves that intent to workspace-owned resources through
a reviewed application plan. Once applied, those resources and settings belong
to the workspace rather than remaining managed by the pack.

## Target design principles (v1 implements the create-only subset)

- **Administrative:** packs are visible and operable only through authorized
  administration workflows; ordinary users see only the resulting workspace.
- **Declarative:** a pack describes desired resources and references; it does
  not contain SQL, scripts, WASM, or arbitrary lifecycle hooks.
- **Portable:** content never depends on workspace UUIDs, database IDs, object
  storage keys, or deployment URLs.
- **Reviewable:** every application begins with a dry-run plan that shows each
  create, map, update, skip, permission request, and conflict.
- **Safe by default:** collisions create choices rather than implicit adoption,
  renaming, or replacement. Destructive and breaking changes require explicit
  approval.
- **Repeatable:** applying the same release with the same choices is idempotent.
- **Composable:** a workspace may install multiple packs without implicit
  precedence or last-writer-wins behavior.
- **Versioned:** pack releases and their resource definitions are immutable;
  applying one does not freeze or subordinate the resulting workspace state.
- **Traceable:** Catalog records pack identity and version, archive digest,
  selected options, mappings, actions, and application history.
- **Trust preserving:** trusting a pack does not trust, enable, configure, or
  grant permissions to an extension named by that pack.

## Future target content at a glance

The following table is roadmap design beyond the implemented v1 blueprint and
bounded workspace-setting boundary.

| Content | Target pack behavior |
| --- | --- |
| Blueprints, attributes, relationships, and views | Declared through logical keys and compiled into ordinary versioned blueprint definitions. |
| Contexts and publication-channel defaults | Not pack content. Contexts are administrator-managed workspace operating structure and are never created, mapped, updated, or referenced by packs. |
| Extension requirements | **Implemented subset:** declare installed-package ID/version compatibility and an optional bounded non-secret literal JSON configuration template. Stable manifest contribution references may supply workspace and new entity-blueprint layout defaults. Installation, upgrade, configuration, grants, and enablement remain separate ordinary approvals. |
| Workspace defaults | **Implemented subset:** append pinned Explore entries and compatible extension contributions without replacing unrelated navigation, layout data, or settings. Other settings remain deferred. |
| Branding, themes, and static assets | Install only declared, digest-verified files of supported media types for host-defined purposes. |
| Documentation and setup | **Implemented subset:** bounded safe Markdown guidance, release notes, a structured checklist, and six host-defined informational checks with immutable run history. |
| Sample data | Optional, separately selected, visibly marked, portable, and idempotently mapped; never treated as production configuration. |

Not every content type ships in the current implementation. The
[initial delivery boundary](#initial-delivery-boundary) implements blueprint creation, bounded Explore navigation and extension-layout defaults,
entity-blueprint extension layouts, read-only extension requirement evaluation,
and bounded guided setup with informational check runs.

## Package and distribution

Solution packs use the same `.tar.zst` container format as extensions, but have
an independent manifest and validation contract. A pack archive has one
`solution-pack.json` at its root. It must not be interpreted as an extension
archive, even if it also contains a file named `manifest.json`.

The `.tar.zst` file is the only installation input. An administrator obtains an
immutable pack archive through an out-of-band distribution process and uploads
that file with `acli`. Catalog does not discover packs, access source
repositories, resolve release tags, fetch URLs, or manage repository
credentials. A pack's private source repository may remain its authoring,
documentation, and release home, but it is outside the Catalog protocol and
trust boundary.

Every uploaded archive receives the same schema, path, size, media-type,
compatibility, digest, and content validation. Catalog identifies the input by
the pack ID and version declared in the manifest plus the server-computed
whole-archive digest. It never relies on a file name or an unverified repository
claim for identity or provenance.

Before planning, Catalog validates at least:

- bounded compressed and expanded sizes, entry counts, individual files, and
  blueprint/include/attribute complexity;
- relative UTF-8 file paths without traversal, links, devices, or duplicates;
- exactly one strict, supported solution-pack manifest;
- a matching immutable pack ID and SemVer release;
- declared files, media types, and cryptographic digests;
- declared pack identity and version plus the whole-archive digest;
- host compatibility and supported resource contract versions;
- uniqueness and referential integrity of logical resource keys; and
- schemas for blueprints, settings, templates, checks, and sample data before
  any workspace mutation.

Unknown manifest fields are rejected at the contract version where they occur.
Archive validity does not imply that its proposed changes are safe for a
particular workspace; the planner decides that separately.

## Manifest

The manifest owns package metadata and indexes content files. Large resource
bodies live in declared files rather than being hidden in the manifest. The
following example is illustrative of the v1 contract; the implementation must
publish a strict JSON Schema before accepting archives:

```json
{
  "manifest_version": 1,
  "id": "attricat.ecommerce",
  "name": "Ecommerce Catalog",
  "version": "1.2.0",
  "description": "Product, category, brand, and channel foundations.",
  "catalog": {
    "host_api": ">=1.0.0 <2.0.0"
  },
  "documentation": {
    "readme": {"path": "README.md", "sha256": "<lowercase-sha256>"},
    "release_notes": {"path": "RELEASE_NOTES.md", "sha256": "<lowercase-sha256>"},
    "setup_checklist": {"path": "setup/checklist.json", "sha256": "<lowercase-sha256>"}
  },
  "checks": {"path": "checks/checks.json", "sha256": "<lowercase-sha256>"},
  "resources": {
    "blueprints": [{
      "key": "blueprints/product",
      "path": "blueprints/product.toml",
      "required": true,
      "sha256": "<lowercase-sha256>"
    }],
    "workspace_settings": [{
      "key": "workspace/explore-navigation",
      "path": "workspace/explore-navigation.json",
      "required": false,
      "sha256": "<lowercase-sha256>"
    }]
  },
  "extensions": [{
    "key": "extensions/shopify",
    "id": "acme.shopify",
    "version": ">=2.1.0 <3.0.0",
    "required": false,
    "configuration_template": {
      "path": "extensions/shopify.json",
      "sha256": "<hex digest>"
    }
  }]
}
```

Every resource entry has a pack-local `key`, a declared resource contract, and
an explicit required/optional state. Pack-global references combine the pack ID
and key, for example:

```text
attricat.ecommerce/blueprints/product
attricat.ecommerce/blueprints/product/attributes/sku
attricat.ecommerce/extensions/shopify
```

Keys are immutable after publication. Renaming a key means removing one logical
resource and adding another and must be treated as such when planning a later
release. Display names are not identifiers.

## Logical identifiers and workspace mappings (future beyond v1 creation)

Pack files reference logical keys, never workspace UUIDs or assumed physical
codes. During application, the planner resolves every key to one
workspace-owned resource and records that mapping in the immutable application
record.

For a new globally code-addressed resource, an administrator chooses or accepts
a pack prefix. The planner can then produce codes such as `ecom_product` and
`ecom_category`. Attribute codes such as `sku` remain local to
the mapped blueprint, but references to them still use logical attribute keys.
Relationships, views, templates, layouts, checks, and settings are compiled
through the same stored mapping.

Contexts are administrator-managed workspace operating structure, not solution-pack
resources. Pack manifests cannot declare context keys or hierarchy references,
and application plans never create, map, or update contexts.

If a proposed code or logical purpose collides, the plan requires one of these
explicit choices:

1. create a new resource with the proposed pack-prefixed code (the safe
   default);
2. map to an existing compatible resource after structural validation;
3. select a different prefix or physical code; or
4. skip an optional component and everything that requires it.

Catalog never silently overwrites, renames, or maps an existing blueprint,
setting, or asset. A successful application preserves its mapping as
provenance. A later application may use that recorded mapping as a planning
input, but it does not make the resource pack-owned or prevent ordinary
workspace changes. This allows packs with overlapping local names to coexist.

The plan distinguishes a newly created resource from a compatible existing
resource selected by the administrator. That distinction is historical
provenance only. Both become or remain ordinary workspace-owned resources as
soon as the plan is applied.

## Multiple packs in one workspace

The implemented v1 can apply multiple create-only plans when their target codes
do not collide. Explore-navigation and extension-layout entries compose through
their bounded item-level merge rules: absent entries append, exact entries are
no-ops, and incompatible placements conflict without replacing unrelated data.
Compatible installed contributions may be shared as declarative availability,
but packs never mutate their lifecycle. Existing-resource compatibility mapping,
configuration composition, and generic workspace-setting composition described
below remain future design.

A workspace may apply multiple solution packs. This is a core composition
requirement, not an exceptional migration path: for example, one workspace may
combine Ecommerce, DAM, SEO, marketplace, and warehouse templates.

Each application has its own prefix, mapping snapshot, and provenance record.
Physical codes must be unique across the workspace, but packs may have
overlapping local key names because references are qualified by pack ID and
resolved while each plan is created.

Installed extension contributions and exact bounded workspace-setting entries
can be shared by multiple plans through `satisfied` actions. Mapping a second
pack to a compatible blueprint produced by an earlier application is
future design. No application gives a pack continuing ownership or creates a
permanent pack dependency; applied resources and settings remain workspace-owned.

Implemented composition follows these rules:

- plans evaluate current target codes, bounded settings, and installed extension
  release snapshots;
- no pack receives implicit precedence because it was applied most recently;
- Explore-navigation and extension-layout entries merge only through their
  registered item-level keys and merge rules;
- incompatible bounded setting proposals become conflicts rather than
  last-writer-wins updates;
- extension version requirements and contribution declarations must match the
  exact compatible release selected when the plan was created; and
- extension grants, configuration, and enablement remain ordinary workspace
  decisions outside pack application.

Generic settings, existing-resource adoption, configuration composition, and
prior-application compatibility mapping remain future design.

Applying the same pack release with the same choices is idempotent. Applying a
different release of the same pack creates a new reviewed application plan; it
is not a second managed instance and does not establish an upgrade relationship
with resources produced by the earlier release.

## Pack contents (v1 blueprints, settings, extensions, and guidance)

### Blueprints and views

A pack may declare entity and mixin blueprints, attributes, includes,
relationships, JSON Schema validation, dropdown views, detail/edit/table views,
and entity-owned extension layout defaults described in
[Blueprint authoring](blueprints.md) and [View configuration](views.md).

Pack authoring uses logical references for blueprint, include, relationship,
and attribute targets. During planning, Catalog resolves those references and
compiles a native blueprint definition using the application's mapped codes.
The resulting definition must pass the ordinary blueprint compiler.

An application may create and publish a new blueprint only when the plan
explicitly says so. A later pack release never edits a published revision in
place; if selected, its plan creates a draft successor and shows schema and
entity-migration consequences before publication.

### Context boundary

Contexts are ordinary, administrator-managed workspace operating structure and
are never solution-pack content. The manifest has no `resources.contexts`
field; strict manifests that include it are rejected as unknown. Pack logical
references cannot use context keys, and planning or
application cannot create, map, update, or reference a context hierarchy.
Guidance and checks do not gain an implicit context prerequisite contract in v1.

### Extension requirements

A pack may name at most 64 unique required or optional extension IDs and SemVer
ranges, each with an optional literal JSON configuration template. Planning
reports whether the workspace's installed release is compatible and whether the
declared non-sensitive configuration subset matches. It never proposes an
extension source, installation, upgrade, grant, configuration write, or
lifecycle transition. Extension packages remain independently sourced and
validated through ordinary administrator workflows.

A pack cannot grant permissions, approve host access, bypass extension source
rules, inject extension artifacts, or enable an extension. Disabled compatible
installations can satisfy a requirement; quarantined installations cannot.
Required operator approvals remain effective even for an official pack.
Compatible manifest-declared UI contributions may be referenced by stable
`<extension-id>:<contribution-id>` keys in the bounded layout defaults described
below. Guided setup may refer to these declarations but never mutates extension
lifecycle state.

### Extension configuration templates and inputs

The implemented template subset is a bounded literal JSON object. Object
matching is recursive containment; scalar and array values match exactly. There
is no interpolation, typed input language, logical-reference resolution, or
mutation behavior. Template files are public pack material, so authors must not
put sensitive values in them. Catalog rejects a bounded denylist of secret-like
keys as defense in depth, but this heuristic is not proof that arbitrary values
are non-sensitive. Template and installed-configuration values are private to
validation and are never included in inspection, plan, application, or audit
responses.

Typed installer inputs, secret references, and post-install setup steps remain
future design. Secret values, access tokens, passwords, private keys, and
connection credentials must not be stored in a pack archive or solution-pack
plan.

### Workspace defaults (Explore navigation and extension layout)

V1 accepts at most two `workspace_settings` resources, one of each fixed kind.
Explore navigation uses key `workspace/explore-navigation` and path
`workspace/explore-navigation.json`. The strict file has `format_version: 1`,
`kind: "explore_navigation"`, and 1–64
entries. Each entry contains a declared entity-blueprint logical key and at most
16 unique role **codes**; UUID references and unknown fields are rejected.

The planner maps logical blueprint keys to deterministic physical codes. An
absent entry is appended, an exact entry (including the canonical role-code set)
is satisfied without a write, and a different role list for the same blueprint
is a conflict. Required unavailable blueprints or unknown roles block a plan;
optional unmet entries are skipped while other valid entries remain actionable.
Pack-created targets require `publish`.
Existing order is preserved and new entries are appended in file order. Malformed
or duplicate existing navigation is a non-ready conflict and is never rewritten.

Application locks the workspace row shared with ordinary navigation replacement,
then revalidates entry absence/exactness, published entity blueprints, and role
codes. This prevents lost updates and makes retries idempotent. Only bounded
entry summaries and outcomes are retained; unrelated navigation and every other
workspace setting are preserved.
Extension layout uses key `workspace/extension-layout` and path
`workspace/extension-layout.json`. Its strict v1 file contains 1–64 entries with
a stable contribution key, manifest outlet, required flag, primary placement
(`hidden` or ordered), and navigation-only `promoted` flag. Contribution keys
are semantically unique across the file even when two JSON entry objects differ;
exact duplicate objects are additionally rejected by the published JSON schema.
A hidden contribution cannot also be promoted. Planning accepts
only a contribution declared at the exact compatible installed release;
disabled releases can satisfy availability, while missing, incompatible,
quarantined, policy-denied, missing-contribution, or wrong-outlet entries are
unmet. Required unmet entries block and optional unmet entries skip. Exact
placements are no-ops, absent placements append in file order, and placement or
promotion mismatches conflict. Application revalidates and locks before merging,
preserving unrelated entries, outlet order, and every other setting. It never
installs, upgrades, configures, grants, enables, or otherwise changes an
extension.

Entity-blueprint `extension_layout` views are also accepted for newly created
pack blueprints, limited by the ordinary compiler to entity-owned outlets.
References are checked against the same immutable release snapshot. Optional
extension requirements remove only unavailable layout keys; required unavailable
keys block blueprint creation. Existing blueprints are never updated or adopted.

Branding/assets, generic settings, adoption, ownership, and uninstall remain out
of scope.

### Branding, themes, and static assets

A pack may contain declared presentation assets with a logical key, file path,
media type, digest, purpose, and bounded size. Catalog accepts only supported
media types, verifies file signatures where applicable, sanitizes formats such
as SVG, and stores assets through normal private object storage. Archive paths
and object-store keys are never runtime resource identifiers.

Assets are presentation content only. HTML, JavaScript, executable files,
remote asset URLs, active embeds, fonts with unverified licensing, and files
not declared by the manifest are not installable pack assets.

### Documentation, checklist, and validation checks

A pack may declare digest-bearing README (64 KiB), release notes (32 KiB), a
strict JSON setup checklist (64 KiB and 64 items), and a strict JSON checks file
(128 KiB and 64 checks). Markdown is persisted as normalized UTF-8 source.
Raw HTML, images, executable/code constructs, autolinks, and links other than
same-document fragments are rejected. Consumers must render with HTML disabled.
Checklist keys use `checklist/<key>` and may refer to one declared `checks/<key>`;
there is no manual completion state or checklist mutation API. The file referenced
by the manifest's `checks` member has this separate shape:

```json
{
  "format_version": 1,
  "checks": [{
    "key": "checks/product-published",
    "title": "Product is published",
    "predicate": {
      "type": "blueprint_published",
      "blueprint": "blueprints/product"
    }
  }]
}
```

Checks are strictly one of `blueprint_published`, `extension_installed`,
`extension_enabled`, `extension_configuration_matches`,
`explore_navigation_entry_present`, or
`workspace_extension_layout_placement_present`. Their operands are pack logical
keys or one declared workspace layout contribution. Unknown fields and
unresolvable archive references are rejected; skipped optional application
mappings evaluate false with `not_resolvable` evidence.

All checks are informational. False results are successful evaluations and
never change plan readiness, application completion, resources, settings, or
extension lifecycle. A completed application receives one idempotent
`post_apply` run. Administrators may create later `manual` runs against current
tenant state. Each run, all ordered results, actor/token identity,
request/correlation IDs, and its audit event commit atomically; evaluator errors
leave no partial run. Definitions and configuration templates remain private.
Public responses expose only bounded guidance, check key/title/type, run counts,
and safe result evidence—never resource source, extension manifests, installed
configuration, or template values.

### Optional sample data

Sample data is a separate, optional component and is never installed by
selecting production configuration alone. It uses pack-local entity keys and
logical resource references, must pass normal entity and relationship
validation, and is visibly labelled sample content.

Sample data cannot contain customer exports, personal data, credentials,
production endpoints, file object keys, or opaque database IDs. Reapplying a
release uses recorded sample-entity mappings and must not duplicate entities.
After application, sample entities are ordinary workspace data and are not
removed through a pack-level operation.

## Content that packs must not include

Packs cannot contain or confer authority through:

- workspace, user, role, entity, context, blueprint, release, or installation
  UUIDs copied from another workspace;
- database dumps, SQL, migrations, triggers, procedures, or direct table data;
- executable install, upgrade, validation, or uninstall scripts;
- extension binaries disguised as pack assets;
- secrets, credentials, session material, signed URLs, or secret defaults;
- arbitrary authorization grants or membership changes;
- arbitrary workspace-settings replacement;
- undeclared remote downloads; or
- customer production data in an official or reusable pack.

## Options and dependency rules (future design)

Optional components and installer options form a declared dependency graph.
The manifest states requirements and incompatibilities using logical keys.
Catalog rejects cycles and an option set that omits a transitive requirement.
Changing an option after application requires a new plan, not an in-place toggle.

A pack may declare that another pack release is a prerequisite template by
immutable pack ID and SemVer range. Prerequisites never resolve by display name,
repository name, or overlapping resource keys. The planner validates them
against successful application records and does not silently apply a transitive
pack. Missing prerequisites, version conflicts, and dependency cycles block the
current plan. This check applies only when the new template is applied; it does
not create ongoing ownership or lifecycle coupling between the packs.

Defaults must be deterministic and safe. Security-sensitive choices—extension
installation, grants, enabling, destructive changes, and sample-data import—are
never selected only because a pack author marked them as default.

## CLI administration

Solution-pack inspection, dry-run planning, application, and application history are administrator-only CLI workflows.
The implemented inspection command uploads an archive to the authoritative
server for read-only validation and returns only safe metadata and resource
summaries:

```sh
acli solution-pack inspect --file pack.tar.zst
```

Inspection does not persist the archive, apply resources, or expose blueprint
source or normalized resource payloads in its response. The implemented uploaded-archive
planner uses an explicit prefix and blueprint publication preference:

```sh
acli solution-pack plan --file pack.tar.zst --prefix ecom --blueprint-publication draft
acli solution-pack plan show <plan-id>
acli solution-pack apply <plan-id>
acli solution-pack applications list
acli solution-pack applications show <application-id>
acli solution-pack checks rerun <application-id>
acli solution-pack checks list <application-id> --limit 25 --offset 0
acli solution-pack checks show <application-id> <run-id>
```

`draft` and `publish` describe what a later apply operation would do; planning
never creates or publishes a blueprint. Prefixes are 1–32 bytes, begin with a
lowercase ASCII letter, contain only lowercase letters, digits, and underscores,
and do not end in an underscore. Optional v1 resources are conservatively
skipped; a selected required resource whose dependency would be skipped is
blocked. Existing target codes are conflicts and are never adopted or replaced.
V1 pack blueprints reject workspace-role publication policies and
extension-provided table renderers. Entity-owned extension layouts are validated
against tenant-scoped immutable installed-release manifests. Planning snapshots
the exact release, lifecycle state, policy compatibility, declared contributions,
and configuration needed for requirements; role and grant state is not part of
requirement satisfaction.

Application accepts only an immutable plan ID and never recomputes choices from
CLI flags. Each create step commits its ordinary Catalog mutation together with
durable step evidence; retries verify completed targets and continue pending
steps without duplicate resources. Planning remains separate from application;
there is no repository-selection or remote-fetch path.

The CLI authenticates normally and the API requires a dedicated
`solution_packs.manage` permission, initially granted to workspace owner and
administrator roles. Possession of an archive or CLI access is not authority to
inspect, plan, or apply it; every operation is authorized by the server.

## Planning and application (implemented v1)

Validation produces no workspace changes. An authorized administrator uploads a
`.tar.zst` archive and creates an immutable, workspace-scoped plan with a prefix and
`draft` or `publish` blueprint choice.

V1 actions are:

- `create`: create a required blueprint at its persisted target;
- `append`: append absent Explore navigation or extension-layout entries;
- `satisfied`: record exact workspace-setting entries without changing them;
- `skip`: omit an optional resource or unmet optional navigation/layout entry;
- `conflict`: a required target code, navigation entry, or contribution
  placement/promotion conflicts with current workspace state; or
- `blocked`: a selected dependency or required navigation/contribution reference
  is unavailable, incompatible, quarantined, policy-denied, or absent from the
  exact installed release manifest.

V1 does not emit `adopt`, `update`, or `keep`. Candidate mappings remain visible
in the plan, including skipped resources. The application mapping snapshot
contains mappings for executed `create`, `append`, and `satisfied` actions.

Extension requirements are immutable plan evidence, not application steps. A
compatible installed release satisfies a requirement when it is enabled or
disabled and its configuration recursively contains the optional template;
object extras are ignored, while arrays and scalar values match exactly.
Quarantined, missing, version-incompatible, or configuration-mismatched releases
do not satisfy a requirement. An unmet required requirement blocks readiness.
An unmet optional requirement is shown as `skipped` and does not block the plan.
Grants, host access, workspace emergency mode, and dependency enablement are not
inspected for satisfaction.

A manifest may declare at most 64 extension requirements, with unique logical
keys, extension IDs, and template paths. Configuration templates are literal
JSON objects at most 64 KiB, depth 16, and 256 total object members/array
elements; keys are at most 128 bytes and strings at most 4 KiB. They are public
pack material and must contain no secrets. Keys equal to or ending in
`password`, `secret`, `token`, `api_key`, `private_key`, `credential`, or
`authorization` (case-insensitive) are rejected recursively. Template values
and installed configuration values are never returned by inspection,
plan, application, or audit responses. Inspection exposes only requirement and
template path/digest summaries.

Applying a pack never installs, upgrades, grants, configures, enables, disables,
quarantines, or removes an extension. Administrators remediate requirements with
the ordinary `acli extension install`, `sideload`, `upgrade`, `configure`,
`grant`, and `enable` lifecycle commands as appropriate, then upload the archive
to create a fresh plan. Apply revalidates each required requirement that was
satisfied when planned. Becoming missing, version-incompatible, quarantined, or
mismatched against the relevant configuration template makes the immutable plan
stale; a still-compatible release or enabled/disabled transition remains
satisfied. Optional skipped requirements remain skipped even if the workspace
later changes.

A new application may start only while its ready plan is unexpired. Each create
step revalidates its persisted `target_absent` code precondition. A target that
appears before or while the ordinary blueprint mutation runs makes the
application invalid with `plan_stale`; Catalog never adopts or overwrites it.
Once an application has started, plan expiry does not strand it: retries
revalidate completed and pending targets and resume durable work.

One durable application exists per immutable plan. Each resource mutation, step
result, audit event, and domain event commits atomically. Repeated and concurrent
apply requests lock the application and converge without duplicate resources.
If a client observes an ambiguous commit error, failure reconciliation checks
the exact attempted step; a step already committed as completed is continued,
not misreported against a later pending step.

Implemented application evidence contains the uploaded-archive source marker
and digest, pack ID/version, blueprint publication choice, executed mapping
snapshot, ordered blueprint/workspace-setting step results, state and
bounded diagnostics, actor token/user columns, timestamps, and
request/correlation identifiers. List
responses are compact and omit mappings and steps; show responses include both.
Neither response contains normalized payloads, blueprint definitions, archive
bytes, or secret values.

Richer source/release provenance, selected options, reused-resource evidence,
generic settings fragments, prerequisites, extension mutation outcomes, and
manual checklist completion are future design. Application records are
administrator-facing evidence, not controllers: they do not retain ownership,
lock resources, detect drift, authorize later changes, or provide uninstall.

## Later releases and workspace changes (future design)

A successful application is not a continuing desired-state declaration.
Administrators may freely edit the resulting blueprints and settings,
extension configuration, and data through their ordinary workflows. Catalog
does not label those edits as drift or try to restore the template.

A later release of the same pack is another template application. Its planner
may use a prior application record and surviving mappings to explain likely
changes, but the current workspace is authoritative. The release has no right
to overwrite earlier output. The plan classifies each proposed resource as new,
already satisfied, compatible update, or conflict and requires explicit choices
where intent is ambiguous.

In particular:

- published blueprints receive new draft revisions, never in-place edits;
- workspace modifications are never silently reset to a pack baseline;
- extension upgrades, grants, configuration, and enabling use the ordinary
  extension lifecycle and approvals;
- workspace settings preserve unrelated keys and user additions; and
- removal of a resource from a later pack release does not propose deletion of
  the corresponding workspace resource.

Application-history commands may show which release originally proposed a
resource and the result recorded at that time. They do not claim that the
current resource still matches the pack.

## No pack-level uninstall

There is no detach or uninstall operation for a solution pack as a whole. Once
a plan is applied, its blueprints, settings, assets, extensions, and
sample entities are workspace state. The application record remains as audit
and provenance history.

Administrators may later edit or remove individual resources through their
normal resource-specific commands, subject to ordinary authorization,
dependency, publication, data-retention, and destructive-change protections.
Removing an application record is not a supported way to remove resources.
Extensions are managed through the existing extension lifecycle, never removed
merely because a pack originally requested them.

## Export to draft (future design)

Export creates a draft authoring tree, never an immediately trusted release or
raw workspace dump. The exporter:

1. lets an administrator select supported resources;
2. replaces workspace IDs and physical cross-references with generated logical
   keys;
3. excludes secrets, grants, users, memberships, audit data, runtime state,
   object-store keys, and customer entities unless explicitly exporting
   sanitized sample data;
4. reports references that cannot be made portable; and
5. emits files that must pass the same pack validator before review and release.

Export does not imply ownership of third-party assets or permission to
redistribute extension packages, fonts, logos, or customer content.

## Longer-term delivery boundary

The intended delivery sequence validates the architecture with blueprint
starter packs:

1. strict manifest/archive validation;
2. logical identifiers, prefix selection, and recorded mapping snapshots;
3. multiple pack IDs coexisting through independent application records;
4. blueprint and view planning;
5. deterministic dry runs with collision and cross-pack conflict choices;
6. durable, idempotent application records and audit history;
7. administrator-only CLI commands for plan review and apply; and
8. export to an untrusted draft.

Extension dependencies, permission review, configuration templates, and Explore
navigation defaults follow the core planner. Other workspace/layout defaults,
later-release planning, sample data, and curated Ecommerce and Warehouse packs
build on those contracts. Pack archives continue to be supplied explicitly as
`.tar.zst` files; no repository-access path is planned.
