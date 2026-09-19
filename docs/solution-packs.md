# Solution Packs

> **Status:** v1 archive validation, administrator inspection, immutable
> create-only planning, and durable application/history for contexts and
> blueprints are implemented. Private-repository fetching and broader resource
> types remain deferred.
>
> **Scope of this document:** Sections that describe private repositories,
> existing-resource adoption, updates, settings, extensions, assets, sample
> data, prerequisites, export, or richer application evidence are future target
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
- **Traceable:** Catalog records source identity, revision, release, archive
  digest, selected options, mappings, actions, and application history.
- **Trust preserving:** trusting a pack does not trust, enable, configure, or
  grant permissions to an extension named by that pack.

## Future target content at a glance

The following table is roadmap design beyond the implemented v1 context and
blueprint boundary.

| Content | Target pack behavior |
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

## Package and distribution (future private-repository design)

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

- bounded compressed and expanded sizes, entry counts, individual files, and
  blueprint/include/attribute complexity;
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
resource and adding another and must be treated as such when planning a later
release. Display names are not identifiers.

## Logical identifiers and workspace mappings (future beyond v1 creation)

Pack files reference logical keys, never workspace UUIDs or assumed physical
codes. During application, the planner resolves every key to one
workspace-owned resource and records that mapping in the immutable application
record.

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

Catalog never silently overwrites, renames, or maps an existing blueprint,
context, setting, or asset. A successful application preserves its mapping as
provenance. A later application may use that recorded mapping as a planning
input, but it does not make the resource pack-owned or prevent ordinary
workspace changes. This allows packs with overlapping local names to coexist.

The plan distinguishes a newly created resource from a compatible existing
resource selected by the administrator. That distinction is historical
provenance only. Both become or remain ordinary workspace-owned resources as
soon as the plan is applied.

## Multiple packs in one workspace (future composition design)

The implemented v1 can apply multiple create-only plans when their target codes
do not collide. The compatibility mapping, settings, and extension composition
rules in this section are future design.

A workspace may apply multiple solution packs. This is a core composition
requirement, not an exceptional migration path: for example, one workspace may
combine Ecommerce, DAM, SEO, marketplace, and warehouse templates.

Each application has its own prefix, options, mapping snapshot, and provenance
record. Physical codes must be unique across the workspace, but packs may have
overlapping local key names because references are qualified by pack ID and
resolved while each plan is created.

Resources and extensions can be shared. A second pack may map to a compatible
resource produced by an earlier application, but only through an explicit,
compatibility-checked plan action. The earlier pack gains no continuing
ownership and the later pack creates no permanent dependency on it; both plans
ultimately operate on the same workspace-owned resource.

Composition follows these rules:

- plans evaluate the workspace's current resources, settings, extensions, and
  prior pack-application records;
- no pack receives implicit precedence because it was applied most recently;
- workspace-setting fragments and navigation/layout entries merge only through
  their registered item-level keys and merge rules;
- incompatible setting proposals become conflicts rather than last-writer-wins
  updates;
- extension version requirements in the plan must be compatible with the
  extension release currently selected for the workspace;
- conflicting extension configuration templates require an administrator to
  choose or provide one workspace configuration; and
- extension grants and enablement remain ordinary workspace decisions after
  application.

Applying the same pack release with the same choices is idempotent. Applying a
different release of the same pack creates a new reviewed application plan; it
is not a second managed instance and does not establish an upgrade relationship
with resources produced by the earlier release.

## Pack contents (future design except v1 blueprints and contexts)

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
setting defines its own merge key, validation granularity, and conflict
behavior so unrelated workspace settings survive application. Afterward, the
merged settings are ordinary workspace configuration.

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
for application completion.

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
The implemented inspection command sends a local archive to the authoritative
server for read-only validation and returns only safe metadata and resource
summaries:

```sh
acli solution-pack inspect --file pack.tar.zst
```

Inspection does not persist the archive, apply resources, or expose blueprint
source and context data in its response. The implemented local-archive planner
uses an explicit prefix and blueprint publication preference:

```sh
acli solution-pack plan --file pack.tar.zst --prefix ecom --blueprint-publication draft
acli solution-pack plan show <plan-id>
acli solution-pack apply <plan-id>
acli solution-pack applications list
acli solution-pack applications show <application-id>
```

