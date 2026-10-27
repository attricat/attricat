use crate::support;

use std::time::Duration;

use api::{repository::AttricatRepository, rule_runtime, task_worker::TaskOutcome};
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
    AttricatRepository::system(pool.clone())
        .ensure_builtin_permissions()
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

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn rule_task_lease_loss_and_crash_after_page_are_fenced(pool: PgPool) {
    let (base_url, server, rule_id, blueprint) = setup_rule(&pool).await;
    let client = authenticated_client();
    let record = create_record(&client, &base_url, &blueprint).await;
    let record_id = record["id"].as_str().unwrap().parse().unwrap();
    let run_id = manual_run(&base_url, rule_id, Some(record_id), "lease-loss").await;
    let repository = AttricatRepository::new(
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

#[sqlx::test(migrator = "api::MIGRATOR")]
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
    let repository = AttricatRepository::new(
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

#[sqlx::test(migrator = "api::MIGRATOR")]
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
    let repository = AttricatRepository::new(
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

#[sqlx::test(migrator = "api::MIGRATOR")]
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

/// Validation runs the save checks against the chosen blueprint revision, so
/// the editor reports an unknown attribute before the rule is saved.
#[sqlx::test(migrator = "api::MIGRATOR")]
async fn validate_checks_the_definition_against_its_blueprint_revision(pool: PgPool) {
    let (base_url, server, _, blueprint) = setup_rule(&pool).await;
    let client = authenticated_client();
    let validate = |definition: String| {
        client
            .post(format!("{base_url}/rules/validate"))
            .json(&json!({
                "blueprint_id": blueprint["blueprint"]["id"],
                "blueprint_version": blueprint["blueprint"]["version"],
                "context_id": null,
                "definition": definition,
            }))
            .send()
    };

    let valid = validate(RULE.replace("required_title", "another_rule"))
        .await
        .unwrap();
    assert_eq!(valid.status(), 200);
    let compiled: Value = valid.json().await.unwrap();
    assert_eq!(compiled["code"], "another_rule");

    let unknown = validate(RULE.replace("\"summary\"", "\"missing\""))
        .await
        .unwrap();
    assert_eq!(unknown.status(), 422);
    let body: Value = unknown.json().await.unwrap();
    assert_eq!(body["error"]["code"], "invalid_rule_definition");
    assert_eq!(
        client
            .get(format!("{base_url}/rules"))
            .send()
            .await
            .unwrap()
            .json::<Vec<Value>>()
            .await
            .unwrap()
            .len(),
        1,
        "validation stores nothing"
    );
    server.abort();
}

/// A record change re-checks the record for every enabled rule of its
/// revision, even one without an event trigger, and the record can be
/// re-checked on demand.
#[sqlx::test(migrator = "api::MIGRATOR")]
async fn record_changes_and_rechecks_run_every_enabled_rule_for_the_record(pool: PgPool) {
    let (base_url, server, rule_id, blueprint) = setup_rule(&pool).await;
    let client = authenticated_client();
    let record = create_record(&client, &base_url, &blueprint).await;
    let record_id: Uuid = record["id"].as_str().unwrap().parse().unwrap();
    let repository = AttricatRepository::new(
        pool.clone(),
        support::BOOTSTRAP_WORKSPACE_ID.parse().unwrap(),
    );
    let event = support::latest_domain_event(&pool, record_id, None).await;
    assert_eq!(repository.fan_out_rule_runs(&event).await.unwrap(), 1);
    // Event delivery is at least once; a redelivered event adds no run.
    assert_eq!(repository.fan_out_rule_runs(&event).await.unwrap(), 0);

    let recheck = || async {
        let response: Value = client
            .post(format!("{base_url}/rule-findings/recheck"))
            .json(&json!({"record_id": record_id, "idempotency_key": "recheck"}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        response["run_ids"].as_array().unwrap().clone()
    };
    let first = recheck().await;
    assert_eq!(first.len(), 1);
    assert_eq!(recheck().await, first);

    let runs: Value = client
        .get(format!("{base_url}/rule-runs?record_id={record_id}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let runs = runs.as_array().unwrap();
    assert_eq!(runs.len(), 2);
    assert!(
        runs.iter()
            .all(|run| run["scope_record_id"] == json!(record_id)
                && run["rule_id"] == json!(rule_id))
    );

    let handler = rule_runtime::task_handler(repository.clone());
    while let Some(task) = repository
        .claim_task("recheck-test", Duration::from_secs(30))
        .await
        .unwrap()
    {
        assert!(matches!(
            handler.handle(task.clone()).await.unwrap(),
            TaskOutcome::Complete
        ));
        repository
            .complete_task(task.id, &task.lease_owner, task.lease_token)
            .await
            .unwrap();
    }
    let findings: Value = client
        .get(format!("{base_url}/rule-findings?record_id={record_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(findings.as_array().unwrap().len(), 1);
    assert_eq!(findings[0]["state"], "open");

    let get_json = |path: String| {
        let client = client.clone();
        async move {
            client
                .get(path)
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()
        }
    };
    let count_path = format!("{base_url}/rule-findings/count?record_id={record_id}");
    let unresolved_path =
        format!("{base_url}/rule-findings?record_id={record_id}&unresolved=true&limit=1");
    assert_eq!(get_json(count_path.clone()).await["count"], 1);
    assert_eq!(get_json(unresolved_path.clone()).await, findings);
    // A resolved finding stays in the full list but leaves the unresolved
    // list and count.
    sqlx::query("UPDATE rule_findings SET state='resolved',resolved_at=now() WHERE record_id=$1")
        .bind(record_id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(get_json(count_path).await["count"], 0);
    assert_eq!(get_json(unresolved_path).await, json!([]));
    let all = get_json(format!("{base_url}/rule-findings?record_id={record_id}")).await;
    assert_eq!(all[0]["state"], "resolved");
    // More severe findings come first even when they are older, so offset
    // pages do not reshuffle when a re-check touches a finding.
    sqlx::query("INSERT INTO rule_findings(id,workspace_id,rule_id,rule_version,record_id,context_id,evaluation_key,severity,message,evidence,created_at,updated_at) SELECT $2,workspace_id,rule_id,rule_version,record_id,context_id,'older-critical','critical',message,evidence,created_at - interval '1 hour',created_at - interval '1 hour' FROM rule_findings WHERE record_id=$1")
        .bind(record_id)
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await
        .unwrap();
    let all = get_json(format!("{base_url}/rule-findings?record_id={record_id}")).await;
    assert_eq!(
        all.as_array()
            .unwrap()
            .iter()
            .map(|finding| finding["severity"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["critical", "error"]
    );

    let missing = client
        .post(format!("{base_url}/rule-findings/recheck"))
        .json(&json!({"record_id": Uuid::new_v4(), "idempotency_key": "missing"}))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), 404);
    server.abort();
}

/// The findings list narrows by rule, blueprint, context, severity and
/// state, and pages through what matches.
#[sqlx::test(migrator = "api::MIGRATOR")]
async fn findings_filter_by_rule_blueprint_context_severity_and_state(pool: PgPool) {
    let (base_url, server, rule_id, blueprint) = setup_rule(&pool).await;
    let client = authenticated_client();
    let blueprint_id = blueprint["blueprint"]["id"].as_str().unwrap();
    let mut record_ids = Vec::new();
    for (key, severity, state) in [
        ("critical-open", "critical", "open"),
        ("warning-acknowledged", "warning", "acknowledged"),
        ("info-resolved", "info", "resolved"),
    ] {
        let record = create_record(&client, &base_url, &blueprint).await;
        let record_id: Uuid = record["id"].as_str().unwrap().parse().unwrap();
        sqlx::query("INSERT INTO rule_findings(id,workspace_id,rule_id,rule_version,record_id,evaluation_key,severity,message,state) VALUES ($1,$2,$3,1,$4,$5,$6,$5,$7)")
            .bind(Uuid::new_v4())
            .bind(Uuid::parse_str(support::BOOTSTRAP_WORKSPACE_ID).unwrap())
            .bind(rule_id)
            .bind(record_id)
            .bind(key)
            .bind(severity)
            .bind(state)
            .execute(&pool)
            .await
            .unwrap();
        record_ids.push(record_id);
    }
    let messages = |query: String| {
        let client = client.clone();
        let base_url = base_url.clone();
        async move {
            let findings: Value = client
                .get(format!("{base_url}/rule-findings?{query}"))
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap()
                .json()
                .await
                .unwrap();
            findings
                .as_array()
                .unwrap()
                .iter()
                .map(|finding| finding["message"].as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        }
    };
    let all = ["critical-open", "warning-acknowledged", "info-resolved"];
    assert_eq!(messages(String::new()).await, all);
    assert_eq!(messages(format!("rule_id={rule_id}")).await, all);
    assert_eq!(messages(format!("blueprint_id={blueprint_id}")).await, all);
    let other = Uuid::new_v4();
    for field in ["rule_id", "blueprint_id", "context_id", "record_id"] {
        assert!(messages(format!("{field}={other}")).await.is_empty());
    }
    assert_eq!(
        messages(format!("record_id={}", record_ids[1])).await,
        ["warning-acknowledged"]
    );
    assert_eq!(messages("severity=info".into()).await, ["info-resolved"]);
    assert_eq!(
        messages("state=acknowledged".into()).await,
        ["warning-acknowledged"]
    );
    assert_eq!(
        messages("unresolved=true".into()).await,
        ["critical-open", "warning-acknowledged"]
    );
    assert!(
        messages("unresolved=true&state=resolved".into())
            .await
            .is_empty()
    );

    // Pages follow the filter and advertise the next offset.
    let page = client
        .get(format!("{base_url}/rule-findings?unresolved=true&limit=1"))
        .send()
        .await
        .unwrap();
    assert_eq!(page.headers()["x-next-offset"], "1");
    assert_eq!(
        messages("unresolved=true&limit=1&offset=1".into()).await,
        ["warning-acknowledged"]
    );

    for query in ["severity=fatal", "state=closed", "colour=red"] {
        let refused = client
            .get(format!("{base_url}/rule-findings?{query}"))
            .send()
            .await
            .unwrap();
        assert_eq!(refused.status(), 400, "{query}");
    }
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn search_summarizes_filters_and_sorts_by_data_quality(pool: PgPool) {
    let (base_url, server, rule_id, blueprint) = setup_rule(&pool).await;
    let client = authenticated_client();
    let workspace = Uuid::parse_str(support::BOOTSTRAP_WORKSPACE_ID).unwrap();
    let mut ids = Vec::new();
    for findings in [
        &[("critical", "open")][..],
        &[("warning", "acknowledged")],
        &[("info", "open"), ("critical", "acknowledged")],
        &[("error", "resolved")],
    ] {
        let record = create_record(&client, &base_url, &blueprint).await;
        let record_id: Uuid = record["id"].as_str().unwrap().parse().unwrap();
        for (severity, state) in findings {
            sqlx::query("INSERT INTO rule_findings(id,workspace_id,rule_id,rule_version,record_id,evaluation_key,severity,message,state) VALUES ($1,$2,$3,1,$4,$5,$6,$5,$7)")
                .bind(Uuid::new_v4())
                .bind(workspace)
                .bind(rule_id)
                .bind(record_id)
                .bind(format!("{record_id}-{severity}"))
                .bind(severity)
                .bind(state)
                .execute(&pool)
                .await
                .unwrap();
        }
        ids.push(Value::String(record_id.to_string()));
    }
    // The findings above bypass rule runs, so tag them as startup would.
    AttricatRepository::system(pool.clone())
        .reconcile_needs_attention_tags(workspace)
        .await
        .unwrap();
    let search = |client: Client, body: Value| {
        let base_url = base_url.clone();
        async move {
            client
                .post(format!("{base_url}/v1/records/search"))
                .json(&body)
                .send()
                .await
                .unwrap()
        }
    };
    let found = |response: &Value| {
        let mut found = response["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["id"].clone())
            .collect::<Vec<_>>();
        found.sort_by_key(|id| ids.iter().position(|candidate| candidate == id));
        found
    };
    let code = json!({"code": "rule_task_product"});

    let all = expect_status(
        search(client.clone(), json!({"blueprint": code})).await,
        StatusCode::OK,
    )
    .await;
    let mixed = all["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == ids[2])
        .unwrap();
    assert_eq!(
        mixed["data_quality"],
        json!({"checked": true, "findings": [
            {"state": "acknowledged", "severity": "critical", "count": 1},
            {"state": "open", "severity": "info", "count": 1},
        ]})
    );
    let clean = all["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == ids[3])
        .unwrap();
    assert_eq!(
        clean["data_quality"],
        json!({"checked": true, "findings": []})
    );

    for (filter, expected) in [
        ("open", vec![ids[0].clone(), ids[2].clone()]),
        ("acknowledged", vec![ids[1].clone()]),
        ("clean", vec![ids[3].clone()]),
    ] {
        let response = expect_status(
            search(
                client.clone(),
                json!({"blueprint": code, "data_quality": filter, "include_total": true}),
            )
            .await,
            StatusCode::OK,
        )
        .await;
        assert_eq!(found(&response), expected, "{filter}");
        assert_eq!(response["total_count"], expected.len(), "{filter}");
    }

    // Records that need attention (an open finding) come first, each group
    // by id; keyset pages walk the whole order.
    let mut cursor: Option<String> = None;
    let mut sorted = Vec::new();
    loop {
        let page = expect_status(
            search(
                client.clone(),
                json!({"blueprint": code,
                    "sort": {"field": "system_tag:attricat.needs_attention", "direction": "desc"},
                    "page": {"size": 1, "cursor": cursor}}),
            )
            .await,
            StatusCode::OK,
        )
        .await;
        sorted.push(page["items"][0]["id"].clone());
        cursor = page["next_cursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    let by_id = |mut group: Vec<Value>| {
        group.sort_by_key(|id| id.as_str().unwrap().to_owned());
        group
    };
    let mut expected = by_id(vec![ids[0].clone(), ids[2].clone()]);
    expected.extend(by_id(vec![ids[1].clone(), ids[3].clone()]));
    assert_eq!(sorted, expected);

    // Findings belong to rules: a reader without rules.read sees no summary
    // and cannot filter by it, but may sort by the tag it sees.
    let reader_role = create_role(&pool, "records_reader", &["records.read"]).await;
    let reader = client_for(member_with_role(&pool, reader_role).await);
    let plain = expect_status(
        search(reader.clone(), json!({"blueprint": code})).await,
        StatusCode::OK,
    )
    .await;
    assert!(plain["items"][0].get("data_quality").is_none());
    assert_eq!(
        search(
            reader.clone(),
            json!({"blueprint": code, "data_quality": "open"})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        search(
            reader.clone(),
            json!({"blueprint": code,
                "sort": {"field": "system_tag:attricat.needs_attention", "direction": "asc"}})
        )
        .await
        .status(),
        StatusCode::OK
    );
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn needs_attention_tag_follows_open_findings_and_is_reserved(pool: PgPool) {
    let (base_url, server, rule_id, blueprint) = setup_rule(&pool).await;
    let client = authenticated_client();
    let workspace: Uuid = BOOTSTRAP_WORKSPACE_ID.parse().unwrap();
    let record = create_record(&client, &base_url, &blueprint).await;
    let record_id: Uuid = record["id"].as_str().unwrap().parse().unwrap();
    let repository = AttricatRepository::new(pool.clone(), workspace);
    let tags = |record_id: Uuid| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Vec<String>>("SELECT system_tags FROM records WHERE id=$1")
                .bind(record_id)
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    let tag = "attricat.needs_attention".to_owned();

    // A run that opens a finding tags the record in the same transaction.
    manual_run(&base_url, rule_id, Some(record_id), "needs-attention").await;
    let task = repository
        .claim_task("needs-attention", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    rule_runtime::task_handler(repository.clone())
        .handle(task)
        .await
        .unwrap();
    assert_eq!(tags(record_id).await, vec![tag.clone()]);

    // Callers cannot add or remove a Core tag; replacing the list keeps it.
    let put = |body: Value| {
        let client = client.clone();
        let base_url = base_url.clone();
        async move {
            client
                .put(format!("{base_url}/v1/records/{record_id}"))
                .json(&body)
                .send()
                .await
                .unwrap()
        }
    };
    expect_status(
        put(json!({"system_tags": ["reviewed"]})).await,
        StatusCode::OK,
    )
    .await;
    assert_eq!(
        tags(record_id).await,
        vec!["reviewed".to_owned(), tag.clone()]
    );
    let other = create_record(&client, &base_url, &blueprint).await;
    let added = client
        .put(format!(
            "{base_url}/v1/records/{}",
            other["id"].as_str().unwrap()
        ))
        .json(&json!({"system_tags": ["attricat.anything"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(added.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Search may filter by it.
    let found = expect_status(
        client
            .post(format!("{base_url}/v1/records/search"))
            .json(&json!({"blueprint": {"code": "rule_task_product"}, "system_tags": [tag]}))
            .send()
            .await
            .unwrap(),
        StatusCode::OK,
    )
    .await;
    assert_eq!(found["items"].as_array().unwrap().len(), 1);

    // Acknowledging the last open finding removes the tag.
    let finding: Uuid = sqlx::query_scalar("SELECT id FROM rule_findings WHERE record_id=$1")
        .bind(record_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    client
        .post(format!("{base_url}/rule-findings/{finding}/acknowledge"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert_eq!(tags(record_id).await, vec!["reviewed".to_owned()]);

    // Startup repairs a tag that disagrees with the findings.
    sqlx::query("UPDATE records SET system_tags=ARRAY[$2] WHERE id=$1")
        .bind(record_id)
        .bind(&tag)
        .execute(&pool)
        .await
        .unwrap();
    AttricatRepository::system(pool.clone())
        .initialize_workspace(workspace)
        .await
        .unwrap();
    assert!(tags(record_id).await.is_empty());
    server.abort();
}
