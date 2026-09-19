//! Durable file-processing and retention worker.
//!
//! The worker owns state transitions: HTTP only stages the original and queues a
//! metadata job.  Claims and all state changes are database transactions, while
//! object writes are deliberately idempotent (stable variant keys).
use std::{io::Cursor, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use image::{DynamicImage, GenericImageView, ImageEncoder, ImageReader};
use metrics::{counter, gauge};
use sha2::Digest;
use sqlx::{PgPool, Row};
use tracing::{Instrument, info_span};
use uuid::Uuid;

use crate::storage::{ObjectStore, ObjectStoreError, StoredObject};

const DEFAULT_MAX_PIXELS: u64 = 40_000_000;
const DEFAULT_MAX_ATTEMPTS: i32 = 5;
/// Longest edge for compact previews used in lists and attachment pickers.
const THUMBNAIL_MAX_DIMENSION: u32 = 320;
/// Longest edge for the high-resolution image variant served to clients.
const DISPLAY_MAX_DIMENSION: u32 = 1600;
const DEFAULT_GRACE_SECONDS: i64 = 86_400;
const STALE_LOCK_SECONDS: i64 = 300;
const MAX_BACKOFF_SECONDS: i64 = 300;

#[derive(Clone, Debug)]
pub struct WorkerConfig {
    pub worker_id: String,
    pub max_pixels: u64,
    pub max_attempts: i32,
    pub delete_grace: Duration,
}

impl WorkerConfig {
    pub fn from_env() -> Result<Self, String> {
        let parse = |name: &str, default: u64| -> Result<u64, String> {
            std::env::var(name).map_or(Ok(default), |value| {
                value
                    .parse()
                    .map_err(|_| format!("{name} must be a positive integer"))
            })
        };
        let max_pixels = parse("FILE_WORKER_MAX_PIXELS", DEFAULT_MAX_PIXELS)?;
        let max_attempts = parse("FILE_WORKER_MAX_ATTEMPTS", DEFAULT_MAX_ATTEMPTS as u64)? as i32;
        let grace = parse("FILE_DELETE_GRACE_SECONDS", DEFAULT_GRACE_SECONDS as u64)?;
        if max_pixels == 0 || max_attempts == 0 {
            return Err("file worker limits must be positive".into());
        }
        Ok(Self {
            worker_id: std::env::var("FILE_WORKER_ID")
                .unwrap_or_else(|_| format!("file-worker-{}", Uuid::new_v4())),
            max_pixels,
            max_attempts,
            delete_grace: Duration::from_secs(grace),
        })
    }
}

#[derive(Debug, Clone)]
pub struct ClaimedJob {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub file_id: Uuid,
    pub kind: String,
    pub attempts: i32,
}

pub struct FileWorker {
    pool: PgPool,
    store: Arc<dyn ObjectStore>,
    config: WorkerConfig,
}

impl FileWorker {
    pub fn new(pool: PgPool, store: Arc<dyn ObjectStore>, config: WorkerConfig) -> Self {
        Self {
            pool,
            store,
            config,
        }
    }

    /// Atomically claims one due or abandoned job. SKIP LOCKED allows multiple
    /// worker processes to poll without serialising each other.
    pub async fn claim(&self) -> Result<Option<ClaimedJob>, sqlx::Error> {
        let job = sqlx::query_as::<_, (Uuid, Uuid, Uuid, String, i32)>(
            r#"WITH candidate AS (
                 SELECT id FROM file_processing_jobs
                 WHERE (status IN ('queued', 'retryable') AND available_at <= now())
                    OR (status = 'running' AND locked_at < now() - make_interval(secs => $1))
                 ORDER BY available_at, id FOR UPDATE SKIP LOCKED LIMIT 1
               )
               UPDATE file_processing_jobs job SET status = 'running', locked_at = now(),
                 worker_id = $2, attempts = job.attempts + 1, updated_at = now()
               FROM candidate WHERE job.id = candidate.id
               RETURNING job.id, job.workspace_id, job.file_id, job.kind, job.attempts"#,
        )
        .bind(STALE_LOCK_SECONDS)
        .bind(&self.config.worker_id)
        .fetch_optional(&self.pool)
        .await?;
        if job.is_some() {
            counter!("catalog_file_worker_jobs_claimed_total").increment(1);
        }
        Ok(
            job.map(|(id, workspace_id, file_id, kind, attempts)| ClaimedJob {
                id,
                workspace_id,
                file_id,
                kind,
                attempts,
            }),
        )
    }

    pub async fn run_once(&self) -> Result<bool, sqlx::Error> {
        self.reconcile().await?;
        let Some(job) = self.claim().await? else {
            self.record_metrics().await?;
            return Ok(false);
        };
        let kind = if job.kind == "purge" {
            "purge"
        } else {
            "metadata"
        };
        let span = info_span!("file_worker.job", job_id = %job.id, file_id = %job.file_id, kind);
        async {
            let result = match kind {
                "purge" => self.purge(&job).await,
                _ => self.process_file(&job).await,
            };
            match result {
                Ok(()) => self.complete(&job).await?,
                Err(error) => self.fail(&job, &error.to_string()).await?,
            }
            Ok::<(), sqlx::Error>(())
        }
        .instrument(span)
        .await?;
        self.record_metrics().await?;
        Ok(true)
    }

    async fn process_file(&self, job: &ClaimedJob) -> Result<(), WorkerError> {
        // `FOR UPDATE` only protects this state transition while it is held in
        // an explicit transaction. Without one PostgreSQL releases the lock at
        // the end of the SELECT statement, allowing reconciliation to delete
        // the file between the read and the processing-state update.
        let mut transaction = self.pool.begin().await?;
        let row = sqlx::query("SELECT original_key, mime_type FROM files WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL FOR UPDATE")
            .bind(job.file_id).bind(job.workspace_id).fetch_optional(&mut *transaction).await?;
        let Some(row) = row else {
            transaction.commit().await?;
            return Ok(());
        };
        sqlx::query("UPDATE files SET status = 'processing', processing_error = NULL, updated_at = now() WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL")
            .bind(job.file_id).bind(job.workspace_id).execute(&mut *transaction).await?;
        let key: String = row.try_get("original_key")?;
        let mime: String = row.try_get("mime_type")?;
        transaction.commit().await?;
        if !mime.starts_with("image/") {
            sqlx::query("UPDATE files SET status = 'ready', processing_error = NULL, updated_at = now() WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL")
                .bind(job.file_id).bind(job.workspace_id).execute(&self.pool).await?;
            return Ok(());
        }
        let object = self.store.get(&key).await.map_err(WorkerError::Storage)?;
        let image = decode_and_orient(&object.bytes, self.config.max_pixels)?;
        let (width, height) = image.dimensions();
        self.put_variant(
            job,
            "thumbnail",
            &image.thumbnail(THUMBNAIL_MAX_DIMENSION, THUMBNAIL_MAX_DIMENSION),
        )
        .await?;
        self.put_variant(
            job,
            "display",
            &image.resize(
                DISPLAY_MAX_DIMENSION,
                DISPLAY_MAX_DIMENSION,
                image::imageops::FilterType::Lanczos3,
            ),
        )
        .await?;
        sqlx::query("UPDATE files SET status = 'ready', width = $2, height = $3, processing_error = NULL, updated_at = now() WHERE id = $1 AND workspace_id = $4 AND deleted_at IS NULL")
            .bind(job.file_id).bind(width as i32).bind(height as i32).bind(job.workspace_id).execute(&self.pool).await?;
        Ok(())
    }

    async fn put_variant(
        &self,
        job: &ClaimedJob,
        kind: &str,
        image: &DynamicImage,
    ) -> Result<(), WorkerError> {
        let rgba = image.to_rgba8();
        let (width, height) = rgba.dimensions();
        let mut bytes = Vec::new();
        image::codecs::webp::WebPEncoder::new_lossless(&mut bytes)
            .write_image(&rgba, width, height, image::ExtendedColorType::Rgba8)
            .map_err(|error| WorkerError::Image(error.to_string()))?;
        let key = format!("files/{}/{}.webp", job.file_id, kind);
        self.store
            .put(
                &key,
                StoredObject {
                    bytes: bytes.clone().into(),
                    content_type: Some("image/webp".into()),
                },
            )
            .await
            .map_err(WorkerError::Storage)?;
        let hash = format!("{:x}", sha2::Sha256::digest(&bytes));
        sqlx::query(r#"INSERT INTO file_variants (id, workspace_id, file_id, kind, mime_type, width, height, byte_size, object_key, sha256)
            VALUES ($1,$2,$3,$4,'image/webp',$5,$6,$7,$8,$9)
            ON CONFLICT (file_id, kind) DO UPDATE SET mime_type = EXCLUDED.mime_type, width = EXCLUDED.width, height = EXCLUDED.height, byte_size = EXCLUDED.byte_size, object_key = EXCLUDED.object_key, sha256 = EXCLUDED.sha256, updated_at = now()"#)
            .bind(Uuid::new_v4()).bind(job.workspace_id).bind(job.file_id).bind(kind).bind(width as i32).bind(height as i32).bind(bytes.len() as i64).bind(key).bind(hash)
            .execute(&self.pool).await?;
        Ok(())
    }

    async fn purge(&self, job: &ClaimedJob) -> Result<(), WorkerError> {
        let row = sqlx::query("SELECT original_key FROM files WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NOT NULL AND purge_after <= now() AND NOT EXISTS (SELECT 1 FROM attribute_file_references r WHERE r.workspace_id = files.workspace_id AND r.file_id = files.id)")
            .bind(job.file_id).bind(job.workspace_id).fetch_optional(&self.pool).await?;
        let Some(row) = row else {
            return Ok(());
        };
        let original: String = row.try_get("original_key")?;
        self.store
            .delete(&original)
            .await
            .map_err(WorkerError::Storage)?;
        let variants: Vec<String> =
            sqlx::query_scalar("SELECT object_key FROM file_variants WHERE file_id = $1")
                .bind(job.file_id)
                .fetch_all(&self.pool)
                .await?;
        for key in variants {
            self.store
                .delete(&key)
                .await
                .map_err(WorkerError::Storage)?;
        }
        Ok(())
    }

    /// Marks unreferenced files for delayed deletion and creates one durable
    /// purge job. Repeated runs are harmless and never delete before grace.
    pub async fn reconcile(&self) -> Result<(), sqlx::Error> {
        let grace = self.config.delete_grace.as_secs() as i64;
        let marked = sqlx::query(r#"UPDATE files f SET status = 'deleted', deleted_at = now(), purge_after = now() + make_interval(secs => $1), updated_at = now()
            WHERE f.deleted_at IS NULL
              AND (f.attachment_expires_at IS NULL OR f.attachment_expires_at <= now())
              AND NOT EXISTS (SELECT 1 FROM attribute_file_references r WHERE r.workspace_id = f.workspace_id AND r.file_id = f.id)
              AND NOT EXISTS (SELECT 1 FROM conversation_message_attachments a WHERE a.workspace_id = f.workspace_id AND a.file_id = f.id)"#).bind(grace).execute(&self.pool).await?;
        metrics::counter!("catalog_file_reconciliation_total", "outcome" => "success").increment(1);
        metrics::counter!("catalog_file_reconciliation_files_marked_total")
            .increment(marked.rows_affected());
        let due: Vec<(Uuid, Uuid, DateTime<Utc>)> = sqlx::query_as(r#"SELECT f.workspace_id, f.id, f.purge_after FROM files f
            WHERE f.deleted_at IS NOT NULL AND f.purge_after <= now()
              AND NOT EXISTS (SELECT 1 FROM file_processing_jobs j WHERE j.file_id = f.id AND j.kind = 'purge' AND j.status IN ('queued','running','retryable','completed'))"#)
            .fetch_all(&self.pool).await?;
        let due_count = due.len() as u64;
        for (workspace_id, file_id, available_at) in due {
            sqlx::query("INSERT INTO file_processing_jobs (id, workspace_id, file_id, kind, status, available_at) VALUES ($1,$2,$3,'purge','queued',$4)")
                .bind(Uuid::new_v4()).bind(workspace_id).bind(file_id).bind(available_at).execute(&self.pool).await?;
        }
        metrics::counter!("catalog_file_purge_jobs_queued_total").increment(due_count);
        tracing::info!(
            files_marked = marked.rows_affected(),
            purge_jobs_queued = due_count,
            "file reconciliation completed"
        );
        Ok(())
    }

    pub async fn retry_job(&self, id: Uuid) -> Result<bool, sqlx::Error> {
        Ok(sqlx::query("UPDATE file_processing_jobs SET status = 'queued', attempts = 0, available_at = now(), locked_at = NULL, worker_id = NULL, last_error = NULL, updated_at = now() WHERE id = $1 AND status = 'failed'").bind(id).execute(&self.pool).await?.rows_affected() == 1)
    }
    async fn complete(&self, job: &ClaimedJob) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE file_processing_jobs SET status = 'completed', locked_at = NULL, worker_id = NULL, updated_at = now() WHERE id = $1 AND status = 'running' AND worker_id = $2")
            .bind(job.id).bind(&self.config.worker_id).execute(&self.pool).await?;
        counter!("catalog_file_worker_jobs_completed_total").increment(1);
        Ok(())
    }
    async fn fail(&self, job: &ClaimedJob, error: &str) -> Result<(), sqlx::Error> {
        let terminal = job.attempts >= self.config.max_attempts;
        let backoff = (1_i64 << job.attempts.min(8)).min(MAX_BACKOFF_SECONDS);
        sqlx::query("UPDATE file_processing_jobs SET status = $2, available_at = now() + make_interval(secs => $3), locked_at = NULL, worker_id = NULL, last_error = $4, updated_at = now() WHERE id = $1 AND status = 'running' AND worker_id = $5")
            .bind(job.id).bind(if terminal { "failed" } else { "retryable" }).bind(backoff).bind(error).bind(&self.config.worker_id).execute(&self.pool).await?;
        if terminal {
            sqlx::query("UPDATE files SET status = 'failed', processing_error = $2, updated_at = now() WHERE id = $1 AND deleted_at IS NULL").bind(job.file_id).bind(error).execute(&self.pool).await?;
        }
        counter!("catalog_file_worker_jobs_failed_total", "terminal" => terminal.to_string())
            .increment(1);
        Ok(())
    }
    async fn record_metrics(&self) -> Result<(), sqlx::Error> {
        let queued: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM file_processing_jobs WHERE status IN ('queued','retryable')",
        )
        .fetch_one(&self.pool)
        .await?;
        gauge!("catalog_file_worker_jobs_queued").set(queued as f64);
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
enum WorkerError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Storage(#[from] ObjectStoreError),
    #[error("image processing failed: {0}")]
    Image(String),
}

fn decode_and_orient(bytes: &[u8], max_pixels: u64) -> Result<DynamicImage, WorkerError> {
    let orientation = exif::Reader::new()
        .read_from_container(&mut Cursor::new(bytes))
        .ok()
        .and_then(|exif| {
            exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
                .and_then(|field| field.value.get_uint(0))
        });
    let image = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| WorkerError::Image(e.to_string()))?
        .decode()
        .map_err(|e| WorkerError::Image(e.to_string()))?;
    let (width, height) = image.dimensions();
    if u64::from(width) * u64::from(height) > max_pixels {
        return Err(WorkerError::Image(format!(
            "image exceeds configured pixel limit of {max_pixels}"
        )));
    }
    Ok(match orientation {
        Some(2) => image.fliph(),
        Some(3) => image.rotate180(),
        Some(4) => image.flipv(),
        Some(5) => image.rotate90().fliph(),
        Some(6) => image.rotate90(),
        Some(7) => image.rotate270().fliph(),
        Some(8) => image.rotate270(),
        _ => image,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_image_dimensions_and_enforces_pixel_limit() {
        let mut png = Cursor::new(Vec::new());
        DynamicImage::new_rgba8(4, 3)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        assert_eq!(
            decode_and_orient(png.get_ref(), 12).unwrap().dimensions(),
            (4, 3)
        );
        assert!(matches!(
            decode_and_orient(png.get_ref(), 11),
            Err(WorkerError::Image(_))
        ));
    }
}
