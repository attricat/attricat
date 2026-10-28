//! Durable file-processing and retention worker.
//!
//! The worker owns state transitions: HTTP only stages the original and queues a
//! metadata job.  Claims and all state changes are database transactions, while
//! object writes are deliberately idempotent (stable variant keys).
use attricat_document_text::EXTRACTOR_VERSION;
use attricat_domain::constants::TEXT_FILE_VARIANT;
use attricat_domain::states::{FileJobKind, FileJobStatus, FileTextExtractionStatus};
use attricat_validation::files::is_text_extraction_mime;
use std::{
    io::Cursor,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use image::{DynamicImage, GenericImageView, ImageReader};
use metrics::{counter, gauge};
use sha2::Digest;
use sqlx::{PgPool, Row};
use tracing::{Instrument, info_span};
use uuid::Uuid;

use crate::repository::{AttricatRepository, RepositoryError, SystemRepository};
use crate::storage::{ObjectStore, ObjectStoreError, StoredObject};

const DEFAULT_MAX_PIXELS: u64 = 40_000_000;
const DEFAULT_MAX_ATTEMPTS: i32 = 5;
/// Edge length of the square `thumbnail` variant. Clients show thumbnails in
/// square tiles of up to 128 CSS pixels, so this covers high-density screens.
const THUMBNAIL_DIMENSION: u32 = 320;
/// Lossy WebP quality of every image variant.
const VARIANT_WEBP_QUALITY: f32 = 80.0;
/// Longest edge for the high-resolution image variant served to clients.
const DISPLAY_MAX_DIMENSION: u32 = 1600;
/// Edge length of the square `avatar` variant, enough for high-density screens.
const AVATAR_DIMENSION: u32 = 256;
const DEFAULT_GRACE_SECONDS: i64 = 86_400;
const STALE_LOCK_SECONDS: i64 = 300;
const LEASE_HEARTBEAT_SECONDS: u64 = 60;
const RECONCILE_INTERVAL: Duration = Duration::from_secs(30);
const METRICS_INTERVAL: Duration = Duration::from_secs(5);
const MAX_BACKOFF_SECONDS: i64 = 300;

#[derive(Clone, Debug)]
pub struct WorkerConfig {
    pub worker_id: String,
    pub max_pixels: u64,
    pub max_attempts: i32,
    pub delete_grace: Duration,
}

impl WorkerConfig {
    /// Parses the file worker's processing settings, defaulting each unset one.
    pub fn from_values(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let parse = |name: &str, default: u64| -> Result<u64, String> {
            get(name).map_or(Ok(default), |value| {
                value
                    .parse()
                    .map_err(|_| format!("{name} must be a positive integer"))
            })
        };
        let max_pixels = parse("FILE_WORKER_MAX_PIXELS", DEFAULT_MAX_PIXELS)?;
        let max_attempts = checked_max_attempts(parse(
            "FILE_WORKER_MAX_ATTEMPTS",
            DEFAULT_MAX_ATTEMPTS as u64,
        )?)?;
        let grace = checked_grace(parse(
            "FILE_DELETE_GRACE_SECONDS",
            DEFAULT_GRACE_SECONDS as u64,
        )?)?;
        if max_pixels == 0 {
            return Err("file worker limits must be positive".into());
        }
        Ok(Self {
            worker_id: get("FILE_WORKER_ID")
                .unwrap_or_else(|| format!("file-worker-{}", Uuid::new_v4())),
            max_pixels,
            max_attempts,
            delete_grace: grace,
        })
    }
}

fn checked_max_attempts(value: u64) -> Result<i32, String> {
    i32::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| "FILE_WORKER_MAX_ATTEMPTS must be between 1 and i32::MAX".into())
}

fn checked_grace(value: u64) -> Result<Duration, String> {
    // Reconciliation binds grace as i64 seconds to PostgreSQL.
    i64::try_from(value).map_err(|_| "FILE_DELETE_GRACE_SECONDS is too large".to_owned())?;
    Ok(Duration::from_secs(value))
}

#[derive(Debug, Clone)]
pub struct ClaimedJob {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub file_id: Uuid,
    pub kind: String,
    pub attempts: i32,
    pub lease_token: Uuid,
}

pub struct FileWorker {
    pool: PgPool,
    repository: SystemRepository,
    store: Arc<dyn ObjectStore>,
    config: WorkerConfig,
    last_reconciled: Mutex<Option<Instant>>,
    last_metrics: Mutex<Option<Instant>>,
}

impl FileWorker {
    pub fn new(pool: PgPool, store: Arc<dyn ObjectStore>, config: WorkerConfig) -> Self {
        Self {
            repository: AttricatRepository::system(pool.clone()),
            pool,
            store,
            config,
            last_reconciled: Mutex::new(None),
            last_metrics: Mutex::new(None),
        }
    }

