---
title: Publishing
description: Approve entities for export per channel, and understand when an approval is withdrawn.
---

Publishing records that a person has approved an entity for a channel. A downstream system, such as a connector export or a storefront integration, can then send only approved entities.

Publication approves the entity as it is now; Attricat does not store a copy. If the entity changes, the approval is withdrawn until someone publishes it again, so nothing reaches a channel without someone having looked at its current state.

## Enable channels

Any context can be a channel. Open **Manage → Exports** and switch **Export channel** on for the contexts you publish to, for example `PL-web` and `DE-web`.

Enabling a channel needs `contexts.write`.

## Publish an entity

On the entity page, the **Publication** section lists each enabled channel with **Published** or **Not published**, and who published it and when.

- **Publish** approves the entity for the selected channel.
- **Publish to all channels** approves it for every enabled channel at once.
- **Unpublish** withdraws the approval for a channel.
- **Republish** approves again after an edit withdrew it.

In the Explorer, select a channel as the context to get a **Publication** column. Sort it to bring unpublished entities to the top.

Publishing needs `entities.publish`. The owner and admin roles have it; editors do not.

With the CLI:

```sh
acli entity publication list <entity-id>
acli entity publication publish <entity-id> --context-id <channel-context-id>
acli entity publication publish-all <entity-id>
acli blueprint publish-entities-all <blueprint-id> <version>
```

## What withdraws a publication

By default, any change to an entity withdraws all of its channel publications: values, relationships, files, system metadata, and blueprint upgrades.

Changing a context withdraws the publications in that channel, because it can change what every entity resolves to there.

## Keep publication after trusted edits

A blueprint can name roles whose edits keep existing publications:

```toml
[publication]
retain_on_edit_roles = ["admin", "product_owner"]
```

The roles must exist when the blueprint revision is published. This setting does not let anyone edit or publish; they still need those permissions. It does not apply to context changes, which always withdraw.

## Exports

Connector export jobs declared on a blueprint run once per enabled channel and include only entities published to that channel at the time each page is read. An entity unpublished during an export drops out of the pages that follow. Data already sent to an external system cannot be recalled. See [Connector jobs](/reference/blueprint/#connector-jobs).
