mod support;

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use api::{
    domain_events::{
        BLUEPRINT_CREATED_V1, BLUEPRINT_PUBLISHED_V1, BLUEPRINT_REVISION_CREATED_V1,
        CONTEXT_CREATED_V1, CONTEXT_DELETED_V1, CONTEXT_UPDATED_V1,
    },
    event_dispatcher::{
        DispatcherConfig, EventHandler, EventHandlerCommandContext, EventHandlerRegistry,
    },
    model::CreateAttributeContext,
    repository::CatalogRepository,
};
use async_trait::async_trait;
use support::{StatusCode, Uuid, authenticated_client, start_server};

struct FollowOnContextHandler {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl EventHandler for FollowOnContextHandler {
    fn name(&self) -> &'static str {
        "test-follow-on-context"
    }

    fn event_types(&self) -> &'static [&'static str] {
        &[CONTEXT_CREATED_V1]
    }

    async fn handle(
        &self,
        event: api::domain_events::DomainEvent,
        context: EventHandlerCommandContext,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if event.source_kind == "worker" && event.source_name == self.name() {
            return Ok(());
        }
        self.calls.fetch_add(1, Ordering::SeqCst);
        context
            .repository()
            .create_context(CreateAttributeContext {
                code: "dispatcher_follow_on".to_owned(),
                data: support::json!({}),
                parent_id: None,
            })
            .await?;
        Ok(())
    }
}

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

