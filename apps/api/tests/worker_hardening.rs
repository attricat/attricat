mod support;

use api::{
    repository::{AttricatRepository, AuditContext, ClaimedTask, RepositoryError, TaskError},
    task_queue::TaskKind,
};
use std::time::Duration;
use support::*;

fn audited(repository: AttricatRepository) -> AttricatRepository {
    repository.with_audit_context(AuditContext {
        actor_user_id: Some(BOOTSTRAP_OWNER_ID.parse().unwrap()),
        actor_token_id: None,
        request_id: Uuid::new_v4(),
        correlation_id: Uuid::new_v4(),
        action: "hardening.task".into(),
        authorization_scope: json!({}),
        target: json!({}),
        metadata: json!({}),
        agent: None,
    })
}

async fn agent_task(repository: &AttricatRepository) -> ClaimedTask {
    let conversation = repository
        .create_conversation(Some(BOOTSTRAP_OWNER_ID.parse().unwrap()), "Fencing")
        .await
        .unwrap();
    repository
        .create_agent_run_for_user(
            conversation.id,
            BOOTSTRAP_OWNER_ID.parse().unwrap(),
            "http://127.0.0.1:1/v1",
            "test",
        )
        .await
        .unwrap();
    repository
        .claim_task_for_kinds("worker-a", Duration::from_secs(60), &[TaskKind::AgentRunV1])
        .await
        .unwrap()
        .unwrap()
}

fn assert_lease_lost<T>(result: Result<T, RepositoryError>) {
    match result {
        Err(RepositoryError::Task(TaskError::LeaseLost)) => {}
        Err(error) => panic!("expected lease loss, got {error}"),
        Ok(_) => panic!("a stale task committed its mutation"),
    }
}

async fn record(owner: &Client, base: &str) -> Uuid {
    let blueprint = create_blueprint(owner, base,
        "format_version = 1\ncode = 'worker_record'\nname = 'Record'\nkind = 'record'\n\n[views.dropdown_option]\ntype = 'dropdown_option'\nfields = ['title']\n\n[[attributes]]\ncode = 'title'\nvalue_type = 'string'").await;
    create_record(owner, base, &blueprint).await["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

#[sqlx::test]
async fn reclaimed_agent_tasks_cannot_commit_comments_saved_views_or_publications(pool: PgPool) {
    let (base, server) = start_server(pool.clone()).await;
    let id = record(&authenticated_client(), &base).await;
    let owner = BOOTSTRAP_OWNER_ID.parse().unwrap();
    let repository = audited(AttricatRepository::new(
        pool.clone(),
        bootstrap_workspace_id(),
    ));
    let context = repository
        .get_context_by_code("default")
        .await
        .unwrap()
        .unwrap()
        .id;
    repository
        .set_publication_channel(context, true)
        .await
        .unwrap();
    repository.publish_record(id, context).await.unwrap();
    repository
        .create_record_comment(id, owner, "Original")
        .await
        .unwrap();
    let comment = repository
        .list_record_comments(id, None)
        .await
        .unwrap()
        .remove(0);
    let state = json!({"blueprint":"worker_record"});
    let view = repository
        .create_saved_view(owner, Some("Original"), None, "private", &state)
        .await
        .unwrap();
    let task = agent_task(&repository).await;
    let stale = repository.for_agent_task(&task);
    sqlx::query("UPDATE tasks SET lease_until=clock_timestamp()-interval '1 second' WHERE id=$1")
        .bind(task.id)
        .execute(&pool)
        .await
        .unwrap();
    let replacement = repository
        .claim_task_for_kinds("worker-b", Duration::from_secs(60), &[TaskKind::AgentRunV1])
        .await
        .unwrap()
        .unwrap();
    assert_ne!(replacement.lease_token, task.lease_token);
    let audit_before: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    let events_before: i64 = sqlx::query_scalar("SELECT count(*) FROM domain_events")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_lease_lost(stale.create_record_comment(id, owner, "Stale").await);
    assert_lease_lost(
        stale
            .update_record_comment(id, comment.id, owner, comment.revision, "Stale")
            .await,
    );
    assert_lease_lost(
        stale
            .create_saved_view(owner, Some("Stale"), None, "private", &state)
            .await,
    );
    assert_lease_lost(
        stale
            .update_saved_view(owner, view.id, "Stale", None, "private", &state)
            .await,
    );
    assert_lease_lost(stale.delete_saved_view(owner, view.id).await);
    assert_lease_lost(stale.publish_record(id, context).await);
    assert_lease_lost(stale.publish_record_all_channels(id).await);
    assert_lease_lost(stale.unpublish_record(id, context).await);

    let comments = repository.list_record_comments(id, None).await.unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].body, "Original");
    let views = repository.list_saved_views(owner, "").await.unwrap();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].name.as_deref(), Some("Original"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM record_channel_publications WHERE record_id=$1"
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM audit_events")
            .fetch_one(&pool)
            .await
            .unwrap(),
        audit_before
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM domain_events")
            .fetch_one(&pool)
            .await
            .unwrap(),
        events_before
    );
    // A current lease must still succeed, including its audit evidence.
    repository
        .for_agent_task(&replacement)
        .create_record_comment(id, owner, "Current")
        .await
        .unwrap();
    assert_eq!(
        repository
            .list_record_comments(id, None)
            .await
            .unwrap()
            .len(),
        2
    );
    server.abort();
}

