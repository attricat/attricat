use serde::Serialize;
use uuid::Uuid;

use super::{AttricatRepository, NewUploadedFile, RepositoryError};

/// Only the variant kind the file worker produces for avatar files is served.
pub const AVATAR_VARIANT_KIND: &str = "avatar";

/// The caller's own avatar, including one still being processed, so the
/// uploader can follow its progress. Other people's avatars are exposed only
/// once ready, as a bare file id.
#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct OwnAvatar {
    pub file_id: Uuid,
    pub status: String,
}

impl AttricatRepository {
    /// Stores an uploaded avatar as a workspace file and makes it the active
    /// member's avatar in one transaction. The replaced file loses its only
    /// reference and is reclaimed by file reconciliation.
    pub async fn persist_avatar_upload(
        &self,
        user_id: Uuid,
        file: NewUploadedFile,
    ) -> Result<OwnAvatar, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let mut transaction = self.pool.begin().await?;
        let membership: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM workspace_memberships WHERE workspace_id = $1 AND user_id = $2 AND state = 'active' FOR UPDATE",
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let membership = membership.ok_or(RepositoryError::NotFound("membership"))?;
        self.finish_file_upload(&mut transaction, &file.object_key)
            .await?;
        let file_id = Uuid::new_v4();
        let status = "queued".to_owned();
        sqlx::query(
            "INSERT INTO files (id, workspace_id, original_filename, display_filename, mime_type, byte_size, sha256, original_key, status, purpose) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,'avatar')",
        )
        .bind(file_id)
        .bind(workspace_id)
        .bind(&file.original_filename)
        .bind(&file.display_filename)
        .bind(&file.mime_type)
        .bind(file.byte_size as i64)
        .bind(&file.sha256)
        .bind(&file.object_key)
        .bind(&status)
        .execute(&mut *transaction)
        .await?;
        sqlx::query("INSERT INTO file_processing_jobs (id, workspace_id, file_id, kind, status) VALUES ($1,$2,$3,'metadata','queued')")
            .bind(Uuid::new_v4())
            .bind(workspace_id)
            .bind(file_id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("UPDATE workspace_memberships SET avatar_file_id = $2, updated_at = clock_timestamp() WHERE id = $1")
            .bind(membership)
            .bind(file_id)
            .execute(&mut *transaction)
            .await?;
        self.commit_mutation(transaction).await?;
        Ok(OwnAvatar { file_id, status })
    }

    /// Removes the active member's avatar in this workspace. Removing an
    /// absent avatar succeeds so the request is idempotent.
    pub async fn clear_avatar(&self, user_id: Uuid) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let mut transaction = self.pool.begin().await?;
        let updated = sqlx::query(
            "UPDATE workspace_memberships SET avatar_file_id = NULL, updated_at = clock_timestamp() WHERE workspace_id = $1 AND user_id = $2 AND state = 'active'",
        )
        .bind(workspace_id)
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
        if updated.rows_affected() == 0 {
            return Err(RepositoryError::NotFound("membership"));
        }
        self.commit_mutation(transaction).await
    }

    /// True when the file is the current avatar of an active member of this
    /// workspace. Any authenticated member may read such an avatar.
    pub async fn is_member_avatar(&self, file_id: Uuid) -> Result<bool, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        Ok(sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM workspace_memberships m JOIN files f ON f.workspace_id = m.workspace_id AND f.id = m.avatar_file_id WHERE m.workspace_id = $1 AND m.avatar_file_id = $2 AND m.state = 'active' AND f.purpose = 'avatar' AND f.deleted_at IS NULL)",
        )
        .bind(workspace_id)
        .bind(file_id)
        .fetch_one(&self.pool)
        .await?)
    }
}

impl<S: super::RepositoryScope> AttricatRepository<S> {
    /// Returns the member's own avatar in `workspace_id`, whatever its state.
    pub async fn own_avatar(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Option<OwnAvatar>, RepositoryError> {
        Ok(sqlx::query_as(
            "SELECT f.id AS file_id, f.status FROM workspace_memberships m JOIN files f ON f.workspace_id = m.workspace_id AND f.id = m.avatar_file_id WHERE m.workspace_id = $1 AND m.user_id = $2 AND f.deleted_at IS NULL",
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?)
    }
}
