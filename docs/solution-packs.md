# Install and operate solution packs

A solution pack is a versioned `.tar.zst` archive supplied by a publisher. It
sets up ordinary workspace resources for a use case: blueprints, navigation,
extension layouts, label translations, presentation assets, setup guidance, and
optional samples.

After applying a pack, users work with its resources through normal Attricat
features. There is no pack service to keep running, and packs do not retain
ownership, synchronize resources, or uninstall them later.

## Before you start

- Obtain an immutable archive from a trusted publisher and review its release
  notes and compatibility requirements. Attricat does not discover packs, fetch
  repository URLs, or download releases on your behalf.
- Use an authenticated `acli` session or personal token for the intended
  workspace. Pack administration requires `solution_packs.manage`, granted to
  the workspace owner and administrator roles by default. See [CLI](cli.md) and
  [authentication](authentication.md).
- Check the selected server and workspace before applying. Test unfamiliar
  packs in a disposable development workspace first, especially with samples.
- A pack may install the extensions it requires from the official Attricat
  extension registry. Applying it installs, configures, grants, and enables
  them without a separate approval; see [Extensions](#extensions). It never
  removes, upgrades, or reconfigures an installed extension.

Do not edit or repackage a supplied archive to work around validation errors.
Ask its publisher for a compatible release.

## Inspect the archive

```sh
acli solution-pack inspect --file pack.tar.zst
```

The server validates the archive and returns its identity, version, digest,
resource summaries, extension requirements, guidance counts, and sample-data
warnings. Inspection does not apply resources or persist the archive. Validation
alone does not mean the pack is suitable for your workspace.

## Create and review a plan

```sh
acli solution-pack plan --file pack.tar.zst \
  --prefix example --blueprint-publication publish
acli solution-pack plan show <plan-id>
```

Planning saves an immutable dry run without changing catalog resources. Review
readiness, all actions, conflicts, extension requirements, and the sample-data
selection.

- `--prefix` determines new blueprint codes, such as `example_product`. It must
  be 1–32 characters, start with a lowercase letter, contain only lowercase
  letters, digits, and underscores, and not end with an underscore.
- `--blueprint-publication draft` creates drafts on apply; `publish` creates
  published blueprints. Navigation entries and sample creation may require
  published targets, so choosing `draft` can block such a pack.
- Sample data is omitted unless you explicitly add `--include-sample-data`
  during planning. Review [sample-data safety](solution-pack-sample-data.md)
  before selecting it.

Extension requirements have their own statuses: `satisfied`, `install` (apply
installs the listed official release), `blocked`, or `skipped`.

| Action | Meaning |
| --- | --- |
| `create` | Create a new blueprint, asset, or selected sample entity. |
| `map` | Reuse an explicitly selected compatible resource. |
| `append` | Add navigation, extension-layout, or translation entries. |
| `satisfied` | The requested setting is already present exactly. |
| `skip` | Leave an optional unavailable item out. |
| `conflict` | Existing workspace state prevents the operation. |
| `blocked` | A requirement is unmet or the requested change is unsupported. |

Only ready plans can be applied. Plans expire after 24 hours if application has
not started. Planning does not reserve ordinary blueprint codes against other
users; apply checks the workspace again.

## Resolve conflicts and requirements

For a code collision, choose a different prefix, or explicitly reuse a published
blueprint whose definition matches the pack exactly. Use resource keys reported
by inspection and the publisher's installation instructions:

```sh
acli solution-pack plan --file pack.tar.zst --prefix example \
  --blueprint-publication publish \
  --map blueprints/product=shared_product
```

You must select existing resources explicitly. If a resource does not match,
Attricat reports a conflict without overwriting or automatically adopting it.

For presentation-asset reuse, inspect existing assets and supply an exact match:

```sh
acli presentation-asset list
acli presentation-asset show <asset-id>
acli solution-pack plan --file pack.tar.zst --prefix example \
  --blueprint-publication publish \
  --map-asset assets/brand-logo=<asset-id>
```

If an extension requirement is blocked, resolve it through normal extension
administration and create a fresh plan. See [Extensions](#extensions).

## Extensions

Planning looks up every extension the pack requires that is not installed in
the workspace. When the official Attricat registry offers a release in the
pack's version range, the requirement's status is `install` and the plan shows
the selected release (`install.version`, `install.repository`,
`install.tag_name`) and the permissions apply will grant (`install.grants`).
Workspace-added registries are never used. Planning therefore needs access to
the official registry and fails with `503` while it is unreachable. Planning
downloads and validates the release but installs nothing.

Before its other steps, apply downloads that exact release again and installs
it. It applies the pack's configuration, grants the release's required
permissions, and enables it. Optional permissions are not granted. Each
installation, configuration, grant, and enablement is recorded in the
extension's lifecycle history and the audit log like a manual change. If the
release changed since review, apply refuses it as `solution_pack_plan_stale`;
create a new plan.

A pack does not change extensions that are already installed:

- A compatible installation satisfies the requirement as it is, enabled or not.
- An incompatible version, a quarantined installation, or a configuration that
  differs from the pack's template blocks a required extension. Resolve it
  through [extension administration](extensions.md) and plan again; packs never
  upgrade or reconfigure an installed extension.
- Re-applying a completed plan does not re-enable an extension an operator
  disabled afterwards.

Extensions a pack installed are ordinary installations. Manage, disable, or
remove them through normal extension administration.

## Apply and verify

```sh
acli solution-pack apply <plan-id>
acli solution-pack applications list
acli solution-pack applications show <application-id>
acli solution-pack checks list <application-id> --limit 25 --offset 0
acli solution-pack checks show <application-id> <run-id>
acli solution-pack checks rerun <application-id>
```

Apply accepts only the reviewed plan ID; you cannot change its choices. The server revalidates
workspace state before each step. Completed applications receive an informational
check run; operators can rerun checks after setup changes. False check results
do not roll back resources or block an otherwise completed application.

Follow the publisher's setup checklist and try the intended business workflow.
The application record documents the resources created or reused. It does not
track later user edits.

## Retry and recovery

Retry an interrupted or resumable failed application with the same plan ID.
The server verifies completed steps and continues pending steps without
duplicating resources. A started application can resume after the original plan expires,
subject to its application-specific retention deadline.

A stale plan cannot silently overwrite changed workspace state. Review the
application's completed steps and diagnostic before planning again. If a later
step fails permanently, previously completed writes remain in the workspace;
Attricat does not roll them back.

Sample-selected plans have additional identity and recovery constraints. Do not
create a fresh plan expecting it to replay the same dataset. See
[sample retry, expiry, and abandonment](solution-pack-sample-data.md#retry-expiry-and-abandonment).

## Apply a newer release

Explicitly name one completed application of the same pack in the same workspace:

```sh
acli solution-pack plan --file pack-v2.tar.zst --prefix example \
  --blueprint-publication publish --from-application <application-id>
```

The new release must have a strictly newer version. This option cannot be
combined with `--map` or `--map-asset`; Attricat does not search history or infer
lineage automatically.

- Unchanged exact published blueprint targets and unchanged assets can be reused.
- New resources can be created.
- Changed definitions are blocked as `update_not_supported`.
- Removed resources are reported without deleting anything.
- Missing, deleted, unpublished, or changed prior targets may cause conflicts.

If a release needs unsupported updates, agree on a supported migration procedure
with the publisher. A new pack version cannot update existing blueprints.

## Translations

A pack may ship entries for the workspace [lexicon](blueprints.md#translated-labels),
shown as the `workspace/lexicon` setting in inspection and as one `append`
action in the plan. Applying it adds entries that are missing and updates
entries a pack supplied earlier. Entries written in the workspace, before or
after installation, are never overwritten, and editing a pack-supplied entry
makes it a workspace entry. Translation entries never conflict, so the step's
result reports how many entries were written (`written_count`). Manage the
entries afterwards with `acli lexicon` ([CLI](cli.md#translations)).

## Limits and removal

Packs cannot create contexts or publication channels, alter membership or role
grants, install extensions from anywhere but the official registry, upgrade
installed extensions, run executable installers, resolve prerequisite packs automatically, or update
existing blueprints. There is no generic settings replacement or continuing
resource ownership.

There is no pack-level uninstall or rollback command. Administrators may edit or
remove individual resources through normal operations, subject to permissions,
dependencies, publication, and retention rules. Review business data before any
removal; never assume everything mentioned in an application record is disposable.
Removing application history is not a supported way to remove resources.
