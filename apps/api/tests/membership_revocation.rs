mod support;

use std::time::Duration;

use api::repository::{AttricatRepository, RepositoryError};
use chrono::Utc;
use sha2::{Digest, Sha256};
use support::*;

async fn member(pool: &PgPool) -> (Uuid, Uuid, Uuid) {
    let (user, membership) = add_workspace_user(pool).await;
    sqlx::query("UPDATE users SET email_verified_at=clock_timestamp() WHERE id=$1")
        .bind(user)
        .execute(pool)
        .await
        .unwrap();
    let viewer = grant_role(pool, membership, VIEWER_ROLE_ID, GrantScope::Workspace).await;
    let tokens = create_role(pool, "member_tokens", &["tokens.manage"]).await;
    grant_role(pool, membership, tokens, GrantScope::Workspace).await;
    (user, membership, viewer)
}

async fn invitation(repository: &AttricatRepository, user: Uuid) -> String {
    let secret = format!("cat_inv_{}", Uuid::new_v4());
    repository
        .create_workspace_invitation(
            Uuid::new_v4(),
            BOOTSTRAP_OWNER_ID.parse().unwrap(),
            bootstrap_workspace_id(),
            &format!("{user}@example.test"),
            VIEWER_ROLE_ID,
            "workspace",
            bootstrap_workspace_id(),
            &Sha256::digest(secret.as_bytes()),
            Utc::now() + chrono::Duration::hours(1),
        )
        .await
        .unwrap();
    secret
}

async fn token_client(user: Uuid, base: &str) -> Client {
    let token: Value = client_for(user)
        .post(format!("{base}/personal-access-tokens"))
        .json(&json!({"label":"revocation", "permissions":["records.read"]}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "authorization",
        format!("Bearer {}", token["secret"].as_str().unwrap())
            .parse()
            .unwrap(),
    );
    Client::builder().default_headers(headers).build().unwrap()
}

async fn accept(client: &Client, base: &str, secret: &str) -> reqwest::Response {
    client
        .post(format!("{base}/workspace/invitations/accept"))
        .json(&json!({"secret":secret}))
        .send()
        .await
        .unwrap()
}

#[sqlx::test]
async fn old_invitation_cannot_reactivate_a_disabled_member_but_a_new_one_can(pool: PgPool) {
    let (base, server) = start_server(pool.clone()).await;
    let repository = AttricatRepository::new(pool.clone(), bootstrap_workspace_id());
    let (user, membership, _) = member(&pool).await;
    let client = token_client(user, &base).await;
    let old = invitation(&repository, user).await;
    authenticated_client()
        .put(format!("{base}/workspace/members/{membership}"))
        .json(&json!({"state":"inactive"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert_eq!(
        client
            .get(format!("{base}/auth/session"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let response = accept(&client, &base, &old).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "invitation_invalid"
    );
    assert_eq!(
        client
            .get(format!("{base}/auth/session"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    // A new invitation is an explicit new authorization by an administrator.
    let fresh = invitation(&repository, user).await;
    assert_eq!(
        accept(&client, &base, &fresh).await.status(),
        StatusCode::OK
    );
    // The inviter learns that the invitation was accepted.
    let inviter_inbox = repository
        .list_notifications(BOOTSTRAP_OWNER_ID.parse().unwrap(), false, None, 10)
        .await
        .unwrap();
    assert_eq!(inviter_inbox.len(), 1);
    assert_eq!(inviter_inbox[0].kind, "workspace.invitation_accepted");
    assert_eq!(inviter_inbox[0].actor_user_id, Some(user));
    assert_eq!(inviter_inbox[0].subject, None);
    assert_eq!(
        client
            .get(format!("{base}/auth/session"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    server.abort();
}

#[sqlx::test]
async fn old_invitation_cannot_restore_a_revoked_role_grant(pool: PgPool) {
    let (base, server) = start_server(pool.clone()).await;
    let repository = AttricatRepository::new(pool.clone(), bootstrap_workspace_id());
    let (user, membership, grant) = member(&pool).await;
    let client = token_client(user, &base).await;
    let old = invitation(&repository, user).await;
    authenticated_client()
        .delete(format!(
            "{base}/workspace/members/{membership}/grants/{grant}"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert_eq!(
        accept(&client, &base, &old).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert!(
        !repository
            .is_authorized(user, bootstrap_workspace_id(), "records.read", None, None)
            .await
            .unwrap()
    );
    server.abort();
}

#[sqlx::test]
async fn acceptance_waiting_behind_deactivation_cannot_restore_membership(pool: PgPool) {
    let (_, server) = start_server(pool.clone()).await;
    let repository = AttricatRepository::new(pool.clone(), bootstrap_workspace_id());
    let (user, membership, _) = member(&pool).await;
    let secret = invitation(&repository, user).await;
    let mut blocker = pool.begin().await.unwrap();
    let blocker_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    sqlx::query("SELECT id FROM workspace_memberships WHERE id=$1 FOR UPDATE")
        .bind(membership)
        .execute(&mut *blocker)
        .await
        .unwrap();
    let revoker = repository.clone();
    let revocation = tokio::spawn(async move {
        revoker
            .set_workspace_membership_state(
                membership,
                BOOTSTRAP_OWNER_ID.parse().unwrap(),
                bootstrap_workspace_id(),
                "inactive",
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let waiting: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND $1=ANY(pg_blocking_pids(pid)))").bind(blocker_pid).fetch_one(&pool).await.unwrap();
            if waiting { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    let recipient = repository.clone();
    let acceptance = tokio::spawn(async move {
        recipient
            .accept_workspace_invitation(&Sha256::digest(secret.as_bytes()), user)
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let waiting: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND cardinality(pg_blocking_pids(pid))>0").fetch_one(&pool).await.unwrap();
            if waiting >= 2 { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    blocker.rollback().await.unwrap();
    let (revoked, accepted) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(revocation, acceptance)
    })
    .await
    .unwrap();
    assert!(revoked.unwrap().unwrap());
    assert!(matches!(
        accepted.unwrap(),
        Err(RepositoryError::InvitationInvalid)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT state FROM workspace_memberships WHERE id=$1")
            .bind(membership)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "inactive"
    );
    server.abort();
}
