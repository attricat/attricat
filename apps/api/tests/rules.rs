mod support;

use std::time::Duration;

use api::{repository::CatalogRepository, rule_runtime, task_worker::TaskOutcome};
use support::*;
use tokio::time::sleep;

const RULE: &str = r#"format_version = 1
code = "required_title"
name = "Title is required"
severity = "error"
[[triggers]]
type = "manual"
[predicate]
type = "required"
attribute_code = "summary"
"#;

async fn setup_rule(pool: &PgPool) -> (String, tokio::task::JoinHandle<()>, Uuid, Value) {
    CatalogRepository::system(pool.clone())
        .ensure_rule_permissions()
        .await
        .unwrap();
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "rule_task_product"
name = "Rule task product"
kind = "record"
[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
[[attributes]]
code = "title"
value_type = "string"
default_value = "untitled"
[[attributes]]
code = "summary"
value_type = "string"
"#,
    )
    .await;
    let created: Value = client
        .post(format!("{base_url}/rules"))
        .json(&json!({
            "blueprint_id": blueprint["blueprint"]["id"],
            "blueprint_version": blueprint["blueprint"]["version"],
            "context_id": null,
            "definition": RULE,
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let rule_id = created["id"].as_str().unwrap().parse().unwrap();
    client
        .post(format!("{base_url}/rules/{rule_id}/versions/1/publish"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    client
        .post(format!("{base_url}/rules/{rule_id}/versions/1/enable"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    (base_url, server, rule_id, blueprint)
}

async fn manual_run(base_url: &str, rule_id: Uuid, record_id: Option<Uuid>, key: &str) -> Uuid {
    let run: Value = authenticated_client()
        .post(format!("{base_url}/rules/{rule_id}/run-now"))
        .json(&json!({"record_id": record_id, "dry_run": false, "idempotency_key": key}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    run["id"].as_str().unwrap().parse().unwrap()
}

#[sqlx::test]
async fn rule_task_lease_loss_and_crash_after_page_are_fenced(pool: PgPool) {
    let (base_url, server, rule_id, blueprint) = setup_rule(&pool).await;
    let client = authenticated_client();
    let record = create_record(&client, &base_url, &blueprint).await;
    let record_id = record["id"].as_str().unwrap().parse().unwrap();
    let run_id = manual_run(&base_url, rule_id, Some(record_id), "lease-loss").await;
    let repository = CatalogRepository::new(
        pool.clone(),
        support::BOOTSTRAP_WORKSPACE_ID.parse().unwrap(),
    );

    // Run creation and task production are one transaction; no worker is
    // running in this test before this assertion.
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM tasks WHERE kind='rule_run.v1' AND subject_id=$1"
        )
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    let stale = repository
        .claim_task("stale", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    sqlx::query("UPDATE tasks SET lease_until=clock_timestamp()-interval '1 second' WHERE id=$1")
        .bind(stale.id)
        .execute(&pool)
        .await
        .unwrap();
    let handler = rule_runtime::task_handler(repository.clone());
    assert!(handler.handle(stale).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM rule_findings WHERE record_id=$1")
            .bind(record_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );

    let claimed = repository
        .claim_task("reclaimer", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        handler.handle(claimed.clone()).await.unwrap(),
        TaskOutcome::Complete
    ));
    // Model a process crash after the fenced page/complete transaction but
    // before the task acknowledgement. Reclaiming must not duplicate findings.
    sqlx::query("UPDATE tasks SET lease_until=clock_timestamp()-interval '1 second' WHERE id=$1")
        .bind(claimed.id)
        .execute(&pool)
        .await
        .unwrap();
    let replay = repository
        .claim_task("reclaimer-2", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        handler.handle(replay.clone()).await.unwrap(),
        TaskOutcome::Complete
    ));
    repository
        .complete_task(replay.id, &replay.lease_owner, replay.lease_token)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM rule_findings WHERE record_id=$1")
            .bind(record_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    server.abort();
}

#[sqlx::test]
async fn checkpoint_failure_on_final_attempt_dead_letters_rule_run_and_replays_generation(
    pool: PgPool,
) {
    let (base_url, server, rule_id, blueprint) = setup_rule(&pool).await;
    let client = authenticated_client();
    let record = create_record(&client, &base_url, &blueprint).await;
    let record_id = record["id"].as_str().unwrap().parse().unwrap();
    let run_id = manual_run(
        &base_url,
        rule_id,
        Some(record_id),
        "checkpoint-final-failure",
    )
    .await;
    let repository = CatalogRepository::new(
        pool.clone(),
        support::BOOTSTRAP_WORKSPACE_ID.parse().unwrap(),
    );

    // This constraint is a test-only database failure injection at the page
    // checkpoint; the run and task terminal transition must still commit.
    sqlx::query("ALTER TABLE rule_findings ADD CONSTRAINT checkpoint_failure CHECK (false)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE tasks SET failures=max_failures-1 WHERE kind='rule_run.v1' AND subject_id=$1",
    )
    .bind(run_id)
    .execute(&pool)
    .await
    .unwrap();
    let task = repository
        .claim_task("checkpoint-final", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();

    assert!(matches!(
        rule_runtime::task_handler(repository.clone())
            .handle(task)
            .await
            .unwrap(),
        TaskOutcome::DeadLettered
    ));
    let (run_status, task_status): (String, String) = sqlx::query_as(
        "SELECT r.status,t.status FROM rule_runs r JOIN tasks t ON t.subject_id=r.id AND t.kind='rule_run.v1' WHERE r.id=$1",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (run_status.as_str(), task_status.as_str()),
        ("dead_letter", "dead_letter")
    );

    assert!(
        repository
            .for_workspace(BOOTSTRAP_WORKSPACE_ID.parse().unwrap())
            .await
            .unwrap()
            .replay_rule_run(run_id)
            .await
            .unwrap()
    );
    let (generation, status): (i32, String) = sqlx::query_as(
        "SELECT generation,status FROM tasks WHERE kind='rule_run.v1' AND subject_id=$1 ORDER BY generation DESC LIMIT 1",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((generation, status.as_str()), (1, "queued"));
    server.abort();
}

#[sqlx::test]
async fn rule_page_continuation_yields_without_failure_budget(pool: PgPool) {
    let (base_url, server, rule_id, blueprint) = setup_rule(&pool).await;
    let client = authenticated_client();
    let first = create_record(&client, &base_url, &blueprint).await;
    let workspace: Uuid = BOOTSTRAP_WORKSPACE_ID.parse().unwrap();
    let blueprint_id: Uuid = blueprint["blueprint"]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let blueprint_version = blueprint["blueprint"]["version"].as_i64().unwrap();
    // Candidate paging uses only record identity and metadata, so build a page
    // boundary directly without creating 500 unrelated HTTP mutations.
    sqlx::query("INSERT INTO records(id,workspace_id,blueprint_id,blueprint_version,projections) SELECT gen_random_uuid(),$1,$2,$3,'{}'::jsonb FROM generate_series(1,500)")
        .bind(workspace).bind(blueprint_id).bind(blueprint_version).execute(&pool).await.unwrap();
    let run_id = manual_run(&base_url, rule_id, None, "continuation").await;
    let repository = CatalogRepository::new(
        pool.clone(),
        support::BOOTSTRAP_WORKSPACE_ID.parse().unwrap(),
    );
    let handler = rule_runtime::task_handler(repository.clone());
    let first_task = repository
        .claim_task("page-worker", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        handler.handle(first_task.clone()).await.unwrap(),
        TaskOutcome::Reschedule { .. }
    ));
    repository
        .reschedule_task_at(
            first_task.id,
            &first_task.lease_owner,
            first_task.lease_token,
            // Keep the immediate-reclaim assertion independent of small host
            // and database-container clock skew.
            chrono::Utc::now() - chrono::Duration::seconds(1),
        )
        .await
        .unwrap();
    sleep(Duration::from_millis(10)).await;
    let second_task = repository
        .claim_task("page-worker", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        handler.handle(second_task.clone()).await.unwrap(),
        TaskOutcome::Complete
    ));
    repository
        .complete_task(
            second_task.id,
            &second_task.lease_owner,
            second_task.lease_token,
        )
        .await
        .unwrap();
    let (evaluated, status, failures): (i64, String, i32) = sqlx::query_as("SELECT r.candidates_evaluated,r.status,t.failures FROM rule_runs r JOIN tasks t ON t.subject_id=r.id AND t.kind='rule_run.v1' WHERE r.id=$1")
        .bind(run_id).fetch_one(&pool).await.unwrap();
    assert_eq!(evaluated, 501);
    assert_eq!(status, "completed");
    assert_eq!(failures, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM rule_findings WHERE rule_id=$1")
            .bind(rule_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        501
    );
    assert!(first["id"].is_string());
    server.abort();
}

#[sqlx::test]
async fn manual_run_of_a_disabled_rule_explains_it_is_not_enabled(pool: PgPool) {
    let (base_url, server, rule_id, _) = setup_rule(&pool).await;
    let client = authenticated_client();
    client
        .post(format!("{base_url}/rules/{rule_id}/disable"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let refused = client
        .post(format!("{base_url}/rules/{rule_id}/run-now"))
        .json(&json!({"dry_run": false, "idempotency_key": "disabled"}))
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), 422);
    let body: Value = refused.json().await.unwrap();
    assert_eq!(body["error"]["code"], "rule_not_enabled");
    assert_eq!(
        body["error"]["message"],
        "rule has no enabled revision to run; enable it first, or start a dry run"
    );

    // Dry runs may still evaluate the latest published revision.
    client
        .post(format!("{base_url}/rules/{rule_id}/run-now"))
        .json(&json!({"dry_run": true, "idempotency_key": "disabled-dry"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    server.abort();
}
