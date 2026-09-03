mod support;

use std::sync::Arc;

use api::{
    agent_provider::OpenAiCompatibleClient,
    agent_runner,
    agents::AgentProviderConfig,
    repository::{ApprovalDecision, CatalogRepository},
    storage::{FakeObjectStore, ObjectStore},
};
use axum::{Router, routing::post};
use support::{BOOTSTRAP_OWNER_ID, BOOTSTRAP_WORKSPACE_ID, authenticated_client, start_server};
use tokio::net::TcpListener;
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

async fn completed_provider() -> &'static str {
    "data: {\"choices\":[{\"delta\":{\"content\":\"Done\"}}]}\n\ndata: [DONE]\n\n"
}

#[sqlx::test(migrations = "./migrations")]
async fn approved_agent_mutation_has_explicit_redacted_provenance(pool: sqlx::PgPool) {
    let (base_url, api_server) = start_server(pool.clone()).await;
    let conversation: support::Value = authenticated_client()
        .post(format!("{base_url}/agent/conversations"))
        .json(&support::json!({"title": "Audited agent"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let provider_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let provider_address = provider_listener.local_addr().unwrap();
    let provider_server = tokio::spawn(async move {
        axum::serve(
            provider_listener,
            Router::new().route("/v1/chat/completions", post(completed_provider)),
        )
        .await
        .unwrap();
    });
    let config = AgentProviderConfig::from_values(|name| match name {
        "LLM_API_KEY" => Some("test-key".to_owned()),
        "LLM_BASE_URL" => Some(format!("http://{provider_address}/v1")),
        _ => None,
    })
    .unwrap()
    .unwrap();
    let workspace = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let actor = BOOTSTRAP_OWNER_ID.parse::<Uuid>().unwrap();
    let repository = CatalogRepository::new(pool.clone())
        .for_workspace(workspace)
        .await
        .unwrap();
    let run = repository
        .create_agent_run_for_user(
            conversation["id"].as_str().unwrap().parse().unwrap(),
            actor,
            config.base_url.as_str(),
            &config.model,
        )
        .await
        .unwrap();
    let call = repository
        .create_agent_tool_call(
            run.id,
            Some("provider-call"),
            "create_context",
            support::json!({"code": "agent-audited", "data": {"token": "not-audit-metadata"}}),
            Some("Create context agent-audited"),
            "pending_approval",
        )
        .await
        .unwrap();
    let _approved = repository
        .decide_tool_call(call.id, actor, ApprovalDecision::Approve)
        .await
        .unwrap();
    let object_store: Arc<dyn ObjectStore> = Arc::new(FakeObjectStore::available());
    agent_runner::resume(
        &repository,
        &OpenAiCompatibleClient::new(&config).unwrap(),
        &object_store,
        run.id,
    )
    .await
    .unwrap();

    let event = sqlx::query_as::<_, (Option<Uuid>, String, Uuid, Uuid, Uuid, String, String, Option<Uuid>, support::Value)>(
        "SELECT actor_user_id, executor_type, agent_run_id, agent_conversation_id, agent_tool_call_id, agent_tool_name, approval_decision, approved_by_user_id, metadata FROM audit_events WHERE agent_tool_call_id = $1",
    )
    .bind(call.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(event.0, Some(actor));
    assert_eq!(event.1, "agent");
    assert_eq!(event.2, run.id);
    assert_eq!(
        event.3,
        conversation["id"]
            .as_str()
            .unwrap()
            .parse::<Uuid>()
            .unwrap()
    );
    assert_eq!(event.4, call.id);
    assert_eq!(event.5, "create_context");
    assert_eq!(event.6, "approved");
    assert_eq!(event.7, Some(actor));
    assert_eq!(event.8, support::json!({}));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM attribute_contexts WHERE code = 'agent-audited'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );

    // Agent mutation audits use the same transaction-owned repository path:
    // rejecting the audit insert must leave the requested catalog change out.
    sqlx::query(
        "ALTER TABLE audit_events ADD CONSTRAINT audit_events_agent_test_reject CHECK (false) NOT VALID",
    )
    .execute(&pool)
    .await
    .unwrap();
    let rollback_run = repository
        .create_agent_run_for_user(
            conversation["id"].as_str().unwrap().parse().unwrap(),
            actor,
            config.base_url.as_str(),
            &config.model,
        )
        .await
        .unwrap();
    let rollback_call = repository
        .create_agent_tool_call(
            rollback_run.id,
            Some("provider-call-rollback"),
            "create_context",
            support::json!({"code": "agent-audit-rollback", "data": {}}),
            Some("Create context agent-audit-rollback"),
            "pending_approval",
        )
        .await
        .unwrap();
    repository
        .decide_tool_call(rollback_call.id, actor, ApprovalDecision::Approve)
        .await
        .unwrap();
    agent_runner::resume(
        &repository,
        &OpenAiCompatibleClient::new(&config).unwrap(),
        &object_store,
        rollback_run.id,
    )
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM attribute_contexts WHERE code = 'agent-audit-rollback'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    api_server.abort();
    provider_server.abort();
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
