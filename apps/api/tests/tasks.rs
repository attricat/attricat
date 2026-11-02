use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use api::{
    repository::{AttricatRepository, ClaimedTask},
    task_queue::{TaskInsert, TaskKind, TaskStatus},
    task_worker::{
        self, TaskHandler, TaskHandlerError, TaskHandlerRegistry, TaskOutcome, TaskWorkerConfig,
    },
};
use chrono::{Duration as ChronoDuration, Utc};
use serde_json::json;
use sqlx::PgPool;
use tokio::sync::watch;
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

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn task_enqueue_is_transactional_and_deduplicated(pool: PgPool) {
    let repository = AttricatRepository::system(pool.clone());
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

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn task_queue_health_reports_only_actionable_statuses_without_payloads(pool: PgPool) {
    let repository = AttricatRepository::system(pool.clone());
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

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn queued_cancellation_and_dead_letter_replay_obey_kind_policy(pool: PgPool) {
    let repository = AttricatRepository::system(pool.clone());
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

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn task_lease_is_exclusive_reclaimable_and_token_fenced(pool: PgPool) {
    let repository = AttricatRepository::system(pool.clone());
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

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn retry_policy_dead_letters_at_the_registered_failure_budget(pool: PgPool) {
    let repository = AttricatRepository::system(pool.clone());
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

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn expired_lease_rejects_every_holder_transition_and_subseconds_round_up(pool: PgPool) {
    let repository = AttricatRepository::system(pool.clone());
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

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn concurrent_workers_claim_distinct_live_tasks(pool: PgPool) {
    let repository = AttricatRepository::system(pool.clone());
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

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn continuation_preserves_failure_budget_and_fairness_rotates_workspaces(pool: PgPool) {
    let repository = AttricatRepository::system(pool.clone());
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

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn oversized_retry_error_is_truncated_and_consumes_failure_budget(pool: PgPool) {
    let repository = AttricatRepository::system(pool.clone());
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
    let end = message.floor_char_boundary(1024);
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

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn revoked_initiator_dead_letters_the_envelope_through_the_shared_helper(pool: PgPool) {
    let system = AttricatRepository::system(pool.clone());
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

    AttricatRepository::new(pool.clone(), workspace_id)
        .for_task(&task)
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

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn in_transaction_holder_transitions_reject_a_lease_that_expired_mid_transaction(
    pool: PgPool,
) {
    let repository = AttricatRepository::system(pool.clone());
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

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn set_based_cancellation_obeys_kind_policy_and_skips_leased_work(pool: PgPool) {
    let repository = AttricatRepository::system(pool.clone());
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

/// A handler whose calls are counted, and which panics when told to.
struct ScriptedHandler {
    handled: Arc<AtomicUsize>,
    panics: bool,
    settles: bool,
}

#[async_trait::async_trait]
impl TaskHandler for ScriptedHandler {
    fn kind(&self) -> TaskKind {
        TaskKind::RuleRunV1
    }

    async fn handle(&self, _: ClaimedTask) -> Result<TaskOutcome, TaskHandlerError> {
        self.handled.fetch_add(1, Ordering::SeqCst);
        assert!(!self.panics, "scripted panic");
        Ok(TaskOutcome::Complete)
    }

    async fn on_exhausted(&self, _: &ClaimedTask, _: &str) -> Result<bool, TaskHandlerError> {
        if self.settles {
            Ok(false)
        } else {
            Err(TaskHandlerError {
                code: "settle",
                message: "the domain row could not be settled".into(),
            })
        }
    }
}

/// Runs the worker with `handler` until `task_id` satisfies `done`, returning its
/// status, failures and last error code.
async fn run_until(
    pool: &PgPool,
    handler: ScriptedHandler,
    task_id: Uuid,
    done: impl Fn(&str, i32) -> bool,
) -> (String, i32, Option<String>) {
    let repository = AttricatRepository::system(pool.clone());
    let (shutdown, receiver) = watch::channel(());
    let worker = task_worker::start(
        repository,
        TaskHandlerRegistry::new(vec![Arc::new(handler)]).unwrap(),
        TaskWorkerConfig {
            worker_id: "task-failure-test".into(),
            concurrency: 1,
            poll_interval: Duration::from_millis(10),
            shutdown_grace: Duration::from_secs(1),
        },
        receiver,
    );
    let row = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let row: (String, i32, Option<String>) =
                sqlx::query_as("SELECT status, failures, last_error_code FROM tasks WHERE id = $1")
                    .bind(task_id)
                    .fetch_one(pool)
                    .await
                    .unwrap();
            if done(&row.0, row.1) {
                return row;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("task did not reach the expected state");
    shutdown.send(()).unwrap();
    worker.await.unwrap().unwrap();
    row
}

/// A task of `RuleRunV1` already leased by a worker that died.
async fn abandoned_task(pool: &PgPool, slug: &str, failures: i32) -> Uuid {
    abandoned_task_of_kind(pool, slug, "rule_run.v1", failures).await
}

async fn abandoned_task_of_kind(pool: &PgPool, slug: &str, kind: &str, failures: i32) -> Uuid {
    let workspace_id = workspace(pool, slug).await;
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO task_workspace_service (workspace_id) VALUES ($1) ON CONFLICT DO NOTHING",
    )
    .bind(workspace_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO tasks (id, workspace_id, kind, subject_id, generation, envelope_version, max_failures, failures, status, lease_owner, lease_token, lease_until, payload)
         VALUES ($1, $2, $5, $3, 0, 1, 5, $4, 'leased', 'dead-worker', $1, now() - interval '1 minute', '{}')",
    )
    .bind(id)
    .bind(workspace_id)
    .bind(Uuid::new_v4())
    .bind(failures)
    .bind(kind)
    .execute(pool)
    .await
    .unwrap();
    id
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_panicking_handler_spends_the_failure_budget(pool: PgPool) {
    let workspace_id = workspace(&pool, "tasks-panic").await;
    let repository = AttricatRepository::system(pool.clone());
    let mut transaction = pool.begin().await.unwrap();
    let task_id = repository
        .enqueue_task(&mut transaction, insert(workspace_id, Uuid::new_v4()))
        .await
        .unwrap()
        .unwrap();
    transaction.commit().await.unwrap();
    let handled = Arc::new(AtomicUsize::new(0));
    let (status, failures, code) = run_until(
        &pool,
        ScriptedHandler {
            handled: handled.clone(),
            panics: true,
            settles: true,
        },
        task_id,
        |_, failures| failures >= 1,
    )
    .await;
    assert_eq!(status, "queued");
    assert_eq!(failures, 1);
    assert_eq!(code.as_deref(), Some("handler_panic"));
    assert_eq!(handled.load(Ordering::SeqCst), 1);
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_task_whose_leases_keep_expiring_is_dead_lettered_without_running(pool: PgPool) {
    let task_id = abandoned_task(&pool, "tasks-crash-loop", 4).await;
    let handled = Arc::new(AtomicUsize::new(0));
    let (_, failures, code) = run_until(
        &pool,
        ScriptedHandler {
            handled: handled.clone(),
            panics: false,
            settles: true,
        },
        task_id,
        |status, _| status == "dead_letter",
    )
    .await;
    assert_eq!(failures, 5);
    assert_eq!(code.as_deref(), Some("lease_expired"));
    assert_eq!(handled.load(Ordering::SeqCst), 0);
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn reclaiming_an_expired_lease_spends_one_failure_and_runs_the_task(pool: PgPool) {
    let task_id = abandoned_task(&pool, "tasks-reclaim", 0).await;
    let handled = Arc::new(AtomicUsize::new(0));
    let (status, failures, _) = run_until(
        &pool,
        ScriptedHandler {
            handled: handled.clone(),
            panics: false,
            settles: true,
        },
        task_id,
        |status, _| status == "succeeded",
    )
    .await;
    assert_eq!(status, "succeeded");
    assert_eq!(failures, 1);
    assert_eq!(handled.load(Ordering::SeqCst), 1);
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_task_that_cannot_settle_its_domain_row_is_not_dead_lettered(pool: PgPool) {
    let task_id = abandoned_task(&pool, "tasks-unsettled", 4).await;
    let (status, _, _) = run_until(
        &pool,
        ScriptedHandler {
            handled: Arc::new(AtomicUsize::new(0)),
            panics: false,
            settles: false,
        },
        task_id,
        |status, _| status == "queued",
    )
    .await;
    assert_eq!(status, "queued");
    let available_later: bool =
        sqlx::query_scalar("SELECT available_at > now() FROM tasks WHERE id = $1")
            .bind(task_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(available_later);
}

/// An event delivery counts each handler's attempts itself, and quarantining the
/// extension that keeps ending the process needs more than the envelope's budget.
#[sqlx::test(migrator = "api::MIGRATOR")]
async fn reclaiming_an_event_delivery_spends_no_failure(pool: PgPool) {
    let task_id = abandoned_task_of_kind(&pool, "tasks-delivery", "event_delivery.v1", 4).await;
    let repository = AttricatRepository::system(pool.clone());
    let task = repository
        .claim_task_for_kinds(
            "reclaimer",
            Duration::from_secs(30),
            &[TaskKind::EventDeliveryV1],
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(task.id, task_id);
    assert_eq!(task.failures, 4);
}
