---
title: Agents and approvals
description: Ask an AI agent to inspect and change your catalog, with every change stopped for your approval.
---

The Attricat agent is a conversational assistant that can read your catalog and propose changes to it. It can look up records, check blueprints, search, and review data health without asking. Any change it wants to make stops and waits for a person to approve it.

Agents are optional. An administrator has to [configure an AI provider](/reference/configuration/#agents) before they appear.

In the public demo the agent is read-only. It can do everything listed under *Without approval*, and when you ask for a change it explains what it would do instead of proposing it. The conversation shows a **Read only** notice.

## Start a conversation

Open **Agents** in the sidebar and choose **New conversation**. Give the thread a title that describes the task, such as *Clean up inactive suppliers*, and describe what you want.

You can also start a conversation from:

- **Ask about this record** on a record page. These conversations are listed on the record's **Conversations** tab; turn on **Show record conversations** to include them in the **Agents** list;
- **Send to agent conversation** after selecting records in the Explorer.

Attach files with **Add files**, up to 16 per message. Images up to 5 MiB are sent to the provider so the agent can see them. The agent also reads the text of PDF, Word (`.docx`) and Excel (`.xlsx`) files: a short document is included with your message, and the agent reads a longer one page by page or sheet by sheet. Other files are described by name and type, and the agent can open them with its file tools.

The agent reads text, not pictures of text. It cannot read a scanned PDF without a text layer or a password-protected file, and tells you so. From a spreadsheet it reads the values Excel last calculated, not the formulas. Very large documents are read up to a size limit.

## What the agent can do

The agent acts as you. It can only see and change what your role allows.

**Without approval** it can:

- list and read blueprints, contexts, records, and their history;
- look up workspace users and teams to fill in user or team attributes;
- search records and read saved searches;
- preview a record migration;
- read data health, rule findings, and workflow runs;
- view images and read text, PDF, Word and Excel files in the workspace;
- read extension operation runs and connector jobs (with `extensions.manage`);
- explain a record's status transitions, approvals, and retention holds;
- read your [inbox](/guides/inbox/).

**With your approval** it can:

- create blueprints and blueprint revisions, and publish blueprints;
- create, update, migrate, and delete records; set, remove, and restore values; change relationships; link files;
- apply several record changes together as one batch;
- set the same values on many records with one approval;
- update system tags and metadata;
- publish and unpublish records;
- create, update, and delete contexts;
- create and update saved searches;
- mark notifications in your inbox as read or unread, or delete them.

It cannot manage rules, workflows, extensions, members, or roles.

## Approve or reject

When the agent wants to change something, the conversation shows **Approval needed** with the tool it wants to use. Expand **Show proposed input JSON** to see exactly what it will send.

- **Approve** runs that one change. When it runs, Attricat checks your permissions again, so approval never lets the agent do more than you could.
- **Reject** stops it. Tell the agent what to do differently.

A decision is final. Approving twice never runs a change twice.

Read proposals carefully. Replacing relationships sets the complete list for that attribute and context; an empty list removes every link.

### Changes to several records

When one request changes several records, such as releasing a new revision and superseding the previous one, the agent proposes a single **batch** (`apply_record_batch`). The approval summary lists every step in order. You approve the batch once, and it is saved completely or not at all: if one step fails, for example because a record changed in the meantime, nothing is saved and the agent is told which step failed.

### The same edit on many records

When you ask for the same value change on many records, such as *set the season to Spring on every Acme product*, the agent proposes one **bulk edit** (`bulk_set_record_values`). It picks the records either by listing them or with a search filter.

When the agent proposes it, the filter is turned into a fixed list of records, and that list is what you approve. The approval card shows a summary such as *Set Brand, Season on 412 Product records* and a table of record names with each current and proposed value: the first 20 records, then **Show all**. A record that someone else saves after the proposal is not overwritten; it fails as changed in the meantime.

Unlike a batch, a bulk edit is not all-or-nothing:

- After you approve, records change one at a time, and the conversation shows the progress.
- Each record is checked against your permissions as it changes, and audited like your own edit.
- The edit **stops at the first record that fails**. Records before it keep their changes, and records after it are not changed. Attricat leaves a comment on the failed record, signed **Attricat** with a **System** badge, with the error and a link to the conversation.
- The result lists the records that changed, the one that failed and why, and the ones that were not changed, with links to each.

Bulk edits cannot change status attributes, relationships, or files. A proposal that selects more records than the workspace allows (500 by default) is not offered for approval; the agent narrows it or splits it instead.

Use a batch when the changes must succeed or fail together, and a bulk edit when you want the same change on many records and can fix a failed record afterwards.

## When a change is refused

Changes made through the agent go through the same validation as your own edits, and the server refuses them for the same reasons. When an approved change is refused, nothing is saved. The agent explains why rather than retrying, and proposes a corrected change if one makes sense, which needs your approval again.

- **Checks and rules.** A failing check (`record_check_failed`), status transition condition (`transition_conditions_unmet`), enforcing rule (`rule_violation`), or channel's required checks (`publication_checks_failed`) refuse the change. The agent explains which checks failed. To explain why a status option is blocked or a record cannot be published yet, it can read the record's status transitions and publication readiness.
- **Unique keys.** If another record already has the same part number or document number, the change is refused and the agent is told which record holds it. It should show you that record and ask whether to update it or use a different value, not retry.
- **Hierarchies.** A link that would make a record its own ancestor, such as a location inside itself, is refused with the path of the loop.
- **Allowed targets.** A relationship can only link to the blueprints it lists.
- **Publishing constraints.** Publishing a blueprint that adds a unique key or hierarchy fails if existing records break it; the agent lists them so you can fix them first.
- **Controlled records.** Blueprints can restrict who makes a status transition, lock finalized records, and tie approvals to reviewed content, and the agent follows the same rules as you. A locked record or a transition you are not permitted to make is refused with a clear reason, and the agent can show which transitions you may make, who must act, and which correction transition unlocks the record. The agent cannot unlock a record itself. When no transition out is open to you, it points you to an owner or administrator, who can use **Unlock…** on the record page.
- A transition that must be made by a different person than an earlier one counts you as the person, because the change runs as you when you approve it.
- If a proposed edit touches approved content, approving it voids the approval and returns the record to an earlier status in the same change.

## Where changes show up

Changes made through the agent are audited like your own edits. On a record's **Changes** page and in **Manage → Activity / Audit log**, they show the agent run, the tool, the approval decision, and who approved.

## Data sent to the provider

Every request to the AI provider includes the conversation so far and the results of the agent's tools, which can contain catalog data. Choose a provider whose data retention terms suit your catalog. Attricat stores the conversation, but not the provider's raw responses or credentials.

## Permissions

Using agents needs `agents.run`, which the owner and admin roles have by default. Grant it to people who should be able to request changes this way.

### Bulk edit approvals

People with `agents.manage` (by default the owner and admin roles) choose how bulk edits are approved in **Manage → Workspace → Agent**, with `acli workspace agent set`, or through `PUT /workspace/agent-settings`:

- **Approve once per proposal** (the default): one approval covers every record a bulk edit lists.
- **Approve each record**: the agent is not offered bulk edits, so it proposes each record's change separately and each needs its own approval.

**Records per bulk edit** limits how many records one proposal can change, from 1 to 5,000 (500 by default).
