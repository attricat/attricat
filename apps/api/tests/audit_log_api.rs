mod support;

use axum::http::StatusCode;
use support::{BOOTSTRAP_WORKSPACE_ID, Uuid, authenticated_client, start_server};

#[sqlx::test(migrations = "./migrations")]
async fn owner_lists_tenant_scoped_audit_events_with_filters(pool: sqlx::PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let created = authenticated_client()
        .post(format!("{base_url}/contexts"))
        .json(&support::json!({"code": "audit-log-context", "data": {}}))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);

    let response = authenticated_client()
        .get(format!(
            "{base_url}/audit-events?action_category=attricat&limit=1"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let page: support::Value = response.json().await.unwrap();
    assert_eq!(page["limit"], 1);
    assert!(page["total"].as_i64().unwrap() >= 1);
    let event = &page["events"][0];
    assert_eq!(event["executor_type"], "human");
    assert_eq!(event["outcome"], "success");
    assert!(event["request_id"].is_string());
    assert!(event["correlation_id"].is_string());

    let invalid = authenticated_client()
        .get(format!("{base_url}/audit-events?executor_type=service"))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let viewer_id = Uuid::new_v4();
    let membership_id = Uuid::new_v4();
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, $2)")
        .bind(viewer_id)
        .bind("audit-viewer@example.test")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(membership_id)
    .bind(workspace_id)
    .bind(viewer_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, '00000000-0000-4000-8000-000000000104', 'workspace', $2)")
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(membership_id)
        .execute(&pool)
        .await
        .unwrap();
    let forbidden = reqwest::Client::new()
        .get(format!("{base_url}/audit-events"))
        .header("x-attricat-user-id", viewer_id.to_string())
        .header("x-attricat-workspace-id", workspace_id.to_string())
        .send()
        .await
        .unwrap();
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
    server.abort();
}
