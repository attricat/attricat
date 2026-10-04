---
title: Permissions reference
description: Every workspace permission, what it allows, and which built-in roles have it.
---

Permissions are granted through roles. See [Workspace administration](/operate/workspaces/#roles) for roles and scopes.

A missing or expired sign-in returns `401`. A signed-in person without the permission gets `403`, and the response does not reveal whether the target exists.

| Permission | Allows | owner | admin | editor | viewer |
| --- | --- | :-: | :-: | :-: | :-: |
| `workspace.manage` | Workspace lifecycle and ownership transfer. | ✓ | | | |
| `members.manage` | Members, invitations, and user creation. | ✓ | ✓ | | |
| `roles.grant` | Granting and revoking roles. Needed together with `members.manage`. | ✓ | ✓ | | |
| `roles.manage` | Custom roles; replaying dead-letter event deliveries. | ✓ | ✓ | | |
| `tokens.manage` | Creating and revoking your personal API tokens. | ✓ | ✓ | | |
| `workspace_navigation.manage` | Explorer sidebar shortcuts. | ✓ | ✓ | | |
| `audit.read` | The audit log. | ✓ | ✓ | | |
| `blueprints.read` | Blueprints and reusable attributes. | ✓ | ✓ | ✓ | ✓ |
| `blueprints.write` | Creating blueprint drafts and revisions; reusable attributes. | ✓ | ✓ | ✓ | |
| `blueprints.publish` | Publishing blueprint revisions. | ✓ | ✓ | | |
| `entities.read` | Entities, search, saved searches, files, and history. | ✓ | ✓ | ✓ | ✓ |
| `entities.write` | Creating and editing entities, uploading files, migrating entities, attaching reusable attributes, running extension commands from the UI. | ✓ | ✓ | ✓ | |
| `entities.delete` | Deleting entities. | ✓ | ✓ | ✓ | |
| `entities.publish` | Publishing and unpublishing entities to channels. | ✓ | ✓ | | |
| `contexts.read` | Contexts and publication channels. | ✓ | ✓ | ✓ | ✓ |
| `contexts.write` | Creating, changing, and deleting contexts; enabling publication channels. | ✓ | ✓ | ✓ | |
| `data_health.read` | Data health, background processing, metrics, and dead-letter event lists. | ✓ | ✓ | ✓ | ✓ |
| `agents.run` | Agent conversations and approvals. | ✓ | ✓ | | |
| `rules.read` | Rules, runs, and findings. | ✓ | ✓ | | |
| `rules.manage` | Creating, publishing, enabling, and running rules; acknowledging findings. | ✓ | ✓ | | |
| `workflows.read` | Workflows and run history. | ✓ | ✓ | | |
| `workflows.manage` | Creating, publishing, enabling, running, and replaying workflows. | ✓ | ✓ | | |
| `extensions.read` | Browsing extension registries and installed extensions. | ✓ | ✓ | | |
| `extensions.manage` | Installing, configuring, granting, enabling, and removing extensions; registries, layout, secrets, operations, and connector jobs. | ✓ | ✓ | | |
| `solution_packs.manage` | Inspecting, planning, and applying solution packs; presentation assets. | ✓ | ✓ | | |
| `files.hold` | Placing and releasing explicit retention holds on files. | ✓ | ✓ | | |

## Status transitions

A blueprint status can require a permission or a role for an individual transition, and can require that a different person makes it than made an earlier transition. These checks come on top of `entities.write` and apply to every writer, including workflows, extensions, and agents. A refused transition returns `403` with the code `status_transition_forbidden` or `status_separation_of_duties`. See [Control a record's lifecycle](/builders/blueprints/#step-10-control-a-records-lifecycle).

Records in a locked status reject changes to locked content with `409 record_locked`, whatever the writer's permissions. Unlocking takes an explicit, permitted correction transition, which is recorded in the audit log.

## Personal API tokens

A token carries its own list of permissions. Each request is allowed only if both the token and its owner's current roles allow it. Granting or revoking roles through a token needs both `members.manage` and `roles.grant` on the token.

## Agents

The agent acts with the permissions of the person who started the conversation, checked again when each approved change runs. See [Agents and approvals](/guides/agents/).
