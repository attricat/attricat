//! Host resources extensions use outside their own data: operator-lowered
//! request budgets of host-permission rules, and public file links an
//! extension issues for records' files.

use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::{AttricatRepository, RepositoryError};

impl AttricatRepository {
    /// The operator-lowered budget of a host-permission rule, if any.
    pub async fn extension_host_permission_budget(
        &self,
        extension_id: &str,
        host_permission_id: &str,
    ) -> Result<Option<(i32, i32)>, RepositoryError> {
        Ok(sqlx::query_as("SELECT requests,per_seconds FROM extension_host_permission_budgets WHERE workspace_id=$1 AND extension_id=$2 AND host_permission_id=$3")
            .bind(self.workspace_id()).bind(extension_id).bind(host_permission_id)
            .fetch_optional(&self.pool).await?)
    }

    /// Every operator-lowered budget of an extension.
    pub async fn extension_host_permission_budgets(
        &self,
        extension_id: &str,
    ) -> Result<Vec<(String, i32, i32)>, RepositoryError> {
        Ok(sqlx::query_as("SELECT host_permission_id,requests,per_seconds FROM extension_host_permission_budgets WHERE workspace_id=$1 AND extension_id=$2 ORDER BY host_permission_id")
            .bind(self.workspace_id()).bind(extension_id)
            .fetch_all(&self.pool).await?)
    }

    /// Sets (or with `None` clears) an operator-lowered budget.
    pub async fn set_extension_host_permission_budget(
        &self,
        extension_id: &str,
        host_permission_id: &str,
        budget: Option<(i32, i32)>,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        match budget {
            Some((requests, per_seconds)) => {
                sqlx::query("INSERT INTO extension_host_permission_budgets(workspace_id,extension_id,host_permission_id,requests,per_seconds) VALUES($1,$2,$3,$4,$5) ON CONFLICT(workspace_id,extension_id,host_permission_id) DO UPDATE SET requests=$4,per_seconds=$5,updated_at=clock_timestamp()")
                    .bind(self.workspace_id()).bind(extension_id).bind(host_permission_id)
                    .bind(requests).bind(per_seconds)
                    .execute(&mut *transaction).await?;
            }
            None => {
                sqlx::query("DELETE FROM extension_host_permission_budgets WHERE workspace_id=$1 AND extension_id=$2 AND host_permission_id=$3")
                    .bind(self.workspace_id()).bind(extension_id).bind(host_permission_id)
                    .execute(&mut *transaction).await?;
            }
        }
        self.write_audit_event(&mut transaction).await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Issues a public link to a file for `lifetime`. Only the token's
    /// SHA-256 is stored; the token itself is returned once.
    pub async fn create_extension_file_link(
        &self,
        extension_id: &str,
        file_id: Uuid,
        record_id: Uuid,
        token_sha256: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        sqlx::query("INSERT INTO extension_file_links(id,workspace_id,extension_id,file_id,record_id,token_sha256,expires_at) VALUES($1,$2,$3,$4,$5,$6,$7)")
            .bind(Uuid::new_v4()).bind(self.workspace_id()).bind(extension_id)
            .bind(file_id).bind(record_id).bind(token_sha256).bind(expires_at)
            .execute(&self.pool).await?;
        Ok(())
    }

    /// Revokes every live link an extension issued. Disabling, quarantining
    /// and removing an extension call it in their transaction, so enabling
    /// the extension again never revives a link.
    pub(crate) async fn revoke_extension_file_links(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        extension_id: &str,
    ) -> Result<u64, RepositoryError> {
        Ok(sqlx::query("UPDATE extension_file_links SET revoked_at=clock_timestamp() WHERE workspace_id=$1 AND extension_id=$2 AND revoked_at IS NULL AND expires_at>clock_timestamp()")
            .bind(self.workspace_id()).bind(extension_id)
            .execute(&mut **transaction).await?.rows_affected())
    }
}

impl AttricatRepository<super::SystemScope> {
    /// Resolves a public file link token (by its SHA-256) across workspaces.
    /// Returns the workspace, file, issuing extension and record of a live
    /// link; the caller checks the grant and that the record still references
    /// the file.
    pub async fn resolve_extension_file_link(
        &self,
        token_sha256: &str,
    ) -> Result<Option<(Uuid, Uuid, String, Uuid)>, RepositoryError> {
        Ok(sqlx::query_as("SELECT l.workspace_id,l.file_id,l.extension_id,l.record_id FROM extension_file_links l JOIN files f ON f.id=l.file_id AND f.workspace_id=l.workspace_id WHERE l.token_sha256=$1 AND l.revoked_at IS NULL AND l.expires_at>clock_timestamp() AND f.deleted_at IS NULL")
            .bind(token_sha256)
            .fetch_optional(&self.pool).await?)
    }
}
