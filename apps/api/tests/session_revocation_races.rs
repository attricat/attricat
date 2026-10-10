mod support;

use std::time::Duration;

use api::{
    account::{Password, SessionSecret, hash_password},
    repository::AttricatRepository,
};
use chrono::{Duration as ChronoDuration, Utc};
use support::*;

async fn renewal_cannot_escape_revocation(pool: PgPool, deactivate: bool) {
    let (_, server) = start_server(pool.clone()).await;
    let repository = AttricatRepository::system(pool.clone());
    let (user, membership) = add_workspace_user(&pool).await;
    let grant = grant_role(&pool, membership, VIEWER_ROLE_ID, GrantScope::Workspace).await;
    let hash = hash_password(&Password::new("session revocation regression password")).unwrap();
    sqlx::query("INSERT INTO local_password_credentials(user_id,password_hash) VALUES($1,$2)")
        .bind(user)
        .bind(hash.as_phc())
        .execute(&pool)
        .await
        .unwrap();
    let credential = repository
        .local_login_credential(&format!("{user}@example.test"))
        .await
        .unwrap()
        .unwrap();
    let old = SessionSecret::generate().digest();
    let new = SessionSecret::generate().digest();
    let csrf = SessionSecret::generate().digest();
    let expiry = Utc::now() + ChronoDuration::hours(1);
    repository
        .issue_login_session(
            Uuid::new_v4(),
            &credential,
            bootstrap_workspace_id(),
            &old,
            &csrf,
            expiry,
        )
        .await
        .unwrap();

    // An uncommitted conflicting ID pauses the real renewal after it locks
    // the old session but before its replacement becomes visible. Its FK
    // locks are compatible with the workspace's NO KEY UPDATE lock.
    let replacement_id = Uuid::new_v4();
    let mut blocker = pool.begin().await.unwrap();
    let blocker_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    sqlx::query("INSERT INTO browser_sessions(id,user_id,workspace_id,issued_security_version,issued_credential_version,session_digest,csrf_digest,expires_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
        .bind(replacement_id).bind(user).bind(bootstrap_workspace_id()).bind(credential.security_version).bind(credential.credential_version)
        .bind(SessionSecret::generate().digest().as_ref()).bind(csrf.as_ref()).bind(expiry).execute(&mut *blocker).await.unwrap();
    let renewing = repository.clone();
    let old_digest = old.clone();
    let new_digest = new.clone();
    let csrf_digest = csrf.clone();
    let renewal = tokio::spawn(async move {
        renewing
            .rotate_browser_session(
                &old_digest,
                replacement_id,
                &new_digest,
                &csrf_digest,
                bootstrap_workspace_id(),
                expiry,
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let waiting: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND $1=ANY(pg_blocking_pids(pid)))")
                .bind(blocker_pid).fetch_one(&pool).await.unwrap();
            if waiting { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    let revoking = repository.clone();
    let revocation = tokio::spawn(async move {
        if deactivate {
            revoking
                .set_workspace_membership_state(
                    membership,
                    BOOTSTRAP_OWNER_ID.parse().unwrap(),
                    bootstrap_workspace_id(),
                    "inactive",
                )
                .await
                .map(|_| ())
        } else {
            revoking
                .revoke_workspace_member_role(
                    BOOTSTRAP_OWNER_ID.parse().unwrap(),
                    bootstrap_workspace_id(),
                    membership,
                    grant,
                )
                .await
        }
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let waiting: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND cardinality(pg_blocking_pids(pid))>0")
                .fetch_one(&pool).await.unwrap();
            if waiting >= 2 { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    blocker.rollback().await.unwrap();
    let (renewed, revoked) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(renewal, revocation)
    })
    .await
    .unwrap();
    renewed.unwrap().unwrap();
    revoked.unwrap().unwrap();
    if deactivate {
        repository
            .set_workspace_membership_state(
                membership,
                BOOTSTRAP_OWNER_ID.parse().unwrap(),
                bootstrap_workspace_id(),
                "active",
            )
            .await
            .unwrap();
    }
    assert!(
        repository
            .validate_browser_session(&old)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        repository
            .validate_browser_session(&new)
            .await
            .unwrap()
            .is_none(),
        "a replacement session escaped the concurrent revocation"
    );
    // A fresh login, unlike renewal of a revoked session, remains supported.
    let fresh = SessionSecret::generate().digest();
    repository
        .issue_login_session(
            Uuid::new_v4(),
            &credential,
            bootstrap_workspace_id(),
            &fresh,
            &csrf,
            expiry,
        )
        .await
        .unwrap();
    assert!(
        repository
            .validate_browser_session(&fresh)
            .await
            .unwrap()
            .is_some()
    );
    server.abort();
}

#[sqlx::test]
async fn member_deactivation_catches_a_concurrent_session_renewal(pool: PgPool) {
    renewal_cannot_escape_revocation(pool, true).await;
}

#[sqlx::test]
async fn role_revocation_catches_a_concurrent_session_renewal(pool: PgPool) {
    renewal_cannot_escape_revocation(pool, false).await;
}
