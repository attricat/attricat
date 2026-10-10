mod support;

use api::repository::AttricatRepository;
use support::*;

async fn create_user(pool: &PgPool, email: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, $2)")
        .bind(id)
        .bind(email)
        .execute(pool)
        .await
        .unwrap();
    id
}

#[sqlx::test]
async fn external_identity_linking_is_explicit_and_unique(pool: PgPool) {
    let existing = create_user(&pool, "existing@example.test").await;
    let linked = create_user(&pool, "linked@example.test").await;
    let repository = AttricatRepository::system(pool.clone());
    repository
        .link_external_identity(linked, "https://idp.example.test", "subject-1")
        .await
        .unwrap();
    assert_eq!(
        repository
            .external_identity_user("https://idp.example.test", "subject-1")
            .await
            .unwrap(),
        Some(linked)
    );
    assert_ne!(linked, existing);
    assert!(
        repository
            .link_external_identity(existing, "https://idp.example.test", "subject-1")
            .await
            .is_err()
    );
    assert_eq!(
        repository
            .external_identity_user("https://idp.example.test", "missing")
            .await
            .unwrap(),
        None
    );
}
