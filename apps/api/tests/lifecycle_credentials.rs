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
    let repository = CatalogRepository::new(pool.clone());
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
    let repository = CatalogRepository::new(pool.clone());
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
