use uuid::Uuid;

use crate::account::{Password, hash_password};

use super::{CatalogRepository, RepositoryError};

const OWNER_ROLE_ID: Uuid = Uuid::from_u128(0x00000000000040008000000000000101);

impl CatalogRepository {
    /// Applies deployment-provided metadata to the workspace selected for bootstrap.
    pub async fn configure_bootstrap_workspace(
        &self,
        workspace_id: Uuid,
        name: &str,
        owner_email: &str,
    ) -> Result<(), RepositoryError> {
        let updated = sqlx::query(
            "UPDATE workspaces SET name = $2, bootstrap_owner_email = $3, updated_at = now() WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(workspace_id)
        .bind(name)
        .bind(owner_email)
        .execute(&self.pool)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(RepositoryError::BootstrapWorkspaceNotActive);
        }
        Ok(())
    }

    /// Creates the first owner grant when the workspace does not yet have one.
    pub async fn ensure_bootstrap_workspace_owner(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        membership_id: Uuid,
        grant_id: Uuid,
        email: &str,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let email = email.trim().to_lowercase();
        sqlx::query("SELECT id FROM workspaces WHERE id = $1 FOR UPDATE")
            .bind(workspace_id)
            .execute(&mut *transaction)
            .await?;
        let owner_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM role_grants g JOIN workspace_memberships m ON m.id = g.membership_id WHERE g.workspace_id = $1 AND g.role_id = $2 AND m.state = 'active')",
        )
        .bind(workspace_id)
        .bind(OWNER_ROLE_ID)
        .fetch_one(&mut *transaction)
        .await?;
        if !owner_exists {
            sqlx::query(
                "INSERT INTO users (id, email) VALUES ($1, $2) ON CONFLICT (email) DO NOTHING",
            )
            .bind(user_id)
            .bind(&email)
            .execute(&mut *transaction)
            .await?;
            let persisted_user: Uuid =
                sqlx::query_scalar("SELECT id FROM users WHERE email = $1 FOR UPDATE")
                    .bind(&email)
                    .fetch_one(&mut *transaction)
                    .await?;
            sqlx::query("INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3) ON CONFLICT (workspace_id, user_id) DO NOTHING")
                .bind(membership_id)
                .bind(workspace_id)
                .bind(persisted_user)
                .execute(&mut *transaction)
                .await?;
            let membership: Uuid = sqlx::query_scalar(
                "SELECT id FROM workspace_memberships WHERE workspace_id = $1 AND user_id = $2",
            )
            .bind(workspace_id)
            .bind(persisted_user)
            .fetch_one(&mut *transaction)
            .await?;
            sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, $4, 'workspace', $2) ON CONFLICT DO NOTHING")
                .bind(grant_id)
                .bind(workspace_id)
                .bind(membership)
                .bind(OWNER_ROLE_ID)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    /// Adds a configured local credential without replacing an existing password.
    pub async fn ensure_bootstrap_local_password(
        &self,
        email: &str,
        plaintext_password: String,
    ) -> Result<(), RepositoryError> {
        let password_hash = hash_password(&Password::new(plaintext_password))
            .map_err(|error| RepositoryError::InvalidBootstrapPassword(error.to_string()))?;
        sqlx::query(
            "INSERT INTO local_password_credentials (user_id, password_hash) SELECT id, $2 FROM users WHERE email = $1 ON CONFLICT (user_id) DO NOTHING",
        )
        .bind(email)
        .bind(password_hash.as_phc())
        .execute(&self.pool)
        .await?;
        // Bootstrap passwords come from trusted deployment configuration. Mark
        // the configured address verified before reset links can issue.
        sqlx::query(
            "UPDATE users SET email_verified_at = COALESCE(email_verified_at, now()) WHERE email = $1",
        )
        .bind(email)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
