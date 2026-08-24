mod support;

use sqlx::Acquire;

use support::*;

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

#[sqlx::test]
async fn external_identity_linking_requires_an_explicit_user_and_never_uses_email(pool: PgPool) {
    let existing_user = create_user(&pool, "existing@example.test").await;
    let explicitly_linked_user = create_user(&pool, "new-account@example.test").await;

    let linked_user: Uuid = sqlx::query_scalar(
        "SELECT link_external_identity($1, 'https://idp.example.test', 'provider-subject-1')",
    )
    .bind(explicitly_linked_user)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(linked_user, explicitly_linked_user);

    let resolved_user: Uuid = sqlx::query_scalar(
        "SELECT find_external_identity_user('https://idp.example.test', 'provider-subject-1')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(resolved_user, explicitly_linked_user);
    assert_ne!(
        resolved_user, existing_user,
        "a provider's matching email claim cannot select or merge an existing account"
    );

    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT column_name FROM information_schema.columns WHERE table_schema = 'public' AND table_name = 'external_identities' ORDER BY ordinal_position",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        columns,
        vec!["issuer", "subject", "user_id", "created_at", "updated_at"],
        "the identity contract has no mutable provider claims such as email"
    );

    assert!(
        sqlx::query(
            "SELECT link_external_identity($1, 'https://idp.example.test', 'provider-subject-1')"
        )
        .bind(existing_user)
        .execute(&pool)
        .await
        .is_err(),
        "an issuer and subject can never be silently relinked to another user"
    );
}

#[sqlx::test]
async fn external_identity_contract_is_unique_and_narrowly_exposed(pool: PgPool) {
    let user = create_user(&pool, "identity@example.test").await;
    sqlx::query(
        "SELECT link_external_identity($1, 'https://idp.example.test', 'provider-subject-1')",
    )
    .bind(user)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("SELECT link_external_identity($1, 'https://another-idp.example.test', 'provider-subject-1')")
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();

    assert!(
        sqlx::query(
            "SELECT link_external_identity($1, 'https://idp.example.test', 'provider-subject-1')"
        )
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await
        .is_err(),
        "the issuer and subject pair is globally unique"
    );
    assert_eq!(
        sqlx::query_scalar::<_, Option<Uuid>>(
            "SELECT find_external_identity_user('https://idp.example.test', 'unknown-subject')",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        None,
        "a missing provider identity has no email fallback"
    );

    let mut adapter = pool.begin().await.unwrap();
    sqlx::query("SET LOCAL ROLE catalog_api")
        .execute(&mut *adapter)
        .await
        .unwrap();
    let mut denied = adapter.begin().await.unwrap();
    assert!(
        sqlx::query("SELECT * FROM external_identities")
            .execute(&mut *denied)
            .await
            .is_err(),
        "the adapter role uses the narrow identity contract instead of table access"
    );
    denied.rollback().await.unwrap();
    let resolved: Uuid = sqlx::query_scalar(
        "SELECT find_external_identity_user('https://idp.example.test', 'provider-subject-1')",
    )
    .fetch_one(&mut *adapter)
    .await
    .unwrap();
    assert_eq!(resolved, user);
    adapter.rollback().await.unwrap();
}
