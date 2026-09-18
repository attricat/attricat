# Solution Packs

> **Status:** design contract; solution-pack installation is not implemented yet.

A solution pack is a versioned, declarative bundle of catalog structure,
workspace defaults, extension requirements, assets, and setup guidance. Packs
let a workspace begin with a reviewed domain foundation—such as Ecommerce or
Warehouse—without copying another workspace's database state.

Authorized workspace administrators plan and install packs through the Catalog
CLI. The server remains authoritative for validation, authorization, planning,
and execution; the CLI does not mutate the database or unpack resources into a
workspace directly.

A pack is not a database export, a backup, an executable installer, or a way to
bypass extension approval. It contains portable intent expressed through stable
logical identifiers. Catalog resolves that intent to workspace-owned resources
through a reviewed installation plan.

## Design principles

- **Declarative:** a pack describes desired resources and references; it does
  not contain SQL, scripts, WASM, or arbitrary lifecycle hooks.
- **Portable:** content never depends on workspace UUIDs, database IDs, object
  storage keys, or deployment URLs.
- **Reviewable:** installation and upgrade always begin with a dry-run plan that
  shows every create, map, update, skip, permission request, and conflict.
- **Safe by default:** collisions create choices rather than implicit adoption,
  renaming, or replacement. Destructive and breaking changes require explicit
  approval.
- **Repeatable:** applying the same release with the same choices is idempotent.
- **Composable:** a workspace may install multiple packs without implicit
  precedence or last-writer-wins behavior.
- **Versioned:** pack releases and their resource definitions are immutable.
  Upgrades are planned migrations between releases.
- **Traceable:** Catalog records source identity, revision, release, archive
  digest, selected options, mappings, actions, and lifecycle history.
- **Trust preserving:** trusting a pack does not trust, enable, configure, or
  grant permissions to an extension named by that pack.

## Supported content at a glance

| Content | Pack behavior |
| --- | --- |
| Blueprints, attributes, relationships, and views | Declared through logical keys and compiled into ordinary versioned blueprint definitions. |
| Contexts and publication-channel defaults | Added to the rooted hierarchy through mapped context codes; the system `default` context is never replaced. |
| Extension requirements | Declare IDs, version ranges, templates, and layout defaults; extension sourcing, validation, grants, and enablement remain separate approvals. |
| Workspace defaults | Merge schema-owned settings such as pinned Explore entries and extension layouts without replacing unrelated settings. |
| Branding, themes, and static assets | Install only declared, digest-verified files of supported media types for host-defined purposes. |
| Documentation and setup | Include Markdown guidance, release notes, structured checklist items, and non-executable validation checks. |
| Sample data | Optional, separately selected, visibly marked, portable, and idempotently mapped; never treated as production configuration. |

