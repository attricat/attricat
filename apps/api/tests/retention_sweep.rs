//! Invariants of the maintenance worker's retention sweep: it deletes only
//! expired terminal rows and never one that live work still needs.
use crate::support;

use std::time::Duration;

use api::{
    maintenance::{RetentionConfig, RetentionFamily, sweep_retention},
    repository::{AttricatRepository, RetentionDays},
    rule_runtime,
    task_queue::TaskKind,
    task_worker::TaskOutcome,
};
use support::*;

fn days(value: i64) -> RetentionDays {
    value.to_string().parse().unwrap()
}

fn workspace() -> Uuid {
    BOOTSTRAP_WORKSPACE_ID.parse().unwrap()
}

async fn post(client: &Client, url: String, body: Value) -> (StatusCode, Value) {
    let response = client.post(url).json(&body).send().await.unwrap();
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

async fn drain_rule_tasks(pool: &PgPool) {
    let repository = AttricatRepository::new(pool.clone(), workspace());
    let handler = rule_runtime::task_handler(repository.clone());
    for _ in 0..50 {
        let Some(task) = repository
            .claim_task("retention-test", Duration::from_secs(30))
            .await
            .unwrap()
        else {
            return;
        };
        if task.kind == TaskKind::RuleRunV1 {
            match handler.handle(task.clone()).await.unwrap() {
                TaskOutcome::Complete => {}
                TaskOutcome::Reschedule { .. } => {
                    repository
                        .reschedule_task_at(
                            task.id,
                            &task.lease_owner,
                            task.lease_token,
                            chrono::Utc::now() - chrono::Duration::seconds(1),
                        )
                        .await
                        .unwrap();
                    continue;
                }
                other => panic!("unexpected rule task outcome {other:?}"),
            }
        }
        repository
            .complete_task(task.id, &task.lease_owner, task.lease_token)
            .await
            .unwrap();
    }
    panic!("rule tasks did not drain");
}

async fn run_rule(client: &Client, base: &str, rule_id: &str, dry_run: bool, key: &str) -> Uuid {
    let (status, body) = post(
        client,
        format!("{base}/rules/{rule_id}/run-now"),
        json!({"dry_run": dry_run, "idempotency_key": key}),
    )
    .await;
    assert!(status.is_success(), "{body}");
    body["id"].as_str().unwrap().parse().unwrap()
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn rule_run_retention_keeps_the_enable_gate_latest_and_live_runs(pool: PgPool) {
    AttricatRepository::system(pool.clone())
        .ensure_builtin_permissions()
        .await
        .unwrap();
    let (base, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(
        &client,
        &base,
        r#"
format_version: 1
code: retention_rule_product
name: Retention rule product
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
attributes:
  - code: title
    value_type: string
    default_value: untitled
  - code: price
    value_type: number
    default_value: 1
"#,
    )
    .await;
    let record = create_record(&client, &base, &blueprint).await;
    let record_id: Uuid = record["id"].as_str().unwrap().parse().unwrap();
    let (status, rule) = post(
        &client,
        format!("{base}/rules"),
        json!({
            "blueprint_id": blueprint["blueprint"]["id"],
            "context_id": null,
            "definition": "format_version: 1\ncode: price-positive\nname: Price positive\nseverity: error\ntriggers:\n  - type: manual\npredicate:\n  type: compare\n  attribute_code: price\n  op: gte\n  value: 0\nenforcement:\n  on_save: true",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{rule}");
    let rule_id = rule["id"].as_str().unwrap().to_owned();
    let rule_uuid: Uuid = rule_id.parse().unwrap();
    let enable = format!("{base}/rules/{rule_id}/enable");
    let disable = format!("{base}/rules/{rule_id}/disable");

    let older_dry_run = run_rule(&client, &base, &rule_id, true, "dry-a").await;
    drain_rule_tasks(&pool).await;
    let latest_dry_run = run_rule(&client, &base, &rule_id, true, "dry-b").await;
    drain_rule_tasks(&pool).await;
    let (status, body) = post(&client, enable.clone(), json!({})).await;
    assert!(status.is_success(), "{body}");
    let latest_run = run_rule(&client, &base, &rule_id, false, "full-c").await;
    drain_rule_tasks(&pool).await;
    let (status, body) = post(&client, disable, json!({})).await;
    assert!(status.is_success(), "{body}");

    // Age every run past both retention windows, then add live work.
    sqlx::query("UPDATE rule_runs SET created_at = created_at - interval '100 days', completed_at = completed_at - interval '100 days', updated_at = updated_at - interval '100 days'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE tasks SET created_at = created_at - interval '100 days', completed_at = completed_at - interval '100 days'")
        .execute(&pool)
        .await
        .unwrap();
    let pending = Uuid::new_v4();
    sqlx::query("INSERT INTO rule_runs (id, workspace_id, rule_id, rule_version, definition_hash, source, status, idempotency_key, created_at) VALUES ($1, $2, $3, 1, $4, 'manual', 'pending', 'pending', now() - interval '200 days')")
        .bind(pending)
        .bind(workspace())
        .bind(rule_uuid)
        .bind(rule["definition_hash"].as_str().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    // A finding that the expiring run resolved keeps its resolution.
    let finding = Uuid::new_v4();
    sqlx::query("INSERT INTO rule_findings (id, workspace_id, rule_id, rule_version, record_id, evaluation_key, severity, message, state, resolved_at, resolved_by_run_id) VALUES ($1, $2, $3, 1, $4, 'k', 'error', 'm', 'resolved', now(), $5)")
        .bind(finding)
        .bind(workspace())
        .bind(rule_uuid)
        .bind(record_id)
        .bind(older_dry_run)
        .execute(&pool)
        .await
        .unwrap();

    let repository = AttricatRepository::system(pool.clone());
    assert_eq!(
        repository
            .purge_rule_runs_batch(days(7), days(90), 1000)
            .await
            .unwrap(),
        1
    );
    let mut remaining: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM rule_runs ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    let mut expected = vec![latest_dry_run, latest_run, pending];
    remaining.sort();
    expected.sort();
    assert_eq!(remaining, expected);
    let orphaned_tasks: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM tasks WHERE kind = 'rule_run.v1' AND subject_id = $1",
    )
    .bind(older_dry_run)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(orphaned_tasks, 0);
    let resolved_by: Option<Uuid> =
        sqlx::query_scalar("SELECT resolved_by_run_id FROM rule_findings WHERE id = $1")
            .bind(finding)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(resolved_by, None);

    // The dry-run gate still finds the latest completed full dry run.
    sqlx::query("DELETE FROM rule_runs WHERE id = $1")
        .bind(pending)
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = post(&client, enable, json!({})).await;
    assert!(status.is_success(), "{body}");
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn task_retention_keeps_queued_leased_dead_lettered_and_recent_envelopes(pool: PgPool) {
    let (_, server) = start_server(pool.clone()).await;
    let insert = |status: &'static str, age_days: i32| {
        let pool = pool.clone();
        async move {
            let id = Uuid::new_v4();
            let leased = status == "leased";
            sqlx::query("INSERT INTO tasks (id, workspace_id, kind, envelope_version, subject_id, generation, status, max_failures, lease_owner, lease_token, lease_until, created_at, completed_at) VALUES ($1, $2, 'workflow_run.v1', 1, $3, 0, $4, 3, CASE WHEN $5 THEN 'w' END, CASE WHEN $5 THEN gen_random_uuid() END, CASE WHEN $5 THEN now() END, now() - make_interval(days => $6), CASE WHEN $4 = 'succeeded' THEN now() - make_interval(days => $6) END)")
                .bind(id)
                .bind(workspace())
                .bind(Uuid::new_v4())
                .bind(status)
                .bind(leased)
                .bind(age_days)
                .execute(&pool)
                .await
                .unwrap();
            id
        }
    };
    let expired = insert("succeeded", 30).await;
    let recent = insert("succeeded", 1).await;
    let queued = insert("queued", 30).await;
    let leased = insert("leased", 30).await;
    let dead_letter = insert("dead_letter", 30).await;
    // A replay supersedes an earlier dead-lettered generation.
    let superseded = Uuid::new_v4();
    sqlx::query("UPDATE tasks SET generation = 1 WHERE id = $1")
        .bind(dead_letter)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO tasks (id, workspace_id, kind, envelope_version, subject_id, generation, status, max_failures, created_at, failed_at) SELECT $1, workspace_id, kind, 1, subject_id, 0, 'dead_letter', 3, created_at, created_at FROM tasks WHERE id = $2")
        .bind(superseded)
        .bind(dead_letter)
        .execute(&pool)
        .await
        .unwrap();
    let repository = AttricatRepository::system(pool.clone());
    assert_eq!(
        repository.purge_tasks_batch(days(7), 1000).await.unwrap(),
        2
    );
    let remaining: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM tasks WHERE id = ANY($1)")
        .bind(vec![
            expired,
            superseded,
            recent,
            queued,
            leased,
            dead_letter,
        ])
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(!remaining.contains(&expired) && !remaining.contains(&superseded));
    // The latest dead letter stays for replay, however old.
    for live in [recent, queued, leased, dead_letter] {
        assert!(remaining.contains(&live), "{live} was purged");
    }
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn event_retention_never_removes_undelivered_or_unconsumed_events(pool: PgPool) {
    let (_, server) = start_server(pool.clone()).await;
    let event = || {
        let pool = pool.clone();
        async move {
            sqlx::query_as::<_, (Uuid, i64)>("INSERT INTO domain_events (id, workspace_id, occurred_at, event_type, aggregate_kind, aggregate_id, correlation_id, source_kind, source_name, payload) VALUES (gen_random_uuid(), $1, now() - interval '200 days', 'record.updated.v1', 'record', gen_random_uuid(), gen_random_uuid(), 'system', 'retention-test', '{}') RETURNING id, sequence")
                .bind(workspace())
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    // Held events come first, so the resumable scan has to step past them.
    let pending = event().await;
    let dead_lettered = event().await;
    let delivered = event().await;
    let unconsumed = event().await;
    let newest = event().await;
    // Every consumer has passed all but the last two events.
    sqlx::query("INSERT INTO event_consumers (id, workspace_id, name, watermark) VALUES (gen_random_uuid(), $1, 'retention.test', 0)")
        .bind(workspace())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE event_consumers SET watermark = $2 WHERE workspace_id = $1")
        .bind(workspace())
        .bind(delivered.1)
        .execute(&pool)
        .await
        .unwrap();
    let consumer: Uuid =
        sqlx::query_scalar("SELECT id FROM event_consumers WHERE name = 'retention.test'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let deliver = |event: Uuid, status: &'static str| {
        let pool = pool.clone();
        async move {
            let delivery = Uuid::new_v4();
            let task = Uuid::new_v4();
            let task_status = match status {
                "completed" => "succeeded",
                "dead_letter" => "dead_letter",
                _ => "queued",
            };
            sqlx::query("INSERT INTO tasks (id, workspace_id, kind, envelope_version, subject_id, generation, status, max_failures, created_at) VALUES ($1, $2, 'event_delivery.v1', 1, $3, 0, $4, 3, now() - interval '200 days')")
                .bind(task)
                .bind(workspace())
                .bind(delivery)
                .bind(task_status)
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO event_deliveries (consumer_id, event_id, status, id, task_id) VALUES ($1, $2, $3, $4, $5)")
                .bind(consumer)
                .bind(event)
                .bind(status)
                .bind(delivery)
                .bind(task)
                .execute(&pool)
                .await
                .unwrap();
            task
        }
    };
    let delivered_task = deliver(delivered.0, "completed").await;
    deliver(pending.0, "pending").await;
    deliver(dead_lettered.0, "dead_letter").await;
    // A replayed delivery also leaves its earlier dead-lettered generation.
    sqlx::query("INSERT INTO tasks (id, workspace_id, kind, envelope_version, subject_id, generation, status, max_failures, created_at) SELECT gen_random_uuid(), workspace_id, kind, 1, subject_id, 1, 'dead_letter', 3, created_at FROM tasks WHERE id = $1")
        .bind(delivered_task)
        .execute(&pool)
        .await
        .unwrap();

    // An extension handler gave up on the delivered event; its outcome goes
    // with the event.
    let release = Uuid::new_v4();
    sqlx::query("INSERT INTO installed_extension_releases (id, workspace_id, extension_id, version, manifest, manifest_sha256, source) VALUES ($1, $2, 'acme.retention', '1.0.0', '{}'::jsonb, repeat('0', 64), 'side_load')")
        .bind(release)
        .bind(workspace())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO extension_handler_attempts (workspace_id, event_id, extension_id, handler_id, installed_release_id, status, attempts, error_code) VALUES ($1, $2, 'acme.retention', 'export', $3, 'exhausted', 5, 'timeout')")
        .bind(workspace())
        .bind(delivered.0)
        .bind(release)
        .execute(&pool)
        .await
        .unwrap();

    let repository = AttricatRepository::system(pool.clone());
    // The completed envelope is not purged while its delivery exists.
    assert_eq!(
        repository.purge_tasks_batch(days(7), 1000).await.unwrap(),
        0
    );
    let workspace_events: i64 =
        sqlx::query_scalar("SELECT count(*) FROM domain_events WHERE workspace_id = $1")
            .bind(workspace())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(workspace_events >= 5);
    // One event per batch: two batches step past the held events.
    for _ in 0..3 {
        repository
            .purge_domain_events_batch(days(90), 1)
            .await
            .unwrap();
    }
    let remaining: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM domain_events WHERE id = ANY($1)")
            .bind(vec![
                delivered.0,
                pending.0,
                dead_lettered.0,
                unconsumed.0,
                newest.0,
            ])
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(!remaining.contains(&delivered.0));
    for kept in [pending.0, dead_lettered.0, unconsumed.0, newest.0] {
        assert!(remaining.contains(&kept), "{kept} was purged");
    }
    let delivered_task_left: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM tasks WHERE subject_id = (SELECT subject_id FROM tasks WHERE id = $1) OR id = $1",
    )
    .bind(delivered_task)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(delivered_task_left, 0);
    let purged_through: i64 = sqlx::query_scalar(
        "SELECT purged_through_sequence FROM domain_event_retention WHERE workspace_id = $1",
    )
    .bind(workspace())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(purged_through >= delivered.1);
    let deliveries: i64 = sqlx::query_scalar("SELECT count(*) FROM event_deliveries")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(deliveries, 2);
    let attempts: i64 = sqlx::query_scalar("SELECT count(*) FROM extension_handler_attempts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(attempts, 0);
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn change_feed_rejects_a_cursor_older_than_purged_events(pool: PgPool) {
    let (_, server) = start_server(pool.clone()).await;
    let blueprint = Uuid::new_v4();
    for _ in 0..3 {
        sqlx::query("INSERT INTO domain_events (id, workspace_id, event_type, aggregate_kind, aggregate_id, correlation_id, source_kind, source_name, payload) VALUES (gen_random_uuid(), $1, 'record.updated.v1', 'record', gen_random_uuid(), gen_random_uuid(), 'system', 'retention-test', jsonb_build_object('blueprint_id', $2::text, 'blueprint_version', 1))")
            .bind(workspace())
            .bind(blueprint)
            .execute(&pool)
            .await
            .unwrap();
    }
    let repository = AttricatRepository::new(pool.clone(), workspace());
    let first = repository
        .extension_attricat_changes(blueprint, 1, None, 1)
        .await
        .unwrap();
    let cursor = first.next_cursor.unwrap();
    let resumed = repository
        .extension_attricat_changes(blueprint, 1, Some(cursor.clone()), 1)
        .await
        .unwrap();
    assert_eq!(resumed.events.len(), 1);

    // Retention deletes the event after the cursor.
    sqlx::query("INSERT INTO domain_event_retention (workspace_id, purged_through_sequence) VALUES ($1, $2)")
        .bind(workspace())
        .bind(resumed.events[0].sequence)
        .execute(&pool)
        .await
        .unwrap();
    let error = repository
        .extension_attricat_changes(blueprint, 1, Some(cursor), 1)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("expired"), "{error}");
    // Paging that starts after the purge reads retained events below it.
    let fresh = repository
        .extension_attricat_changes(blueprint, 1, None, 1)
        .await
        .unwrap();
    let fresh = repository
        .extension_attricat_changes(blueprint, 1, fresh.next_cursor, 1)
        .await
        .unwrap();
    assert_eq!(fresh.events.len(), 1);
    let resumed = repository
        .extension_attricat_changes(blueprint, 1, resumed.next_cursor, 1)
        .await
        .unwrap();
    assert_eq!(resumed.events.len(), 1);
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn auth_ephemera_retention_keeps_live_sessions_windows_and_accepted_invitations(
    pool: PgPool,
) {
    let (_, server) = start_server(pool.clone()).await;
    let owner: Uuid = BOOTSTRAP_OWNER_ID.parse().unwrap();
    let session = |expires: &'static str, revoked: Option<&'static str>| {
        let pool = pool.clone();
        async move {
            let id = Uuid::new_v4();
            sqlx::query("INSERT INTO browser_sessions (id, user_id, workspace_id, session_digest, csrf_digest, issued_security_version, issued_credential_version, expires_at, revoked_at, created_at) VALUES ($1, $2, $3, sha256(gen_random_uuid()::text::bytea), sha256(gen_random_uuid()::text::bytea), 1, 1, now() + $4::interval, now() + $5::interval, now() - interval '400 days')")
                .bind(id)
                .bind(owner)
                .bind(workspace())
                .bind(expires)
                .bind(revoked)
                .execute(&pool)
                .await
                .unwrap();
            id
        }
    };
    let expired = session("-30 days", None).await;
    let revoked = session("1 day", Some("-30 days")).await;
    let recently_expired = session("-1 day", None).await;
    let live = session("1 day", None).await;
    sqlx::query("INSERT INTO password_reset_rate_limits (key_digest, window_started_at, attempts) VALUES ('\\x01', now() - interval '1 hour', 1), ('\\x02', now(), 1)")
        .execute(&pool)
        .await
        .unwrap();

    let repository = AttricatRepository::system(pool.clone());
    assert_eq!(
        repository
            .purge_browser_sessions_batch(days(7), 1000)
            .await
            .unwrap(),
        2
    );
    let remaining: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM browser_sessions WHERE id = ANY($1)")
            .bind(vec![expired, revoked, recently_expired, live])
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(remaining.contains(&recently_expired) && remaining.contains(&live));
    assert_eq!(remaining.len(), 2);
    assert_eq!(
        repository
            .purge_rate_limit_windows_batch(1000)
            .await
            .unwrap(),
        1
    );
    let windows: i64 = sqlx::query_scalar("SELECT count(*) FROM password_reset_rate_limits")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(windows, 1);

    let invitation = |accepted: bool| {
        let pool = pool.clone();
        async move {
            let id = Uuid::new_v4();
            sqlx::query("INSERT INTO workspace_invitations (id, workspace_id, invitee_email, inviter_user_id, role_id, scope_type, scope_target_id, token_digest, expires_at, accepted_at, created_at) SELECT $1, $2, $1::text || '@example.test', $3, r.id, 'workspace', $2, sha256($1::text::bytea), now() - interval '60 days', CASE WHEN $4 THEN now() - interval '61 days' END, now() - interval '70 days' FROM roles r WHERE r.is_system ORDER BY r.code LIMIT 1")
                .bind(id)
                .bind(workspace())
                .bind(owner)
                .bind(accepted)
                .execute(&pool)
                .await
                .unwrap();
            id
        }
    };
    let expired_invitation = invitation(false).await;
    let accepted_invitation = invitation(true).await;
    assert_eq!(
        repository
            .purge_workspace_invitations_batch(days(30), 1000)
            .await
            .unwrap(),
        1
    );
    let invitations: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM workspace_invitations WHERE id = ANY($1)")
            .bind(vec![expired_invitation, accepted_invitation])
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(invitations, vec![accepted_invitation]);
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn extension_retention_keeps_open_deliveries_and_recent_requests(pool: PgPool) {
    let release = Uuid::new_v4();
    sqlx::query("INSERT INTO installed_extension_releases (id, workspace_id, extension_id, version, manifest, manifest_sha256, source) VALUES ($1, $2, 'acme.retention', '1.0.0', '{}'::jsonb, repeat('0', 64), 'side_load')")
        .bind(release)
        .bind(workspace())
        .execute(&pool)
        .await
        .unwrap();
    let consumer: Uuid = sqlx::query_scalar("INSERT INTO event_consumers (id, workspace_id, name, watermark) VALUES (gen_random_uuid(), $1, 'retention.test', 0) RETURNING id")
        .bind(workspace())
        .fetch_one(&pool)
        .await
        .unwrap();
    // One handler outcome per event: (status, age in days, delivery status).
    let outcome = |status: &'static str, age: i32, delivery: Option<&'static str>| {
        let pool = pool.clone();
        async move {
            let event: Uuid = sqlx::query_scalar("INSERT INTO domain_events (id, workspace_id, event_type, aggregate_kind, aggregate_id, correlation_id, source_kind, source_name, payload) VALUES (gen_random_uuid(), $1, 'record.updated.v1', 'record', gen_random_uuid(), gen_random_uuid(), 'system', 'retention-test', '{}') RETURNING id")
                .bind(workspace())
                .fetch_one(&pool)
                .await
                .unwrap();
            if let Some(delivery) = delivery {
                sqlx::query("INSERT INTO event_deliveries (consumer_id, event_id, status) VALUES ($1, $2, $3)")
                    .bind(consumer)
                    .bind(event)
                    .bind(delivery)
                    .execute(&pool)
                    .await
                    .unwrap();
            }
            sqlx::query("INSERT INTO extension_handler_attempts (workspace_id, event_id, extension_id, handler_id, installed_release_id, status, attempts, next_attempt_at, updated_at) VALUES ($1, $2, 'acme.retention', 'export', $3, $4, 1, CASE WHEN $4 IN ('retrying', 'debouncing') THEN now() END, now() - make_interval(days => $5))")
                .bind(workspace())
                .bind(event)
                .bind(release)
                .bind(status)
                .bind(age)
                .execute(&pool)
                .await
                .unwrap();
            event
        }
    };
    let exhausted = outcome("exhausted", 31, Some("pending")).await;
    let recent = outcome("rejected", 29, None).await;
    let retrying = outcome("retrying", 31, Some("pending")).await;
    let dead_lettered = outcome("completed", 31, Some("dead_letter")).await;
    for (key, age) in [("old", 8), ("new", 6)] {
        sqlx::query("INSERT INTO extension_export_retry_requests (workspace_id, extension_id, request_key, created_at) VALUES ($1, 'acme.retention', $2, now() - make_interval(days => $3))")
            .bind(workspace())
            .bind(key)
            .bind(age)
            .execute(&pool)
            .await
            .unwrap();
    }

    let repository = AttricatRepository::system(pool.clone());
    assert_eq!(
        repository
            .purge_extension_handler_outcomes_batch(1000)
            .await
            .unwrap(),
        2
    );
    let kept: Vec<Uuid> = sqlx::query_scalar("SELECT event_id FROM extension_handler_attempts")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(!kept.contains(&exhausted) && !kept.contains(&dead_lettered));
    assert!(kept.contains(&recent) && kept.contains(&retrying));
    assert_eq!(
        repository
            .purge_extension_export_retry_requests_batch(1000)
            .await
            .unwrap(),
        1
    );
    let keys: Vec<String> =
        sqlx::query_scalar("SELECT request_key FROM extension_export_retry_requests")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(keys, ["new"]);
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn sweep_drains_every_family_in_bounded_round_robin_batches(pool: PgPool) {
    let (base, server) = start_server(pool.clone()).await;
    let owner = authenticated_client();
    let blueprint = create_blueprint(
        &owner,
        &base,
        r#"
format_version: 1
code: retention_sweep
name: Retention sweep
kind: record
attributes:
  - code: title
    value_type: string
    default_value: Current
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
"#,
    )
    .await;
    let record = create_record(&owner, &base, &blueprint).await;
    let id: Uuid = record["id"].as_str().unwrap().parse().unwrap();
    sqlx::query("INSERT INTO attribute_value_history (id,workspace_id,record_id,attribute_id,context_id,active,value_text,created_at,archived_at) SELECT gen_random_uuid(),v.workspace_id,v.record_id,v.attribute_id,v.context_id,false,'Old',now()-interval '100 days',now()-interval '91 days' FROM attribute_values v CROSS JOIN generate_series(1,25) WHERE v.record_id=$1")
        .bind(id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO tasks (id, workspace_id, kind, envelope_version, subject_id, generation, status, max_failures, created_at, completed_at) SELECT gen_random_uuid(), $1, 'workflow_run.v1', 1, gen_random_uuid(), 0, 'succeeded', 3, now() - interval '30 days', now() - interval '30 days' FROM generate_series(1, 25)")
        .bind(workspace()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO user_notifications (id, workspace_id, recipient_user_id, kind, title, read_at) SELECT gen_random_uuid(), $1, $2, 'test.kind', 'Read', now() - interval '30 days' FROM generate_series(1, 3)")
        .bind(workspace()).bind(BOOTSTRAP_OWNER_ID.parse::<Uuid>().unwrap()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO user_notifications (id, workspace_id, recipient_user_id, kind, title) VALUES (gen_random_uuid(), $1, $2, 'test.kind', 'Unread')")
        .bind(workspace()).bind(BOOTSTRAP_OWNER_ID.parse::<Uuid>().unwrap()).execute(&pool).await.unwrap();

    let defaults = RetentionConfig::from_values(|_| None).unwrap();
    assert_eq!(defaults.families().len(), RetentionFamily::ALL.len() - 2);
    let config = RetentionConfig::from_values(|name| {
        match name {
            "RETENTION_BATCH_ROWS" => Some("10"),
            "READ_NOTIFICATION_RETENTION_DAYS" => Some("7"),
            "AUDIT_EVENT_RETENTION_DAYS" => Some("36500"),
            _ => None,
        }
        .map(str::to_owned)
    })
    .unwrap();
    let sweeps = sweep_retention(&AttricatRepository::system(pool.clone()), &config).await;
    assert_eq!(sweeps.len(), RetentionFamily::ALL.len());
    for sweep in &sweeps {
        assert_eq!(sweep.outcome, "success", "{sweep:?}");
    }
    let purged = |family| {
        sweeps
            .iter()
            .find(|sweep| sweep.family == family)
            .unwrap()
            .purged
    };
    assert_eq!(purged(RetentionFamily::ValueHistory), 25);
    assert_eq!(purged(RetentionFamily::Tasks), 25);
    assert_eq!(purged(RetentionFamily::ReadNotifications), 3);
    let unread: i64 = sqlx::query_scalar("SELECT count(*) FROM user_notifications")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(unread, 1);
    server.abort();
}

#[test]
fn retention_settings_are_validated() {
    let parse = |name: &'static str, value: &'static str| {
        RetentionConfig::from_values(move |key| (key == name).then(|| value.to_owned()))
    };
    for (name, value) in [
        ("RETENTION_BATCH_ROWS", "0"),
        ("RETENTION_BATCH_ROWS", "10001"),
        ("RETENTION_SWEEP_BUDGET_SECONDS", "61"),
        ("TASK_RETENTION_DAYS", "0"),
        ("AUDIT_EVENT_RETENTION_DAYS", "-1"),
    ] {
        assert!(parse(name, value).is_err(), "{name}={value}");
    }
    let config = parse("WORKFLOW_RUN_RETENTION_DAYS", "30").unwrap();
    assert_eq!(config.workflow_runs.get(), 30);
    assert_eq!(config.batch_rows, 1000);
    assert_eq!(config.audit_events, None);
}
