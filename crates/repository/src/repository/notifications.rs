//! The per-user, per-workspace notification inbox.
//!
//! Producers call [`CatalogRepository::notify_on`] inside the transaction
//! that causes the notification, so a notification exists exactly when its
//! cause committed. Recipients only ever see and change their own rows: every
//! read and write below is keyed by the workspace scope and the recipient.

use super::{CatalogRepository, RepositoryError};
use catalog_validation::principal::{PrincipalKind, PrincipalRef};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{FromRow, Postgres, Transaction};
use uuid::Uuid;

pub const NOTIFICATION_PAGE_SIZE: i64 = 30;
pub const MAX_NOTIFICATION_TITLE_LENGTH: usize = 300;
pub const MAX_NOTIFICATION_BODY_LENGTH: usize = 2_000;

/// What a notification is about, so clients can link to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationSubjectKind {
    Entity,
    AgentConversation,
}

impl NotificationSubjectKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Entity => "entity",
            Self::AgentConversation => "agent_conversation",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "entity" => Some(Self::Entity),
            "agent_conversation" => Some(Self::AgentConversation),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationSubject {
    pub kind: NotificationSubjectKind,
    pub id: Uuid,
}

/// A notification to deliver. `title` is a plain-text, self-contained
/// summary used by the API, CLI and agents; `kind` and `data` let the web
/// app render a translated message instead.
#[derive(Clone, Debug)]
pub struct NewNotification {
    pub kind: &'static str,
    pub title: String,
    pub body: Option<String>,
    pub actor_user_id: Option<Uuid>,
    pub subject: Option<NotificationSubject>,
    pub data: Value,
}

