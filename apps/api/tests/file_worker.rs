mod support;

use std::{io::Cursor, sync::Arc, time::Duration};

use api::{
    file_worker::{FileWorker, WorkerConfig},
    storage::{FakeObjectStore, ObjectStore, StoredObject},
};
use image::{DynamicImage, ImageFormat, Rgba};
use sha2::{Digest, Sha256};
use support::*;

const WORKSPACE_ID: &str = "00000000-0000-4000-8000-000000000002";

fn worker(pool: PgPool, store: Arc<FakeObjectStore>, max_attempts: i32, grace: u64) -> FileWorker {
    FileWorker::new(
        pool,
        store,
        WorkerConfig {
            worker_id: "file-worker-integration-test".into(),
            max_pixels: 40_000_000,
            max_attempts,
            delete_grace: Duration::from_secs(grace),
        },
    )
}

fn png() -> Vec<u8> {
    let image =
        DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(640, 480, Rgba([4, 5, 6, 255])));
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, ImageFormat::Png).unwrap();
    bytes.into_inner()
}

async fn insert_file(pool: &PgPool, file_id: Uuid, original_key: &str, retain: bool) {
    sqlx::query(
        "INSERT INTO files (id, workspace_id, original_filename, display_filename, mime_type, byte_size, sha256, original_key, status, attachment_expires_at) VALUES ($1, $2, 'image.png', 'image.png', 'image/png', $3, $4, $5, 'queued', CASE WHEN $6 THEN now() + interval '1 hour' ELSE NULL END)",
    )
    .bind(file_id)
    .bind(WORKSPACE_ID.parse::<Uuid>().unwrap())
    .bind(png().len() as i64)
    .bind(format!("{:x}", Sha256::digest(png())))
    .bind(original_key)
    .bind(retain)
    .execute(pool)
    .await
    .unwrap();
}

async fn queue_metadata_job(pool: &PgPool, file_id: Uuid) -> Uuid {
    let job_id = Uuid::new_v4();
    sqlx::query("INSERT INTO file_processing_jobs (id, workspace_id, file_id, kind, status) VALUES ($1, $2, $3, 'metadata', 'queued')")
        .bind(job_id)
        .bind(WORKSPACE_ID.parse::<Uuid>().unwrap())
        .bind(file_id)
        .execute(pool)
        .await
        .unwrap();
    job_id
}

