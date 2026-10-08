---
title: Publishing
description: Approve records for export per channel, and understand when an approval is withdrawn.
---

Publishing marks a record as approved by a person for a channel. A downstream system, such as a connector export or a storefront integration, can then send only approved records.

Publication approves the record as it is now; Attricat does not store a copy. If the record changes, the approval is withdrawn until someone publishes it again, so nothing reaches a channel without someone having looked at its current state.

## Enable channels

Any context can be a channel. Open **Manage → Exports** and switch **Export channel** on for the contexts you publish to, for example `PL-web` and `DE-web`.

Enabling a channel needs `contexts.write`.

## Publish a record

On the record page, the **Publication** section lists each enabled channel with **Published** or **Not published**, and who published it and when.

- **Publish** approves the record for the selected channel.
- **Publish to all channels** approves it for every enabled channel at once.
- **Unpublish** withdraws the approval for a channel.
- **Republish** approves again after an edit withdrew it.

In the Explorer, select a channel as the context to get a **Publication** column. Sort it to bring unpublished records to the top.

Publishing needs `entities.publish`. The owner and admin roles have it; editors do not. In the API, the CLI, and permission names, records are called entities.

With the CLI:

```sh
acli entity publication list <entity-id>
acli entity publication publish <entity-id> --context-id <channel-context-id>
acli entity publication publish-all <entity-id>
acli blueprint publish-entities-all <blueprint-id> <version>
```

## Require checks before publication

A channel can refuse records that are not ready, for example a supplier portal that must never receive a product without an SKU or with an expired certificate. In **Manage → Exports**, set the channel's **Publication checks**: **Required rules** and **Require a valid record**. With the API:

```http
PUT /publication-channels/{context_id}
{"enabled": true, "required_rule_codes": ["has-sku", "certificate-valid"], "require_valid_entity": true}
```

- `required_rule_codes` lists up to 32 [data quality rule](/builders/rules/) codes. A listed rule applies to a record when a rule with that code is enabled for the record's blueprint revision and is not attached to a different context. Rules that do not apply are skipped. Any predicate works, including `unique` and `stale`. Each code must name a rule that exists in the workspace, so a typo is rejected with `422 invalid_input` instead of silently turning the check off.
- `require_valid_entity` re-checks the blueprint's record schema and its [checks](/builders/validation/#compare-attributes-with-checks) in the channel context. This catches problems that appear without an edit, such as a `relative_date` check on an expiry date.
- Both are optional. Leaving one out keeps its current setting. `GET /publication-channels` shows them.

Checks are evaluated live, in the channel context, when someone publishes. They do not use stored findings, so a fix counts immediately.

If a check fails, nothing is published and the request returns `422 publication_checks_failed`. `error.details.context` is the channel code, and `error.details.violations` lists the failing rules and checks in the same shape as [validation errors](/builders/validation/#errors-and-how-to-fix-them). This applies to **Publish**, **Publish to all channels**, and publishing all records of a blueprint. In the bulk case, one failing record rejects the whole request, and each violation's `evidence.entity_id` names the record.

The record page's **Publication** section marks channels the record is **Not ready** for. To check readiness from the API without publishing, call `GET /v1/entities/{id}/publications/readiness`. It returns each enabled channel with `ready` and its `violations`. Fix the listed attributes, or the linked records the messages name, then publish again.

## What withdraws a publication

By default, any change to a record withdraws all of its channel publications: values, relationships, files, system metadata, and blueprint upgrades. On the record page each field is saved as its own change, so saving the first field withdraws the publications.

Changing or deleting a context withdraws the publications in that channel, because it can change what every record resolves to there.

Deleting a record withdraws all of its publications.

## Keep publication after trusted edits

A blueprint can name roles whose edits keep existing publications:

```toml
[publication]
retain_on_edit_roles = ["admin", "product_owner"]
```

The roles must exist when the blueprint revision is published. This setting does not let anyone edit or publish; they still need those permissions. It does not apply to context changes or deletions, which always withdraw.

A retained edit still has to pass each channel's [publication checks](#require-checks-before-publication). After the edit, Attricat re-runs the checks of every channel the record is published to and withdraws the publications whose checks now fail. The others stay published. For example, removing the SKU keeps the publication in a channel without checks but withdraws it from a channel that requires `has-sku`.

## Exports

Connector export jobs declared on a blueprint run once per enabled channel and include only records published to that channel at the time each page is read. A record unpublished during an export drops out of the pages that follow. Data already sent to an external system cannot be recalled. See [Connector jobs](/reference/blueprint/#connector-jobs).