#[derive(Debug, Serialize)]
pub struct Notification {
    pub id: Uuid,
    pub kind: String,
    pub title: String,
    pub body: Option<String>,
    pub actor_user_id: Option<Uuid>,
    pub actor_display_name: Option<String>,
    pub actor_email: Option<String>,
    pub subject: Option<NotificationSubject>,
    pub data: Value,
    pub read: bool,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct NotificationRow {
    id: Uuid,
    kind: String,
    title: String,
    body: Option<String>,
    actor_user_id: Option<Uuid>,
    actor_display_name: Option<String>,
    actor_email: Option<String>,
    subject_kind: Option<String>,
    subject_id: Option<Uuid>,
    data: Value,
    read_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

impl From<NotificationRow> for Notification {
    fn from(row: NotificationRow) -> Self {
        let subject = row
            .subject_kind
            .as_deref()
            .and_then(NotificationSubjectKind::parse)
            .zip(row.subject_id)
            .map(|(kind, id)| NotificationSubject { kind, id });
        Self {
            id: row.id,
            kind: row.kind,
            title: row.title,
            body: row.body,
            actor_user_id: row.actor_user_id,
            actor_display_name: row.actor_display_name,
            actor_email: row.actor_email,
            subject,
            data: row.data,
            read: row.read_at.is_some(),
            read_at: row.read_at,
            created_at: row.created_at,
        }
    }
}

/// Keyset cursor: the `created_at` and `id` of the last item of a page.
pub type NotificationCursor = (DateTime<Utc>, Uuid);

/// Truncates to at most `limit` characters, marking a cut with an ellipsis.
pub fn notification_excerpt(text: &str, limit: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let mut excerpt: String = text.chars().take(limit.saturating_sub(1)).collect();
    excerpt.push('…');
    excerpt
}

impl CatalogRepository {
    /// Delivers `notification` to each recipient that is an active member of
    /// this workspace, except the actor who caused it. A notification about
    /// a record only reaches recipients who may read that record, and an
    /// identical notification already delivered in this transaction is not
    /// repeated. Returns how many notifications were created.
    pub(crate) async fn notify_on(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        recipients: &[Uuid],
        notification: &NewNotification,
    ) -> Result<u64, RepositoryError> {
        let mut allowed = Vec::with_capacity(recipients.len());
        for recipient in recipients {
            if Some(*recipient) == notification.actor_user_id || allowed.contains(recipient) {
                continue;
            }
            if let Some(NotificationSubject {
                kind: NotificationSubjectKind::Entity,
                id,
            }) = notification.subject
                && !Self::authorized_entity_ids_on(
                    transaction,
                    *recipient,
                    self.workspace_id_for_runtime(),
                    "entities.read",
                    &[id],
                )
                .await?
                .contains(&id)
            {
                continue;
            }
            allowed.push(*recipient);
        }
        if allowed.is_empty() {
            return Ok(0);
        }
        let title = notification_excerpt(&notification.title, MAX_NOTIFICATION_TITLE_LENGTH);
        let body = notification
            .body
            .as_deref()
            .map(|body| notification_excerpt(body, MAX_NOTIFICATION_BODY_LENGTH))
            .filter(|body| !body.is_empty());
        // now() is the transaction start time, so it identifies rows this
        // transaction wrote: a write path that revalidates the same change
        // twice still notifies once.
        let result = sqlx::query(
            "INSERT INTO user_notifications (id,workspace_id,recipient_user_id,kind,title,body,actor_user_id,subject_kind,subject_id,data)
             SELECT gen_random_uuid(),m.workspace_id,m.user_id,$3,$4,$5,$6,$7,$8,$9
             FROM workspace_memberships m JOIN users u ON u.id=m.user_id
             WHERE m.workspace_id=$1 AND m.user_id=ANY($2) AND m.state='active' AND u.state='active'
               AND NOT EXISTS (SELECT 1 FROM user_notifications d
                 WHERE d.workspace_id=m.workspace_id AND d.recipient_user_id=m.user_id
                   AND d.created_at=now() AND d.kind=$3
                   AND d.subject_id IS NOT DISTINCT FROM $8 AND d.data=$9)",
        )
        .bind(self.workspace_id_for_runtime())
        .bind(&allowed)
        .bind(notification.kind)
        .bind(&title)
        .bind(body)
        .bind(notification.actor_user_id)
        .bind(notification.subject.map(|subject| subject.kind.as_str()))
        .bind(notification.subject.map(|subject| subject.id))
        .bind(&notification.data)
        .execute(&mut **transaction)
        .await?;
        Ok(result.rows_affected())
    }

    /// [`Self::notify_on`] in its own transaction, for producers outside a
    /// repository transaction (workers and the agent runtime).
    pub async fn notify_users(
        &self,
        recipients: &[Uuid],
        notification: &NewNotification,
    ) -> Result<u64, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let created = self
            .notify_on(&mut transaction, recipients, notification)
            .await?;
        transaction.commit().await?;
        Ok(created)
    }

    /// One page of `recipient`'s notifications, newest first, with one extra
    /// row when another page follows.
    pub async fn list_notifications(
        &self,
        recipient: Uuid,
        unread_only: bool,
        before: Option<NotificationCursor>,
        limit: i64,
    ) -> Result<Vec<Notification>, RepositoryError> {
        let (time, id) = before.map_or((None, None), |(time, id)| (Some(time), Some(id)));
        let rows: Vec<NotificationRow> = sqlx::query_as(
            "SELECT n.id,n.kind,n.title,n.body,n.actor_user_id,a.display_name AS actor_display_name,a.email AS actor_email,n.subject_kind,n.subject_id,n.data,n.read_at,n.created_at
             FROM user_notifications n LEFT JOIN users a ON a.id=n.actor_user_id
             WHERE n.workspace_id=$1 AND n.recipient_user_id=$2 AND (NOT $3 OR n.read_at IS NULL)
               AND ($4::timestamptz IS NULL OR (n.created_at,n.id)<($4,$5::uuid))
             ORDER BY n.created_at DESC,n.id DESC LIMIT $6",
        )
        .bind(self.workspace_id_for_runtime())
        .bind(recipient)
        .bind(unread_only)
        .bind(time)
        .bind(id)
        .bind(limit + 1)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Notification::from).collect())
    }

    pub async fn get_notification(
        &self,
        recipient: Uuid,
        notification: Uuid,
    ) -> Result<Notification, RepositoryError> {
        let row: Option<NotificationRow> = sqlx::query_as(
            "SELECT n.id,n.kind,n.title,n.body,n.actor_user_id,a.display_name AS actor_display_name,a.email AS actor_email,n.subject_kind,n.subject_id,n.data,n.read_at,n.created_at
             FROM user_notifications n LEFT JOIN users a ON a.id=n.actor_user_id
             WHERE n.workspace_id=$1 AND n.recipient_user_id=$2 AND n.id=$3",
        )
        .bind(self.workspace_id_for_runtime())
        .bind(recipient)
        .bind(notification)
        .fetch_optional(&self.pool)
        .await?;
        row.map(Notification::from)
            .ok_or(RepositoryError::NotFound("notification"))
    }

    pub async fn count_unread_notifications(
        &self,
        recipient: Uuid,
    ) -> Result<i64, RepositoryError> {
        Ok(sqlx::query_scalar(
            "SELECT count(*) FROM user_notifications WHERE workspace_id=$1 AND recipient_user_id=$2 AND read_at IS NULL",
        )
        .bind(self.workspace_id_for_runtime())
        .bind(recipient)
        .fetch_one(&self.pool)
        .await?)
    }

    /// Marks one notification read or unread. Repeating the same state is a
    /// no-op that keeps the original `read_at`.
    pub async fn set_notification_read(
        &self,
        recipient: Uuid,
        notification: Uuid,
        read: bool,
    ) -> Result<(), RepositoryError> {
        let found: Option<Uuid> = sqlx::query_scalar(
            "UPDATE user_notifications SET read_at=CASE WHEN $4 THEN COALESCE(read_at,now()) ELSE NULL END
             WHERE workspace_id=$1 AND recipient_user_id=$2 AND id=$3 RETURNING id",
        )
        .bind(self.workspace_id_for_runtime())
        .bind(recipient)
        .bind(notification)
        .bind(read)
        .fetch_optional(&self.pool)
        .await?;
        found
            .map(|_| ())
            .ok_or(RepositoryError::NotFound("notification"))
    }

    /// Marks every unread notification created up to `up_to` (default: now)
    /// read, so notifications that arrive while the inbox is open stay
    /// unread. Returns how many changed.
    pub async fn mark_all_notifications_read(
        &self,
        recipient: Uuid,
        up_to: Option<DateTime<Utc>>,
    ) -> Result<u64, RepositoryError> {
        let result = sqlx::query(
            "UPDATE user_notifications SET read_at=now()
             WHERE workspace_id=$1 AND recipient_user_id=$2 AND read_at IS NULL
               AND ($3::timestamptz IS NULL OR created_at<=$3)",
        )
        .bind(self.workspace_id_for_runtime())
        .bind(recipient)
        .bind(up_to)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Marks several notifications read or unread at once; IDs that are not
    /// the recipient's are ignored. Returns how many notifications matched.
    pub async fn set_notifications_read(
        &self,
        recipient: Uuid,
        notifications: &[Uuid],
        read: bool,
    ) -> Result<u64, RepositoryError> {
        let result = sqlx::query(
            "UPDATE user_notifications SET read_at=CASE WHEN $4 THEN COALESCE(read_at,now()) ELSE NULL END
             WHERE workspace_id=$1 AND recipient_user_id=$2 AND id=ANY($3)",
        )
        .bind(self.workspace_id_for_runtime())
        .bind(recipient)
        .bind(notifications)
        .bind(read)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Permanently deletes several notifications; IDs that are not the
    /// recipient's are ignored. Returns how many were deleted.
    pub async fn delete_notifications(
        &self,
        recipient: Uuid,
        notifications: &[Uuid],
    ) -> Result<u64, RepositoryError> {
        let result = sqlx::query(
            "DELETE FROM user_notifications WHERE workspace_id=$1 AND recipient_user_id=$2 AND id=ANY($3)",
        )
        .bind(self.workspace_id_for_runtime())
        .bind(recipient)
        .bind(notifications)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Permanently deletes one notification.
    pub async fn delete_notification(
        &self,
        recipient: Uuid,
        notification: Uuid,
    ) -> Result<(), RepositoryError> {
        let result = sqlx::query(
            "DELETE FROM user_notifications WHERE workspace_id=$1 AND recipient_user_id=$2 AND id=$3",
        )
        .bind(self.workspace_id_for_runtime())
        .bind(recipient)
        .bind(notification)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound("notification"));
        }
        Ok(())
    }
}

