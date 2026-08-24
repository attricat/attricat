mod support;

use support::*;

const BOOTSTRAP_WORKSPACE_ID: &str = "00000000-0000-4000-8000-000000000002";
const OWNER_ROLE_ID: &str = "00000000-0000-4000-8000-000000000101";

async fn create_user(pool: &PgPool, email: &str) -> Uuid {
    let user_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, $2)")
        .bind(user_id)
        .bind(email)
        .execute(pool)
        .await
        .unwrap();
    user_id
}

async fn issue_token(pool: &PgPool, user_id: Uuid, purpose: &str, digest: Vec<u8>) {
    sqlx::query(
        "SELECT issue_user_lifecycle_action_token($1, $2, $3, $4, now() + interval '1 hour')",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(purpose)
    .bind(digest)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test]
async fn local_credentials_are_singleton_and_lifecycle_tokens_are_digest_only(pool: PgPool) {
    let plaintext_user = create_user(&pool, "plaintext@example.test").await;
    assert!(
        sqlx::query("INSERT INTO local_password_credentials (user_id, password_hash) VALUES ($1, 'not-a-hash')")
            .bind(plaintext_user)
            .execute(&pool)
            .await
            .is_err(),
        "the credential table rejects a plaintext-looking value"
    );

    let malformed_user = create_user(&pool, "malformed@example.test").await;
    assert!(
        sqlx::query("INSERT INTO local_password_credentials (user_id, password_hash) VALUES ($1, '$argon2id$v=19$m=0,t=0,p=0$a$a')")
            .bind(malformed_user)
            .execute(&pool)
            .await
            .is_err(),
        "the credential table rejects malformed or zero-cost Argon2id PHC values"
    );

    let user_id = create_user(&pool, "credential@example.test").await;
    sqlx::query(
        "INSERT INTO local_password_credentials (user_id, password_hash) VALUES ($1, '$argon2id$v=19$m=19456,t=2,p=1$c2FsdHNh$ZGlnZXN0ZGlnZXN0')",
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();
    assert!(sqlx::query(
        "INSERT INTO local_password_credentials (user_id, password_hash) VALUES ($1, '$argon2id$v=19$m=19456,t=2,p=1$b3RoZXJz$ZGlnZXN0ZGlnZXN0')",
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .is_err());

    let token_user = create_user(&pool, "digest@example.test").await;
    let digest = vec![0xA5; 32];
    issue_token(&pool, token_user, "email_verification", digest.clone()).await;
    let stored: Vec<u8> = sqlx::query_scalar(
        "SELECT token_digest FROM user_lifecycle_action_tokens WHERE user_id = $1",
    )
    .bind(token_user)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored, digest, "the fixture is stored only as its digest");
    let token_columns: Vec<String> = sqlx::query_scalar(
        "SELECT column_name FROM information_schema.columns WHERE table_schema = 'public' AND table_name = 'user_lifecycle_action_tokens' AND column_name ILIKE '%token%' ORDER BY column_name",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(token_columns, vec!["token_digest"]);
    assert!(sqlx::query(
        "INSERT INTO user_lifecycle_action_tokens (id, user_id, purpose, token_digest, issued_security_version, issued_credential_version, expires_at) VALUES ($1, $2, 'email_verification', $3, 1, 0, now() + interval '1 hour')",
    )
    .bind(Uuid::new_v4())
    .bind(token_user)
    .bind(vec![0xA5; 32])
    .execute(&pool)
    .await
    .is_err(), "digest uniqueness rejects a duplicate token secret");

    let mut account_layer = pool.begin().await.unwrap();
    sqlx::query("SET LOCAL ROLE catalog_api")
        .execute(&mut *account_layer)
        .await
        .unwrap();
    let visible_hash: String = sqlx::query_scalar(
        "SELECT password_hash FROM find_local_password_credential('credential@example.test')",
    )
    .fetch_one(&mut *account_layer)
    .await
    .unwrap();
    assert_eq!(
        visible_hash,
        "$argon2id$v=19$m=19456,t=2,p=1$c2FsdHNh$ZGlnZXN0ZGlnZXN0"
    );
    sqlx::query("CREATE TEMP TABLE users (id UUID, email TEXT, state TEXT)")
        .execute(&mut *account_layer)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO users (id, email, state) VALUES ($1, 'shadow@example.test', 'active')",
    )
    .bind(Uuid::new_v4())
    .execute(&mut *account_layer)
    .await
    .unwrap();
    let original_hash: String = sqlx::query_scalar(
        "SELECT password_hash FROM find_local_password_credential('credential@example.test')",
    )
    .fetch_one(&mut *account_layer)
    .await
    .unwrap();
    assert_eq!(
        original_hash, visible_hash,
        "a temporary table cannot shadow a SECURITY DEFINER function's tables"
    );
    assert!(
        sqlx::query("SELECT email FROM public.users")
            .execute(&mut *account_layer)
            .await
            .is_err(),
        "catalog_api has no broad users-table access"
    );
    account_layer.rollback().await.unwrap();
}

#[sqlx::test]
async fn lifecycle_token_consumption_rejects_expiry_replay_revocation_state_and_versions(
    pool: PgPool,
) {
    let user_id = create_user(&pool, "lifecycle@example.test").await;

    assert!(
        sqlx::query(
            "SELECT issue_user_lifecycle_action_token($1, $2, 'password_setup', $3, now() + interval '1 hour')",
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(vec![9u8; 32])
        .execute(&pool)
        .await
        .is_err(),
        "password setup requires a verified email"
    );
    sqlx::query("UPDATE users SET email_verified_at = now() WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    issue_token(&pool, user_id, "password_setup", vec![9u8; 32]).await;

    let expired_digest = vec![1u8; 32];
    sqlx::query(
        "INSERT INTO user_lifecycle_action_tokens (id, user_id, purpose, token_digest, issued_security_version, issued_credential_version, created_at, expires_at) VALUES ($1, $2, 'email_verification', $3, 1, 0, now() - interval '2 hours', now() - interval '1 hour')",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(expired_digest.clone())
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        sqlx::query_scalar::<_, Uuid>(
            "SELECT consume_user_lifecycle_action_token($1, 'email_verification')",
        )
        .bind(expired_digest)
        .fetch_one(&pool)
        .await
        .is_err()
    );

    let replay_digest = vec![2u8; 32];
    issue_token(&pool, user_id, "email_verification", replay_digest.clone()).await;
    let consumed: Uuid =
        sqlx::query_scalar("SELECT consume_user_lifecycle_action_token($1, 'email_verification')")
            .bind(replay_digest.clone())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(consumed, user_id);
    assert!(
        sqlx::query_scalar::<_, Uuid>(
            "SELECT consume_user_lifecycle_action_token($1, 'email_verification')",
        )
        .bind(replay_digest)
        .fetch_one(&pool)
        .await
        .is_err(),
        "a consumed token cannot be replayed"
    );

    let revoked_digest = vec![3u8; 32];
    issue_token(&pool, user_id, "email_verification", revoked_digest.clone()).await;
    sqlx::query("SELECT revoke_user_lifecycle_action_tokens($1, 'email_verification')")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        sqlx::query_scalar::<_, Uuid>(
            "SELECT consume_user_lifecycle_action_token($1, 'email_verification')",
        )
        .bind(revoked_digest)
        .fetch_one(&pool)
        .await
        .is_err()
    );

    let state_digest = vec![4u8; 32];
    issue_token(&pool, user_id, "email_verification", state_digest.clone()).await;
    sqlx::query("UPDATE users SET state = 'inactive' WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        sqlx::query_scalar::<_, Uuid>(
            "SELECT consume_user_lifecycle_action_token($1, 'email_verification')",
        )
        .bind(state_digest)
        .fetch_one(&pool)
        .await
        .is_err(),
        "an inactive user cannot consume a lifecycle token"
    );
    sqlx::query("UPDATE users SET state = 'active' WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO local_password_credentials (user_id, password_hash, credential_version) VALUES ($1, '$argon2id$v=19$m=19456,t=2,p=1$c2FsdHNh$ZGlnZXN0ZGlnZXN0', 2)",
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();
    let version_digest = vec![5u8; 32];
    sqlx::query(
        "INSERT INTO user_lifecycle_action_tokens (id, user_id, purpose, token_digest, issued_security_version, issued_credential_version, expires_at) VALUES ($1, $2, 'password_reset', $3, 1, 1, now() + interval '1 hour')",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(version_digest)
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        sqlx::query_scalar::<_, Uuid>(
            "SELECT consume_user_lifecycle_action_token($1, 'password_reset')",
        )
        .bind(vec![5u8; 32])
        .fetch_one(&pool)
        .await
        .is_err(),
        "a token issued for a different credential version is rejected"
    );
}

#[sqlx::test]
async fn bootstrap_owner_does_not_reactivate_an_inactive_membership(pool: PgPool) {
    let workspace = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let bootstrap_user = Uuid::new_v4();
    let bootstrap_membership = Uuid::new_v4();
    sqlx::query("SELECT bootstrap_workspace_owner($1, $2, $3, $4, 'owner@example.test')")
        .bind(workspace)
        .bind(bootstrap_user)
        .bind(bootstrap_membership)
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await
        .unwrap();

    // A second active owner permits intentionally suspending the bootstrap user.
    let second_user = create_user(&pool, "second-owner@example.test").await;
    let second_membership = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(second_membership)
    .bind(workspace)
    .bind(second_user)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, $4, 'workspace', $2)")
        .bind(Uuid::new_v4())
        .bind(workspace)
        .bind(second_membership)
        .bind(OWNER_ROLE_ID.parse::<Uuid>().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE workspace_memberships SET state = 'inactive' WHERE id = $1")
        .bind(bootstrap_membership)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("SELECT bootstrap_workspace_owner($1, $2, $3, $4, 'owner@example.test')")
        .bind(workspace)
        .bind(Uuid::new_v4())
        .bind(Uuid::new_v4())
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await
        .unwrap();
    let state: String = sqlx::query_scalar("SELECT state FROM workspace_memberships WHERE id = $1")
        .bind(bootstrap_membership)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(state, "inactive");
}
