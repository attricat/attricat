use crate::support;
use std::sync::Arc;

use api::{
    account::{Password, hash_password},
    http::{AppState, OperatorToken},
    repository::AttricatRepository,
    task_queue::TaskKind,
};
use reqwest::{Response, Url, header::SET_COOKIE};
use sha2::{Digest, Sha256};
use support::*;

const OPERATOR_SECRET: &str = "operator-test-token-with-enough-bytes";
const OWNER_PASSWORD: &str = "Correct-horse-battery-7-staple!";
const OWNER_EMAIL: &str = "new-owner@example.test";
const BOOTSTRAP_OWNER_EMAIL: &str = "api-test-owner@example.test";

fn with_operator_token(state: &mut AppState) {
    state.operator_token = Some(OperatorToken::new(OPERATOR_SECRET));
}

async fn start(pool: &PgPool) -> (String, JoinHandle<()>, Arc<TestMailDelivery>) {
    let mail = Arc::new(TestMailDelivery::default());
    let (base_url, server) =
        start_configured_session_server(pool.clone(), mail.clone(), with_operator_token).await;
    (base_url, server, mail)
}

fn workspace_body(identifier: &str, owner_email: &str) -> Value {
    json!({
        "name": "Acme",
        "login_identifier": identifier,
        "owner_email": owner_email,
    })
}

async fn put_workspace(base_url: &str, id: Uuid, body: &Value) -> Response {
    Client::new()
        .put(format!("{base_url}/operator/workspaces/{id}"))
        .bearer_auth(OPERATOR_SECRET)
        .json(body)
        .send()
        .await
        .unwrap()
}

async fn replace_invitation(base_url: &str, id: Uuid, body: &Value) -> Response {
    Client::new()
        .post(format!(
            "{base_url}/operator/workspaces/{id}/owner-invitation"
        ))
        .bearer_auth(OPERATOR_SECRET)
        .json(body)
        .send()
        .await
        .unwrap()
}

async fn error_code(response: Response) -> String {
    let body: Value = response.json().await.unwrap();
    body["error"]["code"].as_str().unwrap().to_owned()
}

fn query_value(url: &str, name: &str) -> String {
    Url::parse(url)
        .unwrap()
        .query_pairs()
        .find_map(|(key, value)| (key == name).then(|| value.into_owned()))
        .unwrap()
}

/// The `Cookie` header and CSRF value a sign-in response set.
fn session_cookies(response: &Response) -> (String, String) {
    let pairs: Vec<&str> = response
        .headers()
        .get_all(SET_COOKIE)
        .iter()
        .map(|value| value.to_str().unwrap().split(';').next().unwrap())
        .collect();
    let csrf = pairs
        .iter()
        .find_map(|pair| pair.strip_prefix("attricat_csrf="))
        .unwrap()
        .to_owned();
    (pairs.join("; "), csrf)
}

async fn complete_onboarding(
    base_url: &str,
    mail: &TestMailDelivery,
    email: &str,
) -> (String, String) {
    let url = mail.workspace_onboarding_url(email).await.unwrap();
    let response = Client::new()
        .post(format!("{base_url}/onboarding/complete"))
        .json(&json!({
            "invitation_secret": query_value(&url, "invitation_secret"),
            "onboarding_secret": query_value(&url, "onboarding_secret"),
            "password": OWNER_PASSWORD,
        }))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success(), "{}", response.status());
    session_cookies(&response)
}

