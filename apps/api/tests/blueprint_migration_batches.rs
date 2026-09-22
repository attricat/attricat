use api::{
    blueprint_migration_worker::{
        BlueprintMigrationBatchConfig, BlueprintMigrationBatchTaskHandler,
    },
    model::CreateBlueprint,
    repository::CatalogRepository,
    task_worker::TaskHandler,
};
use sqlx::PgPool;

const DEFINITION: &str = r#"
format_version = 1
code = "batch_product"
name = "Batch product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"
"#;

async fn batch_with_two_old_entities(
    repository: &CatalogRepository,
    pool: &PgPool,
) -> (uuid::Uuid, uuid::Uuid, uuid::Uuid) {
    let source = repository
        .create_blueprint(CreateBlueprint {
            definition: DEFINITION.to_owned(),
        })
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(source.blueprint.id, 1)
        .await
        .unwrap();
    let version_one_entity = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO entities (id, blueprint_id, blueprint_version) VALUES ($1, $2, 1)")
        .bind(version_one_entity)
        .bind(source.blueprint.id)
        .execute(pool)
        .await
        .unwrap();
    repository
        .create_blueprint_revision(
            source.blueprint.id,
            CreateBlueprint {
                definition: format!("{DEFINITION}\n# revision two\n"),
            },
        )
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(source.blueprint.id, 2)
        .await
        .unwrap();
    let version_two_entity = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO entities (id, blueprint_id, blueprint_version) VALUES ($1, $2, 2)")
        .bind(version_two_entity)
        .bind(source.blueprint.id)
        .execute(pool)
        .await
        .unwrap();
    repository
        .create_blueprint_revision(
            source.blueprint.id,
            CreateBlueprint {
                definition: format!("{DEFINITION}\n# revision three\n"),
            },
        )
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(source.blueprint.id, 3)
        .await
        .unwrap();
    let batch = repository
        .start_safe_blueprint_migration_batch(source.blueprint.id, 3)
        .await
        .unwrap();
    (batch.id, version_one_entity, version_two_entity)
}

#[sqlx::test]
async fn safe_batch_task_is_transactional_and_migrates_each_entity_once(pool: PgPool) {
    let repository = CatalogRepository::system(pool.clone());
    let (batch_id, version_one_entity, version_two_entity) =
        batch_with_two_old_entities(&repository, &pool).await;
    sqlx::query("UPDATE entities SET created_at = '2026-01-01 00:00:00+00' WHERE id IN ($1, $2)")
        .bind(version_one_entity)
        .bind(version_two_entity)
        .execute(&pool)
        .await
        .unwrap();
    let expected_order = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT id FROM entities WHERE id IN ($1, $2) ORDER BY created_at DESC, id DESC",
    )
    .bind(version_one_entity)
    .bind(version_two_entity)
    .fetch_all(&pool)
    .await
    .unwrap();
    let task = repository
        .claim_task("worker", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .expect("batch creation atomically enqueues its task");
    assert_eq!(task.subject_id, batch_id);
    let handler = BlueprintMigrationBatchTaskHandler::with_config(
        repository.clone(),
        BlueprintMigrationBatchConfig {
            page_size: 1,
            concurrency: 2,
        },
    );
    handler.handle(task.clone()).await.unwrap();
    repository
        .complete_task(task.id, "worker", task.lease_token)
        .await
        .unwrap();

    let versions =
        sqlx::query_scalar::<_, i64>("SELECT blueprint_version FROM entities WHERE id IN ($1, $2)")
            .bind(version_one_entity)
            .bind(version_two_entity)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(versions, vec![3, 3]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM entity_blueprint_migrations WHERE batch_id = $1",
        )
        .bind(batch_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        2
    );
    let migrated_order = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT aggregate_id FROM domain_events WHERE event_type = 'entity.migrated.v1' ORDER BY sequence",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        migrated_order, expected_order,
        "equal timestamps use ID-desc keyset order"
    );
}

#[sqlx::test]
async fn safe_batch_processes_multiple_scalar_candidates_in_one_concurrent_page(pool: PgPool) {
    let repository = CatalogRepository::system(pool.clone());
    let (batch_id, version_one_entity, version_two_entity) =
        batch_with_two_old_entities(&repository, &pool).await;
    let task = repository
        .claim_task("concurrent-worker", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .expect("batch creation enqueues its task");
    let handler = BlueprintMigrationBatchTaskHandler::with_config(
        repository.clone(),
        BlueprintMigrationBatchConfig {
            page_size: 2,
            concurrency: 2,
        },
    );
    handler.handle(task).await.unwrap();

    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM entities WHERE id IN ($1, $2) AND blueprint_version = 3",
        )
        .bind(version_one_entity)
        .bind(version_two_entity)
        .fetch_one(&pool)
        .await
        .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM entity_blueprint_migrations WHERE batch_id = $1 AND status = 'migrated'",
        )
        .bind(batch_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        2
    );
}