/// Notification kinds. Clients render a translated message per kind and fall
/// back to the stored title for kinds they do not know.
pub mod kinds {
    pub const ENTITY_ASSIGNED: &str = "entity.assigned";
    pub const ENTITY_COMMENTED: &str = "entity.commented";
    pub const AGENT_APPROVAL_REQUIRED: &str = "agent.approval_required";
    pub const AGENT_RUN_COMPLETED: &str = "agent.run_completed";
    pub const AGENT_RUN_FAILED: &str = "agent.run_failed";
    pub const TEAM_MEMBER_ADDED: &str = "team.member_added";
    pub const INVITATION_ACCEPTED: &str = "workspace.invitation_accepted";
}

/// Characters of a comment kept in its notification body.
const COMMENT_EXCERPT_LENGTH: usize = 500;

/// Who caused a notification, named for its plain-text title.
struct Actor {
    id: Option<Uuid>,
    name: String,
}

impl CatalogRepository {
    fn notification_actor_id(&self) -> Option<Uuid> {
        self.audit_context
            .as_ref()
            .and_then(|context| context.actor_user_id)
    }

    async fn notification_actor_on(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        actor: Option<Uuid>,
    ) -> Result<Actor, RepositoryError> {
        let name: Option<String> = match actor {
            Some(actor) => {
                sqlx::query_scalar(
                    "SELECT COALESCE(NULLIF(display_name,''),email) FROM users WHERE id=$1",
                )
                .bind(actor)
                .fetch_optional(&mut **transaction)
                .await?
            }
            None => None,
        };
        Ok(Actor {
            id: actor,
            name: name.unwrap_or_else(|| "An automation".to_owned()),
        })
    }

