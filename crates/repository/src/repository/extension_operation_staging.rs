//! Durable, bounded output segments. Allocation precedes object upload so
//! crashes cannot create untracked objects. Batch keys are the current run's
//! lease-fenced checkpoint key, not caller-chosen idempotency namespaces.
use super::extension_operation_artifacts::{
    MAX_OPERATION_ARTIFACT_BYTES, MAX_OPERATION_RUN_ARTIFACT_BYTES,
    MAX_OPERATION_WORKSPACE_ARTIFACT_BYTES,
};
use super::{AttricatRepository, ExtensionOperationArtifact, RepositoryError};
use uuid::Uuid;

#[derive(Debug)]
pub struct OutputChunk {
    pub id: Uuid,
    pub key: String,
    pub uploaded: bool,
}

#[derive(Debug, sqlx::FromRow)]
pub struct StagedChunk {
    pub object_key: String,
    pub content_length: i32,
    pub checksum_sha256: String,
}

fn invalid(reason: &str) -> RepositoryError {
    RepositoryError::InvalidExtension(reason.into())
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        && name != "."
        && name != ".."
}

impl AttricatRepository {
    pub async fn reserve_extension_output_chunk(
        &self,
        run: Uuid,
        name: &str,
        media_type: &str,
        batch_key: &str,
        length: usize,
        checksum: &str,
    ) -> Result<OutputChunk, RepositoryError> {
        if !valid_name(name)
            || media_type.is_empty()
            || media_type.len() > 255
            || !media_type.is_ascii()
            || length == 0
            || length > 65536
            || checksum.len() != 64
        {
            return Err(invalid("invalid bounded output chunk"));
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT id FROM workspaces WHERE id=$1 FOR UPDATE")
            .bind(self.extension_workspace())
            .execute(&mut *tx)
            .await?;
        let batch_number: Option<i32> = sqlx::query_scalar("SELECT batch_number FROM extension_operation_runs WHERE id=$1 AND workspace_id=$2 AND status='leased'")
            .bind(run).bind(self.extension_workspace()).fetch_optional(&mut *tx).await?;
        let Some(batch_number) = batch_number else {
            return Err(invalid("operation is not active"));
        };
        if batch_key != format!("{run}:{batch_number}") {
            return Err(invalid(
                "output batch key does not match the current checkpoint",
            ));
        }
        self.ensure_task_fence(&mut tx).await?;
        let existing: Option<(Uuid,String,bool,i32,String,String)> = sqlx::query_as("SELECT id,object_key,uploaded,content_length,checksum_sha256,media_type FROM extension_operation_output_chunks WHERE operation_run_id=$1 AND name=$2 AND batch_key=$3")
            .bind(run).bind(name).bind(batch_key).fetch_optional(&mut *tx).await?;
        if let Some((id, key, uploaded, size, hash, media)) = existing {
            if size != length as i32 || hash != checksum || media != media_type {
                return Err(invalid("output batch key reused with different content"));
            }
            tx.commit().await?;
            return Ok(OutputChunk { id, key, uploaded });
        }
        let stage: Option<(String,Option<Uuid>)> = sqlx::query_as("SELECT media_type,artifact_id FROM extension_operation_output_staging WHERE operation_run_id=$1 AND name=$2")
            .bind(run).bind(name).fetch_optional(&mut *tx).await?;
        if let Some((media, artifact)) = stage {
            if media != media_type || artifact.is_some() {
                return Err(invalid(
                    "output has already been finalized or has a different media type",
                ));
            }
        } else {
            sqlx::query("INSERT INTO extension_operation_output_staging(workspace_id,operation_run_id,name,media_type) VALUES($1,$2,$3,$4)")
                .bind(self.extension_workspace()).bind(run).bind(name).bind(media_type).execute(&mut *tx).await?;
        }
        let staged_run: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(c.content_length),0)::bigint FROM extension_operation_output_chunks c JOIN extension_operation_output_staging s ON s.operation_run_id=c.operation_run_id AND s.name=c.name WHERE c.operation_run_id=$1 AND s.artifact_id IS NULL")
            .bind(run).fetch_one(&mut *tx).await?;
        let staged_workspace: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(c.content_length),0)::bigint FROM extension_operation_output_chunks c JOIN extension_operation_output_staging s ON s.operation_run_id=c.operation_run_id AND s.name=c.name WHERE c.workspace_id=$1 AND s.artifact_id IS NULL")
            .bind(self.extension_workspace()).fetch_one(&mut *tx).await?;
        let artifact_run: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(content_length),0)::bigint FROM extension_operation_artifacts WHERE operation_run_id=$1 AND state IN ('incomplete','completed')")
            .bind(run).fetch_one(&mut *tx).await?;
        let artifact_workspace: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(content_length),0)::bigint FROM extension_operation_artifacts WHERE workspace_id=$1 AND state IN ('incomplete','completed')")
            .bind(self.extension_workspace()).fetch_one(&mut *tx).await?;
        let size = length as i64;
        if staged_run + size > MAX_OPERATION_ARTIFACT_BYTES
            || staged_run + artifact_run + size > MAX_OPERATION_RUN_ARTIFACT_BYTES
            || staged_workspace + artifact_workspace + size > MAX_OPERATION_WORKSPACE_ARTIFACT_BYTES
        {
            return Err(invalid("output staging quota exhausted"));
        }
        let id = Uuid::new_v4();
        let key = format!("extension-operation-staging/v1/{id}");
        sqlx::query("INSERT INTO extension_operation_output_chunks(id,workspace_id,operation_run_id,name,media_type,batch_key,content_length,checksum_sha256,object_key) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)")
            .bind(id).bind(self.extension_workspace()).bind(run).bind(name).bind(media_type).bind(batch_key).bind(length as i32).bind(checksum).bind(&key).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(OutputChunk {
            id,
            key,
            uploaded: false,
        })
    }

    pub async fn confirm_extension_output_chunk(
        &self,
        id: Uuid,
        run: Uuid,
    ) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;
        self.ensure_task_fence(&mut tx).await?;
        let changed = sqlx::query("UPDATE extension_operation_output_chunks SET uploaded=true WHERE id=$1 AND workspace_id=$2 AND operation_run_id=$3 AND EXISTS(SELECT 1 FROM extension_operation_runs WHERE id=$3 AND workspace_id=$2 AND status='leased')")
            .bind(id).bind(self.extension_workspace()).bind(run).execute(&mut *tx).await?.rows_affected();
        if changed != 1 {
            return Err(invalid("output chunk cannot be committed"));
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn extension_output_chunks(
        &self,
        run: Uuid,
        name: &str,
    ) -> Result<(String, Option<Uuid>, Vec<StagedChunk>), RepositoryError> {
        if !valid_name(name) {
            return Err(invalid("invalid output name"));
        }
        let stage: Option<(String,Option<Uuid>)> = sqlx::query_as("SELECT s.media_type,s.artifact_id FROM extension_operation_output_staging s JOIN extension_operation_runs r ON r.id=s.operation_run_id AND r.workspace_id=s.workspace_id WHERE s.workspace_id=$1 AND s.operation_run_id=$2 AND s.name=$3 AND r.status='leased'")
            .bind(self.extension_workspace()).bind(run).bind(name).fetch_optional(&self.pool).await?;
        let (media, artifact) = stage.ok_or_else(|| invalid("output has no staged chunks"))?;
        let pending: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM extension_operation_output_chunks WHERE workspace_id=$1 AND operation_run_id=$2 AND name=$3 AND NOT uploaded)")
            .bind(self.extension_workspace()).bind(run).bind(name).fetch_one(&self.pool).await?;
        if pending {
            return Err(invalid("output still has pending uploads"));
        }
        let chunks = sqlx::query_as("SELECT object_key,content_length,checksum_sha256 FROM extension_operation_output_chunks WHERE workspace_id=$1 AND operation_run_id=$2 AND name=$3 AND uploaded=true ORDER BY created_at,id")
            .bind(self.extension_workspace()).bind(run).bind(name).fetch_all(&self.pool).await?;
        Ok((media, artifact, chunks))
    }

    pub async fn extension_output_artifact(
        &self,
        run: Uuid,
        id: Uuid,
    ) -> Result<ExtensionOperationArtifact, RepositoryError> {
        sqlx::query_as("SELECT id,workspace_id,extension_id,installed_release_id,operation_run_id,direction,state,content_length,media_type,checksum_sha256,object_key,created_at,completed_at FROM extension_operation_artifacts WHERE id=$1 AND workspace_id=$2 AND operation_run_id=$3 AND direction='output'")
            .bind(id).bind(self.extension_workspace()).bind(run).fetch_optional(&self.pool).await?
            .ok_or_else(|| invalid("staged output artifact is missing"))
    }

    pub async fn set_extension_output_artifact(
        &self,
        run: Uuid,
        name: &str,
        artifact: Uuid,
    ) -> Result<Uuid, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        self.ensure_task_fence(&mut tx).await?;
        let id: Option<Uuid> = sqlx::query_scalar("UPDATE extension_operation_output_staging SET artifact_id=COALESCE(artifact_id,$4) WHERE workspace_id=$1 AND operation_run_id=$2 AND name=$3 RETURNING artifact_id")
            .bind(self.extension_workspace()).bind(run).bind(name).bind(artifact).fetch_optional(&mut *tx).await?;
        if let Some(id) = id {
            sqlx::query("UPDATE extension_operation_artifacts SET output_name=COALESCE(output_name,$3) WHERE id=$1 AND workspace_id=$2")
                .bind(id).bind(self.extension_workspace()).bind(name).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        id.ok_or_else(|| invalid("output staging disappeared"))
    }

    /// Retention: completed runs keep downloadable output for 30 days; their
    /// staging segments are discarded after one hour. Cancelled runs are
    /// cleaned after one hour; dead-letter runs can be replayed for 30 days.
    /// Storage deletions use a durable tombstone queue to survive worker death.
    pub async fn expire_extension_operation_storage(&self) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;
        let candidates: Vec<(Uuid, bool)> = sqlx::query_as(
            "SELECT r.id, COALESCE(r.completed_at,r.cancelled_at,r.updated_at) < clock_timestamp()-interval '30 days' AS expire_all FROM extension_operation_runs r WHERE r.workspace_id=$1 AND r.status IN ('completed','cancelled','dead_letter') AND NOT r.outputs_expired AND ((r.status='completed' AND (r.completed_at < clock_timestamp()-interval '30 days' OR EXISTS(SELECT 1 FROM extension_operation_output_staging s WHERE s.operation_run_id=r.id AND r.completed_at < clock_timestamp()-interval '1 hour'))) OR (r.status='cancelled' AND r.cancelled_at < clock_timestamp()-interval '1 hour') OR (r.status='dead_letter' AND r.updated_at < clock_timestamp()-interval '30 days')) ORDER BY r.updated_at LIMIT 16 FOR UPDATE OF r SKIP LOCKED"
        ).bind(self.extension_workspace()).fetch_all(&mut *tx).await?;
        for (run, old) in candidates {
            // Completed run staging can be removed without expiring its output.
            sqlx::query("INSERT INTO extension_operation_object_cleanup(object_key,workspace_id) SELECT object_key,workspace_id FROM extension_operation_output_chunks WHERE operation_run_id=$1 AND workspace_id=$2 ON CONFLICT DO NOTHING")
                .bind(run).bind(self.extension_workspace()).execute(&mut *tx).await?;
            sqlx::query("DELETE FROM extension_operation_output_chunks WHERE operation_run_id=$1 AND workspace_id=$2")
                .bind(run).bind(self.extension_workspace()).execute(&mut *tx).await?;
            sqlx::query("DELETE FROM extension_operation_output_staging WHERE operation_run_id=$1 AND workspace_id=$2")
                .bind(run).bind(self.extension_workspace()).execute(&mut *tx).await?;
            if old
                || sqlx::query_scalar::<_, String>(
                    "SELECT status FROM extension_operation_runs WHERE id=$1",
                )
                .bind(run)
                .fetch_one(&mut *tx)
                .await?
                    == "cancelled"
            {
                sqlx::query("INSERT INTO extension_operation_object_cleanup(object_key,workspace_id) SELECT object_key,workspace_id FROM extension_operation_artifacts WHERE operation_run_id=$1 AND workspace_id=$2 AND object_key IS NOT NULL AND (object_key LIKE 'extension-operation-artifacts/v1/%' OR object_key LIKE 'extension-operation-http-inputs/v1/%') ON CONFLICT DO NOTHING")
                    .bind(run).bind(self.extension_workspace()).execute(&mut *tx).await?;
                sqlx::query("UPDATE extension_operation_artifacts SET state='aborted',aborted_at=clock_timestamp(),updated_at=clock_timestamp() WHERE operation_run_id=$1 AND workspace_id=$2 AND state IN ('incomplete','completed')")
                    .bind(run).bind(self.extension_workspace()).execute(&mut *tx).await?;
                sqlx::query("UPDATE extension_operation_runs SET outputs_expired=true WHERE id=$1 AND workspace_id=$2")
                    .bind(run).bind(self.extension_workspace()).execute(&mut *tx).await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn pending_extension_operation_object_deletions(
        &self,
    ) -> Result<Vec<String>, RepositoryError> {
        sqlx::query_scalar("SELECT object_key FROM extension_operation_object_cleanup WHERE workspace_id=$1 ORDER BY created_at LIMIT 64")
            .bind(self.extension_workspace()).fetch_all(&self.pool).await.map_err(Into::into)
    }

    pub async fn confirm_extension_operation_object_deletion(
        &self,
        key: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query("DELETE FROM extension_operation_object_cleanup WHERE workspace_id=$1 AND object_key=$2")
            .bind(self.extension_workspace()).bind(key).execute(&self.pool).await?;
        Ok(())
    }
}