`draft` and `publish` describe what a later apply operation would do; planning
never creates or publishes a blueprint. Prefixes are 1–32 bytes, begin with a
lowercase ASCII letter, contain only lowercase letters, digits, and underscores,
and do not end in an underscore. Optional v1 resources are conservatively
skipped; a selected required resource whose dependency would be skipped is
blocked. Existing target codes are conflicts and are never adopted or replaced.
V1 pack blueprints reject workspace-role publication policies, extension layouts, and
extension-provided table renderers; planning does not snapshot extension or role
state yet.

Application accepts only an immutable plan ID and never recomputes choices from
CLI flags. Each create step commits its ordinary Catalog mutation together with
durable step evidence; retries verify completed targets and continue pending
steps without duplicate resources. Private-repository release selection remains
deferred. Planning remains separate from application.

The CLI authenticates normally and the API requires a dedicated
`solution_packs.manage` permission, initially granted to workspace owner and
administrator roles. Possession of a local archive or CLI access is not
authority to install it. Local/sideload commands additionally require the
server-side development or administrator control described above.

## Planning and application (implemented v1)

Validation produces no workspace changes. An authorized administrator uploads a
local archive and creates an immutable, workspace-scoped plan with a prefix and
`draft` or `publish` blueprint choice.

V1 actions are exactly:

- `create`: create a required context or blueprint at its persisted target;
- `skip`: omit an optional resource;
- `conflict`: a required target code was present when planning; or
- `blocked`: a selected dependency cannot be created.

V1 does not emit `adopt`, `update`, or `keep`. It does not inspect prior
applications, settings, or extensions to choose an action. Candidate mappings
remain visible in the plan, including candidates for skipped resources. The
application mapping snapshot contains only mappings for executed `create`
actions.

A new application may start only while its ready plan is unexpired. Each create
step revalidates its persisted `target_absent` code precondition. A target that
appears before or while the ordinary context/blueprint mutation runs makes the
application invalid with `plan_stale`; Catalog never adopts or overwrites it.
Once an application has started, plan expiry does not strand it: retries
revalidate completed and pending targets and resume durable work.

One durable application exists per immutable plan. Each resource mutation, step
result, audit event, and domain event commits atomically. Repeated and concurrent
apply requests lock the application and converge without duplicate resources.
If a client observes an ambiguous commit error, failure reconciliation checks
the exact attempted step; a step already committed as completed is continued,
not misreported against a later pending step.

Implemented application evidence contains the local-archive source marker and
archive digest, pack ID/version, blueprint publication choice, executed mapping
snapshot, ordered context/blueprint step results, state and bounded diagnostics,
actor token/user columns, timestamps, and request/correlation identifiers. List
responses are compact and omit mappings and steps; show responses include both.
Neither response contains normalized payloads, blueprint definitions, context
data, archive bytes, or secret values.

Richer source/release provenance, selected options, reused-resource evidence,
settings fragments, prerequisites, extension outcomes, checklists, and
validation reports are future design. Application records are
administrator-facing evidence, not controllers: they do not retain ownership,
lock resources, detect drift, authorize later changes, or provide uninstall.

## Later releases and workspace changes (future design)

A successful application is not a continuing desired-state declaration.
Administrators may freely edit the resulting blueprints, contexts, settings,
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
a plan is applied, its blueprints, contexts, settings, assets, extensions, and
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

The intended delivery sequence validates the architecture with blueprint and
context starter packs:

1. strict manifest/archive validation;
2. logical identifiers, prefix selection, and recorded mapping snapshots;
3. multiple pack IDs coexisting through independent application records;
4. blueprint, view, and context planning;
5. deterministic dry runs with collision and cross-pack conflict choices;
6. durable, idempotent application records and audit history;
7. administrator-only CLI commands for plan review and apply; and
8. export to an untrusted draft.

Private-repository inspection and release selection through the CLI follow
next. Extension dependencies, permission review, configuration templates, and
guided setup come after the core planner. Workspace/layout defaults,
later-release planning, sample data, and curated Ecommerce and Warehouse packs
build on those contracts.
