---
title: Agents and approvals
description: Use conversational agents while retaining explicit control over catalog changes.
---

Agents can inspect your workspace and propose catalog changes from a conversation. They are optional: an administrator must configure a trusted provider before the feature is available.

## Start a conversation

Open **Conversations** and create a focused thread. Explain the result you want, then review the agent's response and any proposed tool calls.

Read-only work can run automatically. Every catalog mutation stops for a durable, explicit approval.

## Approve changes deliberately

Before approving a change, inspect its proposed arguments and change summary. Approving authorizes only that proposed action. Reject it when the request is broader than intended or the result needs adjustment.

Scheduled runs follow the same rule: an approved run is re-authorized as its initiating user before it writes.

## Keep access narrow

Agent providers receive conversation content and tool results. Use a trusted provider account, grant `agents.run` only to people who should request changes, and review the activity log when needed.