    /// The blueprint code and name of a live entity. Notifications name the
    /// blueprint rather than the record: record labels can contain values a
    /// recipient may not read, so clients resolve them through the
    /// authorized label lookup instead.
    async fn notification_entity_on(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: Uuid,
    ) -> Result<Option<(String, String)>, RepositoryError> {
        Ok(sqlx::query_as(
            "SELECT b.code,b.name FROM entities e JOIN blueprints b ON b.id=e.blueprint_id AND b.version=e.blueprint_version
             WHERE e.workspace_id=$1 AND e.id=$2 AND e.deleted_at IS NULL",
        )
        .bind(self.workspace_id_for_runtime())
        .bind(entity)
        .fetch_optional(&mut **transaction)
        .await?)
    }

    /// Notifies the users named by newly assigned values, and the active
    /// members of newly assigned teams, that they were assigned to `entity`.
    pub(super) async fn notify_assignments_on(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: Uuid,
        assignments: &[(String, PrincipalRef)],
    ) -> Result<(), RepositoryError> {
        if assignments.is_empty() {
            return Ok(());
        }
        let Some((blueprint_code, blueprint_name)) =
            self.notification_entity_on(transaction, entity).await?
        else {
            return Ok(());
        };
        let actor = self
            .notification_actor_on(transaction, self.notification_actor_id())
            .await?;
        for (attribute_code, reference) in assignments {
            let mut data = json!({
                "blueprint_code": blueprint_code,
                "blueprint_name": blueprint_name,
                "attribute_code": attribute_code,
            });
            let (recipients, title) = match reference.kind {
                PrincipalKind::User => (
                    vec![reference.id],
                    format!("{} assigned you to a {blueprint_name} record", actor.name),
                ),
                PrincipalKind::Team => {
                    let Some(team) = sqlx::query_scalar::<_, String>(
                        "SELECT name FROM teams WHERE workspace_id=$1 AND id=$2 AND deleted_at IS NULL",
                    )
                    .bind(self.workspace_id_for_runtime())
                    .bind(reference.id)
                    .fetch_optional(&mut **transaction)
                    .await?
                    else {
                        continue;
                    };
                    let members = sqlx::query_scalar(
                        "SELECT m.user_id FROM team_members tm JOIN workspace_memberships m ON m.id=tm.membership_id
                         WHERE tm.workspace_id=$1 AND tm.team_id=$2",
                    )
                    .bind(self.workspace_id_for_runtime())
                    .bind(reference.id)
                    .fetch_all(&mut **transaction)
                    .await?;
                    data["team_id"] = json!(reference.id);
                    data["team_name"] = json!(team);
                    (
                        members,
                        format!(
                            "{} assigned your team {team} to a {blueprint_name} record",
                            actor.name
                        ),
                    )
                }
            };
            self.notify_on(
                transaction,
                &recipients,
                &NewNotification {
                    kind: kinds::ENTITY_ASSIGNED,
                    title,
                    body: None,
                    actor_user_id: actor.id,
                    subject: Some(NotificationSubject {
                        kind: NotificationSubjectKind::Entity,
                        id: entity,
                    }),
                    data,
                },
            )
            .await?;
        }
        Ok(())
    }

