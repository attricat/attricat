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
    // Token issuance receives every audited, security-relevant input explicitly.
    #[allow(clippy::too_many_arguments)]
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
        if label.trim() != label
            || label.is_empty()
            || label.len() > 120
            || digest.len() != 32
            || permissions.is_empty()
        {
            return Err(RepositoryError::NotFound("valid token input"));
        }
        let mut tx = self.pool.begin().await?;
        let permitted = self
            .is_authorized(user_id, workspace_id, "tokens.manage", None, None)
            .await?;
        if !permitted {
            return Err(RepositoryError::NotFound("token authority"));
        }
        // Keep issuance aligned with the token-permissions UI: a personal token
        // may only contain permissions currently granted to its owner. Route
        // authorization still rechecks live scoped grants when the token is used.
        let available: i64 = sqlx::query_scalar(
            "SELECT count(DISTINCT rp.permission_code) FROM workspace_memberships m JOIN role_grants g ON g.membership_id = m.id AND g.workspace_id = m.workspace_id JOIN role_permissions rp ON rp.role_id = g.role_id WHERE m.user_id = $1 AND m.workspace_id = $2 AND m.state = 'active' AND rp.permission_code = ANY($3)",
        )
        .bind(user_id)
        .bind(workspace_id)
        .bind(permissions)
        .fetch_one(&mut *tx)
        .await?;
        if available != permissions.len() as i64 {
            return Err(RepositoryError::TokenPermissionsUnavailable);
        }
        sqlx::query("INSERT INTO personal_api_tokens (id, user_id, workspace_id, label, token_digest, expires_at) VALUES ($1, $2, $3, $4, $5, $6)").bind(token_id).bind(user_id).bind(workspace_id).bind(label).bind(digest).bind(expires_at).execute(&mut *tx).await?;
        for permission in permissions {
            sqlx::query("INSERT INTO personal_api_token_permissions (token_id, permission_code) VALUES ($1, $2)").bind(token_id).bind(permission).execute(&mut *tx).await?;
        }
        self.commit_mutation(tx).await?;
        Ok(())
    }

    pub async fn authenticate_personal_api_token(
        &self,
        digest: &[u8],
    ) -> Result<Option<(Uuid, Uuid, Uuid)>, RepositoryError> {
        Ok(sqlx::query_as(
            "UPDATE personal_api_tokens SET last_used_at = clock_timestamp() WHERE token_digest = $1 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > clock_timestamp()) RETURNING id AS token_id, user_id, workspace_id",
        )
        .bind(digest)
        .fetch_optional(&self.pool)
        .await
        .map(|row: Option<(Uuid, Uuid, Uuid)>| {
            row
        })?)
    }

    pub async fn personal_api_token_permissions(
        &self,
        token_id: Uuid,
    ) -> Result<Vec<String>, RepositoryError> {
        Ok(sqlx::query_scalar(
            "SELECT permission_code FROM personal_api_token_permissions WHERE token_id = $1",
        )
        .bind(token_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn personal_api_token_permits(
        &self,
        token_id: Uuid,
        permission: &str,
    ) -> Result<bool, RepositoryError> {
        Ok(sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM personal_api_token_permissions WHERE token_id = $1 AND permission_code = $2)").bind(token_id).bind(permission).fetch_one(&self.pool).await?)
    }

    pub async fn list_personal_api_tokens(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<PersonalApiToken>, RepositoryError> {
        Ok(sqlx::query_as("SELECT token.id, token.label, coalesce(array_agg(permission.permission_code ORDER BY permission.permission_code) FILTER (WHERE permission.permission_code IS NOT NULL), ARRAY[]::text[]) AS permissions, token.expires_at, token.revoked_at, token.last_used_at, token.created_at FROM personal_api_tokens token LEFT JOIN personal_api_token_permissions permission ON permission.token_id = token.id WHERE token.user_id = $1 AND token.workspace_id = $2 GROUP BY token.id ORDER BY token.created_at DESC").bind(user_id).bind(workspace_id).fetch_all(&self.pool).await?)
    }

    pub async fn revoke_personal_api_token(
        &self,
        token_id: Uuid,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        let revoked = sqlx::query("UPDATE personal_api_tokens SET revoked_at = clock_timestamp() WHERE id = $1 AND user_id = $2 AND workspace_id = $3 AND revoked_at IS NULL").bind(token_id).bind(user_id).bind(workspace_id).execute(&mut *tx).await?.rows_affected() == 1;
        if revoked {
            self.commit_mutation(tx).await?;
        }
        Ok(revoked)
    }
}
