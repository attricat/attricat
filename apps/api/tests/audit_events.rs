mod support;

use support::{BOOTSTRAP_OWNER_ID, BOOTSTRAP_WORKSPACE_ID, authenticated_client, start_server};
use uuid::Uuid;

#[sqlx::test(migrations = "./migrations")]
async fn catalog_mutation_creates_a_redacted_audit_event(pool: sqlx::PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let request_id = Uuid::new_v4();
    let correlation_id = Uuid::new_v4();
    let response = authenticated_client()
        .post(format!("{base_url}/blueprints"))
        .header("x-request-id", request_id.to_string())
        .header("x-correlation-id", correlation_id.to_string())
        .json(&support::json!({
            "definition": "format_version = 1\ncode = 'audit_product'\nname = 'Audit product'\nkind = 'entity'\n\n[views.dropdown_option]\ntype = 'dropdown_option'\nfields = ['title']\n\n[[attributes]]\ncode = 'title'\nvalue_type = 'string'"
        }))
        .send()
        .await
        .unwrap();
    let status = response.status();
    let body = response.text().await.unwrap();
    assert_eq!(status, support::StatusCode::CREATED, "{body}");

    let event = sqlx::query_as::<_, (Uuid, Option<Uuid>, Uuid, Uuid, String, String, support::Value)>(
        "SELECT workspace_id, actor_user_id, request_id, correlation_id, action, outcome, metadata FROM audit_events WHERE request_id = $1",
    )
    .bind(request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(event.0, BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap());
    assert_eq!(event.1, Some(BOOTSTRAP_OWNER_ID.parse::<Uuid>().unwrap()));
    assert_eq!(event.2, request_id);
    assert_eq!(event.3, correlation_id);
    assert_eq!(event.4, "catalog.blueprints.create_or_apply");
    assert_eq!(event.5, "success");
    assert_eq!(
        event.6,
        support::json!({ "method": "POST", "route": "/blueprints" })
    );

    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn lifecycle_audit_trigger_does_not_store_a_credential_hash(pool: sqlx::PgPool) {
    let user_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, 'audit-user@example.test')")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    let hash = "$argon2id$v=19$m=19456,t=2,p=1$c2FsdHNh$bG9uZ2hhc2h2YWx1ZQ";
    sqlx::query("INSERT INTO local_password_credentials (user_id, password_hash, credential_version) VALUES ($1, $2, 1)")
        .bind(user_id)
        .bind(hash)
        .execute(&pool)
        .await
        .unwrap();

    let metadata: support::Value = sqlx::query_scalar(
        "SELECT metadata FROM audit_events WHERE action = 'security.credential.changed' AND target->>'id' = $1 ORDER BY occurred_at DESC LIMIT 1",
    )
    .bind(user_id.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(metadata, support::json!({}));
    let serialized_event: String = sqlx::query_scalar(
        "SELECT row_to_json(audit_events)::text FROM audit_events WHERE action = 'security.credential.changed' AND target->>'id' = $1 ORDER BY occurred_at DESC LIMIT 1",
    )
    .bind(user_id.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!serialized_event.contains(hash));
}