    /// Notifies the record's current assignees (teams expanded to their
    /// members) and everyone who commented on it before, except the author.
    pub(super) async fn notify_comment_on(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: Uuid,
        comment: Uuid,
        author: Uuid,
        body: &str,
    ) -> Result<(), RepositoryError> {
        let Some((blueprint_code, blueprint_name)) =
            self.notification_entity_on(transaction, entity).await?
        else {
            return Ok(());
        };
        // Current assignees come from the preview projection: attribute
        // values keep history, so they also name earlier assignees.
        let assigned: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT preview.values ->> a.code
             FROM entities e
             JOIN attributes a ON a.workspace_id=e.workspace_id AND a.deleted_at IS NULL
              AND ((a.blueprint_id=e.blueprint_id AND a.blueprint_version=e.blueprint_version) OR a.entity_id=e.id)
              AND a.value_schema ? 'x-attricat-principal'
             CROSS JOIN LATERAL jsonb_each(COALESCE(e.projections -> 'preview', '{}'::jsonb)) AS preview(context_code, values)
             WHERE e.workspace_id=$1 AND e.id=$2 AND jsonb_typeof(preview.values -> a.code) = 'string'",
        )
        .bind(self.workspace_id_for_runtime())
        .bind(entity)
        .fetch_all(&mut **transaction)
        .await?;
        let references: Vec<PrincipalRef> = assigned
            .iter()
            .filter_map(|value| PrincipalRef::parse(value))
            .collect();
        let ids = |kind: PrincipalKind| -> Vec<Uuid> {
            references
                .iter()
                .filter(|reference| reference.kind == kind)
                .map(|reference| reference.id)
                .collect()
        };
        let mut recipients = ids(PrincipalKind::User);
        recipients.extend(
            sqlx::query_scalar::<_, Uuid>(
                "SELECT m.user_id FROM team_members tm JOIN teams t ON t.id=tm.team_id
                 JOIN workspace_memberships m ON m.id=tm.membership_id
                 WHERE tm.workspace_id=$1 AND tm.team_id=ANY($2) AND t.deleted_at IS NULL
                 UNION SELECT author_user_id FROM entity_comments WHERE workspace_id=$1 AND entity_id=$3",
            )
            .bind(self.workspace_id_for_runtime())
            .bind(ids(PrincipalKind::Team))
            .bind(entity)
            .fetch_all(&mut **transaction)
            .await?,
        );
        let actor = self
            .notification_actor_on(transaction, Some(author))
            .await?;
        self.notify_on(
            transaction,
            &recipients,
            &NewNotification {
                kind: kinds::ENTITY_COMMENTED,
                title: format!("{} commented on a {blueprint_name} record", actor.name),
                body: Some(notification_excerpt(body, COMMENT_EXCERPT_LENGTH)),
                actor_user_id: actor.id,
                subject: Some(NotificationSubject {
                    kind: NotificationSubjectKind::Entity,
                    id: entity,
                }),
                data: json!({
                    "blueprint_code": blueprint_code,
                    "blueprint_name": blueprint_name,
                    "comment_id": comment,
                }),
            },
        )
        .await?;
        Ok(())
    }

    /// Notifies users who were added to `team`.
    pub(super) async fn notify_team_members_added_on(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        team: Uuid,
        team_name: &str,
        added_users: &[Uuid],
    ) -> Result<(), RepositoryError> {
        if added_users.is_empty() {
            return Ok(());
        }
        let actor = self
            .notification_actor_on(transaction, self.notification_actor_id())
            .await?;
        self.notify_on(
            transaction,
            added_users,
            &NewNotification {
                kind: kinds::TEAM_MEMBER_ADDED,
                title: format!("{} added you to the team {team_name}", actor.name),
                body: None,
                actor_user_id: actor.id,
                subject: None,
                data: json!({"team_id": team, "team_name": team_name}),
            },
        )
        .await?;
        Ok(())
    }

    /// Notifies the inviter that `member` accepted their invitation.
    pub(super) async fn notify_invitation_accepted_on(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        inviter: Uuid,
        member: Uuid,
    ) -> Result<(), RepositoryError> {
        let actor = self
            .notification_actor_on(transaction, Some(member))
            .await?;
        self.notify_on(
            transaction,
            &[inviter],
            &NewNotification {
                kind: kinds::INVITATION_ACCEPTED,
                title: format!(
                    "{} accepted your invitation and joined the workspace",
                    actor.name
                ),
                body: None,
                actor_user_id: actor.id,
                subject: None,
                data: json!({}),
            },
        )
        .await?;
        Ok(())
    }
}

