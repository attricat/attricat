mod support;

use support::*;

const ENDPOINT: &str = "/data-health/background-processing";

#[sqlx::test]
async fn background_processing_requires_identity_and_data_health_permission(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    assert_eq!(
        Client::new()
            .get(format!("{base_url}{ENDPOINT}"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let user_id = Uuid::new_v4();
    let workspace_id: Uuid = BOOTSTRAP_WORKSPACE_ID.parse().unwrap();
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, 'queue-reader@example.test')")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    let membership_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(membership_id)
    .bind(workspace_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();
    let client = Client::new();
    let response = client
        .get(format!("{base_url}{ENDPOINT}"))
        .header("x-catalog-user-id", user_id.to_string())
        .header("x-catalog-workspace-id", workspace_id.to_string())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    // The reader role grants data_health.read without task-management privileges.
    sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, '00000000-0000-4000-8000-000000000104', 'workspace', $2)")
        .bind(Uuid::new_v4()).bind(workspace_id).bind(membership_id).execute(&pool).await.unwrap();
    let response = client
        .get(format!("{base_url}{ENDPOINT}"))
        .header("x-catalog-user-id", user_id.to_string())
        .header("x-catalog-workspace-id", workspace_id.to_string())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.json::<Value>().await.unwrap(), json!([]));
    server.abort();
}

async fn task(pool: &PgPool, workspace_id: Uuid, kind: &str, status: &str, offset: i32) {
    sqlx::query(
        "INSERT INTO tasks (id, workspace_id, kind, subject_id, generation, envelope_version, max_failures, status, available_at, lease_owner, lease_token, lease_until, payload, last_error_code, last_error_message)
         VALUES ($1, $2, $3, $4, 0, 1, 3, $5, now() + ($6 * interval '1 second'),
           CASE WHEN $5 = 'leased' THEN 'private-worker' END,
           CASE WHEN $5 = 'leased' THEN $1 END,
           CASE WHEN $5 = 'leased' THEN now() + ($6 * interval '1 second') END,
           '{\"secret\":\"not-for-the-ui\"}', 'private-error', 'secret raw exception')",
    ).bind(Uuid::new_v4()).bind(workspace_id).bind(kind).bind(Uuid::new_v4())
        .bind(status).bind(offset).execute(pool).await.unwrap();
}

#[sqlx::test]
async fn background_processing_is_workspace_scoped_and_excludes_finished_tasks_and_secrets(
    pool: PgPool,
) {
    let workspace_id: Uuid = BOOTSTRAP_WORKSPACE_ID.parse().unwrap();
    let other_workspace = Uuid::new_v4();
    sqlx::query("INSERT INTO workspaces (id, slug, name, login_identifier) VALUES ($1, 'other-queue', 'Other queue', 'other-queue.local')")
        .bind(other_workspace).execute(&pool).await.unwrap();
    task(&pool, workspace_id, "rule_run.v1", "queued", -120).await;
    task(&pool, workspace_id, "rule_run.v1", "queued", 3600).await;
    task(&pool, workspace_id, "rule_run.v1", "leased", -60).await;
    task(&pool, workspace_id, "rule_run.v1", "leased", 3600).await;
    task(&pool, workspace_id, "rule_run.v1", "dead_letter", -60).await;
    task(&pool, workspace_id, "rule_run.v1", "succeeded", -60).await;
    task(&pool, workspace_id, "rule_run.v1", "cancelled", -60).await;
    task(&pool, workspace_id, "workflow_run.v1", "queued", 3600).await;
    task(&pool, workspace_id, "agent_run.v1", "succeeded", -60).await;
    for status in ["queued", "leased", "dead_letter"] {
        task(&pool, other_workspace, "rule_run.v1", status, -9000).await;
        task(&pool, other_workspace, "event_delivery.v1", status, -9000).await;
    }
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let response = client
        .get(format!("{base_url}{ENDPOINT}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    let rows = body.as_array().unwrap();
    assert_eq!(rows.len(), 2);
    let row = &rows[0];
    assert_eq!(row["kind"], "rule_run.v1");
    assert_eq!(row["queued"], 2);
    assert_eq!(row["running"], 2);
    assert_eq!(row["failed"], 1);
    assert_eq!(row["expired_leases"], 1);
    let wait = row["oldest_due_seconds"].as_f64().unwrap();
    assert!((120.0..300.0).contains(&wait));
    assert_eq!(row.as_object().unwrap().len(), 6);
    assert_eq!(
        rows[1],
        json!({"kind":"workflow_run.v1", "queued":1, "running":0, "failed":0, "expired_leases":0, "oldest_due_seconds":null})
    );
    for forbidden in [
        "secret",
        "private",
        "payload",
        "workspace_id",
        "subject_id",
        "last_error",
    ] {
        assert!(!body.to_string().contains(forbidden));
    }
    // No data-health cache: refreshing reflects subsequent queue changes.
    sqlx::query(
        "UPDATE tasks SET status = 'succeeded' WHERE workspace_id = $1 AND status = 'queued'",
    )
    .bind(workspace_id)
    .execute(&pool)
    .await
    .unwrap();
    let refreshed: Value = client
        .get(format!("{base_url}{ENDPOINT}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(refreshed.as_array().unwrap().len(), 1);
    assert_eq!(refreshed[0]["queued"], 0);
    assert_eq!(refreshed[0]["oldest_due_seconds"], Value::Null);
    server.abort();
}
