use api::{
    blueprint_migration_worker::BlueprintMigrationBatchTaskHandler, model::CreateBlueprint,
    repository::CatalogRepository, task_worker::TaskHandler,
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
    let repository = CatalogRepository::new(pool.clone());
    let (batch_id, version_one_entity, version_two_entity) =
        batch_with_two_old_entities(&repository, &pool).await;
    let task = repository
        .claim_task("worker", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .expect("batch creation atomically enqueues its task");
    assert_eq!(task.subject_id, batch_id);
    let handler = BlueprintMigrationBatchTaskHandler::new(repository.clone());
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
}

#[sqlx::test]
async fn expired_batch_task_cannot_checkpoint_and_reclaim_reuses_migration_rows(pool: PgPool) {
    let repository = CatalogRepository::new(pool.clone());
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
