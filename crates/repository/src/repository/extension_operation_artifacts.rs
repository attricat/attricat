//! Durable, workspace-scoped artifacts for extension operation streams.
//!
//! Object keys are internal implementation details.  Components exchange only
//! opaque UUIDs and the HTTP download path resolves the same workspace-scoped
//! metadata before it asks object storage for bytes.

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{FromRow, Postgres, Transaction};
use uuid::Uuid;

use super::{AttricatRepository, RepositoryError};

pub const MAX_OPERATION_ARTIFACT_BYTES: i64 = 1024 * 1024 * 1024;
pub const MAX_OPERATION_RUN_ARTIFACT_BYTES: i64 = 2 * 1024 * 1024 * 1024;
pub const MAX_OPERATION_WORKSPACE_ARTIFACT_BYTES: i64 = 8 * 1024 * 1024 * 1024;

#[derive(Clone, Debug, FromRow)]
pub struct ExtensionOperationArtifact {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub extension_id: String,
    pub installed_release_id: Uuid,
    pub operation_run_id: Uuid,
    pub direction: String,
    pub state: String,
    pub content_length: i64,
    pub media_type: String,
    pub checksum_sha256: Option<String>,
    pub object_key: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

fn artifact_error(message: &str) -> RepositoryError {
    RepositoryError::InvalidExtension(message.into())
}

impl AttricatRepository {
    /// Attaches a ready, workspace-owned Attricat file as this run's approved
    /// input. `source_reference.input_file_id` is the only supported public
    /// attachment shape; callers cannot supply an object key, checksum, or
    /// arbitrary artifact metadata.
    pub(crate) async fn attach_extension_operation_input_file(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        run_id: Uuid,
        extension_id: &str,
        release_id: Uuid,
        source_reference: &Value,
    ) -> Result<Option<Uuid>, RepositoryError> {
        let Some(value) = source_reference.get("input_file_id") else {
            return Ok(None);
        };
        let file_id: Uuid = value
            .as_str()
            .ok_or_else(|| artifact_error("input_file_id must be a UUID"))?
            .parse()
            .map_err(|_| artifact_error("input_file_id must be a UUID"))?;
        let row: Option<(String, i64, String, String)> = sqlx::query_as(
            "SELECT mime_type,byte_size,sha256,original_key FROM files WHERE id=$1 AND workspace_id=$2 AND status='ready' AND deleted_at IS NULL FOR UPDATE",
        )
        .bind(file_id).bind(self.extension_workspace()).fetch_optional(&mut **transaction).await?;
        let Some((media_type, content_length, checksum_sha256, object_key)) = row else {
            return Err(artifact_error(
                "input file is not an approved workspace file",
            ));
        };
        let artifact_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO extension_operation_artifacts(id,workspace_id,extension_id,installed_release_id,operation_run_id,direction,state,content_length,media_type,checksum_sha256,object_key,completed_at) VALUES($1,$2,$3,$4,$5,'input','completed',$6,$7,$8,$9,clock_timestamp())",
        )
        .bind(artifact_id).bind(self.extension_workspace()).bind(extension_id).bind(release_id).bind(run_id)
        .bind(content_length).bind(media_type).bind(checksum_sha256).bind(object_key).execute(&mut **transaction).await?;
        Ok(Some(artifact_id))
    }

