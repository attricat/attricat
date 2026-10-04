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
async fn task_queue_health_reports_only_actionable_statuses_without_payloads(pool: PgPool) {
    let repository = CatalogRepository::system(pool.clone());
    let workspace_id = workspace(&pool, "tasks-health").await;
    let subject_id = Uuid::new_v4();
    let mut transaction = pool.begin().await.unwrap();
    let task_id = repository
        .enqueue_task(&mut transaction, insert(workspace_id, subject_id))
        .await
        .unwrap()
        .unwrap();
    transaction.commit().await.unwrap();

    let health = repository
        .task_queue_health(&[TaskKind::RuleRunV1])
        .await
        .unwrap();
    assert!(health.iter().any(|(kind, status, count, age, retries)| {
        kind == TaskKind::RuleRunV1.as_str()
            && status == "queued"
            && *count == 1
            && *age >= 0.0
            && *retries == 0
    }));

    sqlx::query("UPDATE tasks SET status='succeeded',completed_at=now() WHERE id=$1")
        .bind(task_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        repository
            .task_queue_health(&[TaskKind::RuleRunV1])
            .await
            .unwrap()
            .is_empty()
    );
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

#[sqlx::test]
async fn revoked_initiator_dead_letters_the_envelope_through_the_shared_helper(pool: PgPool) {
    let system = CatalogRepository::system(pool.clone());
    let workspace_id = workspace(&pool, "tasks-revoked-initiator").await;
    let release_id = Uuid::new_v4();
    sqlx::query("INSERT INTO installed_extension_releases (id, workspace_id, extension_id, version, manifest, manifest_sha256, source) VALUES ($1, $2, 'acme.interactive', '1.0.0', '{}'::jsonb, $3, 'side_load')")
        .bind(release_id)
        .bind(workspace_id)
        .bind("0".repeat(64))
        .execute(&pool)
        .await
        .unwrap();
    let run_id = Uuid::new_v4();
    sqlx::query("INSERT INTO extension_operation_runs (id, workspace_id, extension_id, installed_release_id, abi_version, operation_id, idempotency_key) VALUES ($1, $2, 'acme.interactive', $3, '1.6.0', 'label', $4)")
        .bind(run_id)
        .bind(workspace_id)
        .bind(release_id)
        .bind(run_id.to_string())
        .execute(&pool)
        .await
        .unwrap();
    let mut transaction = pool.begin().await.unwrap();
    system
        .enqueue_task(
            &mut transaction,
            TaskInsert {
                workspace_id,
                kind: TaskKind::ExtensionOperationRunV1,
                subject_id: run_id,
                generation: 0,
                payload: json!({"run_id": run_id.to_string()}),
                correlation_id: None,
                causation_id: None,
            },
        )
        .await
        .unwrap();
    transaction.commit().await.unwrap();
    let task = system
        .claim_task("worker", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    sqlx::query("UPDATE extension_operation_runs SET status = 'leased', lease_token = $2, lease_owner = 'worker', lease_until = now() + interval '30 seconds' WHERE id = $1")
        .bind(run_id)
        .bind(task.lease_token)
        .execute(&pool)
        .await
        .unwrap();

    CatalogRepository::new(pool.clone(), workspace_id)
        .for_extension_operation_task(&task)
        .fail_extension_operation_for_revoked_initiator(&task)
        .await
        .unwrap();

    // A terminal failure exhausts the budget rather than counting one attempt.
    let (status, failures, max_failures): (String, i32, i32) =
        sqlx::query_as("SELECT status, failures, max_failures FROM tasks WHERE id = $1")
            .bind(task.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "dead_letter");
    assert_eq!(failures, max_failures);
}

#[sqlx::test]
async fn in_transaction_holder_transitions_reject_a_lease_that_expired_mid_transaction(
    pool: PgPool,
) {
    let repository = CatalogRepository::system(pool.clone());
    let workspace_id = workspace(&pool, "tasks-in-transaction").await;
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

    let mut transaction = pool.begin().await.unwrap();
    // The lease expires after this transaction started, so `now()` alone
    // would still consider it live.
    sqlx::query("UPDATE tasks SET lease_until = clock_timestamp() + interval '50 milliseconds' WHERE id = $1")
        .bind(task.id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("SELECT 1")
        .execute(&mut *transaction)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        repository
            .dead_letter_task_in_transaction(&mut transaction, &task, "domain", "failed")
            .await
            .is_err()
    );
    assert!(
        repository
            .complete_task_in_transaction(&mut transaction, task.id, "worker", task.lease_token)
            .await
            .is_err()
    );
    transaction.rollback().await.unwrap();
}

#[sqlx::test]
async fn set_based_cancellation_obeys_kind_policy_and_skips_leased_work(pool: PgPool) {
    let repository = CatalogRepository::system(pool.clone());
    let workspace_id = workspace(&pool, "tasks-set-cancel").await;
    let (queued, leased) = (Uuid::new_v4(), Uuid::new_v4());
    let mut transaction = pool.begin().await.unwrap();
    for subject in [leased, queued] {
        repository
            .enqueue_task(&mut transaction, insert(workspace_id, subject))
            .await
            .unwrap();
    }
    transaction.commit().await.unwrap();
    sqlx::query("UPDATE tasks SET status = 'leased', lease_owner = 'worker', lease_token = gen_random_uuid(), lease_until = now() + interval '30 seconds' WHERE subject_id = $1")
        .bind(leased)
        .execute(&pool)
        .await
        .unwrap();

    let mut transaction = pool.begin().await.unwrap();
    assert!(
        repository
            .cancel_queued_tasks_for_subjects(
                &mut transaction,
                workspace_id,
                TaskKind::AgentRunV1,
                &[queued],
            )
            .await
            .is_err()
    );
    let cancelled = repository
        .cancel_queued_tasks_for_subjects(
            &mut transaction,
            workspace_id,
            TaskKind::RuleRunV1,
            &[queued, leased],
        )
        .await
        .unwrap();
    transaction.commit().await.unwrap();
    assert_eq!(cancelled, 1);
    let statuses: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT subject_id, status FROM tasks WHERE workspace_id = $1 ORDER BY status",
    )
    .bind(workspace_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        statuses,
        vec![(queued, "cancelled".into()), (leased, "leased".into())]
    );
}
