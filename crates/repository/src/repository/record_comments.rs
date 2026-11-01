use super::{AttricatRepository, RepositoryError};
use attricat_validation::principal::{PrincipalKind, PrincipalRef};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

pub const MAX_COMMENT_LENGTH: usize = 10_000;
pub const COMMENT_PAGE_SIZE: i64 = 30;

#[derive(Debug, Serialize, FromRow)]
pub struct RecordComment {
    pub id: Uuid,
    pub author_user_id: Uuid,
    pub author_display_name: Option<String>,
    pub author_email: String,
    /// `person`, or `system` for the workspace's system user, whose comments
    /// record automated outcomes such as a failed bulk edit.
    pub author_kind: String,
    pub body: String,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Opens a Markdown link target. A mention is a link whose target is a user
/// reference: `[@Ada](user:<uuid>)`.
const LINK_TARGET_OPENING: &str = "](";
const LINK_TARGET_CLOSING: char = ')';

/// The distinct users a comment mentions, in order of first mention.
pub fn comment_mentions(body: &str) -> Vec<Uuid> {
    let mut mentioned = Vec::new();
    for (start, opening) in body.match_indices(LINK_TARGET_OPENING) {
        let target = &body[start + opening.len()..];
        let Some(end) = target.find(LINK_TARGET_CLOSING) else {
            continue;
        };
        if let Some(PrincipalRef {
            kind: PrincipalKind::User,
            id,
        }) = PrincipalRef::parse(&target[..end])
            && !mentioned.contains(&id)
        {
            mentioned.push(id);
        }
    }
    mentioned
}

fn validate_body(body: &str) -> Result<(), RepositoryError> {
    if body.trim().is_empty() || body.chars().count() > MAX_COMMENT_LENGTH || body.contains('\0') {
        return Err(RepositoryError::InvalidComment);
    }
    Ok(())
}

impl AttricatRepository {
    pub async fn list_record_comments(
        &self,
        record: Uuid,
        before: Option<(DateTime<Utc>, Uuid)>,
    ) -> Result<Vec<RecordComment>, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        self.lock_record(&mut transaction, record).await?;
        let (time, id) = before.map_or((None, None), |(time, id)| (Some(time), Some(id)));
        let rows = sqlx::query_as(
            "SELECT c.id,c.author_user_id,u.display_name AS author_display_name,u.email AS author_email,u.kind AS author_kind,c.body,c.revision,c.created_at,c.updated_at FROM record_comments c JOIN users u ON u.id=c.author_user_id WHERE c.workspace_id=$1 AND c.record_id=$2 AND ($3::timestamptz IS NULL OR (c.created_at,c.id)<($3,$4::uuid)) ORDER BY c.created_at DESC,c.id DESC LIMIT $5",
        )
        .bind(self.workspace_id()).bind(record).bind(time).bind(id)
        .bind(COMMENT_PAGE_SIZE + 1).fetch_all(&mut *transaction).await?;
        transaction.commit().await?;
        Ok(rows)
    }

    pub async fn count_record_comments(&self, record: Uuid) -> Result<i64, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        self.lock_record(&mut transaction, record).await?;
        let count = sqlx::query_scalar(
            "SELECT count(*) FROM record_comments WHERE workspace_id=$1 AND record_id=$2",
        )
        .bind(self.workspace_id())
        .bind(record)
        .fetch_one(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(count)
    }

    pub async fn create_record_comment(
        &self,
        record: Uuid,
        actor: Uuid,
        body: &str,
    ) -> Result<(), RepositoryError> {
        validate_body(body)?;
        let mut transaction = self.pool.begin().await?;
        self.lock_record(&mut transaction, record).await?;
        // Notify before inserting so the author's first comment does not make
        // them a previous commenter; notify_on skips the author anyway.
        let comment = Uuid::new_v4();
        let mentioned = comment_mentions(body);
        self.notify_mentions_on(&mut transaction, record, comment, actor, body, &mentioned)
            .await?;
        self.notify_comment_on(&mut transaction, record, comment, actor, body, &mentioned)
            .await?;
        sqlx::query("INSERT INTO record_comments (id,workspace_id,record_id,author_user_id,body) VALUES ($1,$2,$3,$4,$5)")
            .bind(comment).bind(self.workspace_id()).bind(record).bind(actor).bind(body)
            .execute(&mut *transaction).await?;
        self.commit_mutation(transaction).await?;
        Ok(())
    }

