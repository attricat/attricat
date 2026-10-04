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
        issuer_token_id: Option<Uuid>,
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
        let permitted =
            Self::is_authorized_on(&mut tx, user_id, workspace_id, "tokens.manage", None, None)
                .await?;
        if !permitted {
            return Err(RepositoryError::NotFound("token authority"));
        }
        // A PAT may delegate only its own authority and lifetime. Lock the
        // issuer so revocation cannot race this issuance transaction.
        if let Some(issuer) = issuer_token_id {
            let parent: Option<(Option<DateTime<Utc>>,)> = sqlx::query_as(
                "SELECT expires_at FROM personal_api_tokens WHERE id = $1 AND user_id = $2 AND workspace_id = $3 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > clock_timestamp()) FOR UPDATE",
            ).bind(issuer).bind(user_id).bind(workspace_id).fetch_optional(&mut *tx).await?;
            let Some((parent_expiry,)) = parent else {
                return Err(RepositoryError::TokenPermissionsUnavailable);
            };
            if parent_expiry.is_some_and(|parent| {
                expires_at.is_none_or(|child| child.timestamp_micros() > parent.timestamp_micros())
            }) {
                return Err(RepositoryError::TokenPermissionsUnavailable);
            }
            let allowed: bool = sqlx::query_scalar(
                "SELECT NOT EXISTS (SELECT 1 FROM unnest($2::text[]) requested(permission) WHERE NOT EXISTS (SELECT 1 FROM personal_api_token_permissions p WHERE p.token_id = $1 AND p.permission_code = requested.permission)) AND EXISTS (SELECT 1 FROM personal_api_token_permissions WHERE token_id = $1 AND permission_code = 'tokens.manage')",
            ).bind(issuer).bind(permissions).fetch_one(&mut *tx).await?;
            if !allowed {
                return Err(RepositoryError::TokenPermissionsUnavailable);
            }
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

/// A valid personal API token and the permissions it carried when it was
/// authenticated.
#[derive(Clone, Debug)]
pub struct AuthenticatedToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub permissions: std::collections::HashSet<String>,
    /// The token workspace's cache generations.
    pub generations: Option<super::WorkspaceGenerations>,
}

impl<S: super::RepositoryScope> CatalogRepository<S> {
    pub async fn authenticate_personal_api_token(
        &self,
        digest: &[u8],
    ) -> Result<Option<AuthenticatedToken>, RepositoryError> {
        // Credential validity is independent of usage accounting. Normal
        // requests only read; at most one update per minute is needed per PAT.
        // The token's permissions come back with the credential so request
        // authorization checks them in memory instead of re-reading the token.
        #[allow(clippy::type_complexity)]
        let token: Option<(Uuid, Uuid, Uuid, bool, Vec<String>, Option<i64>, Option<i64>, Option<i64>)> = sqlx::query_as(
            "SELECT t.id, t.user_id, t.workspace_id, (t.last_used_at IS NULL OR t.last_used_at < clock_timestamp() - interval '1 minute'), ARRAY(SELECT p.permission_code FROM personal_api_token_permissions p WHERE p.token_id = t.id), w.catalog_generation, w.contexts_generation, w.extensions_generation FROM personal_api_tokens t JOIN users u ON u.id = t.user_id LEFT JOIN workspaces w ON w.id = t.workspace_id WHERE t.token_digest = $1 AND t.revoked_at IS NULL AND (t.expires_at IS NULL OR t.expires_at > clock_timestamp()) AND u.state = 'active'",
        ).bind(digest).fetch_optional(&self.pool).await?;
        let Some((id, user, workspace, touch, permissions, catalog, contexts, extensions)) = token
        else {
            return Ok(None);
        };
        let generations = catalog.zip(contexts).zip(extensions).map(
            |((catalog_generation, contexts_generation), extensions_generation)| {
                super::WorkspaceGenerations {
                    catalog_generation,
                    contexts_generation,
                    extensions_generation,
                }
            },
        );
        if touch {
            sqlx::query("UPDATE personal_api_tokens SET last_used_at = clock_timestamp() WHERE id = $1 AND (last_used_at IS NULL OR last_used_at < clock_timestamp() - interval '1 minute')")
                .bind(id).execute(&self.pool).await?;
        }
        Ok(Some(AuthenticatedToken {
            id,
            user_id: user,
            workspace_id: workspace,
            permissions: permissions.into_iter().collect(),
            generations,
        }))
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
}