async fn sign_in(base_url: &str, identifier: &str, email: &str) -> (String, String) {
    let response = Client::new()
        .post(format!("{base_url}/auth/login"))
        .json(&json!({
            "login_identifier": identifier,
            "email": email,
            "password": OWNER_PASSWORD,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    session_cookies(&response)
}

async fn get_json(base_url: &str, path: &str, cookie: &str) -> Value {
    let response = Client::new()
        .get(format!("{base_url}{path}"))
        .header("cookie", cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "{path}");
    response.json().await.unwrap()
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn operator_routes_do_not_exist_without_a_configured_token(pool: PgPool) {
    let (base_url, server) = start_session_server(pool).await;
    let response = Client::new()
        .get(format!("{base_url}/operator/workspaces"))
        .bearer_auth(OPERATOR_SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn operator_routes_accept_only_the_operator_token(pool: PgPool) {
    let (base_url, server, _) = start(&pool).await;
    let client = Client::new();
    let list = format!("{base_url}/operator/workspaces");

    let anonymous = client.get(&list).send().await.unwrap();
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);
    let wrong = client.get(&list).bearer_auth("wrong").send().await.unwrap();
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);

    // A live personal token of the deployment's owner is not an operator.
    let owner: Uuid = BOOTSTRAP_OWNER_ID.parse().unwrap();
    let personal_secret = "cat_pat_operator_test_token";
    let digest = Sha256::digest(personal_secret.as_bytes()).to_vec();
    let token = Uuid::new_v4();
    sqlx::query("INSERT INTO personal_api_tokens (id, user_id, workspace_id, label, token_digest) VALUES ($1, $2, $3, 'owner', $4)")
        .bind(token)
        .bind(owner)
        .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
        .bind(&digest)
        .execute(&pool)
        .await
        .unwrap();
    let personal = client
        .get(&list)
        .bearer_auth(personal_secret)
        .send()
        .await
        .unwrap();
    assert_eq!(personal.status(), StatusCode::UNAUTHORIZED);

    // A signed-in browser session is not an operator either.
    let hash = hash_password(&Password::new(OWNER_PASSWORD)).unwrap();
    sqlx::query("INSERT INTO local_password_credentials (user_id, password_hash) VALUES ($1, $2)")
        .bind(owner)
        .bind(hash.as_phc())
        .execute(&pool)
        .await
        .unwrap();
    let (cookie, _) = sign_in(&base_url, "default.local", BOOTSTRAP_OWNER_EMAIL).await;
    let session = client
        .get(&list)
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(session.status(), StatusCode::UNAUTHORIZED);

    // The operator token grants nothing on member routes.
    let member_route = client
        .get(format!("{base_url}/workspace/members"))
        .bearer_auth(OPERATOR_SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(member_route.status(), StatusCode::UNAUTHORIZED);

    let operator = client
        .get(&list)
        .bearer_auth(OPERATOR_SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(operator.status(), StatusCode::OK);
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn operator_routes_are_refused_in_demo_mode(pool: PgPool) {
    let (base_url, server) = start_configured_session_server(
        pool,
        Arc::new(TestMailDelivery::default()),
        |state: &mut AppState| {
            with_operator_token(state);
            state.demo_mode = true;
        },
    )
    .await;
    let response = put_workspace(
        &base_url,
        Uuid::new_v4(),
        &workspace_body("acme.example", OWNER_EMAIL),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(error_code(response).await, "disabled_in_demo");
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn new_owner_onboards_into_a_workspace_created_without_a_restart(pool: PgPool) {
    let (base_url, server, mail) = start(&pool).await;
    let id = Uuid::new_v4();
    let response = put_workspace(
        &base_url,
        id,
        &json!({
            "name": "  Acme  ",
            "login_identifier": " ACME.example ",
            "owner_email": OWNER_EMAIL,
            "owner_display_name": "Ada Owner",
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let created: Value = response.json().await.unwrap();
    assert_eq!(created["id"], id.to_string());
    assert_eq!(created["name"], "Acme");
    assert_eq!(created["login_identifier"], "acme.example");
    assert_eq!(created["owner_email"], OWNER_EMAIL);
    assert_eq!(created["has_active_owner"], false);
    assert!(created["owner_invitation_expires_at"].is_string());

    // Sign-in discovery finds it, and setup ran.
    let discovery = Client::new()
        .post(format!("{base_url}/auth/discover"))
        .json(&json!({ "login_identifier": "acme.example" }))
        .send()
        .await
        .unwrap();
    assert_eq!(discovery.status(), StatusCode::OK);
    let (system_user, slug): (Option<Uuid>, String) =
        sqlx::query_as("SELECT system_user_id, slug FROM workspaces WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(system_user.is_some());
    assert_eq!(slug, "acme_example");

    let (cookie, csrf) = complete_onboarding(&base_url, &mail, OWNER_EMAIL).await;
    let session = get_json(&base_url, "/auth/session", &cookie).await;
    assert_eq!(session["login_identifier"], "acme.example");
    assert_eq!(session["email"], OWNER_EMAIL);
    let contexts = get_json(&base_url, "/contexts", &cookie).await;
    assert_eq!(contexts.as_array().unwrap().len(), 1);
    assert_eq!(contexts[0]["code"], "default");

    // The owner can work in the workspace straight away.
    let context = Client::new()
        .post(format!("{base_url}/contexts"))
        .header("cookie", &cookie)
        .header("x-attricat-csrf", &csrf)
        .json(&json!({ "code": "owner-context", "data": {} }))
        .send()
        .await
        .unwrap();
    assert_eq!(context.status(), StatusCode::CREATED);

    // The operator sees the owner, and the system inviter has no inbox.
    let current: Value = Client::new()
        .get(format!("{base_url}/operator/workspaces/{id}"))
        .bearer_auth(OPERATOR_SECRET)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(current["has_active_owner"], true);
    assert!(current["owner_invitation_expires_at"].is_null());
    let inbox_rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM user_notifications WHERE recipient_user_id = $1 AND workspace_id = $2",
    )
    .bind(system_user)
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(inbox_rows, 0);

    // The operator's actions are audited in the new workspace.
    let actions: Vec<(String, Option<Uuid>, Value)> = sqlx::query_as(
        "SELECT action, actor_user_id, metadata FROM audit_events WHERE workspace_id = $1 AND action LIKE 'workspace.%' ORDER BY occurred_at",
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(actions[0].0, "workspace.create");
    assert_eq!(actions[1].0, "workspace.owner_invitation.create");
    for (_, actor, metadata) in &actions[..2] {
        assert!(actor.is_none());
        assert_eq!(metadata["actor"], "operator");
    }
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn existing_user_accepts_ownership_with_their_current_password(pool: PgPool) {
    let (base_url, server, mail) = start(&pool).await;
    let owner: Uuid = BOOTSTRAP_OWNER_ID.parse().unwrap();
    let hash = hash_password(&Password::new(OWNER_PASSWORD)).unwrap();
    sqlx::query("INSERT INTO local_password_credentials (user_id, password_hash) VALUES ($1, $2)")
        .bind(owner)
        .bind(hash.as_phc())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE users SET email_verified_at = now() WHERE id = $1")
        .bind(owner)
        .execute(&pool)
        .await
        .unwrap();
    let response = put_workspace(
        &base_url,
        Uuid::new_v4(),
        &workspace_body("acme.example", BOOTSTRAP_OWNER_EMAIL),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert!(
        mail.workspace_onboarding_url(BOOTSTRAP_OWNER_EMAIL)
            .await
            .is_none()
    );

    // Being named as owner grants nothing until the person accepts.
    let refused = Client::new()
        .post(format!("{base_url}/auth/login"))
        .json(&json!({
            "login_identifier": "acme.example",
            "email": BOOTSTRAP_OWNER_EMAIL,
            "password": OWNER_PASSWORD,
        }))
        .send()
        .await
        .unwrap();
    assert!(!refused.status().is_success());

    let url = mail
        .workspace_invitation_url(BOOTSTRAP_OWNER_EMAIL)
        .await
        .unwrap();
    let (cookie, csrf) = sign_in(&base_url, "default.local", BOOTSTRAP_OWNER_EMAIL).await;
    let accepted = Client::new()
        .post(format!("{base_url}/workspace/invitations/accept"))
        .header("cookie", &cookie)
        .header("x-attricat-csrf", &csrf)
        .json(&json!({ "secret": query_value(&url, "secret") }))
        .send()
        .await
        .unwrap();
    assert_eq!(accepted.status(), StatusCode::OK);
    let (cookie, _) = sign_in(&base_url, "acme.example", BOOTSTRAP_OWNER_EMAIL).await;
    let session = get_json(&base_url, "/auth/session", &cookie).await;
    assert_eq!(session["login_identifier"], "acme.example");
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn repeating_a_create_request_is_safe_and_conflicts_are_refused(pool: PgPool) {
    let (base_url, server, mail) = start(&pool).await;
    let id = Uuid::new_v4();
    let body = workspace_body("acme.example", OWNER_EMAIL);
    assert_eq!(
        put_workspace(&base_url, id, &body).await.status(),
        StatusCode::CREATED
    );
    assert_eq!(mail.delivered_count().await, 1);

    // The same request confirms the workspace and sends no second email.
    assert_eq!(
        put_workspace(&base_url, id, &body).await.status(),
        StatusCode::OK
    );
    assert_eq!(mail.delivered_count().await, 1);

    let changed = put_workspace(
        &base_url,
        id,
        &json!({
            "name": "Renamed",
            "login_identifier": "acme.example",
            "owner_email": OWNER_EMAIL,
        }),
    )
    .await;
    assert_eq!(changed.status(), StatusCode::CONFLICT);
    assert_eq!(error_code(changed).await, "workspace_conflict");

    let taken = put_workspace(&base_url, Uuid::new_v4(), &body).await;
    assert_eq!(taken.status(), StatusCode::CONFLICT);
    assert_eq!(error_code(taken).await, "login_identifier_taken");
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn retries_finish_work_a_crash_left_undone(pool: PgPool) {
    let (base_url, server, mail) = start(&pool).await;
    // Crash after the insert: setup never ran and no invitation exists.
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO workspaces (id, slug, name, login_identifier, bootstrap_owner_email) VALUES ($1, 'acme_example', 'Acme', 'acme.example', $2)")
        .bind(id)
        .bind(OWNER_EMAIL)
        .execute(&pool)
        .await
        .unwrap();
    let body = workspace_body("acme.example", OWNER_EMAIL);
    assert_eq!(
        put_workspace(&base_url, id, &body).await.status(),
        StatusCode::OK
    );
    assert_eq!(mail.delivered_count().await, 1);
    let (cookie, _) = complete_onboarding(&base_url, &mail, OWNER_EMAIL).await;
    let contexts = get_json(&base_url, "/contexts", &cookie).await;
    assert_eq!(contexts[0]["code"], "default");
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn invalid_input_is_refused_before_a_workspace_exists(pool: PgPool) {
    let (base_url, server, _) = start(&pool).await;
    for (body, status, code) in [
        (
            workspace_body("not a domain", OWNER_EMAIL),
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_login_identifier",
        ),
        (
            workspace_body("acme.example", "not-an-email"),
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_email",
        ),
        (
            workspace_body("acme.example", "system+x@attricat.invalid"),
            StatusCode::UNPROCESSABLE_ENTITY,
            "invitation_invalid",
        ),
        (
            json!({
                "name": "  ",
                "login_identifier": "acme.example",
                "owner_email": OWNER_EMAIL,
            }),
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_input",
        ),
    ] {
        let response = put_workspace(&base_url, Uuid::new_v4(), &body).await;
        assert_eq!(response.status(), status, "{body}");
        assert_eq!(error_code(response).await, code, "{body}");
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM workspaces")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_failed_email_withdraws_the_invitation_so_a_retry_sends_a_new_one(pool: PgPool) {
    let (base_url, server, mail) = start(&pool).await;
    let id = Uuid::new_v4();
    let body = workspace_body("acme.example", OWNER_EMAIL);
    mail.fail_next_delivery();
    let failed = put_workspace(&base_url, id, &body).await;
    assert_eq!(failed.status(), StatusCode::BAD_GATEWAY);
    assert_eq!(error_code(failed).await, "invitation_delivery_failed");
    let pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM workspace_invitations WHERE workspace_id = $1 AND revoked_at IS NULL",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pending, 0);

    assert_eq!(
        put_workspace(&base_url, id, &body).await.status(),
        StatusCode::OK
    );
    assert_eq!(mail.delivered_count().await, 1);
    complete_onboarding(&base_url, &mail, OWNER_EMAIL).await;
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn replacing_an_invitation_retires_the_old_link_and_stops_once_owned(pool: PgPool) {
    let (base_url, server, mail) = start(&pool).await;
    let id = Uuid::new_v4();
    let response = put_workspace(&base_url, id, &workspace_body("acme.example", OWNER_EMAIL)).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let old_url = mail.workspace_onboarding_url(OWNER_EMAIL).await.unwrap();

    // The first address was wrong: invite someone else instead.
    let corrected = "corrected-owner@example.test";
    let replaced = replace_invitation(&base_url, id, &json!({ "owner_email": corrected })).await;
    assert_eq!(replaced.status(), StatusCode::OK);
    let replaced: Value = replaced.json().await.unwrap();
    assert_eq!(replaced["owner_email"], corrected);

    let old_link = Client::new()
        .post(format!("{base_url}/onboarding/complete"))
        .json(&json!({
            "invitation_secret": query_value(&old_url, "invitation_secret"),
            "onboarding_secret": query_value(&old_url, "onboarding_secret"),
            "password": OWNER_PASSWORD,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(old_link.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error_code(old_link).await, "invitation_invalid");

    complete_onboarding(&base_url, &mail, corrected).await;
    let owned = replace_invitation(&base_url, id, &json!({})).await;
    assert_eq!(owned.status(), StatusCode::CONFLICT);
    assert_eq!(error_code(owned).await, "workspace_owner_exists");

    let missing = replace_invitation(&base_url, Uuid::new_v4(), &json!({})).await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn workspaces_are_listed_in_pages_and_read_by_id(pool: PgPool) {
    let (base_url, server, _) = start(&pool).await;
    let client = Client::new();
    for identifier in ["first.example", "second.example"] {
        let id = Uuid::new_v4();
        let owner = format!("owner@{identifier}");
        let response = put_workspace(&base_url, id, &workspace_body(identifier, &owner)).await;
        assert_eq!(response.status(), StatusCode::CREATED);
    }
    let first_page = client
        .get(format!("{base_url}/operator/workspaces?limit=2"))
        .bearer_auth(OPERATOR_SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(first_page.headers()["x-next-offset"], "2");
    let first_page: Value = first_page.json().await.unwrap();
    // The bootstrap workspace is an ordinary entry, oldest first.
    assert_eq!(first_page[0]["login_identifier"], "default.local");
    assert_eq!(first_page[0]["has_active_owner"], true);
    assert_eq!(first_page[1]["login_identifier"], "first.example");
    let last_page = client
        .get(format!("{base_url}/operator/workspaces?limit=2&offset=2"))
        .bearer_auth(OPERATOR_SECRET)
        .send()
        .await
        .unwrap();
    assert!(last_page.headers().get("x-next-offset").is_none());
    let last_page: Value = last_page.json().await.unwrap();
    assert_eq!(last_page.as_array().unwrap().len(), 1);

    let missing = client
        .get(format!("{base_url}/operator/workspaces/{}", Uuid::new_v4()))
        .bearer_auth(OPERATOR_SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn created_workspaces_are_isolated_and_reach_background_work(pool: PgPool) {
    let (base_url, server, mail) = start(&pool).await;
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let second_owner = "second-owner@example.test";
    for (id, identifier, owner) in [
        (first, "first.example", OWNER_EMAIL),
        (second, "second.example", second_owner),
    ] {
        let response = put_workspace(&base_url, id, &workspace_body(identifier, owner)).await;
        assert_eq!(response.status(), StatusCode::CREATED);
    }
    let (first_cookie, first_csrf) = complete_onboarding(&base_url, &mail, OWNER_EMAIL).await;
    let (second_cookie, _) = complete_onboarding(&base_url, &mail, second_owner).await;

    let context = Client::new()
        .post(format!("{base_url}/contexts"))
        .header("cookie", &first_cookie)
        .header("x-attricat-csrf", &first_csrf)
        .json(&json!({ "code": "first-only", "data": {} }))
        .send()
        .await
        .unwrap();
    assert_eq!(context.status(), StatusCode::CREATED);
    let contexts = get_json(&base_url, "/contexts", &second_cookie).await;
    assert_eq!(contexts.as_array().unwrap().len(), 1);
    let members = get_json(&base_url, "/workspace/members", &second_cookie).await;
    assert_eq!(members.as_array().unwrap().len(), 1);
    assert_eq!(members[0]["email"], second_owner);

    // The background loops read this list, and a queued task is claimed for
    // the new workspace with no restart.
    let system = AttricatRepository::system(pool.clone());
    let active = system.active_workspace_ids().await.unwrap();
    assert!(active.contains(&first) && active.contains(&second));
    sqlx::query("INSERT INTO tasks (id, workspace_id, kind, subject_id, generation, envelope_version, max_failures, status, available_at, payload) VALUES ($1, $2, $3, $4, 0, 1, 3, 'queued', now() - interval '1 second', '{}')")
        .bind(Uuid::new_v4())
        .bind(second)
        .bind(TaskKind::RuleRunV1.as_str())
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await
        .unwrap();
    system.materialize_task_workspace_service().await.unwrap();
    let claimed = system
        .claim_task("operator-test-worker", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.workspace_id, second);
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn operator_can_invite_an_owner_only_when_no_owner_can_get_in(pool: PgPool) {
    let (base_url, server, mail) = start(&pool).await;
    let id = Uuid::new_v4();
    let created = put_workspace(&base_url, id, &workspace_body("acme.example", OWNER_EMAIL)).await;
    assert_eq!(created.status(), StatusCode::CREATED);
    complete_onboarding(&base_url, &mail, OWNER_EMAIL).await;

    // An owner with a password can still get in, so ownership stays theirs.
    let refused = replace_invitation(&base_url, id, &json!({})).await;
    assert_eq!(refused.status(), StatusCode::CONFLICT);
    assert_eq!(error_code(refused).await, "workspace_owner_exists");

    // As it is for an owner who only ever signed in through the provider.
    sqlx::query("DELETE FROM local_password_credentials c USING users u WHERE c.user_id = u.id AND u.email = $1")
        .bind(OWNER_EMAIL)
        .execute(&pool)
        .await
        .unwrap();
    let recovered = replace_invitation(&base_url, id, &json!({})).await;
    assert_eq!(recovered.status(), StatusCode::OK);
    let (cookie, _) = complete_onboarding(&base_url, &mail, OWNER_EMAIL).await;
    let session = get_json(&base_url, "/auth/session", &cookie).await;
    assert_eq!(session["workspace_id"], id.to_string());
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn another_workspace_cannot_name_an_existing_account(pool: PgPool) {
    let (user, _) = add_workspace_user(&pool).await;
    let email: String = sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE users SET display_name = NULL WHERE id = $1")
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    let (base_url, server) = start_server_with_config(pool.clone(), |_| {}).await;
    let response = authenticated_client()
        .post(format!("{base_url}/workspace/users"))
        .json(&json!({ "email": email, "display_name": "Set Elsewhere" }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let name: Option<String> = sqlx::query_scalar("SELECT display_name FROM users WHERE id = $1")
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(name, None);

    let invalid = authenticated_client()
        .post(format!("{base_url}/workspace/users"))
        .json(&json!({ "email": "new@example.test", "display_name": "x".repeat(65) }))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    server.abort();
}
