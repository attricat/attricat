use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use crate::extension_registry::GitHubRepository;

use super::{AttricatRepository, RepositoryError};

#[derive(Clone, Debug, serde::Serialize, FromRow)]
pub struct ExtensionRegistrySource {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub kind: String,
    pub owner: String,
    pub repository: String,
    pub created_at: DateTime<Utc>,
}

impl AttricatRepository {
    pub async fn extension_registry_sources(
        &self,
    ) -> Result<Vec<ExtensionRegistrySource>, RepositoryError> {
        Ok(sqlx::query_as("SELECT id, workspace_id, kind, owner, repository, created_at FROM extension_registry_sources WHERE workspace_id = $1 ORDER BY created_at, id")
            .bind(self.workspace_id.0).fetch_all(&self.pool).await?)
    }

    pub async fn add_extension_registry_source(
        &self,
        source: &GitHubRepository,
    ) -> Result<ExtensionRegistrySource, RepositoryError> {
        let workspace_id = self.workspace_id.0;
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
        .bind(self.workspace_id.0)
        .execute(&mut *tx)
        .await?;
        if removed.rows_affected() == 0 {
            return Err(RepositoryError::NotFound("extension registry source"));
        }
        self.commit_mutation(tx).await
    }
}
