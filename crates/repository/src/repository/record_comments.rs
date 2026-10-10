use super::{CatalogRepository, RepositoryError};
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
    pub body: String,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn validate_body(body: &str) -> Result<(), RepositoryError> {
    if body.trim().is_empty() || body.chars().count() > MAX_COMMENT_LENGTH || body.contains('\0') {
        return Err(RepositoryError::InvalidComment);
    }
    Ok(())
}

impl CatalogRepository {
    pub async fn list_record_comments(
        &self,
        record: Uuid,
        before: Option<(DateTime<Utc>, Uuid)>,
    ) -> Result<Vec<RecordComment>, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        self.lock_record(&mut transaction, record).await?;
        let (time, id) = before.map_or((None, None), |(time, id)| (Some(time), Some(id)));
        let rows = sqlx::query_as(
            "SELECT c.id,c.author_user_id,u.display_name AS author_display_name,u.email AS author_email,c.body,c.revision,c.created_at,c.updated_at FROM record_comments c JOIN users u ON u.id=c.author_user_id WHERE c.workspace_id=$1 AND c.record_id=$2 AND ($3::timestamptz IS NULL OR (c.created_at,c.id)<($3,$4::uuid)) ORDER BY c.created_at DESC,c.id DESC LIMIT $5",
        )
        .bind(self.workspace_id_for_runtime()).bind(record).bind(time).bind(id)
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
        .bind(self.workspace_id_for_runtime())
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
        self.notify_comment_on(&mut transaction, record, comment, actor, body)
            .await?;
        sqlx::query("INSERT INTO record_comments (id,workspace_id,record_id,author_user_id,body) VALUES ($1,$2,$3,$4,$5)")
            .bind(comment).bind(self.workspace_id_for_runtime()).bind(record).bind(actor).bind(body)
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
        let current: Option<i64> = sqlx::query_scalar("SELECT revision FROM record_comments WHERE workspace_id=$1 AND record_id=$2 AND id=$3 AND author_user_id=$4 FOR UPDATE")
            .bind(self.workspace_id_for_runtime()).bind(record).bind(comment).bind(actor)
            .fetch_optional(&mut *transaction).await?;
        let current = current.ok_or(RepositoryError::NotFound("comment"))?;
        if current != revision {
            return Err(RepositoryError::CommentConflict);
        }
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
}
