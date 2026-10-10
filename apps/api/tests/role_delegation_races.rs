mod support;

use std::time::Duration;

use api::repository::AttricatRepository;
use chrono::{Duration as ChronoDuration, Utc};
use support::*;

async fn wait_for_blocked(pool: &PgPool, blocker: i32) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let waiting: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND $1=ANY(pg_blocking_pids(pid)))")
                .bind(blocker).fetch_one(pool).await.unwrap();
            if waiting { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap();
}

async fn changed_role_is_not_delegable(pool: PgPool, invite: bool) {
    let (_, server) = start_server(pool.clone()).await;
    let repository = AttricatRepository::system(pool.clone());
    let (user, membership) = add_workspace_user(&pool).await;
    let authority = create_role(
        &pool,
        "limited-granter",
        &["members.manage", "roles.grant", "records.read"],
    )
    .await;
    grant_role(&pool, membership, authority, GrantScope::Workspace).await;
    let role = create_role(&pool, "changing-role", &["records.read"]).await;
    let mut editor = pool.begin().await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *editor)
        .await
        .unwrap();
    // Model an owner's role edit. Preflight readers still see the old role,
    // but a delegating transaction must wait for the role row and reread it.
    sqlx::query("SELECT id FROM roles WHERE id=$1 FOR UPDATE")
        .bind(role)
        .execute(&mut *editor)
        .await
        .unwrap();
    sqlx::query("INSERT INTO role_permissions(role_id,permission_code) VALUES($1,'records.write')")
        .bind(role)
        .execute(&mut *editor)
        .await
        .unwrap();
    let operation = tokio::spawn(async move {
        if invite {
            repository
                .create_workspace_invitation(
                    Uuid::new_v4(),
                    user,
                    bootstrap_workspace_id(),
                    "race@example.test",
                    role,
                    "workspace",
                    bootstrap_workspace_id(),
                    &[13; 32],
                    Utc::now() + ChronoDuration::hours(1),
                )
                .await
        } else {
            repository
                .grant_workspace_member_role(
                    user,
                    bootstrap_workspace_id(),
                    membership,
                    role,
                    "workspace",
                    bootstrap_workspace_id(),
                )
                .await
                .map(|_| ())
        }
    });
    wait_for_blocked(&pool, pid).await;
    editor.commit().await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(5), operation)
            .await
            .unwrap()
            .unwrap()
            .is_err(),
        "a reader delegated a role that gained write permission during its lock wait"
    );
    let artifacts: i64 = if invite {
        sqlx::query_scalar("SELECT count(*) FROM workspace_invitations WHERE role_id=$1")
            .bind(role)
            .fetch_one(&pool)
            .await
            .unwrap()
    } else {
        sqlx::query_scalar("SELECT count(*) FROM role_grants WHERE role_id=$1")
            .bind(role)
            .fetch_one(&pool)
            .await
            .unwrap()
    };
    assert_eq!(artifacts, 0);
    server.abort();
}

#[sqlx::test]
async fn grant_rechecks_the_locked_roles_permissions(pool: PgPool) {
    changed_role_is_not_delegable(pool, false).await;
}

#[sqlx::test]
async fn invitation_rechecks_the_locked_roles_permissions(pool: PgPool) {
    changed_role_is_not_delegable(pool, true).await;
}

async fn former_owner_cannot_delegate(pool: PgPool, invite: bool) {
    let (_, server) = start_server(pool.clone()).await;
    let repository = AttricatRepository::system(pool.clone());
    let owner: Uuid = BOOTSTRAP_OWNER_ID.parse().unwrap();
    let (_, successor) = add_workspace_user(&pool).await;
    let (_, recipient) = add_workspace_user(&pool).await;
    let mut transfer = pool.begin().await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *transfer)
        .await
        .unwrap();
    sqlx::query("SELECT id FROM workspaces WHERE id=$1 FOR UPDATE")
        .bind(bootstrap_workspace_id())
        .execute(&mut *transfer)
        .await
        .unwrap();
    let operation = tokio::spawn(async move {
        if invite {
            repository
                .create_workspace_invitation(
                    Uuid::new_v4(),
                    owner,
                    bootstrap_workspace_id(),
                    "owner-race@example.test",
                    OWNER_ROLE_ID,
                    "workspace",
                    bootstrap_workspace_id(),
                    &[17; 32],
                    Utc::now() + ChronoDuration::hours(1),
                )
                .await
        } else {
            repository
                .grant_workspace_member_role(
                    owner,
                    bootstrap_workspace_id(),
                    recipient,
                    OWNER_ROLE_ID,
                    "workspace",
                    bootstrap_workspace_id(),
                )
                .await
                .map(|_| ())
        }
    });
    wait_for_blocked(&pool, pid).await;
    sqlx::query("UPDATE role_grants SET membership_id=$1 WHERE role_id=$2 AND workspace_id=$3")
        .bind(successor)
        .bind(OWNER_ROLE_ID)
        .bind(bootstrap_workspace_id())
        .execute(&mut *transfer)
        .await
        .unwrap();
    transfer.commit().await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(5), operation)
            .await
            .unwrap()
            .unwrap()
            .is_err(),
        "the former owner delegated owner authority after the transfer committed"
    );
    let owners: i64 =
        sqlx::query_scalar("SELECT count(*) FROM role_grants WHERE role_id=$1 AND workspace_id=$2")
            .bind(OWNER_ROLE_ID)
            .bind(bootstrap_workspace_id())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(owners, 1);
    let invitations: i64 =
        sqlx::query_scalar("SELECT count(*) FROM workspace_invitations WHERE role_id=$1")
            .bind(OWNER_ROLE_ID)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(invitations, 0);
    server.abort();
}

