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
async fn audit_insert_failure_rolls_back_the_mutation(pool: sqlx::PgPool) {
    sqlx::query(
        "ALTER TABLE audit_events ADD CONSTRAINT audit_events_test_reject CHECK (false) NOT VALID",
    )
    .execute(&pool)
    .await
    .unwrap();
    let (base_url, server) = start_server(pool.clone()).await;
    let response = authenticated_client()
        .post(format!("{base_url}/blueprints"))
        .json(&support::json!({
            "definition": "format_version = 1\ncode = 'audit_rollback'\nname = 'Audit rollback'\nkind = 'entity'\n\n[views.dropdown_option]\ntype = 'dropdown_option'\nfields = ['title']\n\n[[attributes]]\ncode = 'title'\nvalue_type = 'string'"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        support::StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM blueprints WHERE code = 'audit_rollback'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn context_and_token_revocation_audits_are_transaction_owned(pool: sqlx::PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let context_request_id = Uuid::new_v4();
    let context = client
        .post(format!("{base_url}/contexts"))
        .header("x-request-id", context_request_id.to_string())
        .json(&support::json!({
            "code": "audited_context",
            "data": {},
            "parent_id": "00000000-0000-4000-8000-000000000001"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(context.status(), support::StatusCode::CREATED);

    let token_id = Uuid::new_v4();
    sqlx::query("INSERT INTO personal_api_tokens (id, user_id, workspace_id, label, token_digest) VALUES ($1, $2, $3, 'audit revoke', $4)")
        .bind(token_id)
        .bind(BOOTSTRAP_OWNER_ID.parse::<Uuid>().unwrap())
        .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
        .bind(vec![7_u8; 32])
        .execute(&pool)
        .await
        .unwrap();
    let token_request_id = Uuid::new_v4();
    let revoke = client
        .delete(format!("{base_url}/personal-access-tokens/{token_id}"))
        .header("x-request-id", token_request_id.to_string())
        .send()
        .await
        .unwrap();
    assert_eq!(revoke.status(), support::StatusCode::NO_CONTENT);

    for request_id in [context_request_id, token_request_id] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM audit_events WHERE request_id = $1")
                .bind(request_id)
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
    }
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn invitation_acceptance_audits_the_invitation_workspace(pool: sqlx::PgPool) {
    use reqwest::header::{HeaderMap, HeaderValue};

    let invitation_workspace = Uuid::new_v4();
    let inviter = BOOTSTRAP_OWNER_ID.parse::<Uuid>().unwrap();
    let invitee = Uuid::new_v4();
    let membership = Uuid::new_v4();
    sqlx::query("INSERT INTO workspaces (id, slug, name, login_identifier) VALUES ($1, 'audit-invitation', 'Audit invitation', 'audit-invitation.local')")
        .bind(invitation_workspace)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, 'audit-inviter@example.test')")
        .bind(inviter)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(membership)
    .bind(invitation_workspace)
    .bind(inviter)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, '00000000-0000-4000-8000-000000000101', 'workspace', $2)")
        .bind(Uuid::new_v4())
        .bind(invitation_workspace)
        .bind(membership)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users (id, email, email_verified_at) VALUES ($1, 'audit-invitee@example.test', clock_timestamp())")
        .bind(invitee)
        .execute(&pool)
        .await
        .unwrap();

    let (base_url, server) = start_server(pool.clone()).await;
    let mut inviter_headers = HeaderMap::new();
    inviter_headers.insert(
        "x-catalog-user-id",
        HeaderValue::from_str(&inviter.to_string()).unwrap(),
    );
    inviter_headers.insert(
        "x-catalog-workspace-id",
        HeaderValue::from_str(&invitation_workspace.to_string()).unwrap(),
    );
    let inviter_client = support::Client::builder()
        .default_headers(inviter_headers)
        .build()
        .unwrap();
    let created = inviter_client
        .post(format!("{base_url}/workspace/invitations"))
        .json(&support::json!({
            "email": "audit-invitee@example.test",
            "role_id": "00000000-0000-4000-8000-000000000101",
            "scope_type": "workspace",
            "scope_target_id": invitation_workspace,
            "expires_at": (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339(),
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        created.status(),
        support::StatusCode::CREATED,
        "{}",
        created.text().await.unwrap()
    );
    let secret = created.json::<support::Value>().await.unwrap()["secret"]
        .as_str()
        .unwrap()
        .to_owned();

    let mut invitee_headers = HeaderMap::new();
    invitee_headers.insert(
        "x-catalog-user-id",
        HeaderValue::from_str(&invitee.to_string()).unwrap(),
    );
    invitee_headers.insert(
        "x-catalog-workspace-id",
        HeaderValue::from_static(BOOTSTRAP_WORKSPACE_ID),
    );
    let invitee_client = support::Client::builder()
        .default_headers(invitee_headers)
        .build()
        .unwrap();
    let request_id = Uuid::new_v4();
    let accepted = invitee_client
        .post(format!("{base_url}/workspace/invitations/accept"))
        .header("x-request-id", request_id.to_string())
        .json(&support::json!({ "secret": secret }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        accepted.status(),
        support::StatusCode::OK,
        "{}",
        accepted.text().await.unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, Uuid>(
            "SELECT workspace_id FROM audit_events WHERE request_id = $1"
        )
        .bind(request_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        invitation_workspace
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn denied_mutations_are_not_audited(pool: sqlx::PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let request_id = Uuid::new_v4();
    let response = support::Client::new()
        .post(format!("{base_url}/blueprints"))
        .header("x-request-id", request_id.to_string())
        .json(&support::json!({ "definition": "format_version = 1" }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), support::StatusCode::UNAUTHORIZED);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM audit_events WHERE request_id = $1")
            .bind(request_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    server.abort();
}