#[sqlx::test(migrations = "./migrations")]
async fn expired_leases_redeliver_the_same_event_and_reject_stale_acknowledgements(
    pool: sqlx::PgPool,
) {
    let repository = CatalogRepository::new(pool.clone());
    repository
        .ensure_event_consumer("test.lease_expiry", &[CONTEXT_CREATED_V1])
        .await
        .unwrap();
    let event_id = Uuid::new_v4();
    sqlx::query("INSERT INTO domain_events (id, workspace_id, event_type, aggregate_kind, aggregate_id, correlation_id, source_kind, source_name, payload) VALUES ($1, '00000000-0000-4000-8000-000000000002', 'context.created.v1', 'context', $2, $3, 'api', 'catalog_api', '{}'::jsonb)")
        .bind(event_id).bind(Uuid::new_v4()).bind(Uuid::new_v4()).execute(&pool).await.unwrap();

    let first = repository
        .claim_event_delivery(
            "test.lease_expiry",
            &[CONTEXT_CREATED_V1],
            "crashed-replica",
            Duration::from_secs(30),
        )
        .await
        .unwrap()
        .unwrap();
    sqlx::query(
        "UPDATE event_deliveries SET lease_until = clock_timestamp() - interval '1 second'",
    )
    .execute(&pool)
    .await
    .unwrap();
    let retry = repository
        .claim_event_delivery(
            "test.lease_expiry",
            &[CONTEXT_CREATED_V1],
            "recovery-replica",
            Duration::from_secs(30),
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(retry.event.id, event_id);
    assert_eq!(
        retry.attempts, 2,
        "an expired lease is at-least-once delivery"
    );
    repository.complete_event_delivery(&first).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM event_deliveries")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "leased",
        "a crashed replica cannot acknowledge a later replica's lease"
    );
    repository.complete_event_delivery(&retry).await.unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn deliveries_can_complete_out_of_order_without_losing_the_deferred_event(
    pool: sqlx::PgPool,
) {
    let repository = CatalogRepository::new(pool.clone());
    repository
        .ensure_event_consumer("test.reordering", &[CONTEXT_CREATED_V1])
        .await
        .unwrap();
    for _ in 0..2 {
        sqlx::query("INSERT INTO domain_events (id, workspace_id, event_type, aggregate_kind, aggregate_id, correlation_id, source_kind, source_name, payload) VALUES ($1, '00000000-0000-4000-8000-000000000002', 'context.created.v1', 'context', $2, $3, 'api', 'catalog_api', '{}'::jsonb)")
            .bind(Uuid::new_v4()).bind(Uuid::new_v4()).bind(Uuid::new_v4()).execute(&pool).await.unwrap();
    }
    let deferred = repository
        .claim_event_delivery(
            "test.reordering",
            &[CONTEXT_CREATED_V1],
            "one",
            Duration::from_secs(30),
        )
        .await
        .unwrap()
        .unwrap();
    repository
        .retry_event_delivery(
            &deferred,
            "temporarily unavailable",
            Duration::from_secs(3600),
            3,
        )
        .await
        .unwrap();
    let later = repository
        .claim_event_delivery(
            "test.reordering",
            &[CONTEXT_CREATED_V1],
            "two",
            Duration::from_secs(30),
        )
        .await
        .unwrap()
        .unwrap();
    assert_ne!(deferred.event.id, later.event.id);
    repository.complete_event_delivery(&later).await.unwrap();
    let mut health = repository.event_delivery_health().await.unwrap();
    health.sort();
    assert_eq!(
        health,
        vec![("completed".to_owned(), 1), ("pending".to_owned(), 1)]
    );
    sqlx::query("UPDATE event_deliveries SET next_attempt_at = clock_timestamp() - interval '1 second' WHERE event_id = $1")
        .bind(deferred.event.id).execute(&pool).await.unwrap();
    let recovered = repository
        .claim_event_delivery(
            "test.reordering",
            &[CONTEXT_CREATED_V1],
            "three",
            Duration::from_secs(30),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recovered.event.id, deferred.event.id);
    repository
        .complete_event_delivery(&recovered)
        .await
        .unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn dispatcher_preserves_causal_lineage_and_suppresses_its_own_follow_on_event(
    pool: sqlx::PgPool,
) {
    let repository = CatalogRepository::new(pool.clone());
    repository
        .ensure_event_consumer("test-follow-on-context", &[CONTEXT_CREATED_V1])
        .await
        .unwrap();
    let event_id = Uuid::new_v4();
    let correlation_id = Uuid::new_v4();
    sqlx::query("INSERT INTO domain_events (id, workspace_id, event_type, aggregate_kind, aggregate_id, correlation_id, source_kind, source_name, payload) VALUES ($1, '00000000-0000-4000-8000-000000000002', 'context.created.v1', 'context', $2, $3, 'api', 'catalog_api', '{}'::jsonb)")
        .bind(event_id).bind(Uuid::new_v4()).bind(correlation_id).execute(&pool).await.unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let registry = EventHandlerRegistry::new(vec![Arc::new(FollowOnContextHandler {
        calls: calls.clone(),
    })])
    .unwrap();
    let (shutdown, receiver) = tokio::sync::watch::channel(());
    let handles = api::event_dispatcher::start(
        repository,
        registry,
        DispatcherConfig::new(
            Duration::from_millis(20),
            Duration::from_millis(5),
            Duration::from_millis(20),
            3,
            Duration::from_millis(5),
        )
        .unwrap(),
        receiver,
    );
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if calls.load(Ordering::SeqCst) == 1
                && sqlx::query_scalar::<_, i64>("SELECT count(*) FROM domain_events WHERE source_kind = 'worker' AND source_name = 'test-follow-on-context'")
                    .fetch_one(&pool).await.unwrap() == 1
                && sqlx::query_scalar::<_, String>("SELECT status FROM event_deliveries WHERE consumer_id = (SELECT id FROM event_consumers WHERE name = 'test-follow-on-context') AND event_id = $1")
                    .bind(event_id).fetch_one(&pool).await.unwrap() == "completed"
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    shutdown.send(()).unwrap();
    for handle in handles {
        handle.await.unwrap();
    }

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let follow_on: (Uuid, Uuid, String, String) = sqlx::query_as("SELECT correlation_id, causation_id, source_kind, source_name FROM domain_events WHERE source_kind = 'worker'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(follow_on.0, correlation_id);
    assert_eq!(follow_on.1, event_id);
    assert_eq!(follow_on.2, "worker");
    assert_eq!(follow_on.3, "test-follow-on-context");
}

#[sqlx::test(migrations = "./migrations")]
async fn failed_deliveries_wait_until_due_then_dead_letter(pool: sqlx::PgPool) {
    use std::time::Duration;

    let repository = api::repository::CatalogRepository::new(pool.clone());
    repository
        .ensure_event_consumer("test.retry", &[CONTEXT_CREATED_V1])
        .await
        .unwrap();
    sqlx::query("INSERT INTO domain_events (id, workspace_id, event_type, aggregate_kind, aggregate_id, correlation_id, source_kind, source_name, payload) VALUES ($1, '00000000-0000-4000-8000-000000000002', 'context.created.v1', 'context', $2, $3, 'api', 'catalog_api', '{}'::jsonb)")
        .bind(Uuid::new_v4()).bind(Uuid::new_v4()).bind(Uuid::new_v4()).execute(&pool).await.unwrap();
    let first = repository
        .claim_event_delivery(
            "test.retry",
            &[CONTEXT_CREATED_V1],
            "one",
            Duration::from_secs(30),
        )
        .await
        .unwrap()
        .unwrap();
    repository
        .retry_event_delivery(&first, "first failure", Duration::from_secs(3600), 2)
        .await
        .unwrap();
    assert!(
        repository
            .claim_event_delivery(
                "test.retry",
                &[CONTEXT_CREATED_V1],
                "two",
                Duration::from_secs(30)
            )
            .await
            .unwrap()
            .is_none()
    );
    sqlx::query(
        "UPDATE event_deliveries SET next_attempt_at = clock_timestamp() - interval '1 second'",
    )
    .execute(&pool)
    .await
    .unwrap();
    let second = repository
        .claim_event_delivery(
            "test.retry",
            &[CONTEXT_CREATED_V1],
            "two",
            Duration::from_secs(30),
        )
        .await
        .unwrap()
        .unwrap();
    repository
        .retry_event_delivery(&second, "second failure", Duration::from_secs(1), 2)
        .await
        .unwrap();
    let row: (String, i32, String) =
        sqlx::query_as("SELECT status, attempts, last_error FROM event_deliveries")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        row,
        ("dead_letter".to_owned(), 2, "second failure".to_owned())
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn dead_letter_can_be_replayed_without_changing_event(pool: sqlx::PgPool) {
    let repository = api::repository::CatalogRepository::new(pool.clone());
    let consumer = repository
        .create_event_consumer("test.replay")
        .await
        .unwrap();
    let event_id = Uuid::new_v4();
    sqlx::query("INSERT INTO domain_events (id, workspace_id, event_type, aggregate_kind, aggregate_id, correlation_id, source_kind, source_name, payload) VALUES ($1, '00000000-0000-4000-8000-000000000002', 'context.created.v1', 'context', $2, $3, 'api', 'catalog_api', '{}'::jsonb)")
        .bind(event_id).bind(Uuid::new_v4()).bind(Uuid::new_v4()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO event_deliveries (consumer_id, event_id, status, attempts, last_error, failed_at) VALUES ($1, $2, 'dead_letter', 5, 'boom', clock_timestamp())").bind(consumer.id).bind(event_id).execute(&pool).await.unwrap();
    assert_eq!(
        repository
            .list_failed_event_deliveries()
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        repository
            .replay_event_delivery(consumer.id, event_id)
            .await
            .unwrap()
    );
    let row: (String, i32, String) = sqlx::query_as("SELECT d.status, d.attempts, e.event_type FROM event_deliveries d JOIN domain_events e ON e.id = d.event_id WHERE d.consumer_id = $1 AND d.event_id = $2").bind(consumer.id).bind(event_id).fetch_one(&pool).await.unwrap();
    assert_eq!(
        row,
        ("pending".to_owned(), 5, "context.created.v1".to_owned())
    );
}
