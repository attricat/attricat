---
title: Permissions reference
description: Every workspace permission, what it allows, and which built-in roles have it.
---

Permissions are granted through roles. See [Workspace administration](/operate/workspaces/#roles) for roles and scopes.

Permissions for records are named `records.*`, as in the API.

A missing or expired sign-in returns `401`. A signed-in person without the permission gets `403`, and the response does not reveal whether the target exists.

| Permission | Allows | owner | admin | editor | viewer |
| --- | --- | :-: | :-: | :-: | :-: |
| `workspace.manage` | Workspace lifecycle and ownership transfer. | ✓ | | | |
| `members.manage` | Members, invitations, user creation, and teams (`/workspace/teams`, `acli team`). | ✓ | ✓ | | |
| `roles.grant` | Granting and revoking roles. Needed together with `members.manage`. | ✓ | ✓ | | |
| `roles.manage` | Custom roles; replaying dead-letter event deliveries. | ✓ | ✓ | | |
| `tokens.manage` | Creating and revoking your personal API tokens. | ✓ | ✓ | | |
| `workspace_navigation.manage` | Explorer sidebar shortcuts. | ✓ | ✓ | | |
| `audit.read` | The audit log. | ✓ | ✓ | | |
| `blueprints.read` | Blueprints and reusable attributes. | ✓ | ✓ | ✓ | ✓ |
| `blueprints.write` | Creating blueprint drafts and revisions; reusable attributes. | ✓ | ✓ | ✓ | |
| `blueprints.publish` | Publishing blueprint revisions. | ✓ | ✓ | | |
| `records.read` | Records, search, saved searches, files, and history; the user and team directory for assignments (`GET /directory`, `acli directory`), which lists member names and email addresses. | ✓ | ✓ | ✓ | ✓ |
| `records.write` | Creating and editing records, uploading files, migrating records, attaching reusable attributes, running extension commands from the UI. | ✓ | ✓ | ✓ | |
| `records.delete` | Deleting records. | ✓ | ✓ | ✓ | |
| `records.publish` | Publishing and unpublishing records to channels. | ✓ | ✓ | | |
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

A blueprint status can require a permission or role for a transition, or a different person than made an earlier one, on top of `records.write` and for every writer; locked records refuse changes whatever the writer's permissions. See [Control a record's lifecycle](/builders/validation/#control-a-records-lifecycle).

## Attribute visibility

Permissions apply to whole records. Anyone who can read a record can read every attribute value it has, in every context, including value history and the changes it shows. Search, filters, display labels, agent tools, and extensions see the same values. You cannot hide single attributes, such as a valuation or provenance notes, from people who can read the rest of the record.

To keep sensitive details from some people, store them in a separate blueprint linked to the record, and grant `records.read` on that blueprint only to the people who need it. Keep in mind:

- Grants on a single blueprint do not include search, filters, or saved searches. Those need a workspace-wide grant, and a workspace-wide grant reads every blueprint.
- Relationship lists and previews on a readable record can show the linked record's display label and values. Keep the sensitive blueprint's fields out of the readable blueprint's views and out of its `dropdown_option` label.
- Extensions, workflows, and connector exports read all data.

## Personal API tokens

A token carries its own list of permissions. Each request is allowed only if both the token and its owner's current roles allow it. Granting or revoking roles through a token needs both `members.manage` and `roles.grant` on the token.

## Agents

The agent acts with the permissions of the person who started the conversation, checked again when each approved change runs. See [Agents and approvals](/guides/agents/).
