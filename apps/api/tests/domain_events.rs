mod support;

use api::domain_events::{
    BLUEPRINT_CREATED_V1, BLUEPRINT_PUBLISHED_V1, BLUEPRINT_REVISION_CREATED_V1,
    CONTEXT_CREATED_V1, CONTEXT_DELETED_V1, CONTEXT_UPDATED_V1,
};
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
async fn context_updates_and_deletes_emit_lifecycle_events(pool: sqlx::PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let contexts = client
        .get(format!("{base_url}/contexts"))
        .send()
        .await
        .unwrap()
        .json::<Vec<support::Value>>()
        .await
        .unwrap();
    let default_id = contexts
        .iter()
        .find(|context| context["code"] == "default")
        .unwrap()["id"]
        .clone();
    let context = client
        .post(format!("{base_url}/contexts"))
        .json(&support::json!({"code": "lifecycle", "data": {}}))
        .send()
        .await
        .unwrap()
        .json::<support::Value>()
        .await
        .unwrap();
    let id = context["id"].as_str().unwrap();
    client
        .put(format!("{base_url}/contexts/id/{id}"))
        .json(&support::json!({"parent_id": default_id, "data": {"changed": true}}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    client
        .delete(format!("{base_url}/contexts/id/{id}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let event_types =
        sqlx::query_scalar::<_, String>("SELECT event_type FROM domain_events ORDER BY sequence")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        event_types,
        vec![CONTEXT_CREATED_V1, CONTEXT_UPDATED_V1, CONTEXT_DELETED_V1]
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn blueprint_lifecycle_emits_versioned_events(pool: sqlx::PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let first = client.post(format!("{base_url}/blueprints"))
        .json(&support::json!({"definition": "format_version = 1\ncode = \"evented\"\nname = \"Evented\"\nkind = \"entity\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"title\"]\n\n[[attributes]]\ncode = \"title\"\nvalue_type = \"string\""}))
        .send().await.unwrap().error_for_status().unwrap().json::<support::Value>().await.unwrap();
    let id = first["blueprint"]["id"].as_str().unwrap();
    client.post(format!("{base_url}/blueprints/{id}/versions"))
        .json(&support::json!({"definition": "format_version = 1\ncode = \"evented\"\nname = \"Evented revision\"\nkind = \"entity\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"title\"]\n\n[[attributes]]\ncode = \"title\"\nvalue_type = \"string\""}))
        .send().await.unwrap().error_for_status().unwrap();
    client
        .post(format!("{base_url}/blueprints/{id}/versions/2/publish"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let event_types =
        sqlx::query_scalar::<_, String>("SELECT event_type FROM domain_events ORDER BY sequence")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        event_types,
        vec![
            BLUEPRINT_CREATED_V1,
            BLUEPRINT_REVISION_CREATED_V1,
            BLUEPRINT_PUBLISHED_V1
        ]
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

#[sqlx::test(migrations = "./migrations")]
async fn delivery_claims_are_exclusive_and_completion_is_durable(pool: sqlx::PgPool) {
    use std::time::Duration;

    let repository = api::repository::CatalogRepository::new(pool.clone());
    repository
        .ensure_event_consumer("test.dispatcher", &[CONTEXT_CREATED_V1])
        .await
        .unwrap();
    let event_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO domain_events (id, workspace_id, event_type, aggregate_kind, aggregate_id, correlation_id, source_kind, source_name, payload) VALUES ($1, '00000000-0000-4000-8000-000000000002', 'context.created.v1', 'context', $2, $3, 'api', 'catalog_api', '{}'::jsonb)",
    )
    .bind(event_id)
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await
    .unwrap();

    let first = repository.clone();
    let second = repository.clone();
    let (left, right) = tokio::join!(
        first.claim_event_delivery(
            "test.dispatcher",
            &[CONTEXT_CREATED_V1],
            "one",
            Duration::from_secs(30)
        ),
        second.claim_event_delivery(
            "test.dispatcher",
            &[CONTEXT_CREATED_V1],
            "two",
            Duration::from_secs(30)
        ),
    );
    let delivery = match (left.unwrap(), right.unwrap()) {
        (Some(delivery), None) | (None, Some(delivery)) => delivery,
        other => panic!("expected exactly one lease, got {other:?}"),
    };
    assert_eq!(delivery.event.id, event_id);
    repository.complete_event_delivery(&delivery).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM event_deliveries")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "completed"
    );
}
