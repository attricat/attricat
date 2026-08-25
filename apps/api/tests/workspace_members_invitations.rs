mod support;

use chrono::{Duration, Utc};
use support::*;

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
    assert!(created.get("secret").is_none());
    assert!(created.get("token_digest").is_none());
    let invitation_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM workspace_invitations WHERE workspace_id = $1 AND invitee_email = 'invitee@example.test'",
    )
    .bind(workspace)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(invitation_count, 1);
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
    assert!(created.get("secret").is_none());
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
