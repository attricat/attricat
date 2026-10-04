mod support;

use api::repository::CatalogRepository;
use chrono::{Duration, Utc};
use support::*;

async fn create_user(pool: &PgPool, email: &str, verified: bool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email, email_verified_at) VALUES ($1, $2, CASE WHEN $3 THEN clock_timestamp() ELSE NULL END)")
        .bind(id).bind(email).bind(verified).execute(pool).await.unwrap();
    id
}

#[sqlx::test]
async fn lifecycle_tokens_are_digest_only_and_single_use(pool: PgPool) {
    let user = create_user(&pool, "lifecycle@example.test", false).await;
    let repository = CatalogRepository::system(pool.clone());
    let digest = vec![7; 32];
    repository
        .issue_lifecycle_token(
            Uuid::new_v4(),
            user,
            "email_verification",
            &digest,
            Utc::now() + Duration::hours(1),
        )
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, Vec<u8>>(
            "SELECT token_digest FROM user_lifecycle_action_tokens WHERE user_id = $1"
        )
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap(),
        digest
    );
    assert_eq!(
        repository
            .consume_lifecycle_token(&digest, "email_verification")
            .await
            .unwrap(),
        user
    );
    assert!(
        repository
            .consume_lifecycle_token(&digest, "email_verification")
            .await
            .is_err()
    );
}

#[sqlx::test]
async fn lifecycle_token_validation_uses_account_state_and_revocation(pool: PgPool) {
    let user = create_user(&pool, "verified@example.test", true).await;
    let repository = CatalogRepository::system(pool.clone());
    let setup = vec![8; 32];
    repository
        .issue_lifecycle_token(
            Uuid::new_v4(),
            user,
            "password_setup",
            &setup,
            Utc::now() + Duration::hours(1),
        )
        .await
        .unwrap();
    repository
        .revoke_lifecycle_tokens(user, Some("password_setup"))
        .await
        .unwrap();
    assert!(
        repository
            .consume_lifecycle_token(&setup, "password_setup")
            .await
            .is_err()
    );
    assert!(
        repository
            .issue_lifecycle_token(
                Uuid::new_v4(),
                user,
                "email_verification",
                &[1; 31],
                Utc::now() + Duration::hours(1)
            )
            .await
            .is_err()
    );
}

const VIEWER_ROLE_ID: &str = "00000000-0000-4000-8000-000000000104";
const OWNER_ROLE_ID: &str = "00000000-0000-4000-8000-000000000101";

/// Makes `owner` the owner of `workspace_id`.
async fn grant_owner(pool: &PgPool, workspace_id: Uuid, owner: Uuid) {
    let membership_id = Uuid::new_v4();
    sqlx::query("INSERT INTO workspace_memberships (id, workspace_id, user_id, state) VALUES ($1, $2, $3, 'active')")
        .bind(membership_id).bind(workspace_id).bind(owner).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, $4, 'workspace', $2)")
        .bind(Uuid::new_v4()).bind(workspace_id).bind(membership_id).bind(OWNER_ROLE_ID.parse::<Uuid>().unwrap()).execute(pool).await.unwrap();
}

/// The bootstrap workspace and a second one, both owned by a new owner.
async fn two_owned_workspaces(pool: &PgPool, slug: &str) -> (Uuid, Uuid, Uuid) {
    let owner = create_user(pool, &format!("owner-{slug}@example.test"), true).await;
    let first = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let second = Uuid::new_v4();
    sqlx::query("INSERT INTO workspaces (id, slug, name, login_identifier) VALUES ($1, $2, $2, $2 || '.local')")
        .bind(second).bind(slug).execute(pool).await.unwrap();
    grant_owner(pool, first, owner).await;
    grant_owner(pool, second, owner).await;
    (owner, first, second)
}

async fn invite(
    repository: &api::repository::SystemRepository,
    owner: Uuid,
    workspace_id: Uuid,
    email: &str,
    invitation_digest: Vec<u8>,
    action_digest: Vec<u8>,
) {
    let created = repository
        .create_workspace_user(
            owner,
            workspace_id,
            email,
            None,
            Some((
                Uuid::new_v4(),
                VIEWER_ROLE_ID.parse().unwrap(),
                "workspace".into(),
                workspace_id,
                invitation_digest,
                Utc::now() + Duration::hours(1),
            )),
            Some(action_digest),
        )
        .await
        .unwrap();
    assert!(created.needs_password_setup);
}

#[sqlx::test]
async fn a_second_onboarding_link_after_password_setup_is_an_invalid_invitation(pool: PgPool) {
    let repository = CatalogRepository::system(pool.clone());
    let email = "two-workspaces@example.test";
    create_user(&pool, email, true).await;
    let (owner, first, second) = two_owned_workspaces(&pool, "second-onboarding").await;
    invite(&repository, owner, first, email, vec![1; 32], vec![2; 32]).await;
    invite(&repository, owner, second, email, vec![3; 32], vec![4; 32]).await;
    repository
        .complete_workspace_onboarding(&[3; 32], &[4; 32], "hash-one")
        .await
        .unwrap();
    // The first workspace's setup token is still live, but it was issued for
    // an account without a credential; once one exists, the link no longer
    // applies.
    assert!(matches!(
        repository
            .complete_workspace_onboarding(&[1; 32], &[2; 32], "hash-two")
            .await,
        Err(api::repository::RepositoryError::InvitationInvalid)
    ));
}

#[sqlx::test]
async fn an_invitation_to_another_workspace_keeps_a_pending_onboarding_link(pool: PgPool) {
    let repository = CatalogRepository::system(pool.clone());
    let email = "pending-elsewhere@example.test";
    let user = create_user(&pool, email, true).await;
    let (owner, first, second) = two_owned_workspaces(&pool, "pending-elsewhere").await;
    invite(&repository, owner, first, email, vec![7; 32], vec![8; 32]).await;
    invite(&repository, owner, second, email, vec![9; 32], vec![10; 32]).await;

    let completed = repository
        .complete_workspace_onboarding(&[7; 32], &[8; 32], "hash")
        .await
        .unwrap();
    assert_eq!(completed.user_id, user);
    assert_eq!(completed.workspace_id, first);
}

#[sqlx::test]
async fn revoking_workspace_access_keeps_other_workspaces_and_account_tokens(pool: PgPool) {
    let repository = CatalogRepository::system(pool.clone());
    let email = "scoped-revoke@example.test";
    let user = create_user(&pool, email, true).await;
    let (owner, first, second) = two_owned_workspaces(&pool, "scoped-revoke").await;
    // A pending onboarding in the second workspace.
    invite(&repository, owner, second, email, vec![5; 32], vec![6; 32]).await;
    // An active membership in the first workspace.
    let membership_id = Uuid::new_v4();
    sqlx::query("INSERT INTO workspace_memberships (id, workspace_id, user_id, state) VALUES ($1, $2, $3, 'active')")
        .bind(membership_id).bind(first).bind(user).execute(&pool).await.unwrap();

    assert!(
        repository
            .set_workspace_membership_state(membership_id, owner, first, "inactive",)
            .await
            .unwrap()
    );

    repository
        .complete_workspace_onboarding(&[5; 32], &[6; 32], "hash")
        .await
        .unwrap();
}
