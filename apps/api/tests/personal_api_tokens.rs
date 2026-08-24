mod support;

use chrono::{Duration, Utc};
use sha2::{Digest, Sha256};
use support::*;

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
    sqlx::query("SELECT issue_personal_api_token($1, $2, $3, $4, $5, $6, $7)")
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(workspace_id)
        .bind("scoped")
        .bind(Sha256::digest(secret.as_bytes()).as_slice())
        .bind(vec!["contexts.read"])
        .bind(Option::<chrono::DateTime<Utc>>::None)
        .execute(&pool)
        .await
        .unwrap();
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
    sqlx::query("SELECT issue_personal_api_token($1, $2, $3, $4, $5, $6, $7)")
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(workspace_id)
        .bind("expired")
        .bind(Sha256::digest(expired.as_bytes()).as_slice())
        .bind(vec!["contexts.read"])
        .bind(Utc::now() + Duration::seconds(1))
        .execute(&pool)
        .await
        .unwrap();
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
