mod support;
use api::{
    account::SessionDigest,
    repository::{AttricatRepository, SYSTEM_USER_DISPLAY_NAME, system_user_email},
};
use support::*;

const BLUEPRINT: &str = r#"
format_version = 1
code = "system_note_record"
name = "System note record"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"
"#;

async fn system_user_of(pool: &PgPool, workspace: Uuid) -> Option<Uuid> {
    sqlx::query_scalar("SELECT system_user_id FROM workspaces WHERE id = $1")
        .bind(workspace)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn workspace_initialization_creates_and_backfills_one_system_user(pool: PgPool) {
    let (_base, _server) = start_server(pool.clone()).await;
    let workspace = bootstrap_workspace_id();
    let system_user = system_user_of(&pool, workspace).await.unwrap();
    let (email, name, kind, state): (String, Option<String>, String, String) =
        sqlx::query_as("SELECT email, display_name, kind, state FROM users WHERE id = $1")
            .bind(system_user)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(email, system_user_email(workspace));
    assert_eq!(name.as_deref(), Some(SYSTEM_USER_DISPLAY_NAME));
    assert_eq!((kind.as_str(), state.as_str()), ("system", "active"));

    // A workspace that predates system users gets one when it is
    // initialized at startup, and initializing again keeps that user.
    let other = Uuid::new_v4();
    sqlx::query("INSERT INTO workspaces (id, slug, name, login_identifier) VALUES ($1, 'system-other', 'Other', 'system-other.local')")
        .bind(other)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(system_user_of(&pool, other).await, None);
    let system = AttricatRepository::system(pool.clone());
    system.initialize_workspace(other).await.unwrap();
    let backfilled = system_user_of(&pool, other).await.unwrap();
    assert_ne!(backfilled, system_user);
    system.initialize_workspace(other).await.unwrap();
    assert_eq!(
        system.ensure_workspace_system_user(other).await.unwrap(),
        backfilled
    );
    assert_eq!(system_user_of(&pool, other).await, Some(backfilled));

    let (memberships, credentials, tokens): (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM workspace_memberships WHERE user_id = ANY($1)), (SELECT count(*) FROM local_password_credentials WHERE user_id = ANY($1)), (SELECT count(*) FROM personal_api_tokens WHERE user_id = ANY($1))",
    )
    .bind(vec![system_user, backfilled])
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((memberships, credentials, tokens), (0, 0, 0));
}

#[sqlx::test(migrations = "./migrations")]
async fn system_users_are_not_members_and_cannot_be_invited(pool: PgPool) {
    let (base, _server) = start_server(pool.clone()).await;
    let owner = authenticated_client();
    let workspace = bootstrap_workspace_id();
    let system_user = system_user_of(&pool, workspace).await.unwrap();
    for path in ["workspace/members", "directory"] {
        let body = owner
            .get(format!("{base}/{path}"))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(!body.contains(&system_user.to_string()), "{path}: {body}");
        assert!(!body.contains("attricat.invalid"), "{path}: {body}");
    }
    let response = owner
        .post(format!("{base}/workspace/users"))
        .json(&json!({"email": system_user_email(workspace)}))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_client_error(), "{}", response.status());
    let memberships: i64 =
        sqlx::query_scalar("SELECT count(*) FROM workspace_memberships WHERE user_id = $1")
            .bind(system_user)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(memberships, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn system_comments_never_notify_the_system_user(pool: PgPool) {
    let (base, _server) = start_server(pool.clone()).await;
    let owner = authenticated_client();
    let blueprint = create_blueprint(&owner, &base, BLUEPRINT).await;
    let record = create_record(&owner, &base, &blueprint).await;
    let record_id: Uuid = record["id"].as_str().unwrap().parse().unwrap();
    let workspace = bootstrap_workspace_id();
    let system_user = system_user_of(&pool, workspace).await.unwrap();
    let repository = AttricatRepository::new(pool.clone(), workspace);
    repository
        .create_record_comment(record_id, system_user, "A bulk edit stopped here.")
        .await
        .unwrap();
    // A person's later comment would notify earlier commenters; the system
    // user is never one of them.
    let path = format!("{base}/v1/records/{record_id}/comments");
    owner
        .post(&path)
        .json(&json!({"body": "Noted."}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let notified: i64 =
        sqlx::query_scalar("SELECT count(*) FROM user_notifications WHERE recipient_user_id = $1")
            .bind(system_user)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(notified, 0);

    let page: Value = owner
        .get(&path)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let kinds: Vec<(&str, &str)> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            (
                item["author_kind"].as_str().unwrap(),
                item["author_display_name"].as_str().unwrap_or_default(),
            )
        })
        .collect();
    assert!(
        kinds.contains(&("system", SYSTEM_USER_DISPLAY_NAME)),
        "{kinds:?}"
    );
    assert!(kinds.iter().any(|(kind, _)| *kind == "person"), "{kinds:?}");

    // Comments stay editable by their author only.
    let system_comment = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["author_kind"] == "system")
        .unwrap();
    let response = owner
        .patch(format!("{path}/{}", system_comment["id"].as_str().unwrap()))
        .json(&json!({"body": "Rewritten", "revision": 1}))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_client_error(), "{}", response.status());
}

#[sqlx::test(migrations = "./migrations")]
async fn system_users_cannot_authenticate(pool: PgPool) {
    let (_base, _server) = start_server(pool.clone()).await;
    let workspace = bootstrap_workspace_id();
    let system_user = system_user_of(&pool, workspace).await.unwrap();
    let repository = AttricatRepository::system(pool.clone());
    // Even with every credential row in place, which nothing creates for a
    // system user, no lookup accepts it.
    sqlx::query("UPDATE users SET email_verified_at = now() WHERE id = $1")
        .bind(system_user)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(Uuid::new_v4())
    .bind(workspace)
    .bind(system_user)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO local_password_credentials (user_id, password_hash) VALUES ($1, 'unused')",
    )
    .bind(system_user)
    .execute(&pool)
    .await
    .unwrap();
    let session = [7_u8; 32];
    sqlx::query("INSERT INTO browser_sessions (id, user_id, workspace_id, session_digest, csrf_digest, issued_security_version, issued_credential_version, expires_at) VALUES ($1, $2, $3, $4, $4, 1, 1, now() + interval '1 hour')")
        .bind(Uuid::new_v4())
        .bind(system_user)
        .bind(workspace)
        .bind(&session[..])
        .execute(&pool)
        .await
        .unwrap();
    let token = [9_u8; 32];
    sqlx::query("INSERT INTO personal_api_tokens (id, user_id, workspace_id, label, token_digest) VALUES ($1, $2, $3, 'system', $4)")
        .bind(Uuid::new_v4())
        .bind(system_user)
        .bind(workspace)
        .bind(&token[..])
        .execute(&pool)
        .await
        .unwrap();

    assert!(
        repository
            .validate_browser_session(&SessionDigest::from_slice(&session).unwrap())
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        repository
            .authenticate_personal_api_token(&token)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        repository
            .local_login_credential(&system_user_email(workspace))
            .await
            .unwrap()
            .is_none()
    );
}
