mod support;

use api::{http::StreamControl, repository::AttricatRepository, storage::FakeObjectStore};
use std::{sync::Arc, time::Duration};
use support::*;

#[sqlx::test]
async fn history_cleanup_is_batched_concurrent_and_preserves_current_and_recent_values(
    pool: PgPool,
) {
    let (base, server) = start_server(pool.clone()).await;
    let owner = authenticated_client();
    let blueprint = create_blueprint(
        &owner,
        &base,
        r#"
format_version = 1
code = "retention_test"
name = "Retention"
kind = "record"
[[attributes]]
code = "title"
value_type = "string"
default_value = "Current"
[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
"#,
    )
    .await;
    let record = create_record(&owner, &base, &blueprint).await;
    let id: Uuid = record["id"].as_str().unwrap().parse().unwrap();
    sqlx::query("INSERT INTO attribute_value_history (id,workspace_id,record_id,attribute_id,context_id,active,value_text,created_at,archived_at) SELECT gen_random_uuid(),v.workspace_id,v.record_id,v.attribute_id,v.context_id,false,'Old',now()-interval '100 days',now()-interval '91 days' FROM attribute_values v CROSS JOIN generate_series(1,1505) WHERE v.record_id=$1")
        .bind(id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO attribute_value_history (id,workspace_id,record_id,attribute_id,context_id,active,value_text,created_at,archived_at) SELECT gen_random_uuid(),v.workspace_id,v.record_id,v.attribute_id,v.context_id,false,'Recent',now(),now() FROM attribute_values v WHERE v.record_id=$1")
        .bind(id).execute(&pool).await.unwrap();
    let repository = AttricatRepository::system(pool.clone());
    let retention = "90".parse().unwrap();
    let (a, b) = tokio::join!(
        repository.purge_value_history_batch(retention),
        repository.purge_value_history_batch(retention)
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert!(a <= 1000 && b <= 1000);
    assert_eq!(a + b, 1505);
    assert_eq!(
        repository
            .purge_value_history_batch(retention)
            .await
            .unwrap(),
        0
    );
    let recent: Vec<String> =
        sqlx::query_scalar("SELECT value_text FROM attribute_value_history WHERE record_id=$1")
            .bind(id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(recent, vec!["Recent"]);
    let current: String =
        sqlx::query_scalar("SELECT value_text FROM attribute_values WHERE record_id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(current, "Current");
    server.abort();
}

#[sqlx::test]
async fn successful_uploads_consume_intents_atomically_and_are_not_cleaned(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let owner = authenticated_client();
    let conversation = owner
        .post(format!("{base}/agent/conversations"))
        .json(&json!({"title":"durable upload"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let response = owner
        .post(format!(
            "{base}/agent/conversations/{}/uploads",
            conversation["id"].as_str().unwrap()
        ))
        .multipart(
            reqwest::multipart::Form::new().part(
                "file",
                reqwest::multipart::Part::text("committed")
                    .file_name("file.txt")
                    .mime_str("text/plain")
                    .unwrap(),
            ),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let intents: i64 = sqlx::query_scalar("SELECT count(*) FROM file_upload_intents")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(intents, 0);
    assert!(
        !api::maintenance::cleanup_upload_once(
            &AttricatRepository::system(pool.clone()),
            store.as_ref()
        )
        .await
        .unwrap()
    );
    assert_eq!(store.object_count().await, 1);
    server.abort();
}

#[sqlx::test]
async fn event_replay_drains_all_pages_through_terminal_and_supports_terminal_reconnect(
    pool: PgPool,
) {
    let (base, server) = start_server(pool.clone()).await;
    let repository = AttricatRepository::new(pool.clone(), BOOTSTRAP_WORKSPACE_ID.parse().unwrap());
    let conversation = repository
        .create_conversation(Some(BOOTSTRAP_OWNER_ID.parse().unwrap()), "replay")
        .await
        .unwrap();
    let run = Uuid::new_v4();
    sqlx::query("INSERT INTO agent_runs (id,workspace_id,conversation_id,origin,status,provider_base_url,model,finished_at) VALUES ($1,$2,$3,'interactive','completed','http://example.test','test',now())")
        .bind(run).bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap()).bind(conversation.id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO agent_run_events (id,run_id,sequence,event_type,payload) SELECT gen_random_uuid(),$1,n,'message_delta','{}'::jsonb FROM generate_series(0,299) n")
        .bind(run).execute(&pool).await.unwrap();
    let terminal = Uuid::new_v4();
    sqlx::query("INSERT INTO agent_run_events (id,run_id,sequence,event_type,payload) VALUES ($1,$2,300,'terminal','{\"status\":\"completed\"}')")
        .bind(terminal).bind(run).execute(&pool).await.unwrap();
    assert_eq!(
        repository
            .agent_run_events_after(run, -1)
            .await
            .unwrap()
            .len(),
        128
    );
    let owner = authenticated_client();
    let url = format!("{base}/agent/runs/{run}/events");
    let replay = owner
        .get(&url)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .text()
        .await
        .unwrap();
    assert_eq!(replay.matches("event: message_delta").count(), 300);
    assert_eq!(replay.matches("event: terminal").count(), 1);
    assert!(replay.contains(&terminal.to_string()));
    let replay = tokio::time::timeout(Duration::from_secs(2), async {
        owner
            .get(&url)
            .header("last-event-id", terminal.to_string())
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap()
    })
    .await
    .unwrap();
    assert!(!replay.contains("event:"));
    server.abort();
}

#[sqlx::test]
async fn event_stream_deadline_releases_the_body_permit(pool: PgPool) {
    let control = StreamControl::new(1, 1, Duration::from_millis(200));
    let (base, server) =
        start_server_with_config(pool.clone(), |state| state.stream_control = control).await;
    let repo = AttricatRepository::new(pool.clone(), BOOTSTRAP_WORKSPACE_ID.parse().unwrap());
    let conversation = repo
        .create_conversation(Some(BOOTSTRAP_OWNER_ID.parse().unwrap()), "deadline")
        .await
        .unwrap();
    let run = Uuid::new_v4();
    sqlx::query("INSERT INTO agent_runs (id,workspace_id,conversation_id,origin,status,provider_base_url,model) VALUES ($1,$2,$3,'interactive','awaiting_approval','http://example.test','test')")
        .bind(run).bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap()).bind(conversation.id).execute(&pool).await.unwrap();
    let owner = authenticated_client();
    for _ in 0..2 {
        let response = tokio::time::timeout(Duration::from_secs(2), async {
            owner
                .get(format!("{base}/agent/runs/{run}/events"))
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap()
                .text()
                .await
                .unwrap()
        })
        .await
        .unwrap();
        assert!(!response.contains("event:"));
    }
    server.abort();
}