#[sqlx::test]
async fn batch_restart_from_newest_edge_finds_entity_inserted_between_pages(pool: PgPool) {
    let repository = CatalogRepository::system(pool.clone());
    let (batch_id, _, _) = batch_with_two_old_entities(&repository, &pool).await;
    let newest_entity: uuid::Uuid = sqlx::query_scalar(
        "SELECT id FROM entities WHERE blueprint_version < 3 ORDER BY created_at DESC, id DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let (blueprint_id, source_version): (uuid::Uuid, i64) =
        sqlx::query_as("SELECT blueprint_id, blueprint_version FROM entities WHERE id = $1")
            .bind(newest_entity)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query(
        "INSERT INTO entity_blueprint_migrations (id, batch_id, entity_id, blueprint_id, source_version, target_version, status, issues, task_owned) VALUES ($1, $2, $3, $4, $5, 3, 'pending', '[]'::jsonb, true)",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(batch_id)
    .bind(newest_entity)
    .bind(blueprint_id)
    .bind(source_version)
    .execute(&pool)
    .await
    .unwrap();
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM entities WHERE id = $1 FOR UPDATE")
        .bind(newest_entity)
        .execute(&mut *blocker)
        .await
        .unwrap();
    let task = repository
        .claim_task("boundary-worker", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    let handler = BlueprintMigrationBatchTaskHandler::with_config(
        repository.clone(),
        BlueprintMigrationBatchConfig {
            page_size: 1,
            concurrency: 1,
        },
    );
    let running = tokio::spawn(async move { handler.handle(task).await });
    let mut first_page_started = false;
    for _ in 0..500 {
        let migrating = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM entity_blueprint_migrations WHERE batch_id = $1 AND entity_id = $2 AND status = 'migrating')",
        )
        .bind(batch_id)
        .bind(newest_entity)
        .fetch_one(&pool)
        .await
        .unwrap();
        if migrating {
            first_page_started = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(first_page_started, "the first candidate reached migration");
    let inserted_entity = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO entities (id, blueprint_id, blueprint_version, created_at) VALUES ($1, $2, 1, '2100-01-01 00:00:00+00')",
    )
    .bind(inserted_entity)
    .bind(blueprint_id)
    .execute(&pool)
    .await
    .unwrap();
    blocker.commit().await.unwrap();
    running.await.unwrap().unwrap();

    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT blueprint_version FROM entities WHERE id = $1")
            .bind(inserted_entity)
            .fetch_one(&pool)
            .await
            .unwrap(),
        3
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM entity_blueprint_migrations WHERE batch_id = $1",
        )
        .bind(batch_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        3
    );
}

#[sqlx::test]
async fn expired_batch_task_cannot_checkpoint_and_reclaim_reuses_migration_rows(pool: PgPool) {
    let repository = CatalogRepository::system(pool.clone());
    let (batch_id, _, _) = batch_with_two_old_entities(&repository, &pool).await;
    let first = repository
        .claim_task("worker-a", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    let handler = BlueprintMigrationBatchTaskHandler::new(repository.clone());

    // Simulate a crash after the domain batch committed but before generic task
    // acknowledgement. Reclaiming must not create a second batch/entity row.
    handler.handle(first.clone()).await.unwrap();
    sqlx::query("UPDATE tasks SET lease_until = now() - interval '1 second' WHERE id = $1")
        .bind(first.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        repository
            .complete_task(first.id, "worker-a", first.lease_token)
            .await
            .is_err()
    );
    assert!(
        handler.handle(first.clone()).await.is_err(),
        "an expired task must not checkpoint the batch"
    );
    let second = repository
        .claim_task("worker-b", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .expect("expired task is reclaimable");
    assert_ne!(first.lease_token, second.lease_token);
    handler.handle(second.clone()).await.unwrap();
    repository
        .complete_task(second.id, "worker-b", second.lease_token)
        .await
        .unwrap();

    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM entity_blueprint_migrations WHERE batch_id = $1",
        )
        .bind(batch_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        2,
        "the unique batch/entity reservation survives a reclaimed task"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT status FROM blueprint_migration_batches WHERE id = $1",
        )
        .bind(batch_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "completed"
    );
}
