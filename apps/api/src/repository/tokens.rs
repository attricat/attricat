use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{CatalogRepository, RepositoryError};

#[derive(Debug, sqlx::FromRow)]
pub struct PersonalApiToken {
    pub id: Uuid,
    pub label: String,
    pub permissions: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl CatalogRepository {
    pub async fn issue_personal_api_token(
        &self,
        token_id: Uuid,
        user_id: Uuid,
        workspace_id: Uuid,
        label: &str,
        digest: &[u8],
        permissions: &[String],
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<(), RepositoryError> {
        sqlx::query("SELECT issue_personal_api_token($1, $2, $3, $4, $5, $6, $7)")
            .bind(token_id)
            .bind(user_id)
            .bind(workspace_id)
            .bind(label)
            .bind(digest)
            .bind(permissions)
            .bind(expires_at)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn authenticate_personal_api_token(
        &self,
        digest: &[u8],
    ) -> Result<Option<(Uuid, Uuid, Uuid)>, RepositoryError> {
        Ok(sqlx::query_as(
            "SELECT token_id, user_id, workspace_id FROM authenticate_personal_api_token($1)",
        )
        .bind(digest)
        .fetch_optional(&self.pool)
        .await
        .map(|row: Option<(Uuid, Uuid, Uuid)>| {
            row.map(|(token_id, user_id, workspace_id)| (token_id, user_id, workspace_id))
        })?)
    }

    pub async fn personal_api_token_permits(
        &self,
        token_id: Uuid,
        permission: &str,
    ) -> Result<bool, RepositoryError> {
        Ok(
            sqlx::query_scalar("SELECT personal_api_token_permits($1, $2)")
                .bind(token_id)
                .bind(permission)
                .fetch_one(&self.pool)
                .await?,
        )
    }

    pub async fn list_personal_api_tokens(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<PersonalApiToken>, RepositoryError> {
        Ok(
            sqlx::query_as("SELECT * FROM list_personal_api_tokens($1, $2)")
                .bind(user_id)
                .bind(workspace_id)
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn revoke_personal_api_token(
        &self,
        token_id: Uuid,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        Ok(
            sqlx::query_scalar("SELECT revoke_personal_api_token($1, $2, $3)")
                .bind(token_id)
                .bind(user_id)
                .bind(workspace_id)
                .fetch_one(&self.pool)
                .await?,
        )
    }
}
