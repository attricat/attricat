# Notifications

Every workspace member has a personal inbox of notifications in each workspace
they belong to. Parts of the system create notifications for the people a
change affects. The member reads them in the web app's **Inbox**, through the
API, with `acli notification`, or through agent tools, and can mark them read or
unread or delete them permanently.

## Model

`user_notifications` stores one row per recipient:

| Column | Meaning |
| --- | --- |
| `workspace_id`, `recipient_user_id` | Whose inbox the row is in. Every read and write is keyed by both. |
| `kind` | A dotted code such as `record.assigned` (`^[a-z][a-z0-9_.]{0,99}$`). |
| `title` | A plain-text, self-contained summary of 1–300 characters, used by the API, the CLI and agents, and by the web app for kinds it does not know. |
| `body` | Optional detail of up to 2,000 characters, for example a comment excerpt. |
| `actor_user_id` | Who caused it, or `NULL` for automation and agent runs. |
| `subject_kind`, `subject_id` | Optional link target: `record` or `agent_conversation`. Both are set or neither is; a notification without a subject is a plain message. |
| `data` | A JSON object of kind-specific values that the web app uses to render a translated message. |
| `read_at` | `NULL` while unread. |

Deleting a notification removes the row; there is no soft delete or archive.

## Delivery rules

Producers call `AttricatRepository::notify_on` inside the transaction that
causes the notification, so the notification exists exactly when its cause
commits and disappears with a rollback. `notify_users` opens its own
transaction for producers that have none. Both apply these rules:

- Only active members of the workspace (with an active user account) receive
  notifications.
- The actor who caused a notification never receives it.
- A notification whose subject is a record only reaches recipients who hold
  `records.read` on that record when it is created.
- The same notification (recipient, kind, subject and `data`) is created at most
  once per transaction, so write paths that revalidate a change twice notify
  once.
- Titles name a record by its blueprint, never by its label: labels can contain
  values a recipient may not read. Clients resolve the current label through the
  authorized `POST /v1/records/labels` and fall back to the blueprint name.

## Producers

| Kind | Recipients | Subject | Created in |
| --- | --- | --- | --- |
| `record.assigned` | A newly assigned user, or each member of a newly assigned team (`data.team_id`, `data.team_name`) | Record | Every record write, through assignment validation (`validate_principal_values`). Saving an unchanged assignment notifies nobody. |
| `record.commented` | The record's current user and team assignees and everyone who commented on it before | Record | `create_record_comment`; `body` holds the first 500 characters of the comment. |
| `agent.approval_required` | The user who started the run | Agent conversation | `transition_agent_run` to `awaiting_approval` |
| `agent.run_failed` | The user who started the run | Agent conversation | A run that fails, times out or is interrupted |
| `agent.run_completed` | The user who started the run | Agent conversation | Completion of scheduled and manual runs; interactive completions are visible in the conversation |
| `team.member_added` | Users added to a team | None | Team creation and member changes |
| `workspace.invitation_accepted` | The member who sent the invitation | None | Invitation acceptance and invited-user onboarding |

To add a producer:

1. Add the kind to `repository::notifications::kinds`.
2. Call `notify_on` with the producer's transaction, or `notify_users` outside
   one. Write a self-contained English `title` and put everything a client
   needs to render the message in `data`.
3. Add a translated message for the kind in
   `apps/web/src/features/notifications/notificationMessage.ts` and both
   locale files. Clients that do not know the kind show `title`.
4. Document the kind in the table above and in the user guide.

## Access

The inbox needs no catalog permission: every active workspace member can use
their own. The HTTP routes authenticate like `/auth/session`; a personal API
token reaches its owner's inbox regardless of its permission list. Requests for
another member's notification return `404`.

The mutations are not written to the audit log: they change only the caller's
own inbox. Notifications themselves are a consequence of audited changes.

## Interfaces

- **API:** `GET /notifications`, `GET /notifications/unread-count`,
  `GET|PATCH|DELETE /notifications/{id}`, `POST /notifications/read-all`; see
  [API reference](api.md#notifications).
- **CLI:** `acli notification list|count|get|read|unread|read-all|delete`; see
  [CLI](cli.md#notifications).
- **Agents:** `list_notifications` reads the initiating user's inbox.
  `mark_notifications_read`, `mark_all_notifications_read` and
  `delete_notifications` change it after approval, like every other agent
  write. They act only on the initiating user's notifications and need only an
  active membership.
- **Web app:** the **Inbox** navigation item shows the unread count, refreshed
  every 30 seconds and after each inbox change. `/inbox` lists notifications
  with **All** and **Unread** tabs, per-item read/unread and delete actions, and
  **Mark all as read**. Opening a notification with a subject marks it read and
  goes to the record or conversation.
