mod support;

use api::account::{Password, hash_password};
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
    assert_eq!(
        login.json::<Value>().await.unwrap()["login_identifier"],
        json!("default.local")
    );
    let cookie = format!("{session}; {csrf}");
    let current = client
        .get(format!("{base_url}/auth/session"))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(current.status(), StatusCode::OK);
    assert_eq!(
        current.json::<Value>().await.unwrap()["login_identifier"],
        json!("default.local")
    );
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
async fn session_workspace_selects_the_rls_catalog_pool(pool: PgPool) {
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
        .json(&json!({ "definition": "format_version = 1\ncode = \"second_only\"\nname = \"Second only\"\nkind = \"entity\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"title\"]\n\n[[attributes]]\ncode = \"title\"\nvalue_type = \"string\"" }))
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

    // This models a password reset completing after the password verifier read
    // the old credential but before it attempts to issue the login session.
    sqlx::query("UPDATE users SET security_version = security_version + 1 WHERE id = $1")
        .bind(owner_id)
        .execute(&pool)
        .await
        .unwrap();
    let result = sqlx::query(
        "SELECT issue_browser_login_session($1, $2, '00000000-0000-4000-8000-000000000002', 1, 1, $3, $4, now() + interval '1 hour')",
    )
    .bind(Uuid::new_v4())
    .bind(owner_id)
    .bind(vec![7_u8; 32])
    .bind(vec![8_u8; 32])
    .execute(&pool)
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
