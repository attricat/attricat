---
title: Agents and approvals
description: Ask an AI agent to inspect and change your catalog, with every change stopped for your approval.
---

The Attricat agent is a conversational assistant that can read your catalog and propose changes to it. It can look up entities, check blueprints, search, and review data health without asking. Any change it wants to make stops and waits for a person to approve it.

Agents are optional. An administrator has to [configure an AI provider](/reference/configuration/#agents) before they appear.

In the public demo the agent is read-only. It can do everything listed under *Without approval*, and when you ask for a change it explains what it would do instead of proposing it. The conversation shows a **Read only** notice.

## Start a conversation

Open **Agents** in the sidebar and choose **New conversation**. Give the thread a title that describes the task, such as *Clean up inactive suppliers*, and describe what you want.

You can also start a conversation from:

- **Ask about this entity** on an entity page;
- **Send to agent conversation** after selecting entities in the Explorer.

Attach files with **Add files**, up to 16 per message. Images up to 5 MiB are sent to the provider so the agent can see them. Other files are described by name and type, and the agent can open them with its file tools.

## What the agent can do

The agent acts as you. It can only see and change what your role allows.

**Without approval** it can:

- list and read blueprints, contexts, entities, and their history;
- look up workspace users and teams to fill in user or team attributes;
- search entities and read saved searches;
- preview an entity migration;
- read data health, rule findings, and workflow runs;
- view images and read text files in the workspace;
- read extension operation runs and connector jobs (with `extensions.manage`);
- explain an entity's status transitions, approvals, and retention holds.

**With your approval** it can:

- create blueprints and blueprint revisions, and publish blueprints;
- create, update, migrate, and delete entities; set, remove, and restore values; change relationships; link files;
- apply several entity changes together as one batch;
- update system tags and metadata;
- publish and unpublish entities;
- create, update, and delete contexts;
- create and update saved searches.

It cannot manage rules, workflows, extensions, members, or roles.

## Approve or reject

When the agent wants to change something, the conversation shows **Approval needed** with the tool it wants to use. Expand **Show proposed input JSON** to see exactly what it will send.

- **Approve** runs that one change. When it runs, Attricat checks your permissions again, so approval never lets the agent do more than you could.
- **Reject** stops it. Tell the agent what to do differently.

A decision is final. Approving twice never runs a change twice.

Read proposals carefully. Replacing relationships sets the complete list for that attribute and context; an empty list removes every link.

### Changes to several entities

When one request changes several entities, such as releasing a new revision and superseding the previous one, the agent proposes a single **batch** (`apply_entity_batch`). The approval summary lists every step in order. You approve the batch once, and it is saved completely or not at all: if one step fails, for example because an entity changed in the meantime, nothing is saved and the agent is told which step failed.

## When a change is refused

Changes made through the agent go through the same validation as your own edits, and the server refuses them for the same reasons. When an approved change is refused, nothing is saved. The agent explains why rather than retrying, and proposes a corrected change if one makes sense, which needs your approval again.

- **Checks and rules.** A failing check (`entity_check_failed`), status transition condition (`transition_conditions_unmet`), enforcing rule (`rule_violation`), or channel's required checks (`publication_checks_failed`) refuse the change. The agent explains which checks failed. To explain why a status option is blocked or an entity cannot be published yet, it can read the entity's status transitions and publication readiness.
- **Unique keys.** If another entity already has the same part number or document number, the change is refused and the agent is told which entity holds it. It should show you that entity and ask whether to update it or use a different value, not retry.
- **Hierarchies.** A link that would make an entity its own ancestor, such as a location inside itself, is refused with the path of the loop.
- **Allowed targets.** A relationship can only link to the blueprints it lists.
- **Publishing constraints.** Publishing a blueprint that adds a unique key or hierarchy fails if existing entities break it; the agent lists them so you can fix them first.
- **Controlled records.** Blueprints can restrict who makes a status transition, lock finalized records, and tie approvals to reviewed content, and the agent follows the same rules as you. A locked record or a transition you are not permitted to make is refused with a clear reason, and the agent can show which transitions you may make, who must act, and which correction transition unlocks the record.
- A transition that must be made by a different person than an earlier one counts you as the person, because the change runs as you when you approve it.
- If a proposed edit touches approved content, approving it voids the approval and returns the record to an earlier status in the same change.

## Where changes show up

Changes made through the agent are audited like your own edits. On an entity's **Changes** page and in **Manage → Activity / Audit log**, they show the agent run, the tool, the approval decision, and who approved.

## Data sent to the provider

Every request to the AI provider includes the conversation so far and the results of the agent's tools, which can contain catalog data. Choose a provider whose data retention terms suit your catalog. Attricat stores the conversation, but not the provider's raw responses or credentials.

## Permissions

Using agents needs `agents.run`, which the owner and admin roles have by default. Grant it to people who should be able to request changes this way.