impl CatalogRepository {
    /// Notifies a run's initiating user when it waits for their approval or
    /// fails, and when a run they did not start interactively completes; an
    /// interactive run's completion is already visible in the conversation.
    pub(super) async fn notify_agent_run_on(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        run: Uuid,
        status: &str,
        error_code: Option<&str>,
    ) -> Result<(), RepositoryError> {
        let Some((recipient, conversation, title, origin)): Option<(Option<Uuid>, Uuid, String, String)> =
            sqlx::query_as(
                "SELECT r.initiated_by_user_id,r.conversation_id,c.title,r.origin FROM agent_runs r
                 JOIN conversations c ON c.id=r.conversation_id WHERE r.workspace_id=$1 AND r.id=$2",
            )
            .bind(self.workspace_id_for_runtime())
            .bind(run)
            .fetch_optional(&mut **transaction)
            .await?
        else {
            return Ok(());
        };
        let Some(recipient) = recipient else {
            return Ok(());
        };
        let conversation_title = if title.trim().is_empty() {
            "Untitled conversation".to_owned()
        } else {
            title.trim().to_owned()
        };
        let (kind, summary) = match status {
            "awaiting_approval" => (
                kinds::AGENT_APPROVAL_REQUIRED,
                "The agent is waiting for your approval",
            ),
            "failed" => (kinds::AGENT_RUN_FAILED, "The agent run failed"),
            "completed" if origin != "interactive" => {
                (kinds::AGENT_RUN_COMPLETED, "The agent run completed")
            }
            _ => return Ok(()),
        };
        self.notify_on(
            transaction,
            &[recipient],
            &NewNotification {
                kind,
                title: format!("{summary} in \"{conversation_title}\""),
                body: None,
                actor_user_id: None,
                subject: Some(NotificationSubject {
                    kind: NotificationSubjectKind::AgentConversation,
                    id: conversation,
                }),
                data: json!({
                    "run_id": run,
                    "conversation_title": conversation_title,
                    "origin": origin,
                    "error_code": error_code,
                }),
            },
        )
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excerpts_count_characters_and_mark_cuts() {
        assert_eq!(notification_excerpt("  short  ", 10), "short");
        assert_eq!(notification_excerpt(&"🦀".repeat(5), 5), "🦀".repeat(5));
        assert_eq!(
            notification_excerpt(&"🦀".repeat(6), 5),
            format!("{}…", "🦀".repeat(4))
        );
    }

    #[test]
    fn subject_kinds_round_trip_their_stored_names() {
        for kind in [
            NotificationSubjectKind::Entity,
            NotificationSubjectKind::AgentConversation,
        ] {
            assert_eq!(NotificationSubjectKind::parse(kind.as_str()), Some(kind));
        }
    }
}
