use std::time::Duration;

use api::{domain_events::CONTEXT_CREATED_V1, repository::CatalogRepository, task_queue::TaskKind};
use sqlx::PgPool;
use uuid::Uuid;

async fn insert_event(pool: &PgPool, event_type: &str, correlation_id: Uuid) -> Uuid {
    let event_id = Uuid::new_v4();
    sqlx::query("INSERT INTO domain_events (id, workspace_id, event_type, aggregate_kind, aggregate_id, correlation_id, source_kind, source_name, payload) VALUES ($1, '00000000-0000-4000-8000-000000000002', $2, 'context', $3, $4, 'api', 'catalog_api', '{}'::jsonb)")
        .bind(event_id).bind(event_type).bind(Uuid::new_v4()).bind(correlation_id)
        .execute(pool).await.unwrap();
    event_id
}

#[sqlx::test(migrations = "./migrations")]
async fn event_delivery_materialization_is_atomic_filtered_and_deduplicated(pool: PgPool) {
    let repository = CatalogRepository::system(pool.clone());
    repository
        .ensure_event_consumer("catalog.extensions.wasm", &[CONTEXT_CREATED_V1])
        .await
        .unwrap();
    let correlation_id = Uuid::new_v4();
    let event_id = insert_event(&pool, CONTEXT_CREATED_V1, correlation_id).await;
    insert_event(&pool, "context.updated.v1", Uuid::new_v4()).await;

    repository
        .materialize_event_delivery_tasks("catalog.extensions.wasm", &[CONTEXT_CREATED_V1])
        .await
        .unwrap();
    repository
        .materialize_event_delivery_tasks("catalog.extensions.wasm", &[CONTEXT_CREATED_V1])
        .await
        .unwrap();

    let row: (Uuid, Uuid, Uuid, Uuid, String, Uuid) = sqlx::query_as(
        "SELECT d.id, d.task_id, d.event_id, t.subject_id, t.kind, t.correlation_id FROM event_deliveries d JOIN tasks t ON t.id = d.task_id",
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(row.0, row.3, "the delivery ID is the stable task subject");
    assert_eq!(row.2, event_id);
    assert_eq!(row.4, TaskKind::EventDeliveryV1.as_str());
    assert_eq!(row.5, correlation_id);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM tasks")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT watermark FROM event_consumers WHERE name = 'catalog.extensions.wasm'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        2
    );

    sqlx::query("UPDATE tasks SET status = 'dead_letter', failures = max_failures, failed_at = now() WHERE id = $1")
        .bind(row.1)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        repository
            .replay_event_delivery(
                sqlx::query_scalar("SELECT consumer_id FROM event_deliveries")
                    .fetch_one(&pool)
                    .await
                    .unwrap(),
                event_id,
            )
            .await
            .unwrap()
    );
    let replay: (Uuid, Uuid, i32, String) = sqlx::query_as(
        "SELECT d.id, t.subject_id, t.generation, t.status FROM event_deliveries d JOIN tasks t ON t.id = d.task_id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(replay.0, row.0);
    assert_eq!(replay.1, row.0);
    assert_eq!(replay.2, 1);
    assert_eq!(replay.3, "queued");
}

#[sqlx::test(migrations = "./migrations")]
async fn materialization_is_idempotent_across_coordinator_restart(pool: PgPool) {
    let repository = CatalogRepository::system(pool.clone());
    repository
        .ensure_event_consumer("catalog.extensions.wasm", &[CONTEXT_CREATED_V1])
        .await
        .unwrap();
    let event_id = insert_event(&pool, CONTEXT_CREATED_V1, Uuid::new_v4()).await;
    repository
        .materialize_event_delivery_tasks("catalog.extensions.wasm", &[CONTEXT_CREATED_V1])
        .await
        .unwrap();

    // A replacement coordinator gets a fresh repository handle but must retain
    // the durable consumer, receipt, and task instead of delivering twice.
    let restarted = CatalogRepository::system(pool.clone());
    restarted
        .ensure_event_consumer("catalog.extensions.wasm", &[CONTEXT_CREATED_V1])
        .await
        .unwrap();
    restarted
        .materialize_event_delivery_tasks("catalog.extensions.wasm", &[CONTEXT_CREATED_V1])
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM event_deliveries")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, Uuid>("SELECT event_id FROM event_deliveries")
            .fetch_one(&pool)
            .await
            .unwrap(),
        event_id
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM tasks")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn newly_eligible_plugin_event_is_backfilled_after_watermark_advanced(pool: PgPool) {
    let repository = CatalogRepository::system(pool.clone());
    repository
        .ensure_event_consumer("catalog.extensions.wasm", &[CONTEXT_CREATED_V1])
        .await
        .unwrap();

    insert_event(&pool, "context.updated.v1", Uuid::new_v4()).await;
    repository
        .materialize_event_delivery_tasks("catalog.extensions.wasm", &[CONTEXT_CREATED_V1])
        .await
        .unwrap();
    let plugin_event = insert_event(
        &pool,
        "plugin.acme.producer.inventory_changed.v1",
        Uuid::new_v4(),
    )
    .await;
    repository
        .materialize_event_delivery_tasks("catalog.extensions.wasm", &[CONTEXT_CREATED_V1])
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT watermark FROM event_consumers WHERE name = 'catalog.extensions.wasm'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        2
    );

    repository
        .materialize_event_delivery_tasks(
            "catalog.extensions.wasm",
            &["plugin.acme.producer.inventory_changed.v1"],
        )
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, Uuid>("SELECT event_id FROM event_deliveries")
            .fetch_one(&pool)
            .await
            .unwrap(),
        plugin_event
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn expired_event_task_lease_rejects_stale_receipts_and_redelivers(pool: PgPool) {
    let repository = CatalogRepository::system(pool.clone());
    repository
        .ensure_event_consumer("catalog.extensions.wasm", &[CONTEXT_CREATED_V1])
        .await
        .unwrap();
    let event_id = insert_event(&pool, CONTEXT_CREATED_V1, Uuid::new_v4()).await;
    repository
        .materialize_event_delivery_tasks("catalog.extensions.wasm", &[CONTEXT_CREATED_V1])
        .await
        .unwrap();

    let first = repository
        .claim_task("worker-a", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    let first_delivery = repository
        .begin_task_event_delivery(&first)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first_delivery.event.id, event_id);
    sqlx::query("UPDATE tasks SET lease_until = now() - interval '1 second' WHERE id = $1")
        .bind(first.id)
        .execute(&pool)
        .await
        .unwrap();
    let second = repository
        .claim_task("worker-b", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert_ne!(first.lease_token, second.lease_token);
    assert!(
        repository
            .complete_task_event_delivery(&first)
            .await
            .is_err()
    );
    let second_delivery = repository
        .begin_task_event_delivery(&second)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(second_delivery.delivery_id, first_delivery.delivery_id);
    repository
        .complete_task_event_delivery(&second)
        .await
        .unwrap();
    repository
        .complete_task(second.id, &second.lease_owner, second.lease_token)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM event_deliveries")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "completed"
    );
}
