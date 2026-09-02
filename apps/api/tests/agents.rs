mod support;

use std::{sync::Arc, time::Duration};

use api::{
    file_worker::{FileWorker, WorkerConfig},
    storage::FakeObjectStore,
};
use support::*;

#[sqlx::test]
async fn agent_conversation_reads_and_persisted_sse_replay(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let conversation: Value = client
        .post(format!("{base_url}/agent/conversations"))
        .json(&json!({"title": "nightly catalog review"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let conversation_id = conversation["id"].as_str().unwrap();
    let conversations: Value = client
        .get(format!("{base_url}/agent/conversations"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(conversations[0]["id"], conversation["id"]);

    let message_id = Uuid::new_v4();
    let file_id = Uuid::new_v4();
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    sqlx::query("INSERT INTO files (id, workspace_id, original_filename, display_filename, mime_type, byte_size, sha256, original_key, status) VALUES ($1, $2, 'report.pdf', 'report.pdf', 'application/pdf', 7, $3, 'files/report.pdf', 'ready')")
        .bind(file_id).bind(workspace_id).bind("0".repeat(64)).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO conversation_messages (id, conversation_id, sequence, role, content) VALUES ($1, $2, 0, 'user', '\"Review this report\"')")
        .bind(message_id).bind(conversation_id.parse::<Uuid>().unwrap()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO conversation_message_attachments (id, workspace_id, message_id, file_id, position) VALUES ($1, $2, $3, $4, 0)")
        .bind(Uuid::new_v4()).bind(workspace_id).bind(message_id).bind(file_id).execute(&pool).await.unwrap();
    let messages: Value = client
        .get(format!(
            "{base_url}/agent/conversations/{conversation_id}/messages"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(messages[0]["attachments"][0]["id"], file_id.to_string());
    assert_eq!(messages[0]["attachments"][0]["filename"], "report.pdf");

    let run_id = Uuid::new_v4();
    sqlx::query("INSERT INTO agent_runs (id, workspace_id, conversation_id, origin, status, provider_base_url, model, finished_at) VALUES ($1, $2, $3, 'manual', 'skipped', 'https://provider.test/v1', 'test', now())")
        .bind(run_id).bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap()).bind(conversation_id.parse::<Uuid>().unwrap()).execute(&pool).await.unwrap();
    let event_id = Uuid::new_v4();
    sqlx::query("INSERT INTO agent_run_events (id, run_id, sequence, event_type, payload) VALUES ($1, $2, 0, 'terminal', '{\"status\":\"skipped\"}')")
        .bind(event_id).bind(run_id).execute(&pool).await.unwrap();
    let response = client
        .get(format!("{base_url}/agent/runs/{run_id}/events"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let replay = response.text().await.unwrap();
    assert!(replay.contains(&format!("id: {event_id}")));
    assert!(replay.contains("event: terminal"));
    server.abort();
}

#[sqlx::test]
async fn reconciliation_retains_conversation_attachments(pool: PgPool) {
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let conversation_id = Uuid::new_v4();
    let message_id = Uuid::new_v4();
    let attached_file_id = Uuid::new_v4();
    let orphan_file_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO conversations (id, workspace_id, title) VALUES ($1, $2, 'Attachment retention')",
    )
    .bind(conversation_id)
    .bind(workspace_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO conversation_messages (id, conversation_id, sequence, role, content) VALUES ($1, $2, 0, 'user', '\"Keep this file\"')")
        .bind(message_id)
        .bind(conversation_id)
        .execute(&pool)
        .await
        .unwrap();
    for file_id in [attached_file_id, orphan_file_id] {
        sqlx::query("INSERT INTO files (id, workspace_id, original_filename, display_filename, mime_type, byte_size, sha256, original_key, status) VALUES ($1, $2, 'report.pdf', 'report.pdf', 'application/pdf', 7, $3, 'files/report.pdf', 'ready')")
            .bind(file_id)
            .bind(workspace_id)
            .bind("0".repeat(64))
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO conversation_message_attachments (id, workspace_id, message_id, file_id, position) VALUES ($1, $2, $3, $4, 0)")
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(message_id)
        .bind(attached_file_id)
        .execute(&pool)
        .await
        .unwrap();

    let worker = FileWorker::new(
        pool.clone(),
        Arc::new(FakeObjectStore::available()),
        WorkerConfig {
            worker_id: "test-worker".into(),
            max_pixels: 1,
            max_attempts: 1,
            delete_grace: Duration::from_secs(60),
        },
    );
    worker.reconcile().await.unwrap();

    let attached_deleted_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT deleted_at FROM files WHERE id = $1")
            .bind(attached_file_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let orphan_deleted_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT deleted_at FROM files WHERE id = $1")
            .bind(orphan_file_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(attached_deleted_at.is_none());
    assert!(orphan_deleted_at.is_some());
}

#[sqlx::test]
async fn agent_message_requires_a_configured_dispatcher(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let conversation: Value = client
        .post(format!("{base_url}/agent/conversations"))
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let response = client
        .post(format!(
            "{base_url}/agent/conversations/{}/messages",
            conversation["id"].as_str().unwrap()
        ))
        .json(&json!({"content":"hello"}))
        .send()
        .await
        .unwrap();
    let status = response.status();
    let response: Value = response.json().await.unwrap();
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{response}");
    assert_eq!(response["error"]["code"], "service_unavailable");
    server.abort();
}