    pub async fn update_record_comment(
        &self,
        record: Uuid,
        comment: Uuid,
        actor: Uuid,
        revision: i64,
        body: &str,
    ) -> Result<(), RepositoryError> {
        validate_body(body)?;
        let mut transaction = self.pool.begin().await?;
        self.lock_record(&mut transaction, record).await?;
        let current: Option<(i64, String)> = sqlx::query_as("SELECT revision,body FROM record_comments WHERE workspace_id=$1 AND record_id=$2 AND id=$3 AND author_user_id=$4 FOR UPDATE")
            .bind(self.workspace_id()).bind(record).bind(comment).bind(actor)
            .fetch_optional(&mut *transaction).await?;
        let (current, previous_body) = current.ok_or(RepositoryError::NotFound("comment"))?;
        if current != revision {
            return Err(RepositoryError::CommentConflict);
        }
        // Only people the edit newly mentions are told about it.
        let previously_mentioned = comment_mentions(&previous_body);
        let newly_mentioned: Vec<Uuid> = comment_mentions(body)
            .into_iter()
            .filter(|user| !previously_mentioned.contains(user))
            .collect();
        self.notify_mentions_on(
            &mut transaction,
            record,
            comment,
            actor,
            body,
            &newly_mentioned,
        )
        .await?;
        sqlx::query(
            "UPDATE record_comments SET body=$2,revision=revision+1,updated_at=now() WHERE id=$1",
        )
        .bind(comment)
        .bind(body)
        .execute(&mut *transaction)
        .await?;
        self.commit_mutation(transaction).await?;
        Ok(())
    }

    /// Deletes the author's own comment when it is still at `revision`.
    pub async fn delete_record_comment(
        &self,
        record: Uuid,
        comment: Uuid,
        actor: Uuid,
        revision: i64,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        self.lock_record(&mut transaction, record).await?;
        let current: Option<i64> = sqlx::query_scalar("SELECT revision FROM record_comments WHERE workspace_id=$1 AND record_id=$2 AND id=$3 AND author_user_id=$4 FOR UPDATE")
            .bind(self.workspace_id()).bind(record).bind(comment).bind(actor)
            .fetch_optional(&mut *transaction).await?;
        if current.ok_or(RepositoryError::NotFound("comment"))? != revision {
            return Err(RepositoryError::CommentConflict);
        }
        sqlx::query("DELETE FROM record_comments WHERE id=$1")
            .bind(comment)
            .execute(&mut *transaction)
            .await?;
        self.commit_mutation(transaction).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_unicode_length_and_blank_comments() {
        assert!(validate_body(" \n\t").is_err());
        assert!(validate_body("\0").is_err());
        assert!(validate_body(&"🦀".repeat(MAX_COMMENT_LENGTH)).is_ok());
        assert!(validate_body(&"🦀".repeat(MAX_COMMENT_LENGTH + 1)).is_err());
        assert!(validate_body("**Markdown**\n\n<script>text</script>").is_ok());
    }

    #[test]
    fn mentions_are_links_to_user_references() {
        let ada = Uuid::new_v4();
        let bob = Uuid::new_v4();
        let team = Uuid::new_v4();
        let body = format!(
            "[@Ada](user:{ada}), [@Bob](user:{bob}) and [@Ada again](user:{ada}); \
             [team](team:{team}), [site](https://example.test), [bad](user:{}), [open](user:{bob}",
            ada.simple()
        );
        assert_eq!(comment_mentions(&body), [ada, bob]);
        assert_eq!(
            comment_mentions("No mentions, just @Ada."),
            Vec::<Uuid>::new()
        );
    }
}
