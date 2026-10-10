mod support;

use api::{
    account::{Password, SessionDigest, hash_password},
    repository::CatalogRepository,
};
use reqwest::header::SET_COOKIE;
use support::*;

const OWNER_ID: &str = "00000000-0000-4000-8000-000000000201";

fn cookie_pair(response: &reqwest::Response) -> (String, String) {
    let cookies: Vec<_> = response
        .headers()
        .get_all(SET_COOKIE)
        .iter()
        .map(|value| {
            value
                .to_str()
                .unwrap()
                .split(';')
                .next()
                .unwrap()
                .to_owned()
        })
        .collect();
    (cookies[0].clone(), cookies[1].clone())
}

#[sqlx::test]
async fn login_rotates_sessions_and_csrf_protects_mutations(pool: PgPool) {
    let (base_url, server) = start_session_server(pool.clone()).await;
    let password = Password::new("correct horse battery staple");
    let hash = hash_password(&password).unwrap();
    sqlx::query("INSERT INTO local_password_credentials (user_id, password_hash) VALUES ($1, $2)")
        .bind(OWNER_ID.parse::<Uuid>().unwrap())
        .bind(hash.as_phc())
        .execute(&pool)
        .await
        .unwrap();
    let client = Client::new();
    let discovery = client
        .post(format!("{base_url}/auth/discover"))
        .json(&json!({ "login_identifier": " DEFAULT.LOCAL " }))
        .send()
        .await
        .unwrap();
    assert_eq!(discovery.status(), StatusCode::OK);
    assert_eq!(
        discovery.json::<Value>().await.unwrap()["sign_in_methods"],
        json!(["local_password"])
    );
    let login = client.post(format!("{base_url}/auth/login"))
        .json(&json!({ "login_identifier": "default.local", "email": "api-test-owner@example.test", "password": "correct horse battery staple" }))
        .send().await.unwrap();
    assert_eq!(login.status(), StatusCode::OK);
    let (session, csrf) = cookie_pair(&login);
    assert!(
        login
            .headers()
            .get_all(SET_COOKIE)
            .iter()
            .any(|value| value.to_str().unwrap().contains("HttpOnly"))
    );
    let login: Value = login.json().await.unwrap();
    assert_eq!(login["login_identifier"], "default.local");
    assert_eq!(login["email"], "api-test-owner@example.test");
    assert!(login.get("display_name").is_some());
    let cookie = format!("{session}; {csrf}");
    let current: Value = client
        .get(format!("{base_url}/auth/session"))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(current["user_id"], OWNER_ID);
    assert_eq!(current["email"], "api-test-owner@example.test");
    assert!(current.get("display_name").is_some());
    assert_eq!(current["login_identifier"], "default.local");
    assert_eq!(
        client
            .post(format!("{base_url}/data-health/refresh"))
            .header("cookie", &cookie)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let csrf_value = csrf.split_once('=').unwrap().1;
    assert_eq!(
        client
            .post(format!("{base_url}/data-health/refresh"))
            .header("cookie", &cookie)
            .header("x-catalog-csrf", csrf_value)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    let renewed = client
        .post(format!("{base_url}/auth/renew"))
        .header("cookie", &cookie)
        .header("x-catalog-csrf", csrf_value)
        .send()
        .await
        .unwrap();
    assert_eq!(renewed.status(), StatusCode::OK);
    assert_eq!(
        renewed.json::<Value>().await.unwrap()["login_identifier"],
        json!("default.local")
    );
    assert_eq!(
        client
            .get(format!("{base_url}/auth/session"))
            .header("cookie", &cookie)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    server.abort();
}

#[sqlx::test]
async fn password_reset_requests_are_rate_limited_without_disclosing_or_issuing_more_tokens(
    pool: PgPool,
) {
    let (base_url, server) = start_session_server(pool.clone()).await;
    let hash = hash_password(&Password::new("correct horse battery staple")).unwrap();
    sqlx::query("INSERT INTO local_password_credentials (user_id, password_hash) VALUES ($1, $2)")
        .bind(OWNER_ID.parse::<Uuid>().unwrap())
        .bind(hash.as_phc())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE users SET email_verified_at = clock_timestamp() WHERE id = $1")
        .bind(OWNER_ID.parse::<Uuid>().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let client = Client::new();
    for _ in 0..6 {
        let response = client
            .post(format!("{base_url}/auth/password-reset"))
            .json(&json!({ "email": "api-test-owner@example.test" }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }
    let issued: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM user_lifecycle_action_tokens WHERE purpose = 'password_reset'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(issued, 5);
    server.abort();
}

#[sqlx::test]
async fn session_workspace_selects_the_explicit_catalog_scope(pool: PgPool) {
    let (base_url, server) = start_session_server(pool.clone()).await;
    let owner_id = OWNER_ID.parse::<Uuid>().unwrap();
    let workspace_id = Uuid::new_v4();
    let membership_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO workspaces (id, slug, name, login_identifier) VALUES ($1, 'second', 'Second workspace', 'second.local')",
    )
    .bind(workspace_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(membership_id)
    .bind(workspace_id)
    .bind(owner_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, '00000000-0000-4000-8000-000000000101', 'workspace', $2)")
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(membership_id)
        .execute(&pool)
        .await
        .unwrap();
    let hash = hash_password(&Password::new("correct horse battery staple")).unwrap();
    sqlx::query("INSERT INTO local_password_credentials (user_id, password_hash) VALUES ($1, $2)")
        .bind(owner_id)
        .bind(hash.as_phc())
        .execute(&pool)
        .await
        .unwrap();

    let client = Client::new();
    let default_login = client
        .post(format!("{base_url}/auth/login"))
        .json(&json!({ "login_identifier": "default.local", "email": "api-test-owner@example.test", "password": "correct horse battery staple" }))
        .send()
        .await
        .unwrap();
    let (default_session, default_csrf) = cookie_pair(&default_login);
    let second_login = client
        .post(format!("{base_url}/auth/login"))
        .json(&json!({ "login_identifier": "second.local", "email": "api-test-owner@example.test", "password": "correct horse battery staple" }))
        .send()
        .await
        .unwrap();
    assert_eq!(second_login.status(), StatusCode::OK);
    let (second_session, second_csrf) = cookie_pair(&second_login);
    let second_cookie = format!("{second_session}; {second_csrf}");
    let csrf_value = second_csrf.split_once('=').unwrap().1;
    let created = client
        .post(format!("{base_url}/blueprints"))
        .header("cookie", &second_cookie)
        .header("x-catalog-csrf", csrf_value)
        .json(&json!({ "definition": "format_version = 1\ncode = \"second_only\"\nname = \"Second only\"\nkind = \"record\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"title\"]\n\n[[attributes]]\ncode = \"title\"\nvalue_type = \"string\"" }))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);

    let default_cookie = format!("{default_session}; {default_csrf}");
    let default_catalogue: Value = client
        .get(format!("{base_url}/blueprints/catalogue"))
        .header("cookie", default_cookie)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let second_catalogue: Value = client
        .get(format!("{base_url}/blueprints/catalogue"))
        .header("cookie", second_cookie)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        !default_catalogue
            .as_array()
            .unwrap()
            .iter()
            .any(|blueprint| blueprint["code"] == "second_only")
    );
    assert!(
        second_catalogue
            .as_array()
            .unwrap()
            .iter()
            .any(|blueprint| blueprint["code"] == "second_only")
    );
    server.abort();
}

#[sqlx::test]
async fn stale_password_verification_cannot_issue_a_session(pool: PgPool) {
    let (_base_url, server) = start_session_server(pool.clone()).await;
    let password = Password::new("correct horse battery staple");
    let hash = hash_password(&password).unwrap();
    let owner_id = OWNER_ID.parse::<Uuid>().unwrap();
    sqlx::query("INSERT INTO local_password_credentials (user_id, password_hash) VALUES ($1, $2)")
        .bind(owner_id)
        .bind(hash.as_phc())
        .execute(&pool)
        .await
        .unwrap();

    let repository = CatalogRepository::system(pool.clone());
    let credential = repository
        .local_login_credential("api-test-owner@example.test")
        .await
        .unwrap()
        .unwrap();
    // This models a password reset completing after the password verifier read
    // the old credential but before it attempts to issue the login session.
    sqlx::query("UPDATE users SET security_version = security_version + 1 WHERE id = $1")
        .bind(owner_id)
        .execute(&pool)
        .await
        .unwrap();
    let result = repository
        .issue_login_session(
            Uuid::new_v4(),
            &credential,
            "00000000-0000-4000-8000-000000000002".parse().unwrap(),
            &SessionDigest::from_slice(&[7_u8; 32]).unwrap(),
            &SessionDigest::from_slice(&[8_u8; 32]).unwrap(),
            chrono::Utc::now() + chrono::Duration::hours(1),
        )
        .await;
    assert!(
        result.is_err(),
        "stale password verification must not create a session"
    );
    server.abort();
}

#[sqlx::test]
async fn login_rate_limit_rejects_attempts_after_the_fixed_window_threshold(pool: PgPool) {
    let (base_url, server) = start_session_server(pool).await;
    let client = Client::new();
    for _ in 0..5 {
        let response = client
            .post(format!("{base_url}/auth/login"))
            .json(&json!({ "login_identifier": "default.local", "email": "limited@example.test", "password": "wrong" }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    let limited = client
        .post(format!("{base_url}/auth/login"))
        .json(&json!({ "login_identifier": "default.local", "email": "limited@example.test", "password": "wrong" }))
        .send()
        .await
        .unwrap();
    assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
    server.abort();
}

#[sqlx::test]
async fn time_zone_preference_persists_on_the_account_and_rejects_unknown_zones(pool: PgPool) {
    let (base_url, server) = start_session_server(pool.clone()).await;
    let password = Password::new("correct horse battery staple");
    let hash = hash_password(&password).unwrap();
    let owner_id = OWNER_ID.parse::<Uuid>().unwrap();
    sqlx::query("INSERT INTO local_password_credentials (user_id, password_hash) VALUES ($1, $2)")
        .bind(owner_id)
        .bind(hash.as_phc())
        .execute(&pool)
        .await
        .unwrap();
    let client = Client::new();
    let login = client.post(format!("{base_url}/auth/login"))
        .json(&json!({ "login_identifier": "default.local", "email": "api-test-owner@example.test", "password": "correct horse battery staple" }))
        .send().await.unwrap();
    let (session, csrf) = cookie_pair(&login);
    let login: Value = login.json().await.unwrap();
    assert_eq!(login["time_zone"], Value::Null);
    let cookie = format!("{session}; {csrf}");
    let csrf_value = csrf.split_once('=').unwrap().1;
    let patch = |body: Value, csrf: Option<&str>| {
        let mut request = client
            .patch(format!("{base_url}/auth/preferences"))
            .header("cookie", &cookie)
            .json(&body);
        if let Some(csrf) = csrf {
            request = request.header("x-catalog-csrf", csrf);
        }
        request.send()
    };

    assert_eq!(
        patch(json!({ "time_zone": "Europe/Warsaw" }), None)
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let updated = patch(json!({ "time_zone": "Europe/Warsaw" }), Some(csrf_value))
        .await
        .unwrap();
    assert_eq!(updated.status(), StatusCode::OK);
    let updated: Value = updated.json().await.unwrap();
    assert_eq!(updated["time_zone"], "Europe/Warsaw");
    assert_eq!(updated["user_id"], OWNER_ID);

    // The preference belongs to the account, so a separate login sees it.
    let second_login: Value = client.post(format!("{base_url}/auth/login"))
        .json(&json!({ "login_identifier": "default.local", "email": "api-test-owner@example.test", "password": "correct horse battery staple" }))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(second_login["time_zone"], "Europe/Warsaw");

    for invalid in [json!("Mars/Olympus"), json!(""), json!("+02:00")] {
        let rejected = patch(json!({ "time_zone": invalid }), Some(csrf_value))
            .await
            .unwrap();
        assert_eq!(rejected.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
    let stored: Option<String> = sqlx::query_scalar("SELECT time_zone FROM users WHERE id = $1")
        .bind(owner_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored.as_deref(), Some("Europe/Warsaw"));

    let utc: Value = patch(json!({ "time_zone": "UTC" }), Some(csrf_value))
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(utc["time_zone"], "UTC");
    let cleared: Value = patch(json!({ "time_zone": null }), Some(csrf_value))
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(cleared["time_zone"], Value::Null);
    let current: Value = client
        .get(format!("{base_url}/auth/session"))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(current["time_zone"], Value::Null);
    server.abort();
}

#[sqlx::test]
async fn display_name_updates_the_account_and_rejects_invalid_names(pool: PgPool) {
    let (base_url, server) = start_session_server(pool.clone()).await;
    let password = Password::new("correct horse battery staple");
    let hash = hash_password(&password).unwrap();
    let owner_id = OWNER_ID.parse::<Uuid>().unwrap();
    sqlx::query("INSERT INTO local_password_credentials (user_id, password_hash) VALUES ($1, $2)")
        .bind(owner_id)
        .bind(hash.as_phc())
        .execute(&pool)
        .await
        .unwrap();
    let client = Client::new();
    let login = client.post(format!("{base_url}/auth/login"))
        .json(&json!({ "login_identifier": "default.local", "email": "api-test-owner@example.test", "password": "correct horse battery staple" }))
        .send().await.unwrap();
    let (session, csrf) = cookie_pair(&login);
    let cookie = format!("{session}; {csrf}");
    let csrf_value = csrf.split_once('=').unwrap().1;
    let put = |body: Value, csrf: Option<&str>| {
        let mut request = client
            .put(format!("{base_url}/auth/display-name"))
            .header("cookie", &cookie)
            .json(&body);
        if let Some(csrf) = csrf {
            request = request.header("x-catalog-csrf", csrf);
        }
        request.send()
    };

    assert_eq!(
        put(json!({ "display_name": "Ada Lovelace" }), None)
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let updated = put(json!({ "display_name": "Ada Lovelace" }), Some(csrf_value))
        .await
        .unwrap();
    assert_eq!(updated.status(), StatusCode::OK);
    let updated: Value = updated.json().await.unwrap();
    assert_eq!(updated["display_name"], "Ada Lovelace");
    assert_eq!(updated["user_id"], OWNER_ID);

    for invalid in [
        json!("A"),
        json!(" Ada"),
        json!("Ada "),
        json!("Ada-Lovelace"),
        json!("a".repeat(65)),
        Value::Null,
    ] {
        let rejected = put(json!({ "display_name": invalid }), Some(csrf_value))
            .await
            .unwrap();
        assert_eq!(
            rejected.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "{invalid}"
        );
    }
    let stored: Option<String> = sqlx::query_scalar("SELECT display_name FROM users WHERE id = $1")
        .bind(owner_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored.as_deref(), Some("Ada Lovelace"));
    server.abort();
}

#[sqlx::test]
async fn sample_accounts_hold_their_roles_and_are_published(pool: PgPool) {
    let client = Client::new();
    let (base_url, server) = start_server_with_config(pool.clone(), |_| {}).await;
    let body: serde_json::Value = client
        .get(format!("{base_url}/auth/sample-logins"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body, serde_json::Value::Null);
    server.abort();

    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse().unwrap();
    let system = CatalogRepository::system(pool.clone());
    // Seeding twice leaves one membership with one grant.
    for _ in 0..2 {
        system
            .ensure_sample_account(workspace_id, "Editor@Example.test", "editor")
            .await
            .unwrap();
    }
    let roles: Vec<String> = sqlx::query_scalar("SELECT r.code FROM users u JOIN workspace_memberships m ON m.user_id = u.id JOIN role_grants g ON g.membership_id = m.id JOIN roles r ON r.id = g.role_id WHERE u.email = 'editor@example.test' AND m.workspace_id = $1")
        .bind(workspace_id)
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(roles, ["editor"]);

    let logins = api::http::SampleLogins {
        demo: false,
        login_identifier: "default.local".to_owned(),
        password: "shared-password".to_owned(),
        accounts: vec![api::http::SampleAccount {
            role: "editor".to_owned(),
            email: "editor@example.test".to_owned(),
        }],
    };
    let (base_url, server) = start_server_with_config(pool, |state| {
        state.sample_logins = Some(logins);
    })
    .await;
    let body: serde_json::Value = client
        .get(format!("{base_url}/auth/sample-logins"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        body,
        serde_json::json!({
            "demo": false,
            "login_identifier": "default.local",
            "password": "shared-password",
            "accounts": [{ "role": "editor", "email": "editor@example.test" }],
        })
    );
    server.abort();
}

#[sqlx::test]
async fn demo_mode_refuses_changes_that_could_lock_visitors_out(pool: PgPool) {
    let (base_url, server) = start_server_with_config(pool, |state| state.demo_mode = true).await;
    let anonymous = Client::new();
    let owner = authenticated_client();
    let member = Uuid::new_v4();
    let grant = Uuid::new_v4();
    let role = Uuid::new_v4();
    let requests = [
        anonymous
            .post(format!("{base_url}/auth/password-reset"))
            .json(&serde_json::json!({ "email": "owner@example.test" })),
        anonymous
            .post(format!("{base_url}/auth/password-reset/confirm"))
            .json(&serde_json::json!({ "token": "any", "password": "a new password value" })),
        owner
            .put(format!("{base_url}/workspace/members/{member}"))
            .json(&serde_json::json!({ "state": "inactive" })),
        owner
            .post(format!("{base_url}/workspace/members/{member}/grants"))
            .json(&serde_json::json!({ "role_id": role, "scope_type": "workspace", "scope_target_id": BOOTSTRAP_WORKSPACE_ID })),
        owner.delete(format!("{base_url}/workspace/members/{member}/grants/{grant}")),
        owner.post(format!("{base_url}/workspace/members/{member}/transfer-ownership")),
        owner
            .post(format!("{base_url}/workspace/invitations"))
            .json(&serde_json::json!({ "email": "someone@example.test", "role_id": role, "scope_type": "workspace", "scope_target_id": BOOTSTRAP_WORKSPACE_ID,
                "expires_at": "2099-01-01T00:00:00Z" })),
        owner
            .post(format!("{base_url}/workspace/users"))
            .json(&serde_json::json!({ "email": "someone@example.test", "role_id": role, "scope_type": "workspace", "scope_target_id": BOOTSTRAP_WORKSPACE_ID })),
    ];
    for request in requests {
        let response = request.send().await.unwrap();
        let path = response.url().path().to_owned();
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{path}");
        let error: serde_json::Value = response.json().await.unwrap();
        assert_eq!(error["error"]["code"], "disabled_in_demo", "{path}");
    }
    server.abort();
}
