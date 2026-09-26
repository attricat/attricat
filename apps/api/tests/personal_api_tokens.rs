mod support;

use chrono::{Duration, Utc};
use sha2::{Digest, Sha256};
use support::*;

async fn insert_personal_api_token(
    pool: &PgPool,
    token_id: Uuid,
    user_id: Uuid,
    workspace_id: Uuid,
    label: &str,
    secret: &str,
    expires_at: Option<chrono::DateTime<Utc>>,
) {
    sqlx::query(
        "INSERT INTO personal_api_tokens (id, user_id, workspace_id, label, token_digest, expires_at) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(token_id)
    .bind(user_id)
    .bind(workspace_id)
    .bind(label)
    .bind(Sha256::digest(secret.as_bytes()).as_slice())
    .bind(expires_at)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO personal_api_token_permissions (token_id, permission_code) VALUES ($1, 'contexts.read')",
    )
    .bind(token_id)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test]
async fn personal_api_tokens_are_one_time_secrets_and_enforce_permission_subsets(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let owner = authenticated_client();
    let created = owner
        .post(format!("{base_url}/personal-access-tokens"))
        .json(&json!({"label":"CLI", "permissions":["blueprints.read"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let created = created.json::<Value>().await.unwrap();
    let secret = created["secret"].as_str().unwrap().to_owned();
    assert!(secret.starts_with("cat_pat_"));
    let token_id = created["id"].as_str().unwrap();

    let listed = owner
        .get(format!("{base_url}/personal-access-tokens"))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(listed[0]["id"], token_id);
    assert!(listed[0].get("secret").is_none());
    let digest_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM personal_api_tokens WHERE token_digest = $1")
            .bind(Sha256::digest(secret.as_bytes()).as_slice())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(digest_count, 1);

    let token = Client::builder()
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                "authorization",
                reqwest::header::HeaderValue::from_str(&format!("Bearer {secret}")).unwrap(),
            );
            headers
        })
        .build()
        .unwrap();
    let session = token
        .get(format!("{base_url}/auth/session"))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(session["capabilities"]["tokens_manage"], false);
    assert_eq!(session["capabilities"]["roles_grant"], false);
    assert_eq!(
        token
            .get(format!("{base_url}/blueprints"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        token
            .post(format!("{base_url}/blueprints"))
            .json(&json!({"definition":"kind = \"entity\"\ncode = \"nope\""}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );

    assert_eq!(
        owner
            .delete(format!("{base_url}/personal-access-tokens/{token_id}"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        token
            .get(format!("{base_url}/blueprints"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    server.abort();
}

#[sqlx::test]
async fn token_issuance_rejects_permissions_the_owner_cannot_delegate(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let user_id = Uuid::new_v4();
    let membership_id = Uuid::new_v4();
    let role_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, 'token-issuer@example.test')")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(membership_id)
    .bind(workspace_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO roles (id, code, workspace_id, is_system) VALUES ($1, 'token_issuer', $2, false)")
        .bind(role_id)
        .bind(workspace_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO role_permissions (role_id, permission_code) VALUES ($1, 'tokens.manage')",
    )
    .bind(role_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, $4, 'workspace', $2)")
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(membership_id)
        .bind(role_id)
        .execute(&pool)
        .await
        .unwrap();
    let secret = "cat_pat_token_issuer";
    let token_id = Uuid::new_v4();
    insert_personal_api_token(
        &pool,
        token_id,
        user_id,
        workspace_id,
        "issuer",
        secret,
        None,
    )
    .await;
    sqlx::query("INSERT INTO personal_api_token_permissions (token_id, permission_code) VALUES ($1, 'tokens.manage')")
        .bind(token_id)
        .execute(&pool)
        .await
        .unwrap();
    let token = Client::builder()
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                "authorization",
                reqwest::header::HeaderValue::from_static("Bearer cat_pat_token_issuer"),
            );
            headers
        })
        .build()
        .unwrap();
    assert_eq!(
        token
            .post(format!("{base_url}/personal-access-tokens"))
            .json(&json!({"label":"escalation", "permissions":["blueprints.read"]}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    server.abort();
}

#[sqlx::test]
async fn expired_tokens_fail_and_token_auth_still_obeys_scoped_grants(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let user_id = Uuid::new_v4();
    let membership_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, 'token-scope@example.test')")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(membership_id)
    .bind(workspace_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();
    let root = authenticated_client().post(format!("{base_url}/contexts")).json(&json!({"code":"token-root","data":{},"parent_id":"00000000-0000-4000-8000-000000000001"}))
        .send().await.unwrap().json::<Value>().await.unwrap();
    let child = authenticated_client()
        .post(format!("{base_url}/contexts"))
        .json(&json!({"code":"token-child","data":{},"parent_id":root["id"]}))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, '00000000-0000-4000-8000-000000000104', 'context_subtree', $4)")
        .bind(Uuid::new_v4()).bind(workspace_id).bind(membership_id).bind(child["id"].as_str().unwrap().parse::<Uuid>().unwrap()).execute(&pool).await.unwrap();

    let secret = "cat_pat_test_scoped";
    insert_personal_api_token(
        &pool,
        Uuid::new_v4(),
        user_id,
        workspace_id,
        "scoped",
        secret,
        None,
    )
    .await;
    let token = Client::builder()
        .default_headers({
            let mut h = reqwest::header::HeaderMap::new();
            h.insert(
                "authorization",
                reqwest::header::HeaderValue::from_static("Bearer cat_pat_test_scoped"),
            );
            h
        })
        .build()
        .unwrap();
    assert_eq!(
        token
            .get(format!("{base_url}/contexts/token-child"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        token
            .get(format!("{base_url}/contexts/token-root"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );

    let expired = "cat_pat_expired";
    insert_personal_api_token(
        &pool,
        Uuid::new_v4(),
        user_id,
        workspace_id,
        "expired",
        expired,
        Some(Utc::now() + Duration::seconds(1)),
    )
    .await;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let expired_client = Client::builder()
        .default_headers({
            let mut h = reqwest::header::HeaderMap::new();
            h.insert(
                "authorization",
                reqwest::header::HeaderValue::from_static("Bearer cat_pat_expired"),
            );
            h
        })
        .build()
        .unwrap();
    assert_eq!(
        expired_client
            .get(format!("{base_url}/contexts/token-child"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    server.abort();
}

#[sqlx::test]
async fn member_management_token_cannot_grant_or_revoke_without_roles_grant(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let workspace: Uuid = BOOTSTRAP_WORKSPACE_ID.parse().unwrap();
    let owner: Uuid = BOOTSTRAP_OWNER_ID.parse().unwrap();
    let member: Uuid = sqlx::query_scalar(
        "SELECT id FROM workspace_memberships WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(workspace)
    .bind(owner)
    .fetch_one(&pool)
    .await
    .unwrap();
    let owner_client = authenticated_client();
    let restricted = owner_client
        .post(format!("{base_url}/personal-access-tokens"))
        .json(&json!({"label":"members only", "permissions":["members.manage"]}))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let grant_only = owner_client
        .post(format!("{base_url}/personal-access-tokens"))
        .json(&json!({"label":"grants only", "permissions":["roles.grant"]}))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let permitted = owner_client
        .post(format!("{base_url}/personal-access-tokens"))
        .json(&json!({"label":"role grants", "permissions":["members.manage", "roles.grant"]}))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let grant_url = format!("{base_url}/workspace/members/{member}/grants");
    let grant = json!({
        "role_id":"00000000-0000-4000-8000-000000000104",
        "scope_type":"workspace", "scope_target_id":workspace
    });
    assert_eq!(
        Client::new()
            .post(&grant_url)
            .bearer_auth(restricted["secret"].as_str().unwrap())
            .json(&grant)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        Client::new()
            .post(&grant_url)
            .bearer_auth(grant_only["secret"].as_str().unwrap())
            .json(&grant)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let created = Client::new()
        .post(&grant_url)
        .bearer_auth(permitted["secret"].as_str().unwrap())
        .json(&grant)
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let id = created.json::<Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let revoke_url = format!("{grant_url}/{id}");
    assert_eq!(
        Client::new()
            .delete(&revoke_url)
            .bearer_auth(restricted["secret"].as_str().unwrap())
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        Client::new()
            .delete(&revoke_url)
            .bearer_auth(permitted["secret"].as_str().unwrap())
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    server.abort();
}
