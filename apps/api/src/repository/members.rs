use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use super::{CatalogRepository, RepositoryError};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct WorkspaceMember {
    pub id: Uuid,
    pub user_id: Uuid,
    pub email: String,
    pub display_name: Option<String>,
    pub state: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub grants: Value,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct WorkspaceInvitation {
    pub id: Uuid,
    pub invitee_email: String,
    pub inviter_user_id: Uuid,
    pub inviter_email: String,
    pub role_id: Uuid,
    pub role_code: String,
    pub scope_type: String,
    pub scope_target_id: Uuid,
    pub expires_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub accepted_by_user_id: Option<Uuid>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl CatalogRepository {
    pub async fn list_workspace_members(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<WorkspaceMember>, RepositoryError> {
        Ok(
            sqlx::query_as("SELECT * FROM list_workspace_members($1, $2)")
                .bind(actor_id)
                .bind(workspace_id)
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn set_workspace_membership_state(
        &self,
        id: Uuid,
        actor_id: Uuid,
        workspace_id: Uuid,
        state: &str,
    ) -> Result<bool, RepositoryError> {
        Ok(
            sqlx::query_scalar("SELECT catalog_set_workspace_membership_state($1, $2, $3, $4)")
                .bind(id)
                .bind(actor_id)
                .bind(workspace_id)
                .bind(state)
                .fetch_one(&self.pool)
                .await?,
        )
    }

    pub async fn grant_workspace_member_role(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
        membership_id: Uuid,
        role_id: Uuid,
        scope_type: &str,
        scope_target_id: Uuid,
    ) -> Result<Uuid, RepositoryError> {
        Ok(
            sqlx::query_scalar("SELECT catalog_grant_workspace_role($1, $2, $3, $4, $5, $6, $7)")
                .bind(actor_id)
                .bind(workspace_id)
                .bind(Uuid::new_v4())
                .bind(membership_id)
                .bind(role_id)
                .bind(scope_type)
                .bind(scope_target_id)
                .fetch_one(&self.pool)
                .await?,
        )
    }

    pub async fn revoke_workspace_member_role(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
        grant_id: Uuid,
    ) -> Result<(), RepositoryError> {
        sqlx::query("SELECT catalog_revoke_workspace_role($1, $2, $3)")
            .bind(actor_id)
            .bind(workspace_id)
            .bind(grant_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn transfer_workspace_ownership(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
        target_membership_id: Uuid,
    ) -> Result<(), RepositoryError> {
        sqlx::query("SELECT catalog_transfer_workspace_ownership($1, $2, $3, $4)")
            .bind(actor_id)
            .bind(workspace_id)
            .bind(target_membership_id)
            .bind(Uuid::new_v4())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn create_workspace_invitation(
        &self,
        id: Uuid,
        actor_id: Uuid,
        workspace_id: Uuid,
        email: &str,
        role_id: Uuid,
        scope_type: &str,
        scope_target_id: Uuid,
        digest: &[u8],
        expires_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "SELECT catalog_create_workspace_invitation($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(id)
        .bind(actor_id)
        .bind(workspace_id)
        .bind(email)
        .bind(role_id)
        .bind(scope_type)
        .bind(scope_target_id)
        .bind(digest)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_workspace_invitations(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<WorkspaceInvitation>, RepositoryError> {
        Ok(
            sqlx::query_as("SELECT * FROM list_workspace_invitations($1, $2)")
                .bind(actor_id)
                .bind(workspace_id)
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn revoke_workspace_invitation(
        &self,
        id: Uuid,
        actor_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        Ok(
            sqlx::query_scalar("SELECT catalog_revoke_workspace_invitation($1, $2, $3)")
                .bind(id)
                .bind(actor_id)
                .bind(workspace_id)
                .fetch_one(&self.pool)
                .await?,
        )
    }

    pub async fn accept_workspace_invitation(
        &self,
        digest: &[u8],
        user_id: Uuid,
    ) -> Result<Uuid, RepositoryError> {
        sqlx::query_scalar::<_, Option<Uuid>>(
            "SELECT catalog_accept_workspace_invitation($1, $2, $3, $4)",
        )
        .bind(digest)
        .bind(user_id)
        .bind(Uuid::new_v4())
        .bind(Uuid::new_v4())
        .fetch_one(&self.pool)
        .await?
        .ok_or(RepositoryError::InvitationInvalid)
    }
}