Not every content type must ship in the first implementation. The
[initial delivery boundary](#initial-delivery-boundary) intentionally starts
with blueprints and contexts.

## Package and distribution

Solution packs use the same `.tar.zst` container format as extensions, but have
an independent manifest and validation contract. A pack archive has one
`solution-pack.json` at its root. It must not be interpreted as an extension
archive, even if it also contains a file named `manifest.json`.

Each solution pack lives in its own private repository. The repository is the
pack's canonical source, documentation home, and release history; there is no
separate solution-pack registry or catalog index. A repository contains one
pack identity and publishes immutable `.tar.zst` assets through versioned
releases. Release metadata binds the archive to its repository, tag, and source
commit.

Canonical packs use Attricat-managed private repositories. A customer-specific
fork or derived pack uses its own private repository and release history. The
server fetches releases using deployment-managed repository access and accepts
only repositories allowed by its pack-source policy. Repository credentials
are never pack content, CLI arguments recorded in a plan, or audit payloads.
Public repositories and arbitrary archive URLs are not production sources.

Local archive upload is permitted only behind explicit development or
administrator controls for internal development and testing. A side-loaded
archive receives the same schema, path, size, media-type, compatibility, and
content validation as a repository release and is visibly marked as
side-loaded.

Before planning, Catalog validates at least:

- bounded compressed and expanded sizes, entry counts, and individual files;
- relative UTF-8 file paths without traversal, links, devices, or duplicates;
- exactly one strict, supported solution-pack manifest;
- a matching immutable pack ID and SemVer release;
- declared files, media types, and cryptographic digests;
- source identity, release provenance, and whole-archive digest;
- host compatibility and supported resource contract versions;
- uniqueness and referential integrity of logical resource keys; and
- schemas for blueprints, contexts, settings, templates, checks, and sample
  data before any workspace mutation.

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
    "readme": "README.md",
    "release_notes": "RELEASE_NOTES.md",
    "setup_checklist": "setup/checklist.json"
  },
  "resources": {
    "blueprints": [{
      "key": "blueprints/product",
      "path": "blueprints/product.toml",
      "required": true
    }],
    "contexts": [{
      "key": "contexts/web",
      "path": "contexts/web.json",
      "required": true
    }],
    "workspace_settings": [{
      "key": "workspace/explore-navigation",
      "path": "workspace/explore-navigation.json",
      "required": false
    }]
  },
  "extensions": [{
    "key": "extensions/shopify",
    "id": "acme.shopify",
    "version": ">=2.1.0 <3.0.0",
    "required": false,
    "configuration_template": "extensions/shopify.json"
  }],
  "assets": [{
    "key": "assets/logo",
    "path": "assets/logo.svg",
    "media_type": "image/svg+xml",
    "sha256": "<hex digest>"
  }],
  "checks": [{
    "key": "checks/product-published",
    "path": "checks/product-published.json"
  }]
}
```

Every resource entry has a pack-local `key`, a declared resource contract, and
an explicit required/optional state. Pack-global references combine the pack ID
and key, for example:

```text
attricat.ecommerce/blueprints/product
attricat.ecommerce/blueprints/product/attributes/sku
attricat.ecommerce/contexts/web
attricat.ecommerce/extensions/shopify
```

Keys are immutable after publication. Renaming a key means removing one logical
resource and adding another and must be treated as such by the upgrade planner.
Display names are not identifiers.

## Logical identifiers and workspace mappings

Pack files reference logical keys, never workspace UUIDs or assumed physical
codes. During installation, the planner resolves every key to one
workspace-owned resource and records that mapping in installed-pack state.

For a new globally code-addressed resource, an administrator chooses or accepts
a pack prefix. The planner can then produce codes such as `ecom_product`,
`ecom_category`, and `ecom_web`. Attribute codes such as `sku` remain local to
the mapped blueprint, but references to them still use logical attribute keys.
Relationships, views, templates, layouts, checks, and settings are compiled
through the same stored mapping.

The built-in `default` context is referenced through a reserved system key. A
pack cannot create, rename, or replace it.

If a proposed code or logical purpose collides, the plan requires one of these
explicit choices:

1. create a new resource with the proposed pack-prefixed code (the safe
   default);
2. map to an existing compatible resource after structural validation;
3. select a different prefix or physical code; or
4. skip an optional component and everything that requires it.

Catalog never silently overwrites, renames, or adopts an existing blueprint,
context, setting, or asset. Once an installation succeeds, its mappings and
physical codes are immutable. Upgrades use the recorded mappings instead of
recalculating names. This allows packs with overlapping local names to coexist.

A mapped existing resource is **adopted**, not pack-created. The installation
record preserves that ownership distinction so detach and uninstall do not
remove user-owned resources.

## Multiple packs in one workspace

A workspace may install multiple solution packs. This is a core composition
requirement, not an exceptional migration path: for example, one workspace may
combine Ecommerce, DAM, SEO, marketplace, and warehouse packs.

The initial contract permits one active installation of each immutable pack ID
per workspace. Installing another release with the same pack ID is an upgrade
or repair of that installation, not a second instance. A derived pack that must
coexist with its source pack therefore needs its own pack ID. Changing the
source repository for an installed pack ID is an explicit source-migration
operation, not a normal upgrade.

Each installed pack retains its own prefix, options, mappings, baseline, and
lifecycle. Physical codes must remain unique across the workspace, but packs
may have overlapping local key names because references are qualified by pack
ID and resolved through each installation's mappings.

Workspace resources and extensions can be shared. Installed-pack state records
a separate relationship from every pack to every mapped resource, including
whether that pack created it, adopted it, or merely requires it. A resource
created by one pack may be adopted by another only through an explicit,
compatibility-checked plan action. Creation does not give a pack exclusive
ownership or permission to remove a resource that another pack or user needs.

Composition follows these rules:

- plans evaluate all installed packs and their recorded resource requirements;
- no pack receives implicit precedence because it was installed most recently;
- workspace-setting fragments and navigation/layout entries merge only through
  their registered item-level keys and merge rules;
- incompatible setting proposals become conflicts rather than last-writer-wins
  updates;
- extension version ranges from all installed packs must have a non-empty
  intersection with an available trusted release;
- conflicting extension configuration templates require an administrator to
  choose or provide one workspace configuration;
- extension grants and enablement remain workspace decisions shared by all
  packs; and
- upgrade, detach, and uninstall operations are blocked when they would break
  another installed pack's required resource or version constraint.

A future same-pack multi-instance contract would need instance-qualified
logical keys and separate configuration semantics. It is intentionally outside
the initial design.

## Pack contents

### Blueprints and views

A pack may declare entity and mixin blueprints, attributes, includes,
relationships, JSON Schema validation, dropdown views, detail/edit/table views,
and entity-owned extension layout defaults described in
[Blueprint authoring](blueprints.md) and [View configuration](views.md).

Pack authoring uses logical references for blueprint, include, relationship,
and attribute targets. During planning, Catalog resolves those references and
compiles a native blueprint definition using the installation's physical codes.
The resulting definition must pass the ordinary blueprint compiler.

A first installation may create and publish a new blueprint only when the plan
explicitly says so. An upgrade never edits a published revision in place; it
creates a draft successor and shows schema and entity-migration consequences
before publication.

### Context hierarchy

A pack may declare context nodes, parent relationships, context metadata, and
optional publication-channel defaults. Parent references use logical context
keys. New contexts receive mapped physical codes and must obey the ordinary
single-root, cycle, authorization, and deletion rules.

The pack cannot replace the workspace's root `default` context or assume that a
generic code such as `web` is available.

### Extension requirements

A pack may name required or optional extension IDs, SemVer ranges,
configuration templates, contribution layout defaults, and guided setup steps.
Extension IDs and contribution IDs are global package identifiers and are not
prefixed or renamed. Layout entries use stable
`<extension-id>:<contribution-id>` keys.

Each extension remains an independently sourced and validated package. The plan
must show, separately:

- whether a compatible release is already installed;
- the trusted extension source and selected release;
- install or upgrade work;
- required and optional capabilities and host permissions;
- proposed configuration, with secret fields redacted; and
- whether enabling the extension is requested.

A pack cannot grant permissions, approve host access, bypass extension source
rules, inject extension artifacts, or silently enable an extension. Required
operator approvals remain effective even for an official pack.

### Extension configuration templates and inputs

Templates may contain literal non-secret values, typed installer inputs, and
structured logical references to mapped blueprints, attributes, contexts,
assets, extensions, and contributions. Resolution is structural; Catalog must
not perform unbounded string substitution in arbitrary files.

Inputs declare a type, label, validation constraints, whether they are
required, and whether a default is safe. Sensitive inputs are secret
**references** or post-install setup steps. Secret values, access tokens,
passwords, private keys, and connection credentials are never stored in the
archive, installation plan, selected-option record, or audit payload.

### Workspace defaults

A pack may declare mergeable, schema-owned workspace defaults, including:

- pinned Explore navigation entries;
- workspace extension layout and promoted navigation contributions;
- blueprint-view extension layout defaults;
- branding, logo, and theme selections; and
- other settings explicitly registered as packable by Catalog.

Pinned Explore entries reference logical blueprints and role **codes**, not
blueprint or role IDs. Runtime authorization still determines whether a user
can see and use an entry.

A pack cannot replace the entire workspace settings document. Every packable
setting defines its own merge key, validation, ownership granularity, and
conflict behavior so unrelated workspace settings survive installation and
upgrade.

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

A pack may include Markdown documentation, release notes, a structured setup
checklist, and declarative post-install checks. Checklist items can point to a
Catalog screen, a mapped resource, an extension setup task, or an external
human procedure. Links must follow the host's URL policy.

Checks use a host-defined, versioned catalogue of bounded predicates—for
example, “mapped blueprint has a published revision,” “required extension is
enabled,” or “required configuration field is present.” They cannot execute
code, query the database, make network calls, or read secret values. Results
are informational unless the check contract explicitly marks one as required
for installation completion.

### Optional sample data

Sample data is a separate, optional component and is never installed by
selecting production configuration alone. It uses pack-local entity keys and
logical resource references, must pass normal entity and relationship
validation, and is visibly labelled sample content.

Sample data cannot contain customer exports, personal data, credentials,
production endpoints, file object keys, or opaque database IDs. Reapplying a
release uses recorded sample-entity mappings and must not duplicate entities.
Removing a pack does not delete sample data that has been modified or adopted
as user-owned content.

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

## Options and dependency rules

Optional components and installer options form a declared dependency graph.
The manifest states requirements and incompatibilities using logical keys.
Catalog rejects cycles and an option set that omits a transitive requirement.
Changing an option after installation is a new plan, not an in-place toggle.

A pack may also declare a dependency on another pack by immutable pack ID and
SemVer range. Pack dependencies never resolve by display name, repository name,
or overlapping resource keys. The planner validates them against installed
packs and does not silently install a transitive pack. Missing dependencies,
version conflicts, and dependency cycles block apply until an administrator
creates and reviews the necessary plans.

Defaults must be deterministic and safe. Security-sensitive choices—extension
installation, grants, enabling, destructive changes, and sample-data import—are
never selected only because a pack author marked them as default.

## CLI administration

Solution-pack lifecycle operations are administrator-only CLI workflows. The
intended command shape is:

```sh
acli solution-pack inspect attricat/solution-pack-ecommerce --version 1.2.0
acli solution-pack plan attricat/solution-pack-ecommerce --version 1.2.0 --prefix ecom
acli solution-pack plan show <plan-id>
acli solution-pack apply <plan-id>
acli solution-pack status attricat.ecommerce
acli solution-pack upgrade plan attricat.ecommerce --version 1.3.0
```

The exact flags may evolve with implementation, but planning and applying remain
separate commands. `apply` accepts an immutable plan ID rather than recomputing
choices from command-line flags. Interactive prompts may help construct a plan,
but automation can supply the same typed options non-interactively and receive
structured JSON output.

The CLI authenticates normally and the API requires a dedicated
`solution_packs.manage` permission, initially granted to workspace owner and
administrator roles. Possession of a local archive or CLI access is not
authority to install it. Local/sideload commands additionally require the
server-side development or administrator control described above.

## Planning and installation

Validation produces no workspace changes. An authorized administrator first
creates a plan through the CLI against a specific pack release and a consistent
workspace snapshot.

The plan contains an action for every selected resource:

- `create`: create a new mapped resource;
- `adopt`: map a compatible existing resource;
- `update`: create or apply a safe successor/default;
- `keep`: the mapped resource already has the desired state;
- `skip`: omit an optional component;
- `conflict`: administrator input or remediation is required; or
- `blocked`: compatibility, authorization, trust, or dependency rules prevent
  installation.

For each action it shows the logical key, resolved workspace target, ownership,
before/after summary, dependants, validation result, and whether separate
approval is required. Extension permissions, host access, configuration,
enablement, published-blueprint migration, settings conflicts, and destructive
changes receive dedicated review sections rather than being buried in a generic
diff.

Plans are immutable, expire when their workspace preconditions become stale,
and cannot be applied to another workspace or release. Their preconditions
include the versions and relevant constraints of other installed packs.
Applying a plan revalidates authorization, source/digest, compatibility,
mappings, current resource revisions, cross-pack requirements, and outstanding
approvals.

Installation is durable, idempotent, and resumable. It records per-step state
and uses explicit database transactions for each atomic Catalog mutation. A
failure does not pretend that already completed external or separately approved
work was rolled back; the installation enters a clear failed or
remediation-required state and can resume from verified completed steps.
Reapplying a successful plan performs no duplicate creates.

Completion produces an installed-pack record containing:

- pack ID and version, source repository, release tag, source commit,
  provenance, release-asset identity, and archive digest;
- selected options and non-secret inputs;
- immutable logical-to-workspace mappings and the pack's relationship to each
  shared or dedicated resource;
- installed resource/revision baselines and pack-managed setting fragments;
- declared pack dependencies and resolved cross-pack constraints;
- extension requirements and the separately approved outcomes;
- per-step results, checklist state, and validation report; and
- actor, timestamps, correlation/request identifiers, and append-only lifecycle
  history.

Audit records include bounded summaries and diagnostic codes, never archive
bodies, secret values, credentials, or unredacted sensitive configuration.

## Upgrade and drift

An upgrade compares the prior installed pack baseline, the workspace's current
mapped resources, the proposed release, and constraints from every other
installed pack. This detects pack changes, workspace drift, and cross-pack
breakage before mutation.

The planner classifies each resource as unchanged, pack-only change,
workspace-only change, compatible merge, or conflict. It preserves workspace
overrides when the resource contract has a safe merge rule. Otherwise it
requires an explicit choice rather than treating the new pack release as
authoritative.

In particular:

- published blueprints receive new draft revisions, never in-place edits;
- adopted resources remain user-owned;
- modified pack-created resources are not silently reset;
- extension upgrades, grants, configuration, and enabling are reviewed under
  the extension lifecycle contract;
- workspace settings merge only the paths owned by the installed pack and
  preserve unrelated keys and user additions;
- removed resources are retained by default and reported for detach or manual
  cleanup; and
- mapping keys and physical codes remain unchanged across releases.

The pack status command should show installed and available versions, source
and digest, mappings, applied resources, drift, unresolved checklist items,
validation failures, available upgrades, and remediation actions. It should
support human-readable and structured JSON output.

## Detach and uninstall

**Detach** stops lifecycle management while retaining all workspace resources.
It removes active pack management metadata only after recording the final
mappings and state in lifecycle history. Detach is blocked while another pack
has an explicit dependency on this installation unless that dependency is
removed through a separate reviewed plan.

**Uninstall** is a planned operation, not a blind reverse install. Its preview
classifies resources as safe to remove, retained because they are adopted,
shared, or modified, blocked by dependants/data, or requiring an explicit
destructive approval. A resource linked from another installed pack is not safe
to remove. Extensions are not removed merely because a pack required them:
other packs, users, configuration, grants, or runtime dependencies may still
need them. Entities and customer data are retained by default.

## Export to draft

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

## Initial delivery boundary

The first implementation should validate the architecture with blueprint and
context starter packs:

1. strict manifest/archive validation;
2. logical identifiers, prefix selection, and immutable mappings;
3. multiple pack IDs coexisting with per-pack resource relationships;
4. blueprint, view, and context planning;
5. deterministic dry runs with collision and cross-pack conflict choices;
6. durable, idempotent installation records and audit history;
7. administrator-only CLI commands for plan review and apply; and
8. export to an untrusted draft.

Private-repository inspection and release selection through the CLI follow
next. Extension dependencies, permission review, configuration templates, and
guided setup come after the core planner. Workspace/layout defaults, upgrade
and drift management, sample data, and curated Ecommerce and Warehouse packs
build on those contracts.
