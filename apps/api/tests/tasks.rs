use std::time::Duration;

use api::{
    repository::CatalogRepository,
    task_queue::{TaskInsert, TaskKind, TaskStatus},
};
use chrono::{Duration as ChronoDuration, Utc};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

async fn workspace(pool: &PgPool, slug: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO workspaces (id, slug, name, login_identifier) VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(slug)
    .bind(slug)
    .bind(format!("{slug}.local"))
    .execute(pool)
    .await
    .unwrap();
    id
}

fn insert(workspace_id: Uuid, subject_id: Uuid) -> TaskInsert {
    TaskInsert {
        workspace_id,
        kind: TaskKind::RuleRunV1,
        subject_id,
        generation: 0,
        payload: json!({"rule_run_id": subject_id.to_string(), "version": 1}),
        correlation_id: None,
        causation_id: None,
    }
}

#[sqlx::test]
async fn task_enqueue_is_transactional_and_deduplicated(pool: PgPool) {
    let repository = CatalogRepository::system(pool.clone());
    let workspace_id = workspace(&pool, "tasks-rollback").await;
    let subject_id = Uuid::new_v4();
    let mut transaction = pool.begin().await.unwrap();
    assert!(
        repository
            .enqueue_task(&mut transaction, insert(workspace_id, subject_id))
            .await
            .unwrap()
            .is_some()
    );
    transaction.rollback().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM tasks")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );

    let mut transaction = pool.begin().await.unwrap();
    assert!(
        repository
            .enqueue_task(&mut transaction, insert(workspace_id, subject_id))
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        repository
            .enqueue_task(&mut transaction, insert(workspace_id, subject_id))
            .await
            .unwrap()
            .is_none()
    );
    transaction.commit().await.unwrap();
}

