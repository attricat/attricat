mod support;

use std::time::Duration;

use api::{account::SessionSecret, repository::CatalogRepository};
use chrono::Utc;
use support::*;

async fn run(repository: &CatalogRepository, record: Option<Uuid>) -> Uuid {
    let owner = BOOTSTRAP_OWNER_ID.parse().unwrap();
    let conversation = if let Some(record) = record {
        repository
            .create_record_conversation(owner, "Stream", record, None)
            .await
            .unwrap()
    } else {
        repository
            .create_conversation(Some(owner), "Stream")
            .await
            .unwrap()
    };
    let run = repository
        .create_agent_run_for_user(conversation.id, owner, "http://127.0.0.1:1/v1", "test")
        .await
        .unwrap();
    repository
        .append_run_event(
            run.id,
            "message_delta",
            json!({"text":"visible-before-revocation"}),
        )
        .await
        .unwrap();
    run.id
}

async fn open_stream(client: &Client, base: &str, run: Uuid) -> reqwest::Response {
    let mut response = client
        .get(format!("{base}/agent/runs/{run}/events"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut received = String::new();
        while !received.contains("visible-before-revocation") {
            let chunk = response
                .chunk()
                .await
                .unwrap()
                .expect("stream ended before its first event");
            received.push_str(&String::from_utf8_lossy(&chunk));
        }
    })
    .await
    .unwrap();
    response
}

async fn assert_stream_closed(response: reqwest::Response) {
    let tail = tokio::time::timeout(Duration::from_secs(5), response.text())
        .await
        .expect("an already-open stream retained revoked authority")
        .unwrap();
    assert!(
        tail.contains("event: error"),
        "stream must report abnormal termination: {tail}"
    );
}

#[sqlx::test]
async fn open_agent_streams_stop_after_token_revocation_or_expiry(pool: PgPool) {
    let (base, server) = start_server(pool.clone()).await;
    let owner = authenticated_client();
    let repository = CatalogRepository::new(pool.clone(), bootstrap_workspace_id());
    let run = run(&repository, None).await;
    for expire in [false, true] {
        let token: Value = owner
            .post(format!("{base}/personal-access-tokens"))
            .json(&json!({"label":"stream", "permissions":["agents.run"]}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        let id: Uuid = token["id"].as_str().unwrap().parse().unwrap();
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "authorization",
            format!("Bearer {}", token["secret"].as_str().unwrap())
                .parse()
                .unwrap(),
        );
        let client = Client::builder().default_headers(headers).build().unwrap();
        let response = open_stream(&client, &base, run).await;
        if expire {
            sqlx::query("UPDATE personal_api_tokens SET expires_at=clock_timestamp()+interval '100 milliseconds' WHERE id=$1").bind(id).execute(&pool).await.unwrap();
        } else {
            owner
                .delete(format!("{base}/personal-access-tokens/{id}"))
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap();
        }
        assert_stream_closed(response).await;
        assert_eq!(
            client
                .get(format!("{base}/agent/runs/{run}/events"))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    server.abort();
}

#[sqlx::test]
async fn open_agent_streams_stop_after_browser_session_revocation_or_expiry(pool: PgPool) {
    let (base, server) =
        start_server_with_config(pool.clone(), |state| state.allow_trusted_headers = false).await;
    let repository = CatalogRepository::new(pool.clone(), bootstrap_workspace_id());
    repository
        .ensure_bootstrap_local_password(
            "api-test-owner@example.test",
            "stream-test-password".into(),
        )
        .await
        .unwrap();
    let credential = repository
        .local_login_credential("api-test-owner@example.test")
        .await
        .unwrap()
        .unwrap();
    let run = run(&repository, None).await;
    for expire in [false, true] {
        let id = Uuid::new_v4();
        let session = SessionSecret::generate();
        repository
            .issue_login_session(
                id,
                &credential,
                bootstrap_workspace_id(),
                &session.digest(),
                &SessionSecret::generate().digest(),
                Utc::now() + chrono::Duration::hours(1),
            )
            .await
            .unwrap();
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "cookie",
            format!("catalog_session={}", session.expose_for_delivery())
                .parse()
                .unwrap(),
        );
        let client = Client::builder().default_headers(headers).build().unwrap();
        let response = open_stream(&client, &base, run).await;
        if expire {
            sqlx::query("UPDATE browser_sessions SET expires_at=clock_timestamp()+interval '100 milliseconds' WHERE id=$1").bind(id).execute(&pool).await.unwrap();
        } else {
            repository
                .revoke_browser_session(&session.digest(), bootstrap_workspace_id())
                .await
                .unwrap();
        }
        assert_stream_closed(response).await;
        assert_eq!(
            client
                .get(format!("{base}/agent/runs/{run}/events"))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    server.abort();
}

#[sqlx::test]
async fn open_agent_streams_stop_when_agent_permission_is_removed(pool: PgPool) {
    let (base, server) = start_server(pool.clone()).await;
    let repository = CatalogRepository::new(pool.clone(), bootstrap_workspace_id());
    let (user, membership) = add_workspace_user(&pool).await;
    let role = create_role(&pool, "stream_agent", &["agents.run"]).await;
    let grant = grant_role(&pool, membership, role, GrantScope::Workspace).await;
    let client = client_for(user);
    let run = run(&repository, None).await;
    let response = open_stream(&client, &base, run).await;
    authenticated_client()
        .delete(format!(
            "{base}/workspace/members/{membership}/grants/{grant}"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert_stream_closed(response).await;
    assert_eq!(
        client
            .get(format!("{base}/agent/runs/{run}/events"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    server.abort();
}

#[sqlx::test]
async fn open_agent_streams_stop_when_the_conversation_record_becomes_unreadable(pool: PgPool) {
    let (base, server) = start_server(pool.clone()).await;
    let owner = authenticated_client();
    let blueprint = create_blueprint(&owner, &base, "format_version = 1\ncode = 'stream_record'\nname = 'Record'\nkind = 'record'\n\n[views.dropdown_option]\ntype = 'dropdown_option'\nfields = ['title']\n\n[[attributes]]\ncode = 'title'\nvalue_type = 'string'").await;
    let record: Uuid = create_record(&owner, &base, &blueprint).await["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let repository = CatalogRepository::new(pool.clone(), bootstrap_workspace_id());
    let (user, membership) = add_workspace_user(&pool).await;
    let role = create_role(&pool, "stream_agent", &["agents.run"]).await;
    grant_role(&pool, membership, role, GrantScope::Workspace).await;
    let grant = grant_role(
        &pool,
        membership,
        VIEWER_ROLE_ID,
        GrantScope::Record(record),
    )
    .await;
    let client = client_for(user);
    let run = run(&repository, Some(record)).await;
    let response = open_stream(&client, &base, run).await;
    owner
        .delete(format!(
            "{base}/workspace/members/{membership}/grants/{grant}"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert_stream_closed(response).await;
    assert_eq!(
        client
            .get(format!("{base}/agent/runs/{run}/events"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    server.abort();
}
