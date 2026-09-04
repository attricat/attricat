mod support;

use api::domain_events::CONTEXT_CREATED_V1;
use support::{StatusCode, Uuid, authenticated_client, start_server};

#[sqlx::test(migrations = "./migrations")]
async fn context_creation_commits_a_typed_outbox_event(pool: sqlx::PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let correlation_id = Uuid::new_v4();
    let response = authenticated_client()
        .post(format!("{base_url}/contexts"))
        .header("x-correlation-id", correlation_id.to_string())
        .json(&support::json!({"code": "evented", "data": {}}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let context = response.json::<support::Value>().await.unwrap();

    let event = sqlx::query_as::<_, (String, String, Uuid, Uuid, String, String, support::Value)>(
        "SELECT event_type, aggregate_kind, aggregate_id, correlation_id, source_kind, source_name, payload FROM domain_events",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(event.0, CONTEXT_CREATED_V1);
    assert_eq!(event.1, "context");
    assert_eq!(event.2.to_string(), context["id"].as_str().unwrap());
    assert_eq!(event.3, correlation_id);
    assert_eq!(event.4, "api");
    assert_eq!(event.5, "catalog_api");
    assert_eq!(event.6["code"], "evented");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM audit_events")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn outbox_insert_failure_rolls_back_the_catalog_mutation(pool: sqlx::PgPool) {
    sqlx::query(
        "ALTER TABLE domain_events ADD CONSTRAINT domain_events_test_reject CHECK (false) NOT VALID",
    )
    .execute(&pool)
    .await
    .unwrap();
    let (base_url, server) = start_server(pool.clone()).await;
    let response = authenticated_client()
        .post(format!("{base_url}/contexts"))
        .json(&support::json!({"code": "outbox_rollback", "data": {}}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM attribute_contexts WHERE code = 'outbox_rollback'",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM domain_events")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM audit_events")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn consumer_starts_at_the_current_workspace_watermark(pool: sqlx::PgPool) {
    let repository = api::repository::CatalogRepository::new(pool.clone());
    sqlx::query(
        "INSERT INTO domain_events (id, workspace_id, event_type, aggregate_kind, aggregate_id, correlation_id, source_kind, source_name, payload) VALUES ($1, '00000000-0000-4000-8000-000000000002', 'context.created.v1', 'context', $2, $3, 'api', 'catalog_api', '{}'::jsonb)",
    )
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await
    .unwrap();
    let consumer = repository
        .create_event_consumer("test.consumer")
        .await
        .unwrap();
    assert_eq!(consumer.watermark, 1);
}