#[sqlx::test]
async fn queued_cancellation_and_dead_letter_replay_obey_kind_policy(pool: PgPool) {
    let repository = CatalogRepository::system(pool.clone());
    let workspace_id = workspace(&pool, "tasks-replay").await;
    let subject_id = Uuid::new_v4();
    let mut transaction = pool.begin().await.unwrap();
    let task_id = repository
        .enqueue_task(&mut transaction, insert(workspace_id, subject_id))
        .await
        .unwrap()
        .unwrap();
    repository
        .cancel_queued_task(&mut transaction, task_id)
        .await
        .unwrap();
    transaction.commit().await.unwrap();
    assert_eq!(
        repository
            .task_summary(task_id)
            .await
            .unwrap()
            .unwrap()
            .status,
        "cancelled"
    );

    let mut transaction = pool.begin().await.unwrap();
    let dead_letter_id = repository
        .enqueue_task(
            &mut transaction,
            TaskInsert {
                generation: 1,
                ..insert(workspace_id, subject_id)
            },
        )
        .await
        .unwrap()
        .unwrap();
    transaction.commit().await.unwrap();
    sqlx::query("UPDATE tasks SET status = 'dead_letter', failed_at = now() WHERE id = $1")
        .bind(dead_letter_id)
        .execute(&pool)
        .await
        .unwrap();
    let mut transaction = pool.begin().await.unwrap();
    let replay_id = repository
        .replay_task(&mut transaction, dead_letter_id)
        .await
        .unwrap()
        .unwrap();
    transaction.commit().await.unwrap();
    assert_eq!(
        repository
            .task_summary(replay_id)
            .await
            .unwrap()
            .unwrap()
            .generation,
        2
    );
    assert!(
        repository
            .task_summary_for_workspace(Uuid::new_v4(), replay_id)
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test]
async fn task_lease_is_exclusive_reclaimable_and_token_fenced(pool: PgPool) {
    let repository = CatalogRepository::system(pool.clone());
    let workspace_id = workspace(&pool, "tasks-lease").await;
    let mut transaction = pool.begin().await.unwrap();
    repository
        .enqueue_task(&mut transaction, insert(workspace_id, Uuid::new_v4()))
        .await
        .unwrap();
    transaction.commit().await.unwrap();

    let first = repository
        .claim_task("worker-a", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert!(
        repository
            .claim_task("worker-b", Duration::from_secs(30))
            .await
            .unwrap()
            .is_none()
    );
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
            .complete_task(first.id, "worker-a", first.lease_token)
            .await
            .is_err()
    );
    repository
        .complete_task(second.id, "worker-b", second.lease_token)
        .await
        .unwrap();
}

#[sqlx::test]
async fn retry_policy_dead_letters_at_the_registered_failure_budget(pool: PgPool) {
    let repository = CatalogRepository::system(pool.clone());
    let workspace_id = workspace(&pool, "tasks-retry-budget").await;
    let mut transaction = pool.begin().await.unwrap();
    repository
        .enqueue_task(&mut transaction, insert(workspace_id, Uuid::new_v4()))
        .await
        .unwrap();
    transaction.commit().await.unwrap();

    for failure in 1..=TaskKind::RuleRunV1.policy().max_failures {
        let task = repository
            .claim_task("worker", Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        let status = repository
            .retry_task_at(
                task.id,
                "worker",
                task.lease_token,
                Utc::now() - ChronoDuration::seconds(1),
                "temporary",
                "temporary error",
            )
            .await
            .unwrap();
        assert_eq!(
            status,
            if failure == TaskKind::RuleRunV1.policy().max_failures {
                TaskStatus::DeadLetter
            } else {
                TaskStatus::Queued
            }
        );
    }
}

#[sqlx::test]
async fn expired_lease_rejects_every_holder_transition_and_subseconds_round_up(pool: PgPool) {
    let repository = CatalogRepository::system(pool.clone());
    let workspace_id = workspace(&pool, "tasks-expired").await;
    let mut transaction = pool.begin().await.unwrap();
    repository
        .enqueue_task(&mut transaction, insert(workspace_id, Uuid::new_v4()))
        .await
        .unwrap();
    transaction.commit().await.unwrap();
    // `available_at` is database-clock based; avoid an immediately-following
    // transaction observing an equal timestamp differently under parallel SQLx tests.
    tokio::time::sleep(Duration::from_millis(10)).await;

    let task = repository
        .claim_task("worker", Duration::from_millis(1))
        .await
        .unwrap()
        .unwrap();
    assert!(task.lease_until > Utc::now() + ChronoDuration::milliseconds(500));
    repository
        .heartbeat_task(
            task.id,
            "worker",
            task.lease_token,
            Duration::from_millis(1),
        )
        .await
        .unwrap();
    sqlx::query("UPDATE tasks SET lease_until = now() - interval '1 second' WHERE id = $1")
        .bind(task.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        repository
            .complete_task(task.id, "worker", task.lease_token)
            .await
            .is_err()
    );
    assert!(
        repository
            .reschedule_task_at(task.id, "worker", task.lease_token, Utc::now())
            .await
            .is_err()
    );
    assert!(
        repository
            .retry_task_at(
                task.id,
                "worker",
                task.lease_token,
                Utc::now(),
                "temporary",
                "error"
            )
            .await
            .is_err()
    );
    assert!(
        repository
            .heartbeat_task(task.id, "worker", task.lease_token, Duration::from_secs(1))
            .await
            .is_err()
    );
}

#[sqlx::test]
async fn concurrent_workers_claim_distinct_live_tasks(pool: PgPool) {
    let repository = CatalogRepository::system(pool.clone());
    let first_workspace = workspace(&pool, "tasks-concurrent-a").await;
    let second_workspace = workspace(&pool, "tasks-concurrent-b").await;
    let mut transaction = pool.begin().await.unwrap();
    repository
        .enqueue_task(&mut transaction, insert(first_workspace, Uuid::new_v4()))
        .await
        .unwrap();
    repository
        .enqueue_task(&mut transaction, insert(second_workspace, Uuid::new_v4()))
        .await
        .unwrap();
    transaction.commit().await.unwrap();

    let (first, second) = tokio::join!(
        repository.claim_task("worker-a", Duration::from_secs(30)),
        repository.claim_task("worker-b", Duration::from_secs(30))
    );
    let first = first.unwrap().unwrap();
    let second = second.unwrap().unwrap();
    assert_ne!(first.id, second.id);
    assert_ne!(first.lease_token, second.lease_token);
}

#[sqlx::test]
async fn continuation_preserves_failure_budget_and_fairness_rotates_workspaces(pool: PgPool) {
    let repository = CatalogRepository::system(pool.clone());
    let first_workspace = workspace(&pool, "tasks-fair-a").await;
    let second_workspace = workspace(&pool, "tasks-fair-b").await;
    let mut transaction = pool.begin().await.unwrap();
    repository
        .enqueue_task(&mut transaction, insert(first_workspace, Uuid::new_v4()))
        .await
        .unwrap();
    repository
        .enqueue_task(&mut transaction, insert(second_workspace, Uuid::new_v4()))
        .await
        .unwrap();
    transaction.commit().await.unwrap();

    let first = repository
        .claim_task("worker", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    repository
        .reschedule_task_at(
            first.id,
            "worker",
            first.lease_token,
            Utc::now() - ChronoDuration::seconds(1),
        )
        .await
        .unwrap();
    let second = repository
        .claim_task("worker", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert_ne!(
        first.workspace_id, second.workspace_id,
        "a busy workspace must yield to another ready workspace"
    );
    assert_eq!(
        repository
            .task_summary(first.id)
            .await
            .unwrap()
            .unwrap()
            .failures,
        0
    );

    assert_eq!(
        repository
            .retry_task_at(
                second.id,
                "worker",
                second.lease_token,
                Utc::now(),
                "temporary",
                "temporary error"
            )
            .await
            .unwrap(),
        TaskStatus::Queued
    );
    assert_eq!(
        repository
            .task_summary(second.id)
            .await
            .unwrap()
            .unwrap()
            .failures,
        1
    );
}

#[sqlx::test]
async fn oversized_retry_error_is_truncated_and_consumes_failure_budget(pool: PgPool) {
    let repository = CatalogRepository::system(pool.clone());
    let workspace_id = workspace(&pool, "tasks-oversized-error").await;
    let mut transaction = pool.begin().await.unwrap();
    repository
        .enqueue_task(&mut transaction, insert(workspace_id, Uuid::new_v4()))
        .await
        .unwrap();
    transaction.commit().await.unwrap();

    let task = repository
        .claim_task("worker", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    // The worker bounds this before calling the repository; keep the repository
    // validation strict while proving the bounded transition consumes a retry.
    let message = "é".repeat(600);
    let mut end = 1024;
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    let status = repository
        .retry_task_at(
            task.id,
            "worker",
            task.lease_token,
            Utc::now(),
            "handler",
            &message[..end],
        )
        .await
        .unwrap();
    assert_eq!(status, TaskStatus::Queued);
    assert_eq!(
        repository
            .task_summary(task.id)
            .await
            .unwrap()
            .unwrap()
            .failures,
        1
    );
}
