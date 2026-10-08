---
title: Inbox and notifications
description: See what changed for you in a workspace, mark notifications as read, and delete the ones you no longer need.
---

Your inbox collects notifications about things that concern you in the current workspace. Each workspace has its own inbox, and nobody else can see yours.

## What you are notified about

| Notification | When |
| --- | --- |
| **Assigned to a record** | Someone assigns you, or a team you belong to, to a record through a user or team attribute. Saving a record without changing the assignment does not notify you again. |
| **New comment** | Someone comments on a record you are assigned to (directly or through a team) or have commented on before. The notification shows the start of the comment. |
| **Agent needs approval** | An agent run you started is waiting for you to approve a change. |
| **Agent run failed** | An agent run you started failed or was interrupted. |
| **Agent run finished** | A scheduled agent run you set up has finished. Runs you start in a conversation show their result there instead. |
| **Added to a team** | Someone adds you to a team. |
| **Invitation accepted** | Someone you invited joins the workspace. |

You are never notified about your own actions. Notifications about a record only reach you if you can open that record.

## Open your inbox

Choose **Inbox** in the sidebar. The badge on the icon shows how many notifications you have not read yet; it updates every 30 seconds.

The inbox lists notifications newest first. Unread notifications are bold and marked with a dot. Use the **Unread** tab to see only those.

Select a notification about a record or an agent conversation to open it. Opening a notification marks it as read. Notifications such as *Added to a team* are messages only and do not open anything.

## Mark as read or unread

- **Mark as read** (open envelope) on a notification marks it read without opening it.
- **Mark as unread** (closed envelope) keeps it as a reminder.
- **Mark all as read** at the top marks every unread notification read. Notifications that arrive after you opened the inbox stay unread.

## Delete notifications

**Delete permanently** (bin) removes a notification. Deleting cannot be undone.

## Use the inbox from the CLI, API, or an agent

- The [CLI](/reference/cli/#notifications) has `acli notification` commands.
- The [API](/reference/api/#notifications) has the `/notifications` routes.
- An [agent](/guides/agents/) can read your inbox, and with your approval mark notifications as read or delete them.
