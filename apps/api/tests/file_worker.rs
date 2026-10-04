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
async fn reconciliation_does_not_rescan_on_every_poll(pool: PgPool) {
    let worker = worker(pool.clone(), Arc::new(FakeObjectStore::available()), 3, 60);
    assert!(!worker.run_once().await.unwrap());
    let file_id = Uuid::new_v4();
    insert_file(&pool, file_id, &format!("files/{file_id}/original"), false).await;
    assert!(!worker.run_once().await.unwrap());
    let deleted: bool =
        sqlx::query_scalar("SELECT deleted_at IS NOT NULL FROM files WHERE id = $1")
            .bind(file_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!deleted);
    worker.reconcile().await.unwrap();
    let deleted: bool =
        sqlx::query_scalar("SELECT deleted_at IS NOT NULL FROM files WHERE id = $1")
            .bind(file_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(deleted);
}

#[sqlx::test]
async fn reclaimed_file_job_gets_a_new_fencing_token(pool: PgPool) {
    let file_id = Uuid::new_v4();
    insert_file(&pool, file_id, &format!("files/{file_id}/original"), true).await;
    let job_id = queue_metadata_job(&pool, file_id).await;
    let worker = worker(pool.clone(), Arc::new(FakeObjectStore::available()), 3, 60);
    let first = worker.claim().await.unwrap().unwrap();
    sqlx::query(
        "UPDATE file_processing_jobs SET locked_at = now() - interval '301 seconds' WHERE id = $1",
    )
    .bind(job_id)
    .execute(&pool)
    .await
    .unwrap();
    let second = worker.claim().await.unwrap().unwrap();
    assert_eq!(first.id, second.id);
    assert_ne!(first.lease_token, second.lease_token);
    assert_eq!(second.attempts, first.attempts + 1);
    let stale_ack = sqlx::query("UPDATE file_processing_jobs SET status = 'completed' WHERE id = $1 AND status = 'running' AND worker_id = $2 AND lease_token = $3")
        .bind(first.id).bind("file-worker-integration-test").bind(first.lease_token)
        .execute(&pool).await.unwrap();
    assert_eq!(stale_ack.rows_affected(), 0);
    let (status, token): (String, Option<Uuid>) =
        sqlx::query_as("SELECT status, lease_token FROM file_processing_jobs WHERE id = $1")
            .bind(job_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "running");
    assert_eq!(token, Some(second.lease_token));
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

#[sqlx::test]
async fn reconciliation_retains_files_under_an_active_retention_hold(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let file_id = Uuid::new_v4();
    insert_file(&pool, file_id, &format!("files/{file_id}/original"), false).await;
    let hold_id = Uuid::new_v4();
    sqlx::query("INSERT INTO file_retention_holds (id, workspace_id, file_id, source, reason, held_until) VALUES ($1, $2, $3, 'explicit', 'Legal hold', now() + interval '1 day')")
        .bind(hold_id)
        .bind(WORKSPACE_ID.parse::<Uuid>().unwrap())
        .bind(file_id)
        .execute(&pool)
        .await
        .unwrap();
    let worker = worker(pool.clone(), store, 3, 0);
    let status = || async {
        sqlx::query_scalar::<_, String>("SELECT status FROM files WHERE id = $1")
            .bind(file_id)
            .fetch_one(&pool)
            .await
            .unwrap()
    };
    worker.reconcile().await.unwrap();
    assert_eq!(status().await, "queued");
    // An expired hold no longer protects the file.
    sqlx::query("UPDATE file_retention_holds SET created_at = now() - interval '2 days', held_until = now() - interval '1 second' WHERE id = $1")
        .bind(hold_id)
        .execute(&pool)
        .await
        .unwrap();
    worker.reconcile().await.unwrap();
    assert_eq!(status().await, "deleted");
}

/// Inserts an extension release and an operation run in `status` whose input
/// artifact copies `file_id`'s object key, as run creation does.
async fn insert_run_with_input_file(pool: &PgPool, release_id: Uuid, file_id: Uuid, status: &str) {
    let workspace_id = WORKSPACE_ID.parse::<Uuid>().unwrap();
    let run_id = Uuid::new_v4();
    sqlx::query("INSERT INTO extension_operation_runs (id, workspace_id, extension_id, installed_release_id, abi_version, operation_id, idempotency_key, status, source_reference) VALUES ($1, $2, 'acme.files', $3, '1.6.0', 'import', $4, $5, jsonb_build_object('input_file_id', $6::text))")
        .bind(run_id)
        .bind(workspace_id)
        .bind(release_id)
        .bind(run_id.to_string())
        .bind(status)
        .bind(file_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO extension_operation_artifacts (id, workspace_id, extension_id, installed_release_id, operation_run_id, direction, state, content_length, media_type, checksum_sha256, object_key, completed_at) SELECT $1, workspace_id, 'acme.files', $2, $3, 'input', 'completed', byte_size, mime_type, sha256, original_key, now() FROM files WHERE id = $4")
        .bind(Uuid::new_v4())
        .bind(release_id)
        .bind(run_id)
        .bind(file_id)
        .execute(pool)
        .await
        .unwrap();
}

async fn insert_extension_release(pool: &PgPool) -> Uuid {
    let release_id = Uuid::new_v4();
    sqlx::query("INSERT INTO installed_extension_releases (id, workspace_id, extension_id, version, manifest, manifest_sha256, source) VALUES ($1, $2, 'acme.files', '1.0.0', '{}'::jsonb, $3, 'side_load')")
        .bind(release_id)
        .bind(WORKSPACE_ID.parse::<Uuid>().unwrap())
        .bind("0".repeat(64))
        .execute(pool)
        .await
        .unwrap();
    release_id
}

async fn file_status(pool: &PgPool, file_id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM files WHERE id = $1")
        .bind(file_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test]
async fn reconciliation_retains_files_used_as_extension_and_connector_inputs(pool: PgPool) {
    let workspace_id = WORKSPACE_ID.parse::<Uuid>().unwrap();
    let store = Arc::new(FakeObjectStore::available());
    let [scheduled, connector, pending_run, finished_run] = [(); 4].map(|_| Uuid::new_v4());
    for file_id in [scheduled, connector, pending_run, finished_run] {
        insert_file(&pool, file_id, &format!("files/{file_id}/original"), false).await;
    }
    let release_id = insert_extension_release(&pool).await;
    sqlx::query("INSERT INTO extension_operation_schedules (id, workspace_id, extension_id, installed_release_id, operation_id, input, configuration_snapshot, source_reference, interval_seconds, next_at) VALUES ($1, $2, 'acme.files', $3, 'import', '{}'::jsonb, '{}'::jsonb, jsonb_build_object('input_file_id', $4::text), 3600, now() + interval '1 hour')")
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(release_id)
        .bind(scheduled)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO blueprint_connector_jobs (id, workspace_id, blueprint_id, direction, extension_id, operation_id, context_id, input_file_id) SELECT $1, $2, $3, 'import', 'acme.files', 'import', id, $4 FROM attribute_contexts WHERE workspace_id = $2 AND code = 'default'")
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(Uuid::new_v4())
        .bind(connector)
        .execute(&pool)
        .await
        .unwrap();
    insert_run_with_input_file(&pool, release_id, pending_run, "pending").await;
    insert_run_with_input_file(&pool, release_id, finished_run, "completed").await;

    worker(pool.clone(), store, 3, 60)
        .reconcile()
        .await
        .unwrap();

    for file_id in [scheduled, connector, pending_run] {
        assert_eq!(file_status(&pool, file_id).await, "queued");
    }
    assert_eq!(file_status(&pool, finished_run).await, "deleted");
}

#[sqlx::test]
async fn purge_keeps_objects_of_a_file_referenced_after_deletion_was_scheduled(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let file_id = Uuid::new_v4();
    let original_key = format!("files/{file_id}/original");
    insert_file(&pool, file_id, &original_key, false).await;
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
    let worker = worker(pool.clone(), store.clone(), 3, 0);
    worker.reconcile().await.unwrap();
    assert_eq!(file_status(&pool, file_id).await, "deleted");
    // A reference that reconciliation would honour also stops the purge job,
    // which re-checks the same predicate before deleting any object.
    let release_id = insert_extension_release(&pool).await;
    insert_run_with_input_file(&pool, release_id, file_id, "pending").await;
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
    assert_eq!(store.object_count().await, 1);
}
