use api::{
    blueprint_migration_worker::{
        BlueprintMigrationBatchConfig, BlueprintMigrationBatchTaskHandler,
    },
    model::{CreateBlueprint, NewAttributeValue},
    repository::AttricatRepository,
    task_worker::TaskHandler,
};
use serde_json::json;
use sqlx::PgPool;

const DEFINITION: &str = r#"
format_version = 1
code = "batch_product"
name = "Batch product"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"
"#;

async fn batch_with_two_old_records(
    repository: &AttricatRepository,
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
    let version_one_record = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO records (id, blueprint_id, blueprint_version) VALUES ($1, $2, 1)")
        .bind(version_one_record)
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
    let version_two_record = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO records (id, blueprint_id, blueprint_version) VALUES ($1, $2, 2)")
        .bind(version_two_record)
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
    (batch.id, version_one_record, version_two_record)
}

#[sqlx::test]
async fn safe_batch_uses_the_nearest_published_ancestor_when_drafts_intervene(pool: PgPool) {
    let repository = AttricatRepository::new(
        pool,
        uuid::Uuid::from_u128(0x00000000000040008000000000000002),
    );
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
                definition: format!(
                    "{DEFINITION}\n# leave this revision as a draft\n\n[[attributes]]\ncode = \"draft_only\"\nvalue_type = \"string\"\n"
                ),
            },
        )
        .await
        .unwrap();
    repository
        .create_blueprint_revision(
            source.blueprint.id,
            CreateBlueprint {
                definition: format!("{DEFINITION}\n# publish after the draft\n"),
            },
        )
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(source.blueprint.id, 3)
        .await
        .unwrap();

    let impact = repository
        .safe_blueprint_migration_impact(source.blueprint.id, 3)
        .await
        .unwrap();
    assert_eq!(impact.eligible_records, 0);
    assert!(impact.removed_attribute_codes.is_empty());

    let batch = repository
        .start_safe_blueprint_migration_batch(source.blueprint.id, 3)
        .await
        .unwrap();
    assert_eq!(batch.target_version, 3);
}

