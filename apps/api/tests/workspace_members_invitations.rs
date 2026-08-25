mod support;

use chrono::{Duration, Utc};
use support::*;

fn client_for(user_id: Uuid) -> Client {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "x-catalog-user-id",
        reqwest::header::HeaderValue::from_str(&user_id.to_string()).unwrap(),
    );
    headers.insert(
        "x-catalog-workspace-id",
        reqwest::header::HeaderValue::from_static(BOOTSTRAP_WORKSPACE_ID),
    );
    Client::builder().default_headers(headers).build().unwrap()
}

#[sqlx::test]
async fn invitations_are_digest_only_one_time_and_bound_to_verified_email(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let workspace = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let recipient = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email, email_verified_at) VALUES ($1, 'invitee@example.test', now())")
        .bind(recipient).execute(&pool).await.unwrap();

    let created = authenticated_client().post(format!("{base_url}/workspace/invitations"))
        .json(&json!({"email":"Invitee@Example.Test", "role_id":"00000000-0000-4000-8000-000000000104", "scope_type":"workspace", "scope_target_id":workspace, "expires_at": Utc::now() + Duration::hours(1)}))
        .send().await.unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let created: Value = created.json().await.unwrap();
    assert_eq!(created["invitee_email"], "invitee@example.test");
    let secret = created["secret"].as_str().unwrap().to_owned();
    assert!(secret.starts_with("cat_inv_"));
    assert!(created.get("token_digest").is_none());

    let wrong_recipient = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users (id, email, email_verified_at) VALUES ($1, 'other@example.test', now())",
    )
    .bind(wrong_recipient)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        client_for(wrong_recipient)
            .post(format!("{base_url}/workspace/invitations/accept"))
            .json(&json!({"secret":secret}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let accepted = client_for(recipient)
        .post(format!("{base_url}/workspace/invitations/accept"))
        .json(&json!({"secret":secret}))
        .send()
        .await
        .unwrap();
    assert_eq!(accepted.status(), StatusCode::OK);
    assert_eq!(
        client_for(recipient)
            .post(format!("{base_url}/workspace/invitations/accept"))
            .json(&json!({"secret":created["secret"]}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let active: String = sqlx::query_scalar(
        "SELECT state FROM workspace_memberships WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(workspace)
    .bind(recipient)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(active, "active");
    server.abort();
}

#[sqlx::test]
async fn revoked_or_expired_invitations_never_activate_a_member(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let workspace = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let recipient = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email, email_verified_at) VALUES ($1, 'revoked@example.test', now())").bind(recipient).execute(&pool).await.unwrap();
    let created: Value = authenticated_client().post(format!("{base_url}/workspace/invitations"))
        .json(&json!({"email":"revoked@example.test", "role_id":"00000000-0000-4000-8000-000000000104", "scope_type":"workspace", "scope_target_id":workspace, "expires_at":Utc::now() + Duration::hours(1)}))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(
        authenticated_client()
            .delete(format!(
                "{base_url}/workspace/invitations/{}",
                created["id"].as_str().unwrap()
            ))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        client_for(recipient)
            .post(format!("{base_url}/workspace/invitations/accept"))
            .json(&json!({"secret":created["secret"]}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM workspace_memberships WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(workspace)
    .bind(recipient)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
    server.abort();
}
