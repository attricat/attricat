---
title: Install and manage solution packs
description: Inspect, plan, apply, and operate existing solution packs safely.
---

A solution pack is a versioned `.tar.zst` archive supplied by a publisher. It sets up a new installation for a use case with blueprints, contexts and publication channels, rules, workflows, saved searches, navigation, extension layouts, label translations, presentation assets, guidance, and optional sample data.

After applying a pack, users work with its resources through normal Attricat features. There is no pack service to keep running, and packs do not own, synchronize, or remove those resources later.

## Before you start

- Obtain an archive from a trusted publisher and review its release notes. Attricat does not download packs or fetch repository URLs.
- Authenticate the CLI to the intended workspace. Pack administration requires `solution_packs.manage`, available to owners and administrators by default.
- Test unfamiliar packs in a disposable workspace, especially when selecting sample data.
- A pack may install the extensions it requires from the official Attricat extension registry. Applying it installs, configures, grants permissions to, and enables them without a separate approval. See [Extensions](#extensions).

If an archive is rejected, ask its publisher for a compatible release rather than editing or repackaging it.

## 1. Inspect

```sh
acli solution-pack inspect --file pack.tar.zst
```

The server validates the archive and returns its identity, version, digest, resource summaries, and sample warnings. Its `seeds` summary lists the packs this one requires, the contexts and channels it declares, its rules and workflows with whether each will be enabled, and its saved searches. It does not apply resources or persist the archive. Validation is not a guarantee that the pack is suitable for your workspace.

## 2. Plan

```sh
acli solution-pack plan --file pack.tar.zst \
  --prefix example --blueprint-publication publish
acli solution-pack plan show <plan-id>
```

Planning saves an immutable dry run without changing catalog resources. Review readiness, actions, conflicts, and extension requirements. A requirement with status `install` lists the official release apply will install and the permissions it will grant.

- `--prefix` produces new blueprint codes such as `example_product`. Use 1–32 lowercase letters, digits, or underscores, beginning with a letter and not ending with an underscore.
- `--blueprint-publication` is `draft` or `publish`. Navigation and samples can require published blueprints, so selecting `draft` may block a pack that needs them.
- `--include-sample-data` explicitly selects fictional sample entities. Leave it out unless you want them.

| Action | Meaning |
| --- | --- |
| `create` | Create a blueprint, context, publication channel, rule, workflow, saved search, asset, or selected sample entity. |
| `map` | Reuse an explicitly selected compatible resource or context, or a resource a required pack installed. |
| `append` | Add navigation, extension-layout, or translation entries. |
| `satisfied` | The requested setting or publication channel already exists exactly. |
| `skip` | Leave an optional unavailable item out. |
| `conflict` | Existing workspace state prevents the operation. |
| `blocked` | A requirement is unmet or the change is unsupported. |

Only ready plans can be applied. Plans expire after 24 hours if application has not started.

### Rules, workflows, and saved searches

- **Rules** are created for the pack's blueprints and published. The plan summary shows whether each will be enabled; the others stay disabled until you enable them. Rules need published blueprints, so `--blueprint-publication draft` blocks a rule for a newly created blueprint.
- A rule that **rejects invalid changes** is not enabled on a blueprint you mapped or reused, because that blueprint may already have entities. It is installed disabled, and the plan summary shows `enable_deferred_reason: enforcing_rule_requires_dry_run`. Run a dry run of the rule, review its findings, and enable it as you would any rule.
- **Workflows** are always published. The plan summary shows whether each will be enabled; the others stay disabled until you enable them.
- **Saved searches** are shared with the whole workspace and appear under **Saved searches** in Explore. You own the ones created by the plan you apply. A search that would be invalid with your workspace's codes, for example larger than 32 KiB after a long context code is substituted, is blocked as `saved_search_state_invalid`, and the plan summary's `invalid_state_reason` says why.

New rule and workflow codes start with your prefix. Enabled rules and workflows react to later changes, including sample entities created by the same plan, so review them before applying. After installation they are ordinary resources you manage as usual.

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

You must select existing resources explicitly; Attricat does not silently overwrite them. If an extension requirement is blocked, resolve it through extension administration and create a fresh plan.

### Contexts and publication channels

A pack can create [contexts](/guides/contexts/), for example for markets or languages, and make some of them [publication channels](/guides/publishing/). New contexts get prefixed codes such as `example_pl`. To use a context you already have, map it:

```sh
acli context list
acli solution-pack plan --file pack.tar.zst --prefix example \
  --blueprint-publication publish \
  --map-context contexts/poland=PL
```

A channel can require some of the pack's rules, or a valid entity, before an entity is published there; the plan summary lists them (`required_rule_codes`, `require_valid_entity`). The channel is planned after those rules. A channel whose required rule can't be created or found is blocked as `dependency_not_creatable`. A required rule that is installed disabled doesn't count toward the channel's checks until you enable it.

A mapped context is used as it is. If the pack wants it as a channel, an existing channel that matches exactly, including its required checks, is `satisfied` and a missing one is created. A channel whose settings differ, for example one that is switched off when the pack expects it on, is a `publication_channel_mismatch` conflict: change the channel yourself or skip the mapping. Rules, saved searches, and sample values that belong to the pack's context use the created or mapped context.

### Required packs

A pack can depend on other packs, for example to share a Supplier blueprint. The plan shows each as a `prerequisite` action:

- `map`: a completed installation of the required pack, with a version the pack accepts, is reused.
- `blocked` with `prerequisite_missing` or `prerequisite_incompatible`: install a suitable version of the required pack first, then create a new plan.

Blueprints shared with a required pack are reused only when they still match exactly (`prerequisite_blueprint_match`). If someone changed or removed that blueprint, the plan reports a conflict instead of creating a copy. Attricat never installs required packs for you.

### Extensions

When a required extension is not installed, planning selects the newest release in the pack's version range from the official Attricat registry. Registries added to the workspace are never used. Planning needs access to the official registry, but installs nothing.

Apply installs that exact release before its other steps. It configures the release from the pack, grants the permissions the release requires, and enables it. Each change appears in the extension's lifecycle history and the audit log. If the release changed since you reviewed the plan, apply stops; create a new plan.

Packs leave existing installations as they are. A compatible installation satisfies the requirement, enabled or not. An incompatible version, a quarantined installation, or different configuration blocks the plan; packs never upgrade or reconfigure an installed extension. Extensions installed by a pack are ordinary installations that you manage, disable, or remove as usual.

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

Retry interrupted or resumable failed applications with the same plan ID. The server verifies completed steps and continues pending steps without duplication; each context, channel, rule, workflow, and saved search is created together with the record of its step. A started application can resume after plan expiry, subject to its own retention deadline. If a blueprint the plan created has gained entities by the time a resumed application reaches an enforcing rule for it, that step fails with `rule_dry_run_required`; dry-run the rule and enable it yourself.

If workspace changes make a plan stale, review completed steps and diagnostics before creating another plan. If a later step fails permanently, earlier successful writes remain in the workspace; Attricat does not roll them back.

## Apply a newer release

```sh
acli solution-pack plan --file pack-v2.tar.zst --prefix example \
  --blueprint-publication publish --from-application <application-id>
```

Select one completed application of the same pack in the same workspace. The release must be strictly newer, and `--from-application` cannot be combined with explicit maps.

Unchanged exact published blueprints and unchanged assets can be reused; new resources can be created. Contexts from the earlier installation are reused, and rules, workflows, saved searches, and channels it already created are skipped as `provided_by_prior_application`, never updated. Changed definitions are blocked as `update_not_supported`. Removed resources are reported without deletion. Missing or modified prior targets may conflict. Agree on a supported migration procedure with the publisher for changes the pack cannot apply.

## Optional sample data

Add `--include-sample-data` during planning only after reviewing the warning. Samples are created against published blueprints and receive a visible **Sample** badge. Their values are set in the default context, or in one of the pack's contexts when the pack says so. Samples can also attach files bundled in the pack, such as images or PDFs; planning uploads them to ordinary file storage, and they appear on the sample entities as ordinary files. Attricat checks each file's type and the attribute's file rules, but cannot tell whether its content is fictional. They are ordinary entities: creation emits audit records and `entity.created.v1` events, may run enabled workflows/extensions, and may cause external effects. Values can remain in ordinary audit/event history after temporary pack staging is removed.

Inspection rejects a pack whose samples set a status or a user or team assignment attribute. It also rejects a pack in which two samples of one blueprint share a unique-key value. A sample that collides with an existing entity of a mapped or reused blueprint, for example on a unique key, fails its apply step.

The first plan with samples selected reserves that exact combination of release, archive, and dataset. A second plan cannot select the same combination, even after expiry or abandonment; changing the prefix does not reset the reservation. Retry the original plan. If it expires before application starts, coordinate a new release with the publisher.

A started sample-selected application has a fixed 30-day resumability window. To permanently stop it:

```sh
acli solution-pack applications abandon <application-id>
```

Abandoning an application removes staged inputs, including bundled files that were never attached. Previously created entities and ordinary audit/event history remain unchanged. Deleting a sample entity removes it with its values in every context and its files, like any other entity; stored files follow ordinary file retention. Later releases do not reset live sample values or restore removed sample markers. There is no dataset reset or automatic cleanup command.

## Translations

A pack can ship [label translations](/builders/translations/), shown as the `workspace/lexicon` setting during inspection and as an `append` action in the plan. Applying the pack adds missing entries and updates entries a pack supplied earlier. Entries written in the workspace, before or after installation, are never overwritten, and editing a pack-supplied entry makes it a workspace entry. Translations never cause conflicts.

## Limits and removal

Packs cannot change existing contexts or channels, change membership or role grants, install extensions from anywhere but the official registry, upgrade installed extensions, run executable installers, install required packs, or update existing blueprints, rules, workflows, or saved searches.

There is no pack-level uninstall or rollback. Administrators may edit or remove individual resources through ordinary operations, subject to authorization, dependencies, publication, and retention rules. Review business data before deleting anything; resources referenced by application history may be shared or modified.