    /// Atomically claims one due or abandoned job. SKIP LOCKED allows multiple
    /// worker processes to poll without serialising each other. Text
    /// extraction waits for other due jobs, so it never delays a new upload
    /// becoming ready.
    pub async fn claim(&self) -> Result<Option<ClaimedJob>, sqlx::Error> {
        let job = sqlx::query_as::<_, (Uuid, Uuid, Uuid, String, i32, Uuid)>(
            r#"WITH candidate AS (
                 SELECT id FROM file_processing_jobs
                 WHERE (status IN ('queued', 'retryable') AND available_at <= now())
                    OR (status = 'running' AND locked_at < now() - make_interval(secs => $1))
                 ORDER BY kind = $4, available_at, id FOR UPDATE SKIP LOCKED LIMIT 1
               )
               UPDATE file_processing_jobs job SET status = 'running', locked_at = now(),
                 worker_id = $2, lease_token = $3, attempts = job.attempts + 1, updated_at = now()
               FROM candidate WHERE job.id = candidate.id
               RETURNING job.id, job.workspace_id, job.file_id, job.kind, job.attempts, job.lease_token"#,
        )
        .bind(STALE_LOCK_SECONDS)
        .bind(&self.config.worker_id)
        .bind(Uuid::new_v4())
        .bind(FileJobKind::TextExtraction.as_str())
        .fetch_optional(&self.pool)
        .await?;
        if job.is_some() {
            counter!("attricat_file_worker_jobs_claimed_total").increment(1);
        }
        Ok(job.map(
            |(id, workspace_id, file_id, kind, attempts, lease_token)| ClaimedJob {
                id,
                workspace_id,
                file_id,
                kind,
                attempts,
                lease_token,
            },
        ))
    }

    pub async fn run_once(&self) -> Result<bool, RepositoryError> {
        self.run_once_with_heartbeat(Duration::from_secs(LEASE_HEARTBEAT_SECONDS))
            .await
    }

    async fn run_once_with_heartbeat(
        &self,
        heartbeat_every: Duration,
    ) -> Result<bool, RepositoryError> {
        // Reconciliation is maintenance, not part of every job claim. Avoid
        // rescanning the file population before each item in a busy queue.
        let should_reconcile = {
            let mut last = self
                .last_reconciled
                .lock()
                .expect("reconcile clock poisoned");
            if last.is_none_or(|at| at.elapsed() >= RECONCILE_INTERVAL) {
                *last = Some(Instant::now());
                true
            } else {
                false
            }
        };
        if should_reconcile && let Err(error) = self.reconcile().await {
            *self
                .last_reconciled
                .lock()
                .expect("reconcile clock poisoned") = None;
            return Err(error);
        }
        let Some(job) = self.claim().await? else {
            self.record_metrics_if_due().await?;
            return Ok(false);
        };
        let kind = FileJobKind::parse(&job.kind).unwrap_or(FileJobKind::Metadata);
        let span = info_span!("file_worker.job", job_id = %job.id, file_id = %job.file_id, kind = kind.as_str());
        let operation = async {
            let result = match kind {
                FileJobKind::Purge => self.purge(&job).await,
                FileJobKind::TextExtraction => self.extract_text(&job).await,
                FileJobKind::Metadata => self.process_file(&job).await,
            };
            match result {
                Ok(()) => self.complete(&job).await?,
                Err(error) => self.fail(&job, &error.to_string()).await?,
            }
            Ok::<(), RepositoryError>(())
        }
        .instrument(span);
        let renewals = async {
            let mut heartbeat = tokio::time::interval(heartbeat_every);
            heartbeat.tick().await;
            loop {
                heartbeat.tick().await;
                if !self.renew_lease(&job).await? {
                    return Ok::<(), sqlx::Error>(());
                }
            }
        };
        match crate::heartbeat::with_heartbeat(operation, renewals).await {
            Ok(result) => result?,
            Err(renewal) => {
                renewal?;
                tracing::warn!(job_id = %job.id, "file job lease lost; cancelling processing");
            }
        }
        self.record_metrics_if_due().await?;
        Ok(true)
    }

    async fn record_metrics_if_due(&self) -> Result<(), sqlx::Error> {
        // Queue-depth aggregation scans job statuses; it should not run once
        // per processed file when the queue is busy.
        let due = {
            let mut last = self.last_metrics.lock().expect("metrics clock poisoned");
            if last.is_none_or(|at| at.elapsed() >= METRICS_INTERVAL) {
                *last = Some(Instant::now());
                true
            } else {
                false
            }
        };
        if due && let Err(error) = self.record_metrics().await {
            *self.last_metrics.lock().expect("metrics clock poisoned") = None;
            return Err(error);
        }
        Ok(())
    }

    async fn renew_lease(&self, job: &ClaimedJob) -> Result<bool, sqlx::Error> {
        Ok(sqlx::query("UPDATE file_processing_jobs SET locked_at = now() WHERE id = $1 AND status = 'running' AND worker_id = $2 AND lease_token = $3 AND locked_at > now() - make_interval(secs => $4)")
            .bind(job.id).bind(&self.config.worker_id).bind(job.lease_token).bind(STALE_LOCK_SECONDS)
            .execute(&self.pool).await?.rows_affected() == 1)
    }

    async fn process_file(&self, job: &ClaimedJob) -> Result<(), WorkerError> {
        // `FOR UPDATE` only protects this state transition while it is held in
        // an explicit transaction. Without one PostgreSQL releases the lock at
        // the end of the SELECT statement, allowing reconciliation to delete
        // the file between the read and the processing-state update.
        let mut transaction = self.pool.begin().await?;
        let still_owned: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM file_processing_jobs WHERE id = $1 AND status = 'running' AND lease_token = $2 AND locked_at > now() - make_interval(secs => $3) FOR UPDATE)")
            .bind(job.id).bind(job.lease_token).bind(STALE_LOCK_SECONDS)
            .fetch_one(&mut *transaction).await?;
        if !still_owned {
            return Err(WorkerError::LeaseLost);
        }
        let row = sqlx::query("SELECT original_key, mime_type, purpose FROM files WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL FOR UPDATE")
            .bind(job.file_id).bind(job.workspace_id).fetch_optional(&mut *transaction).await?;
        let Some(row) = row else {
            transaction.commit().await?;
            return Ok(());
        };
        sqlx::query("UPDATE files SET status = 'processing', processing_error = NULL, updated_at = now() WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL")
            .bind(job.file_id).bind(job.workspace_id).execute(&mut *transaction).await?;
        let key: String = row.try_get("original_key")?;
        let mime: String = row.try_get("mime_type")?;
        let avatar = row.try_get::<String, _>("purpose")? == "avatar";
        transaction.commit().await?;
        if !mime.starts_with("image/") {
            sqlx::query("UPDATE files SET status = 'ready', processing_error = NULL, updated_at = now() WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL AND EXISTS (SELECT 1 FROM file_processing_jobs WHERE id = $3 AND status = 'running' AND lease_token = $4)")
                .bind(job.file_id).bind(job.workspace_id).bind(job.id).bind(job.lease_token).execute(&self.pool).await?;
            return Ok(());
        }
        let object = self.store.get(&key).await.map_err(WorkerError::Storage)?;
        let max_pixels = self.config.max_pixels;
        // Decoding/resizing/WebP encoding are synchronous CPU work. Running
        // them on a blocking thread keeps the async lease heartbeat alive.
        let (width, height, variants) = tokio::task::spawn_blocking(move || {
            let image = decode_and_orient(&object.bytes, max_pixels)?;
            let (width, height) = image.dimensions();
            if avatar {
                return Ok::<_, WorkerError>((
                    width,
                    height,
                    vec![encode_variant("avatar", &square_avatar(&image))?],
                ));
            }
            let variants = vec![
                encode_variant("thumbnail", &square_thumbnail(&image))?,
                encode_variant("display", &display_image(&image))?,
            ];
            Ok::<_, WorkerError>((width, height, variants))
        })
        .await
        .map_err(|error| WorkerError::Image(error.to_string()))??;
        for variant in variants {
            self.put_variant(job, variant).await?;
        }
        sqlx::query("UPDATE files SET status = 'ready', width = $2, height = $3, processing_error = NULL, updated_at = now() WHERE id = $1 AND workspace_id = $4 AND deleted_at IS NULL AND EXISTS (SELECT 1 FROM file_processing_jobs WHERE id = $5 AND status = 'running' AND lease_token = $6)")
            .bind(job.file_id).bind(width as i32).bind(height as i32).bind(job.workspace_id).bind(job.id).bind(job.lease_token).execute(&self.pool).await?;
        Ok(())
    }

    async fn put_variant(
        &self,
        job: &ClaimedJob,
        variant: EncodedVariant,
    ) -> Result<(), WorkerError> {
        let key = format!("files/{}/{}.webp", job.file_id, variant.kind);
        let bytes = variant.bytes;
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
        let changed = sqlx::query(r#"INSERT INTO file_variants (id, workspace_id, file_id, kind, mime_type, width, height, byte_size, object_key, sha256)
            SELECT $1,$2,$3,$4,'image/webp',$5,$6,$7,$8,$9 WHERE EXISTS (SELECT 1 FROM file_processing_jobs WHERE id = $10 AND status = 'running' AND lease_token = $11)
            ON CONFLICT (file_id, kind) DO UPDATE SET mime_type = EXCLUDED.mime_type, width = EXCLUDED.width, height = EXCLUDED.height, byte_size = EXCLUDED.byte_size, object_key = EXCLUDED.object_key, sha256 = EXCLUDED.sha256, updated_at = now()"#)
            .bind(Uuid::new_v4()).bind(job.workspace_id).bind(job.file_id).bind(variant.kind).bind(variant.width as i32).bind(variant.height as i32).bind(bytes.len() as i64).bind(key).bind(hash).bind(job.id).bind(job.lease_token)
            .execute(&self.pool).await?.rows_affected();
        if changed == 0 {
            return Err(WorkerError::LeaseLost);
        }
        Ok(())
    }

    /// Extracts a document's text into its `text` variant and records the
    /// outcome. A document that cannot be read is a recorded outcome, not a
    /// job failure, and never changes the file's own status.
    async fn extract_text(&self, job: &ClaimedJob) -> Result<(), WorkerError> {
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT original_key, mime_type FROM files WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL",
        )
        .bind(job.file_id)
        .bind(job.workspace_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some((key, mime)) = row.filter(|(_, mime)| is_text_extraction_mime(mime)) else {
            return Ok(());
        };
        let object = self.store.get(&key).await.map_err(WorkerError::Storage)?;
        let extraction = crate::document_text::extract_text(mime, object.bytes).await;
        counter!(
            "attricat_file_text_extractions_total",
            "outcome" => extraction
                .as_ref()
                .err()
                .map_or(FileTextExtractionStatus::Ready.as_str(), |failure| failure.as_str())
        )
        .increment(1);
        let text_key = format!("files/{}/{TEXT_FILE_VARIANT}.json", job.file_id);
        // The object key is stable, so a write from a job that then loses its
        // lease is overwritten by the next attempt.
        let stored = match &extraction {
            Ok(document) => {
                let bytes = serde_json::to_vec(document).expect("documents serialize");
                let byte_size = bytes.len() as i64;
                let hash = format!("{:x}", sha2::Sha256::digest(&bytes));
                self.store
                    .put(
                        &text_key,
                        StoredObject {
                            bytes: bytes.into(),
                            content_type: Some("application/json".into()),
                        },
                    )
                    .await
                    .map_err(WorkerError::Storage)?;
                Ok((document, byte_size, hash))
            }
            Err(failure) => Err(*failure),
        };
        let mut transaction = self.pool.begin().await?;
        let still_owned: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM file_processing_jobs WHERE id = $1 AND status = 'running' AND lease_token = $2 FOR UPDATE)")
            .bind(job.id).bind(job.lease_token).fetch_one(&mut *transaction).await?;
        if !still_owned {
            return Err(WorkerError::LeaseLost);
        }
        let mut removed_variant = false;
        let (status, failure, sections, chars, truncated) = match stored {
            Ok((document, byte_size, hash)) => {
                sqlx::query(r#"INSERT INTO file_variants (id, workspace_id, file_id, kind, mime_type, byte_size, object_key, sha256)
                    VALUES ($1,$2,$3,$4,'application/json',$5,$6,$7)
                    ON CONFLICT (file_id, kind) DO UPDATE SET mime_type = EXCLUDED.mime_type, byte_size = EXCLUDED.byte_size, object_key = EXCLUDED.object_key, sha256 = EXCLUDED.sha256, updated_at = now()"#)
                    .bind(Uuid::new_v4()).bind(job.workspace_id).bind(job.file_id).bind(TEXT_FILE_VARIANT).bind(byte_size).bind(&text_key).bind(hash)
                    .execute(&mut *transaction).await?;
                (
                    FileTextExtractionStatus::Ready,
                    None,
                    document.sections.len() as i32,
                    document.char_count() as i64,
                    document.truncated,
                )
            }
            Err(failure) => {
                // Text from an earlier extractor must not outlive a failed rerun.
                removed_variant = sqlx::query("DELETE FROM file_variants WHERE file_id = $1 AND workspace_id = $2 AND kind = $3")
                    .bind(job.file_id).bind(job.workspace_id).bind(TEXT_FILE_VARIANT).execute(&mut *transaction).await?
                    .rows_affected() > 0;
                (
                    FileTextExtractionStatus::Failed,
                    Some(failure.as_str()),
                    0,
                    0,
                    false,
                )
            }
        };
        sqlx::query(r#"INSERT INTO file_text_extractions (file_id, workspace_id, extractor_version, status, failure_reason, section_count, char_count, truncated)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
            ON CONFLICT (file_id) DO UPDATE SET extractor_version = EXCLUDED.extractor_version, status = EXCLUDED.status, failure_reason = EXCLUDED.failure_reason, section_count = EXCLUDED.section_count, char_count = EXCLUDED.char_count, truncated = EXCLUDED.truncated, updated_at = now()"#)
            .bind(job.file_id).bind(job.workspace_id).bind(EXTRACTOR_VERSION as i32).bind(status.as_str()).bind(failure).bind(sections).bind(chars).bind(truncated)
            .execute(&mut *transaction).await?;
        transaction.commit().await?;
        if removed_variant {
            // Deleted after the commit so a rolled-back transaction never
            // leaves a variant row pointing at a missing object.
            self.store
                .delete(&text_key)
                .await
                .map_err(WorkerError::Storage)?;
        }
        Ok(())
    }

    async fn purge(&self, job: &ClaimedJob) -> Result<(), WorkerError> {
        let Some(keys) = self
            .repository
            .purgeable_file_objects(job.workspace_id, job.file_id)
            .await?
        else {
            return Ok(());
        };
        for key in keys {
            self.store
                .delete(&key)
                .await
                .map_err(WorkerError::Storage)?;
        }
        Ok(())
    }

    /// Marks unreferenced files without an active retention hold for delayed
    /// deletion and creates one durable purge job. Repeated runs are harmless
    /// and never delete before grace.
    pub async fn reconcile(&self) -> Result<(), RepositoryError> {
        let grace = i64::try_from(self.config.delete_grace.as_secs()).unwrap_or(i64::MAX);
        let marked = self
            .repository
            .mark_unreferenced_files_deleted(grace)
            .await?;
        metrics::counter!("attricat_file_reconciliation_total", "outcome" => "success")
            .increment(1);
        metrics::counter!("attricat_file_reconciliation_files_marked_total").increment(marked);
        let due_count = self.repository.queue_due_file_purges().await?;
        metrics::counter!("attricat_file_purge_jobs_queued_total").increment(due_count);
        let text_count = self
            .repository
            .queue_text_extraction_backfill(EXTRACTOR_VERSION)
            .await?;
        metrics::counter!("attricat_file_text_extraction_jobs_queued_total").increment(text_count);
        tracing::info!(
            files_marked = marked,
            purge_jobs_queued = due_count,
            text_extraction_jobs_queued = text_count,
            "file reconciliation completed"
        );
        Ok(())
    }

    pub async fn retry_job(&self, id: Uuid) -> Result<bool, sqlx::Error> {
        Ok(sqlx::query("UPDATE file_processing_jobs SET status = 'queued', attempts = 0, available_at = now(), locked_at = NULL, worker_id = NULL, lease_token = NULL, last_error = NULL, updated_at = now() WHERE id = $1 AND status = 'failed'").bind(id).execute(&self.pool).await?.rows_affected() == 1)
    }
    async fn complete(&self, job: &ClaimedJob) -> Result<(), sqlx::Error> {
        let changed = sqlx::query("UPDATE file_processing_jobs SET status = 'completed', locked_at = NULL, worker_id = NULL, lease_token = NULL, updated_at = now() WHERE id = $1 AND status = 'running' AND worker_id = $2 AND lease_token = $3")
            .bind(job.id).bind(&self.config.worker_id).bind(job.lease_token).execute(&self.pool).await?.rows_affected();
        if changed == 1 {
            counter!("attricat_file_worker_jobs_completed_total").increment(1);
        } else {
            tracing::warn!(job_id = %job.id, "file job lease lost before completion");
        }
        Ok(())
    }
    async fn fail(&self, job: &ClaimedJob, error: &str) -> Result<(), sqlx::Error> {
        let terminal = job.attempts >= self.config.max_attempts;
        let backoff = (1_i64 << job.attempts.min(8)).min(MAX_BACKOFF_SECONDS);
        let changed = sqlx::query("UPDATE file_processing_jobs SET status = $2, available_at = now() + make_interval(secs => $3), locked_at = NULL, worker_id = NULL, lease_token = NULL, last_error = $4, updated_at = now() WHERE id = $1 AND status = 'running' AND worker_id = $5 AND lease_token = $6")
            .bind(job.id).bind(if terminal { FileJobStatus::Failed } else { FileJobStatus::Retryable }.as_str()).bind(backoff).bind(error).bind(&self.config.worker_id).bind(job.lease_token).execute(&self.pool).await?.rows_affected();
        if changed != 1 {
            tracing::warn!(job_id = %job.id, "file job lease lost before failure could be recorded");
            return Ok(());
        }
        // Only the metadata job decides whether a file is usable.
        // Unknown kinds are processed as metadata jobs, so they fail as one.
        if terminal
            && FileJobKind::parse(&job.kind).unwrap_or(FileJobKind::Metadata)
                == FileJobKind::Metadata
        {
            sqlx::query("UPDATE files SET status = 'failed', processing_error = $2, updated_at = now() WHERE id = $1 AND deleted_at IS NULL").bind(job.file_id).bind(error).execute(&self.pool).await?;
        }
        counter!("attricat_file_worker_jobs_failed_total", "terminal" => terminal.to_string())
            .increment(1);
        Ok(())
    }
    pub async fn record_metrics(&self) -> Result<(), sqlx::Error> {
        // Gauges cover the unfinished jobs and the failures waiting for an operator.
        const STATUSES: [FileJobStatus; 4] = [
            FileJobStatus::Queued,
            FileJobStatus::Running,
            FileJobStatus::Retryable,
            FileJobStatus::Failed,
        ];
        for status in STATUSES.map(FileJobStatus::as_str) {
            gauge!("attricat_file_worker_queue_depth", "status" => status).set(0.0);
            gauge!("attricat_file_worker_oldest_age_seconds", "status" => status).set(0.0);
            gauge!("attricat_file_worker_retries", "status" => status).set(0.0);
        }
        let rows: Vec<(String, i64, f64, i64)> = sqlx::query_as(
            "SELECT status,count(*)::bigint,COALESCE(extract(epoch FROM (clock_timestamp()-min(created_at))),0)::float8,COALESCE(sum(GREATEST(attempts-1,0)),0)::bigint FROM file_processing_jobs WHERE status IN ('queued','running','retryable','failed') GROUP BY status",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut queued = 0_i64;
        for (status, count, oldest_age, retries) in rows {
            if matches!(
                FileJobStatus::parse(&status),
                Some(FileJobStatus::Queued | FileJobStatus::Retryable)
            ) {
                queued += count;
            }
            gauge!("attricat_file_worker_queue_depth", "status" => status.clone())
                .set(count as f64);
            gauge!("attricat_file_worker_oldest_age_seconds", "status" => status.clone())
                .set(oldest_age.max(0.0));
            gauge!("attricat_file_worker_retries", "status" => status).set(retries as f64);
        }
        // Compatibility aggregate retained for existing dashboards.
        gauge!("attricat_file_worker_jobs_queued").set(queued as f64);
        Ok(())
    }
}

struct EncodedVariant {
    kind: &'static str,
    width: u32,
    height: u32,
    bytes: Vec<u8>,
}

fn encode_variant(kind: &'static str, image: &DynamicImage) -> Result<EncodedVariant, WorkerError> {
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    // Lossless WebP made photo variants larger than their JPEG originals.
    let bytes = webp::Encoder::from_rgba(&rgba, width, height)
        .encode_simple(false, VARIANT_WEBP_QUALITY)
        .map_err(|error| WorkerError::Image(format!("WebP encoding failed: {error:?}")))?
        .to_vec();
    Ok(EncodedVariant {
        kind,
        width,
        height,
        bytes,
    })
}

#[derive(Debug, thiserror::Error)]
enum WorkerError {
    #[error("file job lease was lost")]
    LeaseLost,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error(transparent)]
    Storage(#[from] ObjectStoreError),
    #[error("image processing failed: {0}")]
    Image(String),
}

/// Center-crops to a square and scales it down to at most `max_side` pixels.
/// Smaller images are cropped but never upscaled.
fn center_square(image: &DynamicImage, max_side: u32) -> DynamicImage {
    let (width, height) = image.dimensions();
    let side = width.min(height);
    let square = image.crop_imm((width - side) / 2, (height - side) / 2, side, side);
    if side <= max_side {
        square
    } else {
        square.resize_exact(max_side, max_side, image::imageops::FilterType::Lanczos3)
    }
}

/// Clients render thumbnails as square `object-fit: cover` tiles, so the
/// variant is the centered square they show rather than a letterboxed fit
/// whose short edge would have to be upscaled to fill the tile.
fn square_thumbnail(image: &DynamicImage) -> DynamicImage {
    center_square(image, THUMBNAIL_DIMENSION)
}

/// Scales the image down to fit the display bound, never upscaling.
fn display_image(image: &DynamicImage) -> DynamicImage {
    let (width, height) = image.dimensions();
    if width.max(height) <= DISPLAY_MAX_DIMENSION {
        image.clone()
    } else {
        image.resize(
            DISPLAY_MAX_DIMENSION,
            DISPLAY_MAX_DIMENSION,
            image::imageops::FilterType::Lanczos3,
        )
    }
}

/// Center-crops to a square, scales to the avatar size, and flattens any
/// transparency onto white so avatars look the same on every background.
/// Images smaller than the avatar size are cropped but never upscaled.
fn square_avatar(image: &DynamicImage) -> DynamicImage {
    let square = center_square(image, AVATAR_DIMENSION);
    let (width, height) = square.dimensions();
    let mut canvas = image::RgbaImage::from_pixel(width, height, image::Rgba([255, 255, 255, 255]));
    image::imageops::overlay(&mut canvas, &square.to_rgba8(), 0, 0);
    DynamicImage::ImageRgba8(canvas)
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

    #[sqlx::test(migrations = "../../apps/api/migrations")]
    async fn blocked_file_processing_can_finish_while_its_heartbeat_waits(pool: PgPool) {
        let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
        let file = Uuid::new_v4();
        let job = Uuid::new_v4();
        sqlx::query("INSERT INTO files (id,workspace_id,original_filename,display_filename,mime_type,byte_size,sha256,original_key,status,attachment_expires_at) VALUES ($1,$2,'test.txt','test.txt','text/plain',1,$3,$4,'queued',now()+interval '1 hour')")
            .bind(file).bind(workspace).bind("0".repeat(64)).bind(format!("files/{file}/original")).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO file_processing_jobs (id,workspace_id,file_id,kind,status) VALUES ($1,$2,$3,'metadata','queued')")
            .bind(job).bind(workspace).bind(file).execute(&pool).await.unwrap();
        let worker = FileWorker::new(
            pool.clone(),
            Arc::new(crate::storage::FakeObjectStore::available()),
            WorkerConfig {
                worker_id: "heartbeat-lock-test".into(),
                max_pixels: DEFAULT_MAX_PIXELS,
                max_attempts: 3,
                delete_grace: Duration::from_secs(60),
            },
        );
        let mut blocker = pool.begin().await.unwrap();
        let blocker_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *blocker)
            .await
            .unwrap();
        sqlx::query("SELECT id FROM files WHERE id=$1 FOR UPDATE")
            .bind(file)
            .execute(&mut *blocker)
            .await
            .unwrap();
        let mut processing = tokio::spawn(async move {
            worker
                .run_once_with_heartbeat(Duration::from_millis(20))
                .await
        });
        // Wait for the actual lock chain: the job holds its lease row and
        // waits for our file lock; renewal in turn waits for that job row.
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let blocked: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_stat_activity job JOIN pg_stat_activity renewal ON job.pid=ANY(pg_blocking_pids(renewal.pid)) WHERE job.datname=current_database() AND $1=ANY(pg_blocking_pids(job.pid)))")
                    .bind(blocker_pid).fetch_one(&pool).await.unwrap();
                if blocked { break; }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.unwrap();
        blocker.rollback().await.unwrap();
        let result = tokio::time::timeout(Duration::from_secs(3), &mut processing).await;
        if result.is_err() {
            processing.abort();
        }
        assert!(
            result
                .expect("heartbeat stopped polling the job that must release its lock")
                .unwrap()
                .unwrap()
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT status FROM file_processing_jobs WHERE id=$1")
                .bind(job)
                .fetch_one(&pool)
                .await
                .unwrap(),
            "completed"
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT status FROM files WHERE id=$1")
                .bind(file)
                .fetch_one(&pool)
                .await
                .unwrap(),
            "ready"
        );
    }

    const TEST_WORKSPACE: Uuid = Uuid::from_u128(0x00000000000040008000000000000002);
    const SPECIFICATION_DOCX: &[u8] =
        include_bytes!("../../document-text/tests/fixtures/specification.docx");
    const ENCRYPTED_PDF: &[u8] = include_bytes!("../../document-text/tests/fixtures/encrypted.pdf");

    fn test_worker(pool: &PgPool, store: Arc<dyn ObjectStore>) -> FileWorker {
        FileWorker::new(
            pool.clone(),
            store,
            WorkerConfig {
                worker_id: "text-extraction-test".into(),
                max_pixels: DEFAULT_MAX_PIXELS,
                max_attempts: 1,
                delete_grace: Duration::from_secs(60),
            },
        )
    }

    /// A ready file whose original is stored, with one queued job of `kind`.
    async fn stored_file(
        pool: &PgPool,
        store: &dyn ObjectStore,
        mime: &str,
        bytes: &[u8],
        kind: FileJobKind,
    ) -> (Uuid, Uuid) {
        let file = Uuid::new_v4();
        let job = Uuid::new_v4();
        let key = format!("files/{file}/original");
        store
            .put(
                &key,
                StoredObject {
                    bytes: bytes.to_vec().into(),
                    content_type: Some(mime.into()),
                },
            )
            .await
            .unwrap();
        sqlx::query("INSERT INTO files (id,workspace_id,original_filename,display_filename,mime_type,byte_size,sha256,original_key,status,attachment_expires_at) VALUES ($1,$2,'document','document',$3,$4,$5,$6,'ready',now()+interval '1 hour')")
            .bind(file).bind(TEST_WORKSPACE).bind(mime).bind(bytes.len() as i64).bind("0".repeat(64)).bind(key).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO file_processing_jobs (id,workspace_id,file_id,kind,status) VALUES ($1,$2,$3,$4,'queued')")
            .bind(job).bind(TEST_WORKSPACE).bind(file).bind(kind.as_str()).execute(pool).await.unwrap();
        (file, job)
    }

    async fn extraction(pool: &PgPool, file: Uuid) -> (String, Option<String>, i32) {
        sqlx::query_as("SELECT status, failure_reason, extractor_version FROM file_text_extractions WHERE file_id=$1")
            .bind(file)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    #[sqlx::test(migrations = "../../apps/api/migrations")]
    async fn documents_get_a_stored_text_variant(pool: PgPool) {
        let store: Arc<dyn ObjectStore> = Arc::new(crate::storage::FakeObjectStore::available());
        let (file, job) = stored_file(
            &pool,
            store.as_ref(),
            attricat_validation::files::DOCX_MIME,
            SPECIFICATION_DOCX,
            FileJobKind::TextExtraction,
        )
        .await;
        assert!(test_worker(&pool, store.clone()).run_once().await.unwrap());

        assert_eq!(
            extraction(&pool, file).await,
            ("ready".to_owned(), None, EXTRACTOR_VERSION as i32)
        );
        let key: String = sqlx::query_scalar(
            "SELECT object_key FROM file_variants WHERE file_id=$1 AND kind=$2 AND mime_type='application/json'",
        )
        .bind(file)
        .bind(TEXT_FILE_VARIANT)
        .fetch_one(&pool)
        .await
        .unwrap();
        let document: attricat_document_text::Document =
            serde_json::from_slice(&store.get(&key).await.unwrap().bytes).unwrap();
        assert!(document.sections[0].text.contains("# Widget specification"));
        let job_status: String =
            sqlx::query_scalar("SELECT status FROM file_processing_jobs WHERE id=$1")
                .bind(job)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(job_status, "completed");
    }

    #[sqlx::test(migrations = "../../apps/api/migrations")]
    async fn unreadable_documents_record_why_and_stay_usable(pool: PgPool) {
        let store: Arc<dyn ObjectStore> = Arc::new(crate::storage::FakeObjectStore::available());
        let (file, _) = stored_file(
            &pool,
            store.as_ref(),
            attricat_validation::files::PDF_MIME,
            ENCRYPTED_PDF,
            FileJobKind::TextExtraction,
        )
        .await;
        assert!(test_worker(&pool, store).run_once().await.unwrap());

        assert_eq!(
            extraction(&pool, file).await,
            (
                "failed".to_owned(),
                Some("encrypted".to_owned()),
                EXTRACTOR_VERSION as i32
            )
        );
        let (status, variants): (String, i64) = sqlx::query_as(
            "SELECT f.status, (SELECT count(*) FROM file_variants v WHERE v.file_id = f.id) FROM files f WHERE f.id=$1",
        )
        .bind(file)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!((status.as_str(), variants), ("ready", 0));
    }

    #[sqlx::test(migrations = "../../apps/api/migrations")]
    async fn failed_text_jobs_never_fail_the_file(pool: PgPool) {
        let store = Arc::new(crate::storage::FakeObjectStore::available());
        let (file, job) = stored_file(
            &pool,
            store.as_ref(),
            attricat_validation::files::DOCX_MIME,
            SPECIFICATION_DOCX,
            FileJobKind::TextExtraction,
        )
        .await;
        store.set_available(false);
        assert!(test_worker(&pool, store).run_once().await.unwrap());

        let (job_status, file_status): (String, String) = sqlx::query_as(
            "SELECT j.status, f.status FROM file_processing_jobs j JOIN files f ON f.id = j.file_id WHERE j.id=$1",
        )
        .bind(job)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            (job_status.as_str(), file_status.as_str()),
            ("failed", "ready")
        );
        let extracted: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM file_text_extractions WHERE file_id=$1)",
        )
        .bind(file)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!extracted);
    }

    #[sqlx::test(migrations = "../../apps/api/migrations")]
    async fn backfill_queues_documents_without_current_text_once(pool: PgPool) {
        let store = crate::storage::FakeObjectStore::available();
        let docx = attricat_validation::files::DOCX_MIME;
        // Uploaded before text extraction existed: only a metadata job.
        let (missing, missing_job) = stored_file(
            &pool,
            &store,
            docx,
            SPECIFICATION_DOCX,
            FileJobKind::Metadata,
        )
        .await;
        let (extracted, extracted_job) = stored_file(
            &pool,
            &store,
            docx,
            SPECIFICATION_DOCX,
            FileJobKind::Metadata,
        )
        .await;
        let (_, failed_job) = stored_file(
            &pool,
            &store,
            docx,
            SPECIFICATION_DOCX,
            FileJobKind::TextExtraction,
        )
        .await;
        let (_, plain_job) =
            stored_file(&pool, &store, "text/plain", b"notes", FileJobKind::Metadata).await;
        sqlx::query("UPDATE file_processing_jobs SET status='completed' WHERE id = ANY($1)")
            .bind(vec![missing_job, extracted_job, plain_job])
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE file_processing_jobs SET status='failed' WHERE id=$1")
            .bind(failed_job)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO file_text_extractions (file_id,workspace_id,extractor_version,status) VALUES ($1,$2,$3,'ready')")
            .bind(extracted).bind(TEST_WORKSPACE).bind(EXTRACTOR_VERSION as i32).execute(&pool).await.unwrap();
        let repository = AttricatRepository::system(pool.clone());
        let queued_files = || async {
            sqlx::query_scalar::<_, Uuid>(
                "SELECT file_id FROM file_processing_jobs WHERE kind=$1 AND status='queued' ORDER BY created_at, file_id",
            )
            .bind(FileJobKind::TextExtraction.as_str())
            .fetch_all(&pool)
            .await
            .unwrap()
        };

        let backfill = |version| repository.queue_text_extraction_backfill(version);
        assert_eq!(backfill(EXTRACTOR_VERSION).await.unwrap(), 1);
        assert_eq!(queued_files().await, [missing]);
        // A pending job is not queued twice.
        assert_eq!(backfill(EXTRACTOR_VERSION).await.unwrap(), 0);
        // A newer extractor queues text an older one produced.
        assert_eq!(backfill(EXTRACTOR_VERSION + 1).await.unwrap(), 1);
        assert!(queued_files().await.contains(&extracted));

        // A timeout is retried once retention has removed its text job.
        let (timed_out, timed_out_job) = stored_file(
            &pool,
            &store,
            docx,
            SPECIFICATION_DOCX,
            FileJobKind::TextExtraction,
        )
        .await;
        sqlx::query("UPDATE file_processing_jobs SET status='completed' WHERE id=$1")
            .bind(timed_out_job)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO file_text_extractions (file_id,workspace_id,extractor_version,status,failure_reason) VALUES ($1,$2,$3,'failed','timed_out')")
            .bind(timed_out).bind(TEST_WORKSPACE).bind(EXTRACTOR_VERSION as i32).execute(&pool).await.unwrap();
        assert_eq!(backfill(EXTRACTOR_VERSION).await.unwrap(), 0);
        sqlx::query("DELETE FROM file_processing_jobs WHERE id=$1")
            .bind(timed_out_job)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(backfill(EXTRACTOR_VERSION).await.unwrap(), 1);
        assert!(queued_files().await.contains(&timed_out));
    }

    #[test]
    fn rejects_limits_that_would_wrap_during_database_conversion() {
        assert!(checked_max_attempts(0).is_err());
        assert!(checked_max_attempts(i32::MAX as u64 + 1).is_err());
        assert!(checked_max_attempts(u64::MAX).is_err());
        assert_eq!(checked_max_attempts(5).unwrap(), 5);
        assert!(checked_grace(i64::MAX as u64 + 1).is_err());
    }

    #[test]
    fn avatars_are_center_cropped_squares_that_never_upscale() {
        let wide = DynamicImage::new_rgba8(1000, 400);
        assert_eq!(square_avatar(&wide).dimensions(), (256, 256));
        let tall = DynamicImage::new_rgba8(300, 900);
        assert_eq!(square_avatar(&tall).dimensions(), (256, 256));
        let small = DynamicImage::new_rgba8(120, 80);
        assert_eq!(square_avatar(&small).dimensions(), (80, 80));
    }

    #[test]
    fn thumbnails_are_center_cropped_squares_that_never_upscale() {
        let wide = DynamicImage::new_rgba8(4000, 1000);
        assert_eq!(square_thumbnail(&wide).dimensions(), (320, 320));
        let tall = DynamicImage::new_rgba8(600, 1800);
        assert_eq!(square_thumbnail(&tall).dimensions(), (320, 320));
        let small = DynamicImage::new_rgba8(200, 100);
        assert_eq!(square_thumbnail(&small).dimensions(), (100, 100));
    }

    #[test]
    fn thumbnails_keep_the_center_of_the_image() {
        let mut image = image::RgbaImage::from_pixel(900, 300, image::Rgba([0, 0, 255, 255]));
        for x in 300..600 {
            for y in 0..300 {
                image.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
            }
        }
        let thumbnail = square_thumbnail(&DynamicImage::ImageRgba8(image)).to_rgba8();
        assert_eq!(thumbnail.get_pixel(0, 0), &image::Rgba([255, 0, 0, 255]));
        assert_eq!(
            thumbnail.get_pixel(299, 299),
            &image::Rgba([255, 0, 0, 255])
        );
    }

    #[test]
    fn display_images_fit_the_bound_and_never_upscale() {
        let large = DynamicImage::new_rgba8(3200, 1600);
        assert_eq!(display_image(&large).dimensions(), (1600, 800));
        let small = DynamicImage::new_rgba8(640, 480);
        assert_eq!(display_image(&small).dimensions(), (640, 480));
    }

    #[test]
    fn avatars_flatten_transparency_onto_white() {
        let mut image = image::RgbaImage::from_pixel(10, 10, image::Rgba([0, 0, 0, 0]));
        image.put_pixel(5, 5, image::Rgba([200, 0, 0, 255]));
        let avatar = square_avatar(&DynamicImage::ImageRgba8(image)).to_rgba8();
        assert_eq!(avatar.get_pixel(0, 0), &image::Rgba([255, 255, 255, 255]));
        assert_eq!(avatar.get_pixel(5, 5), &image::Rgba([200, 0, 0, 255]));
    }

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
