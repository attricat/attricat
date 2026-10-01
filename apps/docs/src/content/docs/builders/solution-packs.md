---
title: Install and manage solution packs
description: Inspect, plan, apply, and operate existing solution packs safely.
---

A solution pack is a versioned `.tar.zst` archive supplied by a publisher. It sets up a workspace for a use case with blueprints, navigation, extension layouts, presentation assets, guidance, and optional sample data.

After applying a pack, users work with its resources through normal Attricat features. There is no pack service to keep running, and packs do not own, synchronize, or remove those resources later.

## Before you start

- Obtain an archive from a trusted publisher and review its release notes. Attricat does not download packs or fetch repository URLs.
- Authenticate the CLI to the intended workspace. Pack administration requires `solution_packs.manage`, available to owners and administrators by default.
- Test unfamiliar packs in a disposable workspace, especially when selecting sample data.
- Manage required extensions through the normal [extension workflow](/builders/extensions/). A pack never installs, configures, grants permissions to, enables, or removes extensions.

If an archive is rejected, ask its publisher for a compatible release rather than editing or repackaging it.

## 1. Inspect

```sh
acli solution-pack inspect --file pack.tar.zst
```

The server validates the archive and returns its identity, version, digest, resource summaries, and sample warnings. It does not apply resources or persist the archive. Validation is not a guarantee that the pack is suitable for your workspace.

## 2. Plan

```sh
acli solution-pack plan --file pack.tar.zst \
  --prefix example --blueprint-publication publish
acli solution-pack plan show <plan-id>
```

Planning saves an immutable dry run without changing catalog resources. Review readiness, actions, conflicts, and extension requirements.

- `--prefix` produces new blueprint codes such as `example_product`. Use 1–32 lowercase letters, digits, or underscores, beginning with a letter and not ending with an underscore.
- `--blueprint-publication` is `draft` or `publish`. Navigation and samples can require published blueprints, so selecting `draft` may block a pack that needs them.
- `--include-sample-data` explicitly selects fictional sample entities. Leave it out unless you want them.

| Action | Meaning |
| --- | --- |
| `create` | Create a blueprint, asset, or selected sample entity. |
| `map` | Reuse an explicitly selected compatible resource. |
| `append` | Add navigation or extension-layout entries. |
| `satisfied` | The requested setting already exists exactly. |
| `skip` | Leave an optional unavailable item out. |
| `conflict` | Existing workspace state prevents the operation. |
| `blocked` | A requirement is unmet or the change is unsupported. |

Only ready plans can be applied. Plans expire after 24 hours if application has not started.

## 3. Resolve conflicts

For a code collision, choose another prefix or explicitly map an exactly matching published blueprint. Use the resource keys reported by inspection and the publisher's installation guide:

```sh
acli solution-pack plan --file pack.tar.zst --prefix example \
  --blueprint-publication publish \
  --map blueprints/product=shared_product
```

To reuse an exactly matching asset, find its ID first:

```sh
acli presentation-asset list
acli presentation-asset show <asset-id>
acli solution-pack plan --file pack.tar.zst --prefix example \
  --blueprint-publication publish \
  --map-asset assets/brand-logo=<asset-id>
```

You must select existing resources explicitly; Attricat does not silently overwrite them. If an extension requirement is unmet, resolve it through extension administration and create a fresh plan. A compatible disabled installation can satisfy a requirement; operators must still approve grants and enablement separately.

## 4. Apply and verify

```sh
acli solution-pack apply <plan-id>
acli solution-pack applications list
acli solution-pack applications show <application-id>
acli solution-pack checks list <application-id>
acli solution-pack checks show <application-id> <run-id>
acli solution-pack checks rerun <application-id>
```

Apply accepts only the reviewed plan ID. The server rechecks workspace state before each step. Follow the setup checklist and try the pack's intended business workflow.

Checks are informational. A false result does not undo resources or block an otherwise completed application. Application history records the resources created or reused; it does not track later user edits.

## Retry and recovery

Retry interrupted or resumable failed applications with the same plan ID. The server verifies completed steps and continues pending steps without duplication. A started application can resume after plan expiry, subject to its own retention deadline.

If workspace changes make a plan stale, review completed steps and diagnostics before creating another plan. If a later step fails permanently, earlier successful writes remain in the workspace; Attricat does not roll them back.

## Apply a newer release

```sh
acli solution-pack plan --file pack-v2.tar.zst --prefix example \
  --blueprint-publication publish --from-application <application-id>
```

Select one completed application of the same pack in the same workspace. The release must be strictly newer, and `--from-application` cannot be combined with explicit maps.

Unchanged exact published blueprints and unchanged assets can be reused; new resources can be created. Changed definitions are blocked as `update_not_supported`. Removed resources are reported without deletion. Missing or modified prior targets may conflict. Agree on a supported migration procedure with the publisher for changes the pack cannot apply.

## Optional sample data

Add `--include-sample-data` during planning only after reviewing the warning. Samples are created in the default context against published blueprints and receive a visible **Sample** badge. They are ordinary entities: creation emits audit records and `entity.created.v1` events, may run enabled workflows/extensions, and may cause external effects. Values can remain in ordinary audit/event history after temporary pack staging is removed.

The first plan with samples selected reserves that exact combination of release, archive, and dataset. A second plan cannot select the same combination, even after expiry or abandonment; changing the prefix does not reset the reservation. Retry the original plan. If it expires before application starts, coordinate a new release with the publisher.

A started sample-selected application has a fixed 30-day resumability window. To permanently stop it:

```sh
acli solution-pack applications abandon <application-id>
```

Abandoning an application removes staged inputs. Previously created entities and ordinary audit/event history remain unchanged. Later releases do not reset live sample values or restore removed sample markers. There is no dataset reset or automatic cleanup command.

## Limits and removal

Packs cannot create contexts or publication channels, change membership or grants, run executable installers, resolve prerequisite packs automatically, or update existing blueprints.

There is no pack-level uninstall or rollback. Administrators may edit or remove individual resources through ordinary operations, subject to authorization, dependencies, publication, and retention rules. Review business data before deleting anything; resources referenced by application history may be shared or modified.