#[sqlx::test]
async fn former_owner_cannot_grant_owner_after_waiting_for_transfer(pool: PgPool) {
    former_owner_cannot_delegate(pool, false).await;
}

#[sqlx::test]
async fn former_owner_cannot_invite_owner_after_waiting_for_transfer(pool: PgPool) {
    former_owner_cannot_delegate(pool, true).await;
}

#[sqlx::test]
async fn role_edit_cannot_restore_permissions_removed_during_its_lock_wait(pool: PgPool) {
    let (_, server) = start_server(pool.clone()).await;
    let repository = AttricatRepository::system(pool.clone());
    let (user, member) = add_workspace_user(&pool).await;
    let manager = create_role(&pool, "role-manager", &["roles.manage", "records.read"]).await;
    grant_role(&pool, member, manager, GrantScope::Workspace).await;
    let writer = create_role(&pool, "writer", &["records.write"]).await;
    grant_role(&pool, member, writer, GrantScope::Workspace).await;
    let mut removal = pool.begin().await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *removal)
        .await
        .unwrap();
    sqlx::query("SELECT id FROM workspaces WHERE id=$1 FOR NO KEY UPDATE")
        .bind(bootstrap_workspace_id())
        .execute(&mut *removal)
        .await
        .unwrap();
    sqlx::query("SELECT id FROM roles WHERE id=$1 FOR UPDATE")
        .bind(writer)
        .execute(&mut *removal)
        .await
        .unwrap();
    sqlx::query("DELETE FROM role_permissions WHERE role_id=$1")
        .bind(writer)
        .execute(&mut *removal)
        .await
        .unwrap();
    let editor = repository.clone();
    let operation = tokio::spawn(async move {
        editor
            .update_workspace_role(
                user,
                bootstrap_workspace_id(),
                writer,
                "writer",
                &["records.write".into()],
            )
            .await
    });
    wait_for_blocked(&pool, pid).await;
    removal.commit().await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(5), operation)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    assert!(
        !repository
            .is_authorized(user, bootstrap_workspace_id(), "records.write", None, None)
            .await
            .unwrap()
    );
    // Editing the role to contain authority the user still holds is supported.
    repository
        .update_workspace_role(
            user,
            bootstrap_workspace_id(),
            writer,
            "writer",
            &["records.read".into()],
        )
        .await
        .unwrap();
    server.abort();
}

#[sqlx::test]
async fn role_retirement_cannot_grant_owner_after_a_concurrent_transfer(pool: PgPool) {
    let (_, server) = start_server(pool.clone()).await;
    let repository = AttricatRepository::system(pool.clone());
    let owner = BOOTSTRAP_OWNER_ID.parse().unwrap();
    let (_, successor) = add_workspace_user(&pool).await;
    let (_, recipient) = add_workspace_user(&pool).await;
    let role = create_role(&pool, "retiring-role", &["records.read"]).await;
    let grant = grant_role(&pool, recipient, role, GrantScope::Workspace).await;
    let mut transfer = pool.begin().await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *transfer)
        .await
        .unwrap();
    sqlx::query("SELECT id FROM workspaces WHERE id=$1 FOR UPDATE")
        .bind(bootstrap_workspace_id())
        .execute(&mut *transfer)
        .await
        .unwrap();
    sqlx::query("SELECT id FROM role_grants WHERE id=$1 FOR UPDATE")
        .bind(grant)
        .execute(&mut *transfer)
        .await
        .unwrap();
    let operation = tokio::spawn(async move {
        repository
            .retire_workspace_role(owner, bootstrap_workspace_id(), role, Some(OWNER_ROLE_ID))
            .await
    });
    wait_for_blocked(&pool, pid).await;
    sqlx::query("UPDATE role_grants SET membership_id=$1 WHERE role_id=$2 AND workspace_id=$3")
        .bind(successor)
        .bind(OWNER_ROLE_ID)
        .bind(bootstrap_workspace_id())
        .execute(&mut *transfer)
        .await
        .unwrap();
    transfer.commit().await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(5), operation)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, Uuid>("SELECT role_id FROM role_grants WHERE id=$1")
            .bind(grant)
            .fetch_one(&pool)
            .await
            .unwrap(),
        role
    );
    let owners: i64 =
        sqlx::query_scalar("SELECT count(*) FROM role_grants WHERE role_id=$1 AND workspace_id=$2")
            .bind(OWNER_ROLE_ID)
            .bind(bootstrap_workspace_id())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(owners, 1);
    server.abort();
}
