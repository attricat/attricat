use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use super::{CatalogRepository, RepositoryError};

#[derive(Debug, serde::Serialize, FromRow)]
pub struct WorkspaceRole {
    pub id: Uuid,
    pub code: String,
    pub is_system: bool,
    pub permissions: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, serde::Serialize, FromRow)]
pub struct Permission {
    pub code: String,
    pub description: String,
}

#[derive(Debug, serde::Serialize, FromRow)]
pub struct WorkspaceGrantTarget {
    pub id: Uuid,
    pub label: String,
}

impl CatalogRepository {
    pub async fn list_workspace_roles(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<WorkspaceRole>, RepositoryError> {
        Ok(sqlx::query_as("SELECT * FROM list_workspace_roles($1, $2)")
            .bind(actor_id)
            .bind(workspace_id)
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn list_workspace_assignable_roles(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<WorkspaceRole>, RepositoryError> {
        Ok(
            sqlx::query_as("SELECT * FROM list_workspace_assignable_roles($1, $2)")
                .bind(actor_id)
                .bind(workspace_id)
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn list_workspace_permissions(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<Permission>, RepositoryError> {
        Ok(
            sqlx::query_as("SELECT * FROM list_workspace_permissions($1, $2)")
                .bind(actor_id)
                .bind(workspace_id)
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn list_workspace_token_permissions(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<Permission>, RepositoryError> {
        Ok(
            sqlx::query_as("SELECT * FROM list_workspace_token_permissions($1, $2)")
                .bind(actor_id)
                .bind(workspace_id)
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn list_workspace_grant_targets(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
        scope_type: &str,
    ) -> Result<Vec<WorkspaceGrantTarget>, RepositoryError> {
        Ok(
            sqlx::query_as("SELECT * FROM list_workspace_grant_targets($1, $2, $3)")
                .bind(actor_id)
                .bind(workspace_id)
                .bind(scope_type)
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn create_workspace_role(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
        code: &str,
        permissions: &[String],
    ) -> Result<Uuid, RepositoryError> {
        Ok(
            sqlx::query_scalar("SELECT catalog_create_workspace_role($1, $2, $3, $4, $5)")
                .bind(actor_id)
                .bind(workspace_id)
                .bind(Uuid::new_v4())
                .bind(code)
                .bind(permissions)
                .fetch_one(&self.pool)
                .await?,
        )
    }

    pub async fn update_workspace_role(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
        role_id: Uuid,
        code: &str,
        permissions: &[String],
    ) -> Result<(), RepositoryError> {
        sqlx::query("SELECT catalog_update_workspace_role($1, $2, $3, $4, $5)")
            .bind(actor_id)
            .bind(workspace_id)
            .bind(role_id)
            .bind(code)
            .bind(permissions)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn duplicate_workspace_role(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
        source_role_id: Uuid,
        code: &str,
    ) -> Result<Uuid, RepositoryError> {
        Ok(
            sqlx::query_scalar("SELECT catalog_duplicate_workspace_role($1, $2, $3, $4, $5)")
                .bind(actor_id)
                .bind(workspace_id)
                .bind(source_role_id)
                .bind(Uuid::new_v4())
                .bind(code)
                .fetch_one(&self.pool)
                .await?,
        )
    }

    pub async fn retire_workspace_role(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
        role_id: Uuid,
        replacement_role_id: Option<Uuid>,
    ) -> Result<(), RepositoryError> {
        sqlx::query("SELECT catalog_retire_workspace_role($1, $2, $3, $4)")
            .bind(actor_id)
            .bind(workspace_id)
            .bind(role_id)
            .bind(replacement_role_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
