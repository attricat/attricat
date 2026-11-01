---
title: Workspace administration
description: Manage members, teams, roles, invitations, sidebar navigation, API tokens, and the audit log.
---

A workspace is one catalog with its own members, blueprints, records, contexts, and extensions. Workspaces are fully separated: nothing is shared between them.

Most administration happens under **Manage → Workspace management**, which has up to seven tabs, depending on your permissions: **Members**, **Teams**, **Roles**, **Invitations**, **Navigation**, **Tag colors**, and **Agent**.

## Signing in

Every workspace has a **sign-in identifier** that looks like a domain name, such as `acme.example` or `default.local`. People enter it on the sign-in page, then their email and password. The identifier only routes sign-in; Attricat does not look it up in DNS.

Sessions last eight hours, and renewing one through the API or `acli auth renew` cannot keep it alive more than 24 hours after signing in. Five failed sign-ins for the same email within fifteen minutes block further attempts for a while, in every workspace. **Forgot password** sends a reset link that works once, for 30 minutes. For security, the reset form never says whether an address has an account.

Sign-in is a password or [single sign-on](#single-sign-on). Multi-factor authentication and passkeys are not available yet, though your identity provider can require them for single sign-on.

## Single sign-on

A workspace can let people sign in through one OpenID Connect identity provider, such as Microsoft Entra ID, Okta, Google Workspace, or Keycloak. Your deployment needs `OIDC_CALLBACK_URL` and the `identity_provider_credential_key` secret; see the [configuration reference](/reference/configuration/). Set it up under **Manage → Workspace management → Sign-in**, which requires `workspace.manage`:

1. Register an application with your provider, using the redirect URI that **Sign-in** shows. Make sure the provider shares the `email` claim.
2. Enter the issuer URL, client ID, client secret, and the name for the sign-in button, and save.
3. Select **Run test sign-in**. This proves the settings work and connects your account.
4. Turn on **Offer this provider on the sign-in page**.
5. Optionally, let only owners use a password, so they have a way back in if the provider fails.

Changing the issuer or client ID turns the provider off again and restores password sign-in for everyone until a new test sign-in succeeds. Each sign-in is checked against the workspace's own provider, so a provider set up for one workspace gives no access to another.

### Who can sign in

Single sign-on does not create accounts on its own. Someone gets access by accepting an [invitation](#invitations) through the provider, which requires the provider's address to match the invitation, or by connecting their account under **Profile** while signed in. An email address alone never links an account.

### Removing access

Attricat is not told when your provider disables someone. They cannot start a new sign-in, but a session they already have lasts up to eight hours, or 24 hours if renewed, and their personal API tokens keep working.

To end access at once, set the member **inactive**. That signs them out and revokes their API tokens in this workspace, and reactivating them does not restore those tokens. To disconnect someone from the provider without deactivating them, use **Disconnect single sign-on** under **Members**.

## Members

**Members** lists everyone in the workspace with their role grants. From here you can:

- grant or revoke roles;
- set a member **inactive**, which signs them out and blocks access without deleting their history;
- transfer ownership to another active member (owners only). This removes your own owner role, so Attricat asks you to confirm.

A workspace always has at least one active owner. Revoking the last owner's role or setting them inactive fails with `409 last_workspace_owner`. Membership and role changes sign the affected person out of existing sessions.

## Teams

**Teams** groups members under a name, such as *Quality* or *Field service*, so a [user or team attribute](/builders/modeling/#assign-responsibility) can assign work to a whole team. Anyone with `members.manage` can create a team, rename it, change its members, or delete it. A team's code cannot change after it is created.

Records store the team itself, so changing its members never changes records. **Assigned to me** filters in the Explorer include records assigned to your teams. Deleting a team keeps it on records that already use it, shown as deleted, but it can no longer be assigned. Teams do not grant permissions.

## Roles

A role is a named set of permissions. Four built-in roles cannot be changed:

| Role | Can |
| --- | --- |
| `owner` | Everything, including transferring ownership. |
| `admin` | Everything except workspace ownership and lifecycle. |
| `editor` | Read and write blueprints, records, and contexts; delete records; read data health. Cannot publish blueprints or records, or administer the workspace. |
| `viewer` | Read blueprints, records, contexts, and data health. |

Create **custom roles** under **Roles** to give a narrower or different set. You can only put permissions into a role that you have yourself. Duplicate a built-in role to start from its permissions. You can change a custom role's permissions later. A role needs at least one permission; saving one without any fails with `422 role_permissions_required`. Retiring a custom role can move its grants to a replacement role.

The full permission list is in the [permissions reference](/reference/permissions/).

### Scoped grants

A role grant applies at one scope:

| Scope | Applies to |
| --- | --- |
| **Entire workspace** | Everything. |
| **Blueprint family** | One blueprint and its records, across all revisions. |
| **Record** | One record. |
| **Context subtree** | The context settings of one context and the contexts below it, but not its parent or siblings. It does not cover records or their values. |

Grants add up. A person with `viewer` on the workspace and `editor` on the `PL` context subtree can read everything, and can edit or delete the `PL` context and its children and make them publication channels. Creating contexts needs a workspace grant. A context subtree grant does not let anyone read or edit record values in those contexts: access to records comes from workspace, blueprint family, and record grants.

The owner role can only be granted on the whole workspace.

## Invitations

Under **Invitations**, invite someone by email with a role, a scope, and an expiry date, which defaults to one week. They receive a one-time link. Someone who already has an account accepts and joins; someone new sets their password first.

You can also create a user directly and send them an onboarding link. Inviting someone again before they set their password revokes their earlier pending invitation. Revoke a pending invitation at any time.

Invitation and onboarding emails need [SMTP configured](/reference/configuration/#email).

## Navigation

**Navigation** controls the blueprint shortcuts in the Explorer sidebar. Pin published record blueprints and, optionally, limit each shortcut to certain roles so people see the parts of the catalog they work on.

Changing navigation needs `workspace_navigation.manage`.

## Tag colors

**Tag colors** gives record system tags a color, so tags such as *needs attention* or *reviewed* stand out in the Explorer and on the record page. Pick each color from a preset list; every color has a light and a dark mode variant, so tags stay readable in both.

Attricat's own tags start with a default color: *needs attention* is yellow. You can choose another color for them and **Reset to default** later. Other tags have no color until you add one: type the tag as records store it, for example `reviewed` or `my-extension:synced`, and choose a color. A workspace can color up to 200 tags besides the defaults.

Everyone who can read records sees the colors. Changing them needs `tag_colors.manage`, or use `acli workspace tag-color set <tag> <color>`.

## Your profile

**Profile → Account** shows how other people see you in the workspace.

- **Change display name** sets the name shown on member lists, the audit log, and record history. It must be 2–64 characters of letters, digits, and spaces, and cannot start or end with a space. Your display name is shared across all your workspaces.
- **Upload photo** sets your avatar from a PNG or JPEG image of up to 10 MB. The image is cropped to a centered square, resized, and placed on a white background, so it takes a moment to appear. **Change photo** replaces it and **Remove photo** goes back to your initials.

Your photo belongs to the current workspace: set one in each workspace you use. Every member of the workspace can see it, but only the resized version is shared. The original file you uploaded is never shown to anyone.

## Personal API tokens

Scripts, the CLI, and integrations authenticate with personal API tokens. Create one under **Profile → Personal API tokens**, or with `acli token create`.

- A token has a label, an optional expiry, and an explicit list of permissions. It can never do more than its owner: if the owner loses a permission, the token loses it too.
- The token secret starts with `cat_pat_` and is shown once. Store it in a secret manager.
- Send it as `Authorization: Bearer cat_pat_…`. The token decides the workspace; no header or parameter selects one.
- A token can create another token only with a subset of its own permissions. If the creating token expires, the new one must expire no later.
- Revoke tokens you no longer need. Revoking a token does not revoke tokens it created; revoke each one. The token list shows when each was last used, to within a minute.

Creating tokens needs `tokens.manage`.

## Retention holds

A retention hold keeps a file's exact bytes in storage until a date. While any hold on a file is active, Attricat never reclaims it, even when no record references it any more.

Holds come from two places:

- **Record statuses.** A blueprint status with `retention_days` places a hold on the files of a record when it enters that status, for example when a document is released. See [Retain released files](/builders/validation/#retain-released-files). These holds cannot be released early.
- **Explicit holds**, such as a legal hold. People with the `files.hold` permission (owners and admins by default) can place one on a file for a number of days with a reason, and release it early.

The record page lists the holds on its files and when they expire. Placing and releasing holds is recorded in the audit log.

```sh
curl -X POST "$ATTRICAT_API_URL/files/<file-id>/retention-holds" \
  -H "Authorization: Bearer $ATTRICAT_TOKEN" -H 'Content-Type: application/json' \
  -d '{"days": 365, "reason": "Litigation hold 2026-14"}'
```

## Audit log

**Manage → Activity / Audit log** lists every successful change in the workspace: who did it (person, token, or agent), what changed, when, and the request ID. Filter by time, actor, action category, target type, or whether a person or an agent made the change.

Audit records never contain passwords, tokens, or secrets. Changes made by an agent show the agent run, the tool, and who approved it. Failed or denied requests are not recorded, because they changed nothing.

Reading the audit log needs `audit.read`.

```sh
acli audit list --executor-type agent --occurred-after 2026-03-01T00:00:00Z
```