#[sqlx::test]
async fn worker_generates_image_variants_and_completes_metadata_job(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let file_id = Uuid::new_v4();
    let original_key = format!("files/{file_id}/original");
    insert_file(&pool, file_id, &original_key, true).await;
    store
        .put(
            &original_key,
            StoredObject {
                bytes: png().into(),
                content_type: Some("image/png".into()),
            },
        )
        .await
        .unwrap();
    let job_id = queue_metadata_job(&pool, file_id).await;

    assert!(
        worker(pool.clone(), store.clone(), 3, 60)
            .run_once()
            .await
            .unwrap()
    );

    let variants: Vec<(String, String, i32, i32)> = sqlx::query_as(
        "SELECT kind, mime_type, width, height FROM file_variants WHERE file_id = $1 ORDER BY kind",
    )
    .bind(file_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        variants,
        vec![
            ("display".into(), "image/webp".into(), 1600, 1200),
            ("thumbnail".into(), "image/webp".into(), 320, 240),
        ]
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM files WHERE id = $1")
            .bind(file_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "ready"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM file_processing_jobs WHERE id = $1")
            .bind(job_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "completed"
    );
    assert_eq!(store.object_count().await, 3);
    assert_eq!(
        store
            .get(&format!("files/{file_id}/thumbnail.webp"))
            .await
            .unwrap()
            .content_type
            .as_deref(),
        Some("image/webp")
    );
}

#[sqlx::test]
async fn worker_retries_terminal_failure_and_operator_retry(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let file_id = Uuid::new_v4();
    let original_key = format!("files/{file_id}/original");
    insert_file(&pool, file_id, &original_key, true).await;
    let job_id = queue_metadata_job(&pool, file_id).await;
    let worker = worker(pool.clone(), store.clone(), 2, 60);

    store.set_available(false);
    assert!(worker.run_once().await.unwrap());
    let first: (String, i32, bool) = sqlx::query_as(
        "SELECT status, attempts, available_at > now() + interval '1 second' FROM file_processing_jobs WHERE id = $1",
    )
    .bind(job_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(first, ("retryable".into(), 1, true));

    sqlx::query("UPDATE file_processing_jobs SET available_at = now() WHERE id = $1")
        .bind(job_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(worker.run_once().await.unwrap());
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM file_processing_jobs WHERE id = $1")
            .bind(job_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "failed"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM files WHERE id = $1")
            .bind(file_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "failed"
    );

    assert!(worker.retry_job(job_id).await.unwrap());
    store.set_available(true);
    store
        .put(
            &original_key,
            StoredObject {
                bytes: png().into(),
                content_type: Some("image/png".into()),
            },
        )
        .await
        .unwrap();
    assert!(worker.run_once().await.unwrap());
    assert_eq!(
        sqlx::query_as::<_, (String, i32)>(
            "SELECT status, attempts FROM file_processing_jobs WHERE id = $1"
        )
        .bind(job_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        ("completed".into(), 1)
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM files WHERE id = $1")
            .bind(file_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "ready"
    );
}

#[sqlx::test]
async fn concurrent_reconcilers_enqueue_one_purge_job(pool: PgPool) {
    let file_id = Uuid::new_v4();
    insert_file(&pool, file_id, &format!("files/{file_id}/original"), false).await;
    sqlx::query("UPDATE files SET status = 'deleted', deleted_at = now(), purge_after = now() - interval '1 second' WHERE id = $1")
        .bind(file_id)
        .execute(&pool)
        .await
        .unwrap();
    let store = Arc::new(FakeObjectStore::available());
    let first = worker(pool.clone(), store.clone(), 3, 60);
    let second = worker(pool.clone(), store, 3, 60);
    let (left, right) = tokio::join!(first.reconcile(), second.reconcile());
    left.unwrap();
    right.unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM file_processing_jobs WHERE file_id = $1 AND kind = 'purge'",
    )
    .bind(file_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test]
async fn reconciliation_delays_and_idempotently_purges_unreferenced_files(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let file_id = Uuid::new_v4();
    let original_key = format!("files/{file_id}/original");
    insert_file(&pool, file_id, &original_key, false).await;
    let thumbnail_key = format!("files/{file_id}/thumbnail.webp");
    for key in [&original_key, &thumbnail_key] {
        store
            .put(
                key,
                StoredObject {
                    bytes: png().into(),
                    content_type: Some("image/png".into()),
                },
            )
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO file_variants (id, workspace_id, file_id, kind, mime_type, byte_size, object_key, sha256) VALUES ($1, $2, $3, 'thumbnail', 'image/webp', 1, $4, $5)")
        .bind(Uuid::new_v4())
        .bind(WORKSPACE_ID.parse::<Uuid>().unwrap())
        .bind(file_id)
        .bind(&thumbnail_key)
        .bind("0".repeat(64))
        .execute(&pool)
        .await
        .unwrap();
    let worker = worker(pool.clone(), store.clone(), 3, 60);

    worker.reconcile().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM files WHERE id = $1")
            .bind(file_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "deleted"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM file_processing_jobs WHERE file_id = $1 AND kind = 'purge'"
        )
        .bind(file_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(store.object_count().await, 2);

    sqlx::query("UPDATE files SET purge_after = now() - interval '1 second' WHERE id = $1")
        .bind(file_id)
        .execute(&pool)
        .await
        .unwrap();
    worker.reconcile().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM file_processing_jobs WHERE file_id = $1 AND kind = 'purge'"
        )
        .bind(file_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );

    assert!(worker.run_once().await.unwrap());
    assert_eq!(store.object_count().await, 0);
    worker.reconcile().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM file_processing_jobs WHERE file_id = $1 AND kind = 'purge'"
        )
        .bind(file_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}
