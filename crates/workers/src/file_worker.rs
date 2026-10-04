//! Durable file-processing and retention worker.
//!
//! The worker owns state transitions: HTTP only stages the original and queues a
//! metadata job.  Claims and all state changes are database transactions, while
//! object writes are deliberately idempotent (stable variant keys).
use std::{
    io::Cursor,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

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
    pub fn from_env() -> Result<Self, String> {
        let parse = |name: &str, default: u64| -> Result<u64, String> {
            std::env::var(name).map_or(Ok(default), |value| {
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
            worker_id: std::env::var("FILE_WORKER_ID")
                .unwrap_or_else(|_| format!("file-worker-{}", Uuid::new_v4())),
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
    store: Arc<dyn ObjectStore>,
    config: WorkerConfig,
    last_reconciled: Mutex<Option<Instant>>,
    last_metrics: Mutex<Option<Instant>>,
}

impl FileWorker {
    pub fn new(pool: PgPool, store: Arc<dyn ObjectStore>, config: WorkerConfig) -> Self {
        Self {
            pool,
            store,
            config,
            last_reconciled: Mutex::new(None),
            last_metrics: Mutex::new(None),
        }
    }

    /// Atomically claims one due or abandoned job. SKIP LOCKED allows multiple
    /// worker processes to poll without serialising each other.
    pub async fn claim(&self) -> Result<Option<ClaimedJob>, sqlx::Error> {
        let job = sqlx::query_as::<_, (Uuid, Uuid, Uuid, String, i32, Uuid)>(
            r#"WITH candidate AS (
                 SELECT id FROM file_processing_jobs
                 WHERE (status IN ('queued', 'retryable') AND available_at <= now())
                    OR (status = 'running' AND locked_at < now() - make_interval(secs => $1))
                 ORDER BY available_at, id FOR UPDATE SKIP LOCKED LIMIT 1
               )
               UPDATE file_processing_jobs job SET status = 'running', locked_at = now(),
                 worker_id = $2, lease_token = $3, attempts = job.attempts + 1, updated_at = now()
               FROM candidate WHERE job.id = candidate.id
               RETURNING job.id, job.workspace_id, job.file_id, job.kind, job.attempts, job.lease_token"#,
        )
        .bind(STALE_LOCK_SECONDS)
        .bind(&self.config.worker_id)
        .bind(Uuid::new_v4())
        .fetch_optional(&self.pool)
        .await?;
        if job.is_some() {
            counter!("catalog_file_worker_jobs_claimed_total").increment(1);
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

    pub async fn run_once(&self) -> Result<bool, sqlx::Error> {
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
        let kind = if job.kind == "purge" {
            "purge"
        } else {
            "metadata"
        };
        let span = info_span!("file_worker.job", job_id = %job.id, file_id = %job.file_id, kind);
        let operation = async {
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
        .instrument(span);
        tokio::pin!(operation);
        let mut heartbeat = tokio::time::interval(Duration::from_secs(LEASE_HEARTBEAT_SECONDS));
        heartbeat.tick().await;
        loop {
            tokio::select! {
                biased;
                _ = heartbeat.tick() => {
                    if !self.renew_lease(&job).await? {
                        tracing::warn!(job_id = %job.id, "file job lease lost; cancelling processing");
                        break;
                    }
                }
                result = &mut operation => { result?; break; }
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
                encode_variant(
                    "thumbnail",
                    &image.thumbnail(THUMBNAIL_MAX_DIMENSION, THUMBNAIL_MAX_DIMENSION),
                )?,
                encode_variant(
                    "display",
                    &image.resize(
                        DISPLAY_MAX_DIMENSION,
                        DISPLAY_MAX_DIMENSION,
                        image::imageops::FilterType::Lanczos3,
                    ),
                )?,
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

    async fn purge(&self, job: &ClaimedJob) -> Result<(), WorkerError> {
        let row = sqlx::query("SELECT original_key FROM files WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NOT NULL AND purge_after <= now() AND NOT EXISTS (SELECT 1 FROM attribute_file_references r WHERE r.workspace_id = files.workspace_id AND r.file_id = files.id) AND NOT EXISTS (SELECT 1 FROM workspace_memberships m WHERE m.workspace_id = files.workspace_id AND m.avatar_file_id = files.id) AND NOT EXISTS (SELECT 1 FROM file_retention_holds h WHERE h.workspace_id = files.workspace_id AND h.file_id = files.id AND h.released_at IS NULL AND h.held_until > now())")
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

    /// Marks unreferenced files without an active retention hold for delayed
    /// deletion and creates one durable purge job. Repeated runs are harmless
    /// and never delete before grace.
    pub async fn reconcile(&self) -> Result<(), sqlx::Error> {
        let grace = i64::try_from(self.config.delete_grace.as_secs()).unwrap_or(i64::MAX);
        let mut transaction = self.pool.begin().await?;

        // Attachment creation locks its file before recording the attachment.
        // Lock candidates first (and skip an in-progress attachment) so the
        // reference check and deletion decision share that same lock. This
        // serializes the two transactions: an attachment that gets the lock
        // first is retained; a file reconciliation that gets it first makes a
        // later attachment fail its existing live-file authorization check.
        let candidates: Vec<Uuid> = sqlx::query_scalar(
            r#"SELECT f.id FROM files f
            WHERE f.deleted_at IS NULL
              AND (f.attachment_expires_at IS NULL OR f.attachment_expires_at <= now())
              AND NOT EXISTS (SELECT 1 FROM attribute_file_references r WHERE r.workspace_id = f.workspace_id AND r.file_id = f.id)
              AND NOT EXISTS (SELECT 1 FROM conversation_message_attachments a WHERE a.workspace_id = f.workspace_id AND a.file_id = f.id)
              AND NOT EXISTS (SELECT 1 FROM workspace_memberships m WHERE m.workspace_id = f.workspace_id AND m.avatar_file_id = f.id)
              AND NOT EXISTS (SELECT 1 FROM file_retention_holds h WHERE h.workspace_id = f.workspace_id AND h.file_id = f.id AND h.released_at IS NULL AND h.held_until > now())
            ORDER BY f.id
            LIMIT 256 FOR UPDATE SKIP LOCKED"#,
        )
        .fetch_all(&mut *transaction)
        .await?;
        let marked = if candidates.is_empty() {
            0
        } else {
            sqlx::query(r#"UPDATE files f SET status = 'deleted', deleted_at = now(), purge_after = now() + make_interval(secs => $1), updated_at = now()
                WHERE f.id = ANY($2)
                  AND f.deleted_at IS NULL
                  AND (f.attachment_expires_at IS NULL OR f.attachment_expires_at <= now())
                  AND NOT EXISTS (SELECT 1 FROM attribute_file_references r WHERE r.workspace_id = f.workspace_id AND r.file_id = f.id)
                  AND NOT EXISTS (SELECT 1 FROM conversation_message_attachments a WHERE a.workspace_id = f.workspace_id AND a.file_id = f.id)
              AND NOT EXISTS (SELECT 1 FROM workspace_memberships m WHERE m.workspace_id = f.workspace_id AND m.avatar_file_id = f.id)
              AND NOT EXISTS (SELECT 1 FROM file_retention_holds h WHERE h.workspace_id = f.workspace_id AND h.file_id = f.id AND h.released_at IS NULL AND h.held_until > now())"#)
                .bind(grace)
                .bind(&candidates)
                .execute(&mut *transaction)
                .await?
                .rows_affected()
        };
        transaction.commit().await?;
        metrics::counter!("catalog_file_reconciliation_total", "outcome" => "success").increment(1);
        metrics::counter!("catalog_file_reconciliation_files_marked_total").increment(marked);
        // Lock a bounded batch while inserting. A concurrent SELECT can still
        // use an older snapshot, so the unique index is the final safeguard
        // against duplicate purge jobs; ON CONFLICT handles that race.
        let mut transaction = self.pool.begin().await?;
        let due: Vec<(Uuid, Uuid, DateTime<Utc>)> = sqlx::query_as(r#"SELECT f.workspace_id, f.id, f.purge_after FROM files f
            WHERE f.deleted_at IS NOT NULL AND f.purge_after <= now()
              AND NOT EXISTS (SELECT 1 FROM file_processing_jobs j WHERE j.file_id = f.id AND j.kind = 'purge' AND j.status IN ('queued','running','retryable','completed'))
            ORDER BY f.purge_after, f.id
            LIMIT 256 FOR UPDATE OF f SKIP LOCKED"#)
            .fetch_all(&mut *transaction).await?;
        let mut due_count = 0;
        for (workspace_id, file_id, available_at) in due {
            due_count += sqlx::query("INSERT INTO file_processing_jobs (id, workspace_id, file_id, kind, status, available_at) VALUES ($1,$2,$3,'purge','queued',$4) ON CONFLICT DO NOTHING")
                .bind(Uuid::new_v4()).bind(workspace_id).bind(file_id).bind(available_at).execute(&mut *transaction).await?.rows_affected();
        }
        transaction.commit().await?;
        metrics::counter!("catalog_file_purge_jobs_queued_total").increment(due_count);
        tracing::info!(
            files_marked = marked,
            purge_jobs_queued = due_count,
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
            counter!("catalog_file_worker_jobs_completed_total").increment(1);
        } else {
            tracing::warn!(job_id = %job.id, "file job lease lost before completion");
        }
        Ok(())
    }
    async fn fail(&self, job: &ClaimedJob, error: &str) -> Result<(), sqlx::Error> {
        let terminal = job.attempts >= self.config.max_attempts;
        let backoff = (1_i64 << job.attempts.min(8)).min(MAX_BACKOFF_SECONDS);
        let changed = sqlx::query("UPDATE file_processing_jobs SET status = $2, available_at = now() + make_interval(secs => $3), locked_at = NULL, worker_id = NULL, lease_token = NULL, last_error = $4, updated_at = now() WHERE id = $1 AND status = 'running' AND worker_id = $5 AND lease_token = $6")
            .bind(job.id).bind(if terminal { "failed" } else { "retryable" }).bind(backoff).bind(error).bind(&self.config.worker_id).bind(job.lease_token).execute(&self.pool).await?.rows_affected();
        if changed != 1 {
            tracing::warn!(job_id = %job.id, "file job lease lost before failure could be recorded");
            return Ok(());
        }
        if terminal {
            sqlx::query("UPDATE files SET status = 'failed', processing_error = $2, updated_at = now() WHERE id = $1 AND deleted_at IS NULL").bind(job.file_id).bind(error).execute(&self.pool).await?;
        }
        counter!("catalog_file_worker_jobs_failed_total", "terminal" => terminal.to_string())
            .increment(1);
        Ok(())
    }
    pub async fn record_metrics(&self) -> Result<(), sqlx::Error> {
        const STATUSES: [&str; 4] = ["queued", "running", "retryable", "failed"];
        for status in STATUSES {
            gauge!("catalog_file_worker_queue_depth", "status" => status).set(0.0);
            gauge!("catalog_file_worker_oldest_age_seconds", "status" => status).set(0.0);
            gauge!("catalog_file_worker_retries", "status" => status).set(0.0);
        }
        let rows: Vec<(String, i64, f64, i64)> = sqlx::query_as(
            "SELECT status,count(*)::bigint,COALESCE(extract(epoch FROM (clock_timestamp()-min(created_at))),0)::float8,COALESCE(sum(GREATEST(attempts-1,0)),0)::bigint FROM file_processing_jobs WHERE status IN ('queued','running','retryable','failed') GROUP BY status",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut queued = 0_i64;
        for (status, count, oldest_age, retries) in rows {
            if matches!(status.as_str(), "queued" | "retryable") {
                queued += count;
            }
            gauge!("catalog_file_worker_queue_depth", "status" => status.clone()).set(count as f64);
            gauge!("catalog_file_worker_oldest_age_seconds", "status" => status.clone())
                .set(oldest_age.max(0.0));
            gauge!("catalog_file_worker_retries", "status" => status).set(retries as f64);
        }
        // Compatibility aggregate retained for existing dashboards.
        gauge!("catalog_file_worker_jobs_queued").set(queued as f64);
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
    let mut bytes = Vec::new();
    image::codecs::webp::WebPEncoder::new_lossless(&mut bytes)
        .write_image(&rgba, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|error| WorkerError::Image(error.to_string()))?;
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
    Storage(#[from] ObjectStoreError),
    #[error("image processing failed: {0}")]
    Image(String),
}

/// Center-crops to a square, scales to the avatar size, and flattens any
/// transparency onto white so avatars look the same on every background.
/// Images smaller than the avatar size are cropped but never upscaled.
fn square_avatar(image: &DynamicImage) -> DynamicImage {
    let (width, height) = image.dimensions();
    let side = width.min(height);
    let square = image.crop_imm((width - side) / 2, (height - side) / 2, side, side);
    let square = if side <= AVATAR_DIMENSION {
        square
    } else {
        square.resize_exact(
            AVATAR_DIMENSION,
            AVATAR_DIMENSION,
            image::imageops::FilterType::Lanczos3,
        )
    };
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