#[sqlx::test]
async fn sibling_agent_decisions_queue_one_resume_only_after_the_wave_is_resolved(pool: PgPool) {
    use api::repository::ApprovalDecision;
    let (_, server) = start_server(pool.clone()).await;
    let repository = AttricatRepository::new(pool.clone(), bootstrap_workspace_id());
    let task = agent_task(&repository).await;
    let run_id = task.subject_id;
    let worker = repository.for_agent_task(&task);
    worker
        .claim_queued_agent_run(run_id)
        .await
        .unwrap()
        .unwrap();
    let mut calls = Vec::new();
    for name in ["first", "second", "third"] {
        calls.push(
            worker
                .create_agent_tool_call(
                    run_id,
                    Some(name),
                    "create_context",
                    json!({"code":name}),
                    Some(name),
                    "pending_approval",
                )
                .await
                .unwrap(),
        );
    }
    worker
        .transition_agent_run(run_id, "awaiting_approval", None, None)
        .await
        .unwrap();
    repository
        .complete_task(task.id, &task.lease_owner, task.lease_token)
        .await
        .unwrap();
    repository
        .decide_tool_call(
            calls[0].id,
            BOOTSTRAP_OWNER_ID.parse().unwrap(),
            ApprovalDecision::Approve,
        )
        .await
        .unwrap();
    assert_eq!(
        repository.get_agent_run(run_id).await.unwrap().status,
        "awaiting_approval"
    );
    let (second, third) = tokio::join!(
        repository.decide_tool_call(
            calls[1].id,
            BOOTSTRAP_OWNER_ID.parse().unwrap(),
            ApprovalDecision::Approve
        ),
        repository.decide_tool_call(
            calls[2].id,
            BOOTSTRAP_OWNER_ID.parse().unwrap(),
            ApprovalDecision::Reject
        ),
    );
    second.unwrap();
    third.unwrap();
    assert_eq!(
        repository.get_agent_run(run_id).await.unwrap().status,
        "queued"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM tasks WHERE subject_id=$1 AND status='queued'"
        )
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    let resume = repository
        .claim_task_for_kinds("resume", Duration::from_secs(60), &[TaskKind::AgentRunV1])
        .await
        .unwrap()
        .unwrap();
    assert!(
        repository
            .for_agent_task(&resume)
            .claim_queued_agent_run(run_id)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        repository
            .claim_task_for_kinds(
                "competitor",
                Duration::from_secs(60),
                &[TaskKind::AgentRunV1]
            )
            .await
            .unwrap()
            .is_none()
    );
    server.abort();
}

