mod support;

use api::{http::StreamControl, repository::CatalogRepository, storage::FakeObjectStore};
use chrono::Utc;
use sqlx::postgres::PgPoolOptions;
use std::{sync::Arc, time::Duration};
use support::*;

#[sqlx::test]
async fn token_issuance_is_pool_safe_and_cannot_amplify_its_issuer(pool: PgPool) {
    let bounded = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(1))
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap();
    let (base, server) = start_server(bounded.clone()).await;
    let owner = authenticated_client();
    let expiry = Utc::now() + chrono::Duration::hours(1);
    let parent = owner.post(format!("{base}/personal-access-tokens"))
        .json(&json!({"label":"issuer", "permissions":["tokens.manage", "blueprints.read"], "expires_at":expiry}))
        .send().await.unwrap().error_for_status().unwrap().json::<Value>().await.unwrap();
    let secret = parent["secret"].as_str().unwrap();
    let client = Client::new();
    for permissions in [
        json!(["entities.write"]),
        json!(["tokens.manage", "roles.manage"]),
    ] {
        let response = client
            .post(format!("{base}/personal-access-tokens"))
            .bearer_auth(secret)
            .json(&json!({"label":"amplified", "permissions": permissions, "expires_at":expiry}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    for child_expiry in [Value::Null, json!(expiry + chrono::Duration::seconds(1))] {
        let response = client.post(format!("{base}/personal-access-tokens"))
            .bearer_auth(secret).json(&json!({"label":"outlives parent", "permissions":["blueprints.read"], "expires_at":child_expiry}))
            .send().await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    let response = client
        .post(format!("{base}/personal-access-tokens"))
        .bearer_auth(secret)
        .json(&json!({"label":"narrower", "permissions":["blueprints.read"], "expires_at":expiry}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    // Concurrency cannot produce nested-acquisition starvation even with one slot.
    let mut requests = tokio::task::JoinSet::new();
    for i in 0..8 {
        let owner = owner.clone();
        let base = base.clone();
        requests.spawn(async move {
            owner
                .post(format!("{base}/personal-access-tokens"))
                .json(&json!({"label":format!("parallel-{i}"),"permissions":["blueprints.read"]}))
                .send()
                .await
                .unwrap()
                .status()
        });
    }
    while let Some(result) = requests.join_next().await {
        assert_eq!(result.unwrap(), StatusCode::CREATED);
    }
    server.abort();
    bounded.close().await;
}

#[sqlx::test]
async fn token_sessions_recheck_principals_without_writing_usage_on_every_read(pool: PgPool) {
    let (base, server) = start_server(pool.clone()).await;
    let owner = authenticated_client();
    let token = owner
        .post(format!("{base}/personal-access-tokens"))
        .json(&json!({"label":"reader", "permissions":["blueprints.read"]}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let secret = token["secret"].as_str().unwrap();
    let id: Uuid = token["id"].as_str().unwrap().parse().unwrap();
    let client = Client::new();
    assert_eq!(
        client
            .get(format!("{base}/auth/session"))
            .bearer_auth(secret)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let first: chrono::DateTime<Utc> =
        sqlx::query_scalar("SELECT last_used_at FROM personal_api_tokens WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    for _ in 0..3 {
        assert_eq!(
            client
                .get(format!("{base}/auth/session"))
                .bearer_auth(secret)
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
    }
    let last: chrono::DateTime<Utc> =
        sqlx::query_scalar("SELECT last_used_at FROM personal_api_tokens WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(first, last);
    sqlx::query("UPDATE workspace_memberships SET state='inactive' WHERE user_id=$1")
        .bind(BOOTSTRAP_OWNER_ID.parse::<Uuid>().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        client
            .get(format!("{base}/auth/session"))
            .bearer_auth(secret)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    sqlx::query("UPDATE users SET state='inactive' WHERE id=$1")
        .bind(BOOTSTRAP_OWNER_ID.parse::<Uuid>().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        client
            .get(format!("{base}/auth/session"))
            .bearer_auth(secret)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    server.abort();
}

#[sqlx::test]
async fn probes_have_independent_admission_and_readiness_has_a_deadline(pool: PgPool) {
    let permits = Arc::new(tokio::sync::Semaphore::new(1));
    let held = permits.clone().acquire_owned().await.unwrap();
    let (base, server) =
        start_server_with_config(pool.clone(), |state| state.request_permits = permits).await;
    let client = Client::new();
    for path in ["/health", "/health/live"] {
        assert_eq!(
            client
                .get(format!("{base}{path}"))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
    }
    assert_eq!(
        client
            .get(format!("{base}/blueprints"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        client
            .get(format!("{base}/health/ready"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    drop(held);
    server.abort();

    let bounded = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(20))
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap();
    let (base, server) = start_server(bounded.clone()).await;
    let held = bounded.acquire().await.unwrap();
    let response = tokio::time::timeout(
        Duration::from_secs(4),
        client.get(format!("{base}/health/ready")).send(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        client
            .get(format!("{base}/health/live"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    drop(held);
    server.abort();
    bounded.close().await;
}

#[sqlx::test]
async fn cancelled_uploads_are_reconciled_and_failed_deletions_are_retried(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base, server) = start_server_with_config(pool.clone(), |state| {
        state.object_store = store.clone();
        state.request_timeout = Duration::from_millis(500);
    })
    .await;
    let owner = authenticated_client();
    let conversation = owner
        .post(format!("{base}/agent/conversations"))
        .json(&json!({"title":"cancelled upload"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    // Block persistence after S3 succeeds, exercising the actual HTTP timeout.
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE files IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lock)
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
                reqwest::multipart::Part::text("orphan candidate")
                    .file_name("file.txt")
                    .mime_str("text/plain")
                    .unwrap(),
            ),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    lock.rollback().await.unwrap();
    assert_eq!(store.object_count().await, 1);
    let repository = CatalogRepository::system(pool.clone());
    assert!(
        !api::maintenance::cleanup_upload_once(&repository, store.as_ref())
            .await
            .unwrap()
    );
    sqlx::query("UPDATE file_upload_intents SET cleanup_after=now()-interval '1 second'")
        .execute(&pool)
        .await
        .unwrap();
    store.set_available(false);
    assert!(
        api::maintenance::cleanup_upload_once(&repository, store.as_ref())
            .await
            .is_err()
    );
    let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM file_upload_intents")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(remaining, 1);
    store.set_available(true);
    sqlx::query("UPDATE file_upload_intents SET cleanup_after=now()-interval '1 second'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        api::maintenance::cleanup_upload_once(&repository, store.as_ref())
            .await
            .unwrap()
    );
    assert_eq!(store.object_count().await, 0);
    assert!(
        !api::maintenance::cleanup_upload_once(&repository, store.as_ref())
            .await
            .unwrap()
    );
    server.abort();
}

#[sqlx::test]
async fn event_streams_hold_admission_until_body_close_and_stop_on_shutdown(pool: PgPool) {
    let control = StreamControl::new(4, 1, Duration::from_secs(5));
    let (base, server) =
        start_server_with_config(pool.clone(), |state| state.stream_control = control.clone())
            .await;
    let owner = authenticated_client();
    let repository = CatalogRepository::new(pool.clone(), BOOTSTRAP_WORKSPACE_ID.parse().unwrap());
    let conversation = repository
        .create_conversation(Some(BOOTSTRAP_OWNER_ID.parse().unwrap()), "stream")
        .await
        .unwrap();
    let run_id = Uuid::new_v4();
    sqlx::query("INSERT INTO agent_runs (id,workspace_id,conversation_id,origin,status,provider_base_url,model) VALUES ($1,$2,$3,'interactive','running','http://example.test','test')")
        .bind(run_id).bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap()).bind(conversation.id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO agent_run_events (id,run_id,sequence,event_type,payload) VALUES ($1,$2,0,'status','{\"status\":\"running\"}')")
        .bind(Uuid::new_v4()).bind(run_id).execute(&pool).await.unwrap();
    let url = format!("{base}/agent/runs/{run_id}/events");
    let first = owner.get(&url).send().await.unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(
        owner.get(&url).send().await.unwrap().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    control.shutdown();
    tokio::time::timeout(Duration::from_secs(2), first.text())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        owner.get(&url).send().await.unwrap().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    server.abort();
}