    /// Allocates a write-only output for the exact active run. This is done
    /// before a component receives its opaque handle so tenant/release and
    /// quota checks cannot be bypassed by a forged WIT argument.
    pub async fn create_extension_operation_output_artifact(
        &self,
        run_id: Uuid,
        extension_id: &str,
        release_id: Uuid,
        media_type: &str,
    ) -> Result<ExtensionOperationArtifact, RepositoryError> {
        if media_type.is_empty() || media_type.len() > 255 || !media_type.is_ascii() {
            return Err(artifact_error("invalid artifact media type"));
        }
        let mut tx = self.pool.begin().await?;
        self.ensure_task_fence(&mut tx).await?;
        // Serialize quota accounting for this workspace; aggregate reads alone
        // would permit concurrent streams to over-reserve the shared budget.
        sqlx::query("SELECT id FROM workspaces WHERE id=$1 FOR UPDATE")
            .bind(self.extension_workspace())
            .execute(&mut *tx)
            .await?;
        let valid: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM extension_operation_runs WHERE id=$1 AND workspace_id=$2 AND extension_id=$3 AND installed_release_id=$4 AND status='leased')",
        )
        .bind(run_id).bind(self.extension_workspace()).bind(extension_id).bind(release_id)
        .fetch_one(&mut *tx).await?;
        if !valid {
            return Err(artifact_error("operation artifact is not authorized"));
        }
        let workspace_bytes: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(content_length),0)::bigint FROM extension_operation_artifacts WHERE workspace_id=$1 AND state IN ('incomplete','completed')",
        ).bind(self.extension_workspace()).fetch_one(&mut *tx).await?;
        let run_bytes: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(content_length),0)::bigint FROM extension_operation_artifacts WHERE workspace_id=$1 AND operation_run_id=$2 AND state IN ('incomplete','completed')",
        ).bind(self.extension_workspace()).bind(run_id).fetch_one(&mut *tx).await?;
        // Reserve no bytes at allocation; writes reserve their exact committed
        // length below. These checks make zero-byte handle floods bounded by
        // the component fuel/operation timeout, while byte quotas stay exact.
        if workspace_bytes >= MAX_OPERATION_WORKSPACE_ARTIFACT_BYTES
            || run_bytes >= MAX_OPERATION_RUN_ARTIFACT_BYTES
        {
            return Err(artifact_error("operation artifact quota exhausted"));
        }
        let id = Uuid::new_v4();
        // Reserve the deterministic object key before upload. If the process
        // dies after put_file but before completion, retention can still delete
        // the tracked orphan; incomplete keys never leave this repository.
        let key = format!("extension-operation-artifacts/v1/{id}");
        let row = sqlx::query_as::<_, ExtensionOperationArtifact>(
            "INSERT INTO extension_operation_artifacts(id,workspace_id,extension_id,installed_release_id,operation_run_id,direction,state,media_type,object_key) VALUES($1,$2,$3,$4,$5,'output','incomplete',$6,$7) RETURNING id,workspace_id,extension_id,installed_release_id,operation_run_id,direction,state,content_length,media_type,checksum_sha256,object_key,created_at,completed_at",
        ).bind(id).bind(self.extension_workspace()).bind(extension_id).bind(release_id).bind(run_id).bind(media_type).bind(key).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        Ok(row)
    }

    /// Checks an input artifact against the active run and returns only the
    /// internal storage key to the host runtime, never the component.
    pub async fn extension_operation_input_artifact(
        &self,
        run_id: Uuid,
        extension_id: &str,
        release_id: Uuid,
        selector: &str,
    ) -> Result<ExtensionOperationArtifact, RepositoryError> {
        // `source` selects the one input attached transactionally by durable
        // operation creation. UUID selectors are retained for a future
        // multi-input contract, but still require the exact run/release scope.
        let artifact_id = if selector == "source" {
            None
        } else {
            Some(
                selector
                    .parse::<Uuid>()
                    .map_err(|_| artifact_error("invalid input artifact handle"))?,
            )
        };
        sqlx::query_as(
            "SELECT a.id,a.workspace_id,a.extension_id,a.installed_release_id,a.operation_run_id,a.direction,a.state,a.content_length,a.media_type,a.checksum_sha256,a.object_key,a.created_at,a.completed_at FROM extension_operation_artifacts a JOIN extension_operation_runs r ON r.id=a.operation_run_id WHERE ($1::uuid IS NULL OR a.id=$1) AND a.workspace_id=$2 AND a.operation_run_id=$3 AND a.extension_id=$4 AND a.installed_release_id=$5 AND a.direction='input' AND a.state='completed' AND r.status='leased' ORDER BY a.created_at LIMIT 1",
        ).bind(artifact_id).bind(self.extension_workspace()).bind(run_id).bind(extension_id).bind(release_id).fetch_optional(&self.pool).await?
         .ok_or_else(|| artifact_error("operation input artifact is not authorized"))
    }

    /// Advances a temporary artifact's persisted byte count after a bounded
    /// write.  The conditional quotas make simultaneous streams safe.
    pub async fn reserve_extension_operation_artifact_bytes(
        &self,
        artifact_id: Uuid,
        run_id: Uuid,
        bytes: i64,
    ) -> Result<(), RepositoryError> {
        if !(0..=MAX_OPERATION_ARTIFACT_BYTES).contains(&bytes) {
            return Err(artifact_error("artifact chunk exceeds quota"));
        }
        let mut tx = self.pool.begin().await?;
        self.ensure_task_fence(&mut tx).await?;
        sqlx::query("SELECT id FROM workspaces WHERE id=$1 FOR UPDATE")
            .bind(self.extension_workspace())
            .execute(&mut *tx)
            .await?;
        let row: Option<(i64,)> = sqlx::query_as("SELECT content_length FROM extension_operation_artifacts WHERE id=$1 AND workspace_id=$2 AND operation_run_id=$3 AND direction='output' AND state='incomplete' AND EXISTS(SELECT 1 FROM extension_operation_runs WHERE id=$3 AND workspace_id=$2 AND status='leased') FOR UPDATE")
            .bind(artifact_id).bind(self.extension_workspace()).bind(run_id).fetch_optional(&mut *tx).await?;
        let Some((length,)) = row else {
            return Err(artifact_error("operation output artifact is not writable"));
        };
        let next = length
            .checked_add(bytes)
            .ok_or_else(|| artifact_error("artifact quota exhausted"))?;
        let run_total: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(content_length),0)::bigint FROM extension_operation_artifacts WHERE workspace_id=$1 AND operation_run_id=$2 AND state IN ('incomplete','completed')")
            .bind(self.extension_workspace()).bind(run_id).fetch_one(&mut *tx).await?;
        let workspace_total: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(content_length),0)::bigint FROM extension_operation_artifacts WHERE workspace_id=$1 AND state IN ('incomplete','completed')")
            .bind(self.extension_workspace()).fetch_one(&mut *tx).await?;
        if next > MAX_OPERATION_ARTIFACT_BYTES
            || run_total
                .checked_add(bytes)
                .is_none_or(|v| v > MAX_OPERATION_RUN_ARTIFACT_BYTES)
            || workspace_total
                .checked_add(bytes)
                .is_none_or(|v| v > MAX_OPERATION_WORKSPACE_ARTIFACT_BYTES)
        {
            return Err(artifact_error("operation artifact quota exhausted"));
        }
        sqlx::query("UPDATE extension_operation_artifacts SET content_length=$2,updated_at=clock_timestamp() WHERE id=$1").bind(artifact_id).bind(next).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Completion is one-way. A completed artifact has immutable length,
    /// checksum, content type and object key; later writes/aborts are rejected.
    pub async fn complete_extension_operation_artifact(
        &self,
        artifact_id: Uuid,
        run_id: Uuid,
        checksum: &str,
        object_key: &str,
    ) -> Result<ExtensionOperationArtifact, RepositoryError> {
        if checksum.len() != 64
            || !checksum
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            || object_key.is_empty()
        {
            return Err(artifact_error("invalid artifact completion"));
        }
        let mut tx = self.pool.begin().await?;
        self.ensure_task_fence(&mut tx).await?;
        let artifact = sqlx::query_as("UPDATE extension_operation_artifacts SET state='completed',checksum_sha256=$3,object_key=$4,completed_at=clock_timestamp(),updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$2 AND operation_run_id=$5 AND direction='output' AND state='incomplete' AND EXISTS(SELECT 1 FROM extension_operation_runs WHERE id=$5 AND workspace_id=$2 AND status='leased') RETURNING id,workspace_id,extension_id,installed_release_id,operation_run_id,direction,state,content_length,media_type,checksum_sha256,object_key,created_at,completed_at")
            .bind(artifact_id).bind(self.extension_workspace()).bind(checksum).bind(object_key).bind(run_id).fetch_optional(&mut *tx).await?
            .ok_or_else(|| artifact_error("operation output artifact is not completable"))?;
        tx.commit().await?;
        Ok(artifact)
    }

    pub async fn abort_extension_operation_artifact(
        &self,
        artifact_id: Uuid,
        run_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        Ok(sqlx::query("UPDATE extension_operation_artifacts SET state='aborted',aborted_at=clock_timestamp(),updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$2 AND operation_run_id=$3 AND state='incomplete'")
            .bind(artifact_id).bind(self.extension_workspace()).bind(run_id).execute(&self.pool).await?.rows_affected() == 1)
    }

    /// Lists abandoned temporary records. Callers delete returned object keys
    /// only after this state transition; completed output is never selected.
    pub async fn abort_stale_extension_operation_artifacts(
        &self,
    ) -> Result<Vec<String>, RepositoryError> {
        sqlx::query("WITH aborted AS (UPDATE extension_operation_artifacts a SET state='aborted',aborted_at=clock_timestamp(),updated_at=clock_timestamp() WHERE a.workspace_id=$1 AND a.state='incomplete' AND a.updated_at < clock_timestamp() - interval '1 hour' AND NOT EXISTS (SELECT 1 FROM extension_operation_output_staging s JOIN extension_operation_runs r ON r.id=s.operation_run_id WHERE s.artifact_id=a.id AND r.status IN ('pending','leased','dead_letter')) RETURNING a.object_key) INSERT INTO extension_operation_object_cleanup(object_key,workspace_id) SELECT object_key,$1 FROM aborted WHERE object_key IS NOT NULL AND (object_key LIKE 'extension-operation-artifacts/v1/%' OR object_key LIKE 'extension-operation-http-inputs/v1/%') ON CONFLICT DO NOTHING")
            .bind(self.extension_workspace()).execute(&self.pool).await?;
        self.pending_extension_operation_object_deletions().await
    }

    pub async fn list_completed_extension_operation_artifacts(
        &self,
        run_id: Uuid,
    ) -> Result<Vec<ExtensionOperationArtifact>, RepositoryError> {
        sqlx::query_as("SELECT a.id,a.workspace_id,a.extension_id,a.installed_release_id,a.operation_run_id,a.direction,a.state,a.content_length,a.media_type,a.checksum_sha256,a.object_key,a.created_at,a.completed_at FROM extension_operation_artifacts a JOIN extension_operation_runs r ON r.id=a.operation_run_id WHERE a.operation_run_id=$1 AND a.workspace_id=$2 AND a.direction='output' AND a.state='completed' AND r.status='completed' ORDER BY a.created_at,a.id LIMIT 100")
            .bind(run_id).bind(self.extension_workspace()).fetch_all(&self.pool).await.map_err(Into::into)
    }

    pub async fn completed_extension_operation_artifact(
        &self,
        run_id: Uuid,
        artifact_id: Uuid,
    ) -> Result<ExtensionOperationArtifact, RepositoryError> {
        sqlx::query_as("SELECT a.id,a.workspace_id,a.extension_id,a.installed_release_id,a.operation_run_id,a.direction,a.state,a.content_length,a.media_type,a.checksum_sha256,a.object_key,a.created_at,a.completed_at FROM extension_operation_artifacts a JOIN extension_operation_runs r ON r.id=a.operation_run_id WHERE a.id=$1 AND a.operation_run_id=$2 AND a.workspace_id=$3 AND a.state='completed' AND a.direction='output' AND r.status='completed'")
            .bind(artifact_id).bind(run_id).bind(self.extension_workspace()).fetch_optional(&self.pool).await?
            .ok_or(RepositoryError::NotFound("completed operation artifact"))
    }
}

const _: () = assert!(MAX_OPERATION_ARTIFACT_BYTES > 0);
const _: () = assert!(MAX_OPERATION_ARTIFACT_BYTES <= MAX_OPERATION_RUN_ARTIFACT_BYTES);
const _: () = assert!(MAX_OPERATION_RUN_ARTIFACT_BYTES <= MAX_OPERATION_WORKSPACE_ARTIFACT_BYTES);