#[sqlx::test]
async fn decisions_during_provider_delivery_are_not_lost_and_terminal_runs_cannot_be_approved(
    pool: PgPool,
) {
    use api::repository::ApprovalDecision;
    let (_, server) = start_server(pool.clone()).await;
    let repository = AttricatRepository::new(pool.clone(), bootstrap_workspace_id());
    let task = agent_task(&repository).await;
    let worker = repository.for_agent_task(&task);
    let run_id = task.subject_id;
    worker
        .claim_queued_agent_run(run_id)
        .await
        .unwrap()
        .unwrap();
    let call = worker
        .create_agent_tool_call(
            run_id,
            Some("early"),
            "create_context",
            json!({"code":"early"}),
            Some("early"),
            "pending_approval",
        )
        .await
        .unwrap();
    repository
        .decide_tool_call(
            call.id,
            BOOTSTRAP_OWNER_ID.parse().unwrap(),
            ApprovalDecision::Approve,
        )
        .await
        .unwrap();
    assert_eq!(
        repository.get_agent_run(run_id).await.unwrap().status,
        "running"
    );
    let parked = worker
        .transition_agent_run(run_id, "awaiting_approval", None, None)
        .await
        .unwrap();
    assert_eq!(parked.status, "queued");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM tasks WHERE subject_id=$1 AND status='queued'"
        )
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    worker
        .transition_agent_run(run_id, "running", None, None)
        .await
        .unwrap();
    let pending = worker
        .create_agent_tool_call(
            run_id,
            Some("late"),
            "create_context",
            json!({"code":"late"}),
            Some("late"),
            "pending_approval",
        )
        .await
        .unwrap();
    worker
        .transition_agent_run(run_id, "failed", Some("test"), Some("test"))
        .await
        .unwrap();
    assert!(
        repository
            .decide_tool_call(
                pending.id,
                BOOTSTRAP_OWNER_ID.parse().unwrap(),
                ApprovalDecision::Approve
            )
            .await
            .is_err()
    );
    assert_eq!(
        repository
            .get_agent_tool_call(pending.id)
            .await
            .unwrap()
            .state,
        "pending_approval"
    );
    assert!(
        repository
            .pending_agent_tool_calls(None)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        repository
            .decide_tool_call(
                call.id,
                BOOTSTRAP_OWNER_ID.parse().unwrap(),
                ApprovalDecision::Approve
            )
            .await,
        Err(RepositoryError::ApprovalAlreadyDecided)
    ));
    server.abort();
}

#[sqlx::test]
async fn task_fences_use_wall_clock_after_waiting_for_a_domain_lock(pool: PgPool) {
    let (base, server) = start_server(pool.clone()).await;
    let id = record(&authenticated_client(), &base).await;
    let repository = AttricatRepository::new(pool.clone(), bootstrap_workspace_id());
    let task = agent_task(&repository).await;
    let fenced = repository.for_agent_task(&task);
    let mut blocker = pool.begin().await.unwrap();
    let blocker_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    sqlx::query("SELECT id FROM records WHERE id=$1 FOR UPDATE")
        .bind(id)
        .execute(&mut *blocker)
        .await
        .unwrap();
    let writer = tokio::spawn(async move {
        fenced
            .create_record_comment(
                id,
                BOOTSTRAP_OWNER_ID.parse().unwrap(),
                "Expired in transaction",
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let blocked: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND $1=ANY(pg_blocking_pids(pid)))")
                .bind(blocker_pid).fetch_one(&pool).await.unwrap();
            if blocked { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    // The writer's transaction began before this expiry. PostgreSQL now()
    // would stay frozen at that earlier timestamp and incorrectly pass.
    sqlx::query(
        "UPDATE tasks SET lease_until=clock_timestamp()+interval '100 milliseconds' WHERE id=$1",
    )
    .bind(task.id)
    .execute(&pool)
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    blocker.rollback().await.unwrap();
    assert_lease_lost(writer.await.unwrap());
    assert!(
        repository
            .list_record_comments(id, None)
            .await
            .unwrap()
            .is_empty()
    );
    server.abort();
}