#[sqlx::test]
async fn safe_batch_rejects_an_unsafe_older_source_revision(pool: PgPool) {
    let repository = AttricatRepository::new(
        pool.clone(),
        uuid::Uuid::from_u128(0x00000000000040008000000000000002),
    );
    let source = repository
        .create_blueprint(CreateBlueprint {
            definition: format!(
                "{DEFINITION}\n[[attributes]]\ncode = \"legacy\"\nvalue_type = \"string\"\n"
            ),
        })
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(source.blueprint.id, 1)
        .await
        .unwrap();
    let old_record = repository
        .create_record_with_values(
            source.blueprint.id,
            1,
            vec![NewAttributeValue::Scalar {
                attribute_id: None,
                attribute_code: Some("legacy".to_owned()),
                context_id: None,
                value: json!("keep unless approved"),
            }],
            Vec::new(),
            json!({}),
        )
        .await
        .unwrap();
    repository
        .create_blueprint_revision(
            source.blueprint.id,
            CreateBlueprint {
                definition: DEFINITION.to_owned(),
            },
        )
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(source.blueprint.id, 2)
        .await
        .unwrap();
    repository
        .create_blueprint_revision(
            source.blueprint.id,
            CreateBlueprint {
                definition: format!("{DEFINITION}\n# safe relative to revision two\n"),
            },
        )
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(source.blueprint.id, 3)
        .await
        .unwrap();
    let impact = repository
        .safe_blueprint_migration_impact(source.blueprint.id, 3)
        .await
        .unwrap();
    assert_eq!(impact.removed_attribute_codes, vec!["legacy"]);
    assert!(impact.requires_removal_disposition);
    let result = repository
        .start_safe_blueprint_migration_batch(source.blueprint.id, 3)
        .await;
    assert!(matches!(
        result,
        Err(api::repository::RepositoryError::BlueprintMigrationNotSafe)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT blueprint_version FROM records WHERE id=$1")
            .bind(old_record.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[sqlx::test]
async fn safe_batch_task_is_transactional_and_migrates_each_record_once(pool: PgPool) {
    let repository = AttricatRepository::new(
        pool.clone(),
        uuid::Uuid::from_u128(0x00000000000040008000000000000002),
    );
    let (batch_id, version_one_record, version_two_record) =
        batch_with_two_old_records(&repository, &pool).await;
    sqlx::query("UPDATE records SET created_at = '2026-01-01 00:00:00+00' WHERE id IN ($1, $2)")
        .bind(version_one_record)
        .bind(version_two_record)
        .execute(&pool)
        .await
        .unwrap();
    let expected_order = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT id FROM records WHERE id IN ($1, $2) ORDER BY created_at DESC, id DESC",
    )
    .bind(version_one_record)
    .bind(version_two_record)
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
        sqlx::query_scalar::<_, i64>("SELECT blueprint_version FROM records WHERE id IN ($1, $2)")
            .bind(version_one_record)
            .bind(version_two_record)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(versions, vec![3, 3]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM record_blueprint_migrations WHERE batch_id = $1",
        )
        .bind(batch_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        2
    );
    let migrated_order = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT aggregate_id FROM domain_events WHERE event_type = 'record.migrated.v1' ORDER BY sequence",
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
    let repository = AttricatRepository::new(
        pool.clone(),
        uuid::Uuid::from_u128(0x00000000000040008000000000000002),
    );
    let (batch_id, version_one_record, version_two_record) =
        batch_with_two_old_records(&repository, &pool).await;
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
            "SELECT count(*) FROM records WHERE id IN ($1, $2) AND blueprint_version = 3",
        )
        .bind(version_one_record)
        .bind(version_two_record)
        .fetch_one(&pool)
        .await
        .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM record_blueprint_migrations WHERE batch_id = $1 AND status = 'migrated'",
        )
        .bind(batch_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        2
    );
}

#[sqlx::test]
async fn batch_restart_from_newest_edge_finds_record_inserted_between_pages(pool: PgPool) {
    let repository = AttricatRepository::new(
        pool.clone(),
        uuid::Uuid::from_u128(0x00000000000040008000000000000002),
    );
    let (batch_id, _, _) = batch_with_two_old_records(&repository, &pool).await;
    let newest_record: uuid::Uuid = sqlx::query_scalar(
        "SELECT id FROM records WHERE blueprint_version < 3 ORDER BY created_at DESC, id DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let (blueprint_id, source_version): (uuid::Uuid, i64) =
        sqlx::query_as("SELECT blueprint_id, blueprint_version FROM records WHERE id = $1")
            .bind(newest_record)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query(
        "INSERT INTO record_blueprint_migrations (id, batch_id, record_id, blueprint_id, source_version, target_version, status, issues, task_owned) VALUES ($1, $2, $3, $4, $5, 3, 'pending', '[]'::jsonb, true)",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(batch_id)
    .bind(newest_record)
    .bind(blueprint_id)
    .bind(source_version)
    .execute(&pool)
    .await
    .unwrap();
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM records WHERE id = $1 FOR UPDATE")
        .bind(newest_record)
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
            "SELECT EXISTS (SELECT 1 FROM record_blueprint_migrations WHERE batch_id = $1 AND record_id = $2 AND status = 'migrating')",
        )
        .bind(batch_id)
        .bind(newest_record)
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
    let inserted_record = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO records (id, blueprint_id, blueprint_version, created_at) VALUES ($1, $2, 1, '2100-01-01 00:00:00+00')",
    )
    .bind(inserted_record)
    .bind(blueprint_id)
    .execute(&pool)
    .await
    .unwrap();
    blocker.commit().await.unwrap();
    running.await.unwrap().unwrap();

    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT blueprint_version FROM records WHERE id = $1")
            .bind(inserted_record)
            .fetch_one(&pool)
            .await
            .unwrap(),
        3
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM record_blueprint_migrations WHERE batch_id = $1",
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
    let repository = AttricatRepository::new(
        pool.clone(),
        uuid::Uuid::from_u128(0x00000000000040008000000000000002),
    );
    let (batch_id, _, _) = batch_with_two_old_records(&repository, &pool).await;
    let first = repository
        .claim_task("worker-a", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    let handler = BlueprintMigrationBatchTaskHandler::new(repository.clone());

    // Simulate a crash after the domain batch committed but before generic task
    // acknowledgement. Reclaiming must not create a second batch/record row.
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
            "SELECT count(*) FROM record_blueprint_migrations WHERE batch_id = $1",
        )
        .bind(batch_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        2,
        "the unique batch/record reservation survives a reclaimed task"
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

#[sqlx::test]
async fn safe_batch_archives_explicitly_approved_removed_values(pool: PgPool) {
    let repository = AttricatRepository::new(
        pool.clone(),
        uuid::Uuid::from_u128(0x00000000000040008000000000000002),
    );
    let source = repository
        .create_blueprint(CreateBlueprint {
            definition: format!(
                "{DEFINITION}\n[[attributes]]\ncode = \"obsolete\"\nvalue_type = \"string\"\n"
            ),
        })
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(source.blueprint.id, 1)
        .await
        .unwrap();
    let empty = repository
        .create_record_with_values(source.blueprint.id, 1, Vec::new(), Vec::new(), json!({}))
        .await
        .unwrap();
    let populated = repository
        .create_record_with_values(
            source.blueprint.id,
            1,
            vec![NewAttributeValue::Scalar {
                attribute_id: None,
                attribute_code: Some("obsolete".to_owned()),
                context_id: None,
                value: json!("archive me"),
            }],
            Vec::new(),
            json!({}),
        )
        .await
        .unwrap();
    repository
        .create_blueprint_revision(
            source.blueprint.id,
            CreateBlueprint {
                definition: format!("{DEFINITION}\n# obsolete removed\n"),
            },
        )
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(source.blueprint.id, 2)
        .await
        .unwrap();

    let impact = repository
        .safe_blueprint_migration_impact(source.blueprint.id, 2)
        .await
        .unwrap();
    assert_eq!(impact.removed_attribute_codes, vec!["obsolete"]);
    assert_eq!(impact.records_with_removed_values, 1);
    assert_eq!(impact.removed_values, 1);
    assert!(impact.requires_removal_disposition);
    assert!(
        repository
            .start_safe_blueprint_migration_batch(source.blueprint.id, 2)
            .await
            .is_err()
    );

    let batch = repository
        .start_safe_blueprint_migration_batch_with_removal_disposition(
            source.blueprint.id,
            2,
            Some("archive"),
        )
        .await
        .unwrap();
    let task = repository
        .claim_task("removal-worker", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    BlueprintMigrationBatchTaskHandler::new(repository.clone())
        .handle(task)
        .await
        .unwrap();

    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM records WHERE id IN ($1, $2) AND blueprint_version = 2",
        )
        .bind(empty.id)
        .bind(populated.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM attribute_value_history WHERE record_id = $1",
        )
        .bind(populated.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT removal_policy FROM blueprint_migration_batches WHERE id = $1",
        )
        .bind(batch.id)
        .fetch_one(&pool)
        .await
        .unwrap()["attribute_codes"],
        json!(["obsolete"])
    );
}
