use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use crate::extension_registry::GitHubRepository;

use super::{CatalogRepository, RepositoryError};

#[derive(Clone, Debug, serde::Serialize, FromRow)]
pub struct ExtensionRegistrySource {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub kind: String,
    pub owner: String,
    pub repository: String,
    pub created_at: DateTime<Utc>,
}

impl CatalogRepository {
    /// System permissions are explicitly bootstrapped in application code so
    /// migrations only declare schema.
    pub async fn ensure_extension_registry_permissions(&self) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO permissions (code, description) VALUES ('extensions.read', 'Discover extension releases from trusted registries'), ('extensions.manage', 'Manage trusted extension registries') ON CONFLICT (code) DO NOTHING")
            .execute(&mut *tx).await?;
        for role_id in [
            Uuid::from_u128(0x00000000000040008000000000000101),
            Uuid::from_u128(0x00000000000040008000000000000102),
        ] {
            sqlx::query("INSERT INTO role_permissions (role_id, permission_code) SELECT $1, code FROM permissions WHERE code IN ('extensions.read', 'extensions.manage') ON CONFLICT DO NOTHING")
                .bind(role_id).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn extension_registry_sources(
        &self,
    ) -> Result<Vec<ExtensionRegistrySource>, RepositoryError> {
        Ok(sqlx::query_as("SELECT id, workspace_id, kind, owner, repository, created_at FROM extension_registry_sources WHERE workspace_id = $1 ORDER BY created_at, id")
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID)).fetch_all(&self.pool).await?)
    }

    pub async fn add_extension_registry_source(
        &self,
        source: &GitHubRepository,
    ) -> Result<ExtensionRegistrySource, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut tx = self.pool.begin().await?;
        let source = sqlx::query_as("INSERT INTO extension_registry_sources (id, workspace_id, kind, owner, repository) VALUES ($1, $2, 'github_repository', $3, $4) RETURNING id, workspace_id, kind, owner, repository, created_at")
            .bind(Uuid::new_v4()).bind(workspace_id).bind(&source.owner).bind(&source.repository).fetch_one(&mut *tx).await?;
        self.commit_mutation(tx).await?;
        Ok(source)
    }

    pub async fn remove_extension_registry_source(&self, id: Uuid) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;
        let removed = sqlx::query(
            "DELETE FROM extension_registry_sources WHERE id = $1 AND workspace_id = $2",
        )
        .bind(id)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .execute(&mut *tx)
        .await?;
        if removed.rows_affected() == 0 {
            return Err(RepositoryError::NotFound("extension registry source"));
        }
        self.commit_mutation(tx).await
    }
}
