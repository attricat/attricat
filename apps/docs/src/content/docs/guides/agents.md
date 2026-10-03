---
title: Agents and approvals
description: Ask an AI agent to inspect and change your catalog, with every change stopped for your approval.
---

The Attricat agent is a conversational assistant that can read your catalog and propose changes to it. It can look up entities, check blueprints, search, and review data health without asking. Any change it wants to make stops and waits for a person to approve it.

Agents are optional. An administrator has to [configure an AI provider](/reference/configuration/#agents) before they appear.

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
- search entities and read saved searches;
- preview an entity migration;
- read data health, rule findings, and workflow runs;
- view images and read text files in the workspace;
- read extension operation runs and connector jobs (with `extensions.manage`);
- explain an entity's status transitions, approvals, and retention holds.

**With your approval** it can:

- create blueprints and blueprint revisions, and publish blueprints;
- create, update, migrate, and delete entities; set, remove, and restore values; change relationships; link files;
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

## Controlled records

Blueprints can restrict who makes a status transition, lock finalized records, and tie approvals to reviewed content. The agent follows the same rules as you:

- A change the rules refuse fails with a clear reason, such as a locked record or a transition you are not permitted to make. The agent explains it rather than retrying, and can show which transitions you may make, who must act, and which correction transition unlocks the record.
- A transition that must be made by a different person than an earlier one counts you as the person, because the change runs as you when you approve it.
- If a proposed edit touches approved content, approving it voids the approval and returns the record to an earlier status in the same change.

## Where changes show up

Changes made through the agent go through the same validation and audit as your own edits. On an entity's **Changes** page and in **Manage → Activity / Audit log**, they show the agent run, the tool, the approval decision, and who approved.

## Data sent to the provider

Every request to the AI provider includes the conversation so far and the results of the agent's tools, which can contain catalog data. Choose a provider whose data retention terms suit your catalog. Attricat stores the conversation, but not the provider's raw responses or credentials.

## Permissions

Using agents needs `agents.run`, which the owner and admin roles have by default. Grant it to people who should be able to request changes this way.
