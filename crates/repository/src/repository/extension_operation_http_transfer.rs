//! Durable HTTP transfer records contain no URLs, headers or credentials.
//! Delivery is recorded *before* I/O. A crash after that point has an uncertain
//! outcome and the host will never automatically send it again.
use super::extension_operation_artifacts::{
    MAX_ARTIFACT_MEDIA_TYPE_BYTES, MAX_OPERATION_RUN_ARTIFACT_BYTES,
    MAX_OPERATION_WORKSPACE_ARTIFACT_BYTES,
};
use super::{AttricatRepository, ExtensionOperationArtifact, RepositoryError};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

const MAX_HTTP_INPUT_BYTES: i64 = 16 * 1024 * 1024;

fn denied() -> RepositoryError {
    RepositoryError::InvalidExtension("HTTP transfer is not authorized".into())
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ExtensionHttpDelivery {
    pub id: Uuid,
    pub delivery_key: String,
    pub state: String,
    pub http_status: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct CompletedHttpInput<'a> {
    pub transfer_key: &'a str,
    pub request_digest: &'a str,
    pub source_etag: Option<&'a str>,
    pub length: i64,
    pub sha256: &'a str,
}

#[derive(Debug)]
pub enum DeliveryState {
    Send(Uuid),
    Succeeded(u16),
    Failed(u16),
    Uncertain,
}

impl AttricatRepository {
    pub async fn list_extension_http_deliveries(
        &self,
        run: Uuid,
    ) -> Result<Vec<ExtensionHttpDelivery>, RepositoryError> {
        sqlx::query_as("SELECT d.id,d.delivery_key,d.state,d.http_status,d.created_at,d.updated_at FROM extension_operation_http_deliveries d JOIN extension_operation_runs r ON r.id=d.operation_run_id AND r.workspace_id=d.workspace_id WHERE d.operation_run_id=$1 AND d.workspace_id=$2 ORDER BY d.created_at DESC LIMIT 100")
            .bind(run).bind(self.workspace_id()).fetch_all(&self.pool).await.map_err(Into::into)
    }

    pub async fn previous_extension_http_input(
        &self,
        run: Uuid,
        key: &str,
        digest: &str,
    ) -> Result<Option<(Uuid, Option<String>)>, RepositoryError> {
        let row: Option<(Uuid,Option<String>,String)> = sqlx::query_as("SELECT i.artifact_id,i.source_etag,i.request_digest FROM extension_operation_http_inputs i JOIN extension_operation_artifacts a ON a.id=i.artifact_id WHERE i.workspace_id=$1 AND i.operation_run_id=$2 AND i.transfer_key=$3 AND a.state='completed'")
            .bind(self.workspace_id()).bind(run).bind(key).fetch_optional(&self.pool).await?;
        match row {
            Some((_, _, stored)) if stored != digest => Err(denied()),
            Some((id, etag, _)) => Ok(Some((id, etag))),
            None => Ok(None),
        }
    }

    pub async fn begin_extension_http_input(
        &self,
        run: Uuid,
        extension: &str,
        release: Uuid,
        max_bytes: i64,
        media_type: &str,
    ) -> Result<ExtensionOperationArtifact, RepositoryError> {
        if max_bytes <= 0
            || max_bytes > MAX_HTTP_INPUT_BYTES
            || media_type.len() > MAX_ARTIFACT_MEDIA_TYPE_BYTES
            || media_type.is_empty()
            || !media_type.is_ascii()
        {
            return Err(denied());
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT id FROM workspaces WHERE id=$1 FOR UPDATE")
            .bind(self.workspace_id())
            .execute(&mut *tx)
            .await?;
        self.ensure_task_fence(&mut tx).await?;
        let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM extension_operation_runs WHERE workspace_id=$1 AND id=$2 AND extension_id=$3 AND installed_release_id=$4 AND status='leased')")
            .bind(self.workspace_id()).bind(run).bind(extension).bind(release).fetch_one(&mut *tx).await?;
        if !active {
            return Err(denied());
        }
        let workspace_bytes: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(content_length),0)::bigint FROM extension_operation_artifacts WHERE workspace_id=$1 AND state IN ('incomplete','completed')")
            .bind(self.workspace_id()).fetch_one(&mut *tx).await?;
        let run_bytes: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(content_length),0)::bigint FROM extension_operation_artifacts WHERE workspace_id=$1 AND operation_run_id=$2 AND state IN ('incomplete','completed')")
            .bind(self.workspace_id()).bind(run).fetch_one(&mut *tx).await?;
        if workspace_bytes + max_bytes > MAX_OPERATION_WORKSPACE_ARTIFACT_BYTES
            || run_bytes + max_bytes > MAX_OPERATION_RUN_ARTIFACT_BYTES
        {
            return Err(denied());
        }
        let artifact: ExtensionOperationArtifact = sqlx::query_as("INSERT INTO extension_operation_artifacts(id,workspace_id,extension_id,installed_release_id,operation_run_id,direction,state,media_type,content_length,object_key) VALUES($1,$2,$3,$4,$5,'input','incomplete',$6,$7,$8) RETURNING id,workspace_id,extension_id,installed_release_id,operation_run_id,direction,state,content_length,media_type,checksum_sha256,object_key,created_at,completed_at")
            .bind(Uuid::new_v4()).bind(self.workspace_id()).bind(extension).bind(release).bind(run).bind(media_type).bind(max_bytes)
            .bind(format!("extension-operation-http-inputs/v1/{}", Uuid::new_v4())).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        Ok(artifact)
    }

    pub async fn complete_extension_http_input(
        &self,
        artifact: Uuid,
        run: Uuid,
        input: CompletedHttpInput<'_>,
    ) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;
        self.ensure_task_fence(&mut tx).await?;
        let row: Option<Uuid> = sqlx::query_scalar("UPDATE extension_operation_artifacts SET state='completed',content_length=$4,checksum_sha256=$5,completed_at=clock_timestamp(),updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$2 AND operation_run_id=$3 AND direction='input' AND state='incomplete' AND content_length >= $4 AND EXISTS(SELECT 1 FROM extension_operation_runs WHERE id=$3 AND workspace_id=$2 AND status='leased') RETURNING id")
            .bind(artifact).bind(self.workspace_id()).bind(run).bind(input.length).bind(input.sha256).fetch_optional(&mut *tx).await?;
        if row.is_none() {
            return Err(denied());
        }
        sqlx::query("INSERT INTO extension_operation_http_inputs(workspace_id,operation_run_id,transfer_key,artifact_id,source_etag,request_digest) VALUES($1,$2,$3,$4,$5,$6)")
            .bind(self.workspace_id()).bind(run).bind(input.transfer_key).bind(artifact).bind(input.source_etag).bind(input.request_digest).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn abort_extension_http_input(
        &self,
        artifact: Uuid,
        run: Uuid,
    ) -> Result<(), RepositoryError> {
        sqlx::query("UPDATE extension_operation_artifacts SET state='aborted',aborted_at=clock_timestamp(),updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$2 AND operation_run_id=$3 AND direction='input' AND state='incomplete'")
            .bind(artifact).bind(self.workspace_id()).bind(run).execute(&self.pool).await?;
        Ok(())
    }

    /// The recorded state of a delivery key, without starting a delivery.
    /// A replay returns it before spending a request budget or a token.
    pub async fn previous_extension_http_delivery(
        &self,
        run: Uuid,
        key: &str,
        destination_digest: &str,
    ) -> Result<Option<DeliveryState>, RepositoryError> {
        let existing: Option<(String, String, Option<i32>)> = sqlx::query_as("SELECT destination_digest,state,http_status FROM extension_operation_http_deliveries WHERE operation_run_id=$1 AND workspace_id=$2 AND delivery_key=$3")
            .bind(run).bind(self.workspace_id()).bind(key).fetch_optional(&self.pool).await?;
        existing
            .map(|(digest, state, status)| {
                delivery_state(&digest, destination_digest, &state, status)
            })
            .transpose()
    }

    pub async fn begin_extension_http_delivery(
        &self,
        run: Uuid,
        key: &str,
        artifact: Uuid,
        destination_digest: &str,
    ) -> Result<DeliveryState, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        self.ensure_task_fence(&mut tx).await?;
        // Outputs and completed inputs (an opened record file) can be delivered.
        let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM extension_operation_runs r JOIN extension_operation_artifacts a ON a.operation_run_id=r.id WHERE r.id=$1 AND r.workspace_id=$2 AND r.status='leased' AND a.id=$3 AND a.state='completed')")
            .bind(run).bind(self.workspace_id()).bind(artifact).fetch_one(&mut *tx).await?;
        if !valid {
            return Err(denied());
        }
        let existing: Option<(String,String,Option<i32>)> = sqlx::query_as("SELECT destination_digest,state,http_status FROM extension_operation_http_deliveries WHERE operation_run_id=$1 AND workspace_id=$2 AND delivery_key=$3")
            .bind(run).bind(self.workspace_id()).bind(key).fetch_optional(&mut *tx).await?;
        if let Some((digest, state, status)) = existing {
            return delivery_state(&digest, destination_digest, &state, status);
        }
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO extension_operation_http_deliveries(id,workspace_id,operation_run_id,delivery_key,artifact_id,destination_digest) VALUES($1,$2,$3,$4,$5,$6)")
            .bind(id).bind(self.workspace_id()).bind(run).bind(key).bind(artifact).bind(destination_digest).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(DeliveryState::Send(id))
    }

    /// Forgets a sent delivery the destination refused as unauthorized
    /// (401), so the same key is sent again with a fresh token instead of
    /// replaying the refusal. Nothing was applied, so resending is safe.
    pub async fn forget_extension_http_delivery(&self, id: Uuid) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;
        self.ensure_task_fence(&mut tx).await?;
        sqlx::query("DELETE FROM extension_operation_http_deliveries WHERE id=$1 AND workspace_id=$2 AND state='uncertain'")
            .bind(id).bind(self.workspace_id()).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn complete_extension_http_delivery(
        &self,
        id: Uuid,
        status: u16,
    ) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;
        self.ensure_task_fence(&mut tx).await?;
        sqlx::query("UPDATE extension_operation_http_deliveries SET state=$3,http_status=$4,updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$2 AND state='uncertain'")
            .bind(id).bind(self.workspace_id()).bind(if reqwest::StatusCode::from_u16(status).is_ok_and(|status| status.is_success()) { "succeeded" } else { "failed" }).bind(i32::from(status)).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
}

/// A recorded delivery's state; a different destination under the same key
/// is refused.
fn delivery_state(
    digest: &str,
    destination_digest: &str,
    state: &str,
    status: Option<i32>,
) -> Result<DeliveryState, RepositoryError> {
    if digest != destination_digest {
        return Err(denied());
    }
    Ok(match (state, status) {
        ("succeeded", Some(v)) => DeliveryState::Succeeded(v as u16),
        ("failed", Some(v)) => DeliveryState::Failed(v as u16),
        _ => DeliveryState::Uncertain,
    })
}
