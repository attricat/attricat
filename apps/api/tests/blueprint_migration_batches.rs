use api::{model::CreateBlueprint, repository::CatalogRepository};
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

#[sqlx::test]
async fn safe_batches_are_singleton_and_recover_expired_leases(pool: PgPool) {
    let repository = CatalogRepository::new(pool.clone());
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
    repository
        .create_blueprint_revision(
            source.blueprint.id,
            CreateBlueprint {
                // Revisions retain the same compiled schema but must have a
                // distinct source hash.
                definition: format!("{DEFINITION}\n# revision two\n"),
            },
        )
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(source.blueprint.id, 2)
        .await
        .unwrap();

    let first = repository
        .start_safe_blueprint_migration_batch(source.blueprint.id, 2)
        .await
        .unwrap();
    let retry = repository
        .start_safe_blueprint_migration_batch(source.blueprint.id, 2)
        .await
        .unwrap();
    assert_eq!(first.id, retry.id);
    assert_eq!(first.status, "queued");

    sqlx::query("UPDATE blueprint_migration_batches SET status = 'running', lease_until = now() - interval '1 second' WHERE id = $1")
        .bind(first.id)
        .execute(&pool)
        .await
        .unwrap();
    let recovered = repository
        .recover_safe_blueprint_migration_batches()
        .await
        .unwrap();
    assert!(recovered.iter().any(|(_, batch_id)| *batch_id == first.id));
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT status FROM blueprint_migration_batches WHERE id = $1",
        )
        .bind(first.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "queued"
    );

    // Holding the entity table after the first task claims the batch makes
    // the second task race the lease claim rather than the entity migration.
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE entities IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .unwrap();
    let first_runner = tokio::spawn({
        let repository = repository.clone();
        async move {
            repository
                .run_safe_blueprint_migration_batch(first.id)
                .await
                .unwrap();
        }
    });
    for _ in 0..50 {
        let status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM blueprint_migration_batches WHERE id = $1",
        )
        .bind(first.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        if status == "running" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT status FROM blueprint_migration_batches WHERE id = $1",
        )
        .bind(first.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "running"
    );
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        repository.run_safe_blueprint_migration_batch(first.id),
    )
    .await
    .expect("second worker should observe the active lease")
    .unwrap();
    lock.commit().await.unwrap();
    first_runner.await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT status FROM blueprint_migration_batches WHERE id = $1",
        )
        .bind(first.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "completed"
    );

    // A graceful process shutdown releases a claimed batch immediately rather
    // than making the next process wait for the abandoned-lease timeout.
    let lifecycle_batch = repository
        .start_safe_blueprint_migration_batch(source.blueprint.id, 2)
        .await
        .unwrap();
    let mut lifecycle_lock = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE entities IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lifecycle_lock)
        .await
        .unwrap();
    let (shutdown_sender, shutdown_receiver) = tokio::sync::watch::channel(());
    let (dispatcher, worker) =
        api::blueprint_migration_worker::start(repository.clone(), shutdown_receiver);
    dispatcher.enqueue(repository.clone(), lifecycle_batch.id);
    for _ in 0..50 {
        let status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM blueprint_migration_batches WHERE id = $1",
        )
        .bind(lifecycle_batch.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        if status == "running" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT status FROM blueprint_migration_batches WHERE id = $1",
        )
        .bind(lifecycle_batch.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "running"
    );
    shutdown_sender.send(()).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(1), worker)
        .await
        .expect("worker should stop after releasing its lease")
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT status FROM blueprint_migration_batches WHERE id = $1",
        )
        .bind(lifecycle_batch.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "queued"
    );
    lifecycle_lock.commit().await.unwrap();
}
