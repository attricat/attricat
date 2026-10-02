mod support;

use api::repository::{CatalogRepository, NewUploadedFile};
use chrono::Utc;
use support::*;

#[sqlx::test]
async fn cleanup_fences_late_finalization_and_stale_cleaner_acknowledgements(pool: PgPool) {
    let system = CatalogRepository::system(pool.clone());
    let scoped = system
        .for_workspace(BOOTSTRAP_WORKSPACE_ID.parse().unwrap())
        .await
        .unwrap();
    let conversation = scoped
        .create_conversation(None, "intent fence")
        .await
        .unwrap();
    let key = format!("files/{}", Uuid::new_v4());
    scoped
        .begin_file_uploads(
            std::slice::from_ref(&key),
            Utc::now() + chrono::Duration::hours(1),
        )
        .await
        .unwrap();
    sqlx::query("UPDATE file_upload_intents SET cleanup_after=now()-interval '1 second' WHERE object_key=$1").bind(&key).execute(&pool).await.unwrap();
    let first = system.claim_abandoned_upload().await.unwrap().unwrap();
    assert!(system.claim_abandoned_upload().await.unwrap().is_none());
    let file = NewUploadedFile {
        original_filename: "late.txt".into(),
        display_filename: "late.txt".into(),
        mime_type: "text/plain".into(),
        byte_size: 1,
        sha256: "0".repeat(64),
        object_key: key.clone(),
    };
    assert!(matches!(
        scoped
            .persist_conversation_uploads(conversation.id, Uuid::new_v4(), vec![file])
            .await,
        Err(api::repository::RepositoryError::NotFound(
            "active file upload"
        ))
    ));
    let files: i64 = sqlx::query_scalar("SELECT count(*) FROM files WHERE original_key=$1")
        .bind(&key)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(files, 0);
    sqlx::query("UPDATE file_upload_intents SET cleanup_after=now()-interval '1 second' WHERE object_key=$1").bind(&key).execute(&pool).await.unwrap();
    let second = system.claim_abandoned_upload().await.unwrap().unwrap();
    assert_ne!(first.lease_token, second.lease_token);
    system.complete_abandoned_upload(&first).await.unwrap();
    let intents: i64 =
        sqlx::query_scalar("SELECT count(*) FROM file_upload_intents WHERE object_key=$1")
            .bind(&key)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(intents, 1);
    system.complete_abandoned_upload(&second).await.unwrap();
    let intents: i64 =
        sqlx::query_scalar("SELECT count(*) FROM file_upload_intents WHERE object_key=$1")
            .bind(&key)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(intents, 0);
}
