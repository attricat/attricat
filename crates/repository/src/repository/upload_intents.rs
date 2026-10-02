use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::{CatalogRepository, RepositoryError};

#[derive(Debug, sqlx::FromRow)]
pub struct AbandonedUpload {
    pub object_key: String,
    pub lease_token: Uuid,
}

impl CatalogRepository {
    /// Commit keys before the first external write. Even process death then
    /// leaves enough information for eventual object cleanup.
    pub async fn begin_file_uploads(
        &self,
        keys: &[String],
        cleanup_after: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        sqlx::query("INSERT INTO file_upload_intents (object_key, workspace_id, cleanup_after) SELECT key, $2, $3 FROM unnest($1::text[]) key")
            .bind(keys).bind(self.workspace_id_for_runtime()).bind(cleanup_after)
            .execute(&self.pool).await?;
        Ok(())
    }

    /// Finalization and the file row commit together. A cleaner that has
    /// claimed an intent permanently prevents its key from being finalized.
    pub(super) async fn finish_file_upload(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        key: &str,
    ) -> Result<(), RepositoryError> {
        let finished = sqlx::query("DELETE FROM file_upload_intents WHERE object_key = $1 AND workspace_id = $2 AND state = 'pending' AND cleanup_after > clock_timestamp()")
            .bind(key).bind(self.workspace_id_for_runtime()).execute(&mut **tx).await?;
        if finished.rows_affected() != 1 {
            return Err(RepositoryError::NotFound("active file upload"));
        }
        Ok(())
    }
}

impl<S: super::RepositoryScope> CatalogRepository<S> {
    /// System maintenance claims at most one key without holding a database
    /// connection during S3 I/O. Expired cleaner leases are safely retryable.
    pub async fn claim_abandoned_upload(&self) -> Result<Option<AbandonedUpload>, RepositoryError> {
        Ok(sqlx::query_as("WITH candidate AS (SELECT object_key FROM file_upload_intents WHERE cleanup_after <= clock_timestamp() ORDER BY cleanup_after, object_key FOR UPDATE SKIP LOCKED LIMIT 1) UPDATE file_upload_intents i SET state = 'cleaning', lease_token = $1, attempts = attempts + 1, cleanup_after = clock_timestamp() + interval '5 minutes' FROM candidate c WHERE i.object_key = c.object_key RETURNING i.object_key, i.lease_token")
            .bind(Uuid::new_v4()).fetch_optional(&self.pool).await?)
    }

    pub async fn complete_abandoned_upload(
        &self,
        upload: &AbandonedUpload,
    ) -> Result<(), RepositoryError> {
        sqlx::query("DELETE FROM file_upload_intents WHERE object_key = $1 AND state = 'cleaning' AND lease_token = $2")
            .bind(&upload.object_key).bind(upload.lease_token).execute(&self.pool).await?;
        Ok(())
    }
}
