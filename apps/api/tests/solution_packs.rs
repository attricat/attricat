mod support;

use std::io::Cursor;

use api::{
    account::{Password, hash_password},
    solution_packs::MAX_SOLUTION_PACK_INSPECTION_RESPONSE_BYTES,
};
use reqwest::header::SET_COOKIE;
use sha2::{Digest, Sha256};
use support::*;

const PRODUCT_BLUEPRINT: &[u8] = br#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"

# source-only marker: BLUEPRINT_SOURCE_SECRET
"#;
const WEB_CONTEXT: &[u8] = br#"{"format_version":1,"code":"web","data":{"secret":"CONTEXT_DATA_SECRET"},"parent":"system/default"}"#;

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn valid_archive() -> Vec<u8> {
    archive_with_blueprint(PRODUCT_BLUEPRINT)
}

fn archive_with_blueprint(product_blueprint: &[u8]) -> Vec<u8> {
    archive_with_options(product_blueprint, false)
}

fn archive_with_required_context(product_blueprint: &[u8]) -> Vec<u8> {
    archive_with_options(product_blueprint, true)
}

fn context_only_archive() -> Vec<u8> {
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "id": "attricat.contexts",
        "name": "Contexts",
        "version": "1.0.0",
        "description": "Context starter",
        "catalog": {"host_api": "^1.0"},
        "resources": {
            "blueprints": [],
            "contexts": [{
                "key": "contexts/web",
                "path": "contexts/web.json",
                "required": true,
                "sha256": digest(WEB_CONTEXT)
            }]
        }
    }))
    .unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "solution-pack.json", &manifest);
        append_file(&mut tar, "contexts/web.json", WEB_CONTEXT);
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn archive_with_options(product_blueprint: &[u8], context_required: bool) -> Vec<u8> {
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "id": "attricat.ecommerce",
        "name": "Ecommerce",
        "version": "1.2.0",
        "description": "Starter catalog",
        "catalog": {"host_api": ">=1.0.0, <2.0.0"},
        "resources": {
            "blueprints": [{
                "key": "blueprints/product",
                "path": "blueprints/product.toml",
                "required": true,
                "sha256": digest(product_blueprint)
            }],
            "contexts": [{
                "key": "contexts/web",
                "path": "contexts/web.json",
                "required": context_required,
                "sha256": digest(WEB_CONTEXT)
            }]
        }
    }))
    .unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "solution-pack.json", &manifest);
        append_file(&mut tar, "blueprints/product.toml", product_blueprint);
        append_file(&mut tar, "contexts/web.json", WEB_CONTEXT);
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn append_file(tar: &mut tar::Builder<&mut Vec<u8>>, path: &str, bytes: &[u8]) {
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, path, bytes).unwrap();
}

fn bearer_client(secret: &str) -> Client {
    Client::builder()
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                "authorization",
                reqwest::header::HeaderValue::from_str(&format!("Bearer {secret}")).unwrap(),
            );
            headers
        })
        .build()
        .unwrap()
}

fn session_cookies(response: &reqwest::Response) -> (String, String) {
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

fn principal_client(user_id: Uuid) -> Client {
    Client::builder()
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                "x-catalog-user-id",
                reqwest::header::HeaderValue::from_str(&user_id.to_string()).unwrap(),
            );
            headers.insert(
                "x-catalog-workspace-id",
                reqwest::header::HeaderValue::from_static(BOOTSTRAP_WORKSPACE_ID),
            );
            headers
        })
        .build()
        .unwrap()
}

async fn create_principal_with_role(pool: &PgPool, email: &str, role_id: &str) -> Uuid {
    let user_id = Uuid::new_v4();
    let membership_id = Uuid::new_v4();
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, $2)")
        .bind(user_id)
        .bind(email)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(membership_id)
    .bind(workspace_id)
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, $4::uuid, 'workspace', $2)")
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(membership_id)
        .bind(role_id)
        .execute(pool)
        .await
        .unwrap();
    user_id
}

async fn create_pat(client: &Client, base_url: &str, permissions: &[&str]) -> String {
    let response = client
        .post(format!("{base_url}/personal-access-tokens"))
        .json(&json!({"label": Uuid::new_v4().to_string(), "permissions": permissions}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    response.json::<Value>().await.unwrap()["secret"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn inspect(client: &Client, base_url: &str, archive: Vec<u8>) -> reqwest::Response {
    client
        .post(format!("{base_url}/solution-packs/inspect"))
        .header("content-type", "application/zstd")
        .body(archive)
        .send()
        .await
        .unwrap()
}

async fn create_plan(
    client: &Client,
    base_url: &str,
    archive: Vec<u8>,
    prefix: &str,
) -> reqwest::Response {
    create_plan_with_publication(client, base_url, archive, prefix, "draft").await
}

async fn create_plan_with_publication(
    client: &Client,
    base_url: &str,
    archive: Vec<u8>,
    prefix: &str,
    publication: &str,
) -> reqwest::Response {
    client
        .post(format!(
            "{base_url}/solution-packs/plans?prefix={prefix}&blueprint_publication={publication}"
        ))
        .header("content-type", "application/zstd")
        .body(archive)
        .send()
        .await
        .unwrap()
}

async fn apply_plan(client: &Client, base_url: &str, plan_id: &str) -> reqwest::Response {
    client
        .post(format!("{base_url}/solution-packs/plans/{plan_id}/apply"))
        .send()
        .await
        .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn inspection_requires_authentication_and_solution_pack_permission(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let archive = valid_archive();

    let unauthenticated = inspect(&Client::new(), &base_url, archive.clone()).await;
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        unauthenticated.json::<Value>().await.unwrap()["error"]["code"],
        "unauthenticated"
    );

    let viewer_id = create_principal_with_role(
        &pool,
        "solution-pack-viewer@example.test",
        "00000000-0000-4000-8000-000000000104",
    )
    .await;
    let unauthorized = inspect(&principal_client(viewer_id), &base_url, archive.clone()).await;
    assert_eq!(unauthorized.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        unauthorized.json::<Value>().await.unwrap()["error"]["code"],
        "forbidden"
    );

    assert_eq!(
        inspect(&authenticated_client(), &base_url, archive.clone())
            .await
            .status(),
        StatusCode::OK
    );

    let admin_id = create_principal_with_role(
        &pool,
        "solution-pack-admin@example.test",
        "00000000-0000-4000-8000-000000000102",
    )
    .await;
    assert_eq!(
        inspect(&principal_client(admin_id), &base_url, archive)
            .await
            .status(),
        StatusCode::OK
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn inspection_enforces_personal_access_token_permission_subset(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let owner = authenticated_client();
    let denied_secret = create_pat(&owner, &base_url, &["contexts.read"]).await;
    let permitted_secret = create_pat(&owner, &base_url, &["solution_packs.manage"]).await;

    let denied = inspect(&bearer_client(&denied_secret), &base_url, valid_archive()).await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        denied.json::<Value>().await.unwrap()["error"]["code"],
        "forbidden"
    );
    assert_eq!(
        inspect(
            &bearer_client(&permitted_secret),
            &base_url,
            valid_archive()
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        create_plan(
            &bearer_client(&denied_secret),
            &base_url,
            valid_archive(),
            "pat"
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let plan = create_plan(
        &bearer_client(&permitted_secret),
        &base_url,
        valid_archive(),
        "pat",
    )
    .await;
    assert_eq!(plan.status(), StatusCode::CREATED);
    let plan = plan.json::<Value>().await.unwrap();
    let plan_id = plan["id"].as_str().unwrap();
    assert_eq!(
        apply_plan(&bearer_client(&denied_secret), &base_url, plan_id)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        apply_plan(&bearer_client(&permitted_secret), &base_url, plan_id)
            .await
            .status(),
        StatusCode::OK
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn inspection_requires_browser_session_csrf(pool: PgPool) {
    let (base_url, server) = start_session_server(pool.clone()).await;
    let hash = hash_password(&Password::new("correct horse battery staple")).unwrap();
    sqlx::query("INSERT INTO local_password_credentials (user_id, password_hash) VALUES ($1, $2)")
        .bind(BOOTSTRAP_OWNER_ID.parse::<Uuid>().unwrap())
        .bind(hash.as_phc())
        .execute(&pool)
        .await
        .unwrap();
    let client = Client::new();
    let login = client
        .post(format!("{base_url}/auth/login"))
        .json(&json!({
            "login_identifier": "default.local",
            "email": "api-test-owner@example.test",
            "password": "correct horse battery staple"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(login.status(), StatusCode::OK);
    let (session, csrf) = session_cookies(&login);
    let cookie = format!("{session}; {csrf}");

    let missing_csrf = client
        .post(format!("{base_url}/solution-packs/inspect"))
        .header("content-type", "application/zstd")
        .header("cookie", &cookie)
        .body(valid_archive())
        .send()
        .await
        .unwrap();
    assert_eq!(missing_csrf.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        missing_csrf.json::<Value>().await.unwrap()["error"]["code"],
        "csrf_failed"
    );

    let csrf_value = csrf.split_once('=').unwrap().1;
    let valid_csrf = client
        .post(format!("{base_url}/solution-packs/inspect"))
        .header("content-type", "application/zstd")
        .header("cookie", &cookie)
        .header("x-catalog-csrf", csrf_value)
        .body(valid_archive())
        .send()
        .await
        .unwrap();
    assert_eq!(valid_csrf.status(), StatusCode::OK);

    let missing_plan_csrf = client
        .post(format!(
            "{base_url}/solution-packs/plans?prefix=csrf&blueprint_publication=draft"
        ))
        .header("content-type", "application/zstd")
        .header("cookie", &cookie)
        .body(valid_archive())
        .send()
        .await
        .unwrap();
    assert_eq!(missing_plan_csrf.status(), StatusCode::FORBIDDEN);
    let valid_plan_csrf = client
        .post(format!(
            "{base_url}/solution-packs/plans?prefix=csrf&blueprint_publication=draft"
        ))
        .header("content-type", "application/zstd")
        .header("cookie", &cookie)
        .header("x-catalog-csrf", csrf_value)
        .body(valid_archive())
        .send()
        .await
        .unwrap();
    assert_eq!(valid_plan_csrf.status(), StatusCode::CREATED);
    let plan = valid_plan_csrf.json::<Value>().await.unwrap();
    let apply_url = format!(
        "{base_url}/solution-packs/plans/{}/apply",
        plan["id"].as_str().unwrap()
    );
    let missing_apply_csrf = client
        .post(&apply_url)
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(missing_apply_csrf.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        missing_apply_csrf.json::<Value>().await.unwrap()["error"]["code"],
        "csrf_failed"
    );
    let valid_apply_csrf = client
        .post(apply_url)
        .header("cookie", &cookie)
        .header("x-catalog-csrf", csrf_value)
        .send()
        .await
        .unwrap();
    assert_eq!(valid_apply_csrf.status(), StatusCode::OK);
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn inspection_rejects_missing_or_wrong_media_type_and_invalid_archives(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();

    for content_type in [None, Some("application/octet-stream")] {
        let mut request = client
            .post(format!("{base_url}/solution-packs/inspect"))
            .body(valid_archive());
        if let Some(content_type) = content_type {
            request = request.header("content-type", content_type);
        }
        let response = request.send().await.unwrap();
        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(
            response.json::<Value>().await.unwrap()["error"]["code"],
            "unsupported_media_type"
        );
    }

    let case_variant = client
        .post(format!("{base_url}/solution-packs/inspect"))
        .header("content-type", "Application/Zstd")
        .body(valid_archive())
        .send()
        .await
        .unwrap();
    assert_eq!(case_variant.status(), StatusCode::OK);

    let invalid = inspect(&client, &base_url, b"not a zstd archive".to_vec()).await;
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let error = invalid.json::<Value>().await.unwrap();
    assert_eq!(error["error"]["code"], "invalid_input");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("solution-pack archive")
    );

    let invalid_blueprint = br#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"
leaked = "SENSITIVE_MARKER"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"
"#;
    let invalid = inspect(
        &client,
        &base_url,
        archive_with_blueprint(invalid_blueprint),
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = invalid.text().await.unwrap();
    assert!(!body.contains("SENSITIVE_MARKER"));
    let error: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        error["error"]["message"],
        "blueprint 'blueprints/product' is invalid"
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn valid_inspection_returns_only_safe_summaries_without_persisting(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let archive = valid_archive();
    let archive_sha256 = digest(&archive);
    let blueprints_before: i64 = sqlx::query_scalar("SELECT count(*) FROM blueprints")
        .fetch_one(&pool)
        .await
        .unwrap();
    let contexts_before: i64 = sqlx::query_scalar("SELECT count(*) FROM attribute_contexts")
        .fetch_one(&pool)
        .await
        .unwrap();

    let response = inspect(&authenticated_client(), &base_url, archive).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body_text = response.text().await.unwrap();
    assert!(body_text.len() <= MAX_SOLUTION_PACK_INSPECTION_RESPONSE_BYTES);
    assert!(
        body_text.len() <= 1024 * 1024,
        "response must fit the CLI limit"
    );
    assert!(!body_text.contains("BLUEPRINT_SOURCE_SECRET"));
    assert!(!body_text.contains("CONTEXT_DATA_SECRET"));
    let body: Value = serde_json::from_str(&body_text).unwrap();
    assert_eq!(
        body.as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from(["archive_sha256", "manifest", "resources"])
    );
    assert_eq!(body["archive_sha256"], archive_sha256);
    assert_eq!(body["manifest"]["id"], "attricat.ecommerce");
    assert_eq!(body["manifest"]["version"], "1.2.0");
    assert_eq!(
        body["resources"]["blueprints"][0]["key"],
        "blueprints/product"
    );
    assert_eq!(body["resources"]["blueprints"][0]["code"], "product");
    assert_eq!(body["resources"]["blueprints"][0]["includes"], json!([]));
    assert_eq!(body["resources"]["contexts"][0]["key"], "contexts/web");
    assert_eq!(body["resources"]["contexts"][0]["code"], "web");
    assert_eq!(body["resources"]["contexts"][0]["parent"], "system/default");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM blueprints")
            .fetch_one(&pool)
            .await
            .unwrap(),
        blueprints_before
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attribute_contexts")
            .fetch_one(&pool)
            .await
            .unwrap(),
        contexts_before
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn inspection_enforces_the_32_mib_compressed_body_limit(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let oversized = vec![0_u8; 32 * 1024 * 1024 + 1];
    let response = inspect(&authenticated_client(), &base_url, oversized).await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        response.json::<Value>().await.unwrap(),
        json!({
            "error": {
                "code": "payload_too_large",
                "message": "request body exceeds the configured size limit"
            }
        })
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn plan_creation_persists_an_audited_immutable_dry_run_without_catalog_mutation(
    pool: PgPool,
) {
    let (base_url, server) = start_server(pool.clone()).await;
    let archive = valid_archive();
    let archive_sha256 = digest(&archive);
    let blueprint_count: i64 = sqlx::query_scalar("SELECT count(*) FROM blueprints")
        .fetch_one(&pool)
        .await
        .unwrap();
    let context_count: i64 = sqlx::query_scalar("SELECT count(*) FROM attribute_contexts")
        .fetch_one(&pool)
        .await
        .unwrap();

    let response = create_plan(&authenticated_client(), &base_url, archive, "ecom").await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let text = response.text().await.unwrap();
    assert!(!text.contains("BLUEPRINT_SOURCE_SECRET"));
    assert!(!text.contains("CONTEXT_DATA_SECRET"));
    let body: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(body["source_kind"], "local_archive");
    assert_eq!(body["source_metadata"]["side_loaded"], true);
    assert_eq!(body["archive_sha256"], archive_sha256);
    assert_eq!(body["prefix"], "ecom");
    assert_eq!(body["blueprint_publication"], "draft");
    assert_eq!(body["ready"], true);
    assert_eq!(body["conflicts"], json!([]));
    assert_eq!(body["actions"][0]["logical_key"], "blueprints/product");
    assert_eq!(body["actions"][0]["action"], "create");
    assert_eq!(body["actions"][1]["logical_key"], "contexts/web");
    assert_eq!(body["actions"][1]["action"], "skip");
    assert_eq!(body["mappings"][0]["logical_key"], "blueprints/product");
    assert_eq!(body["mappings"][0]["target_code"], "ecom_product");
    assert_eq!(body["mappings"][1]["logical_key"], "contexts/web");
    assert_eq!(body["mappings"][2]["logical_key"], "system/default");

    let plan_id = body["id"].as_str().unwrap();
    let fetched = authenticated_client()
        .get(format!("{base_url}/solution-packs/plans/{plan_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(fetched.status(), StatusCode::OK);
    assert_eq!(fetched.json::<Value>().await.unwrap(), body);

    let normalized: Value = sqlx::query_scalar("SELECT normalized_payload FROM solution_pack_plan_actions WHERE plan_id = $1 AND logical_key = 'blueprints/product'")
        .bind(plan_id.parse::<Uuid>().unwrap()).fetch_one(&pool).await.unwrap();
    assert!(
        normalized["definition"]
            .as_str()
            .unwrap()
            .contains("code = \"ecom_product\"")
    );
    let retained_archive_columns: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.columns WHERE table_schema = 'public' AND table_name LIKE 'solution_pack_plan%' AND data_type = 'bytea'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(retained_archive_columns, 0);
    let hours: rust_decimal::Decimal = sqlx::query_scalar("SELECT EXTRACT(EPOCH FROM (expires_at - created_at)) / 3600 FROM solution_pack_plans WHERE id = $1")
        .bind(plan_id.parse::<Uuid>().unwrap()).fetch_one(&pool).await.unwrap();
    assert_eq!(hours, rust_decimal::Decimal::from(24));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM blueprints")
            .fetch_one(&pool)
            .await
            .unwrap(),
        blueprint_count
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attribute_contexts")
            .fetch_one(&pool)
            .await
            .unwrap(),
        context_count
    );
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM audit_events WHERE action = 'catalog.solution_packs.plans.create_or_apply'").fetch_one(&pool).await.unwrap(), 1);
    let audit_target: Value = sqlx::query_scalar(
        "SELECT target FROM audit_events WHERE action = 'catalog.solution_packs.plans.create_or_apply'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        audit_target,
        json!({"type": "solution_pack_plan", "id": plan_id})
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn plan_validates_prefix_and_reports_existing_code_conflicts(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let invalid = create_plan(&client, &base_url, valid_archive(), "Bad-").await;
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        invalid.json::<Value>().await.unwrap()["error"]["code"],
        "invalid_input"
    );

    let definition = std::str::from_utf8(PRODUCT_BLUEPRINT)
        .unwrap()
        .replace("code = \"product\"", "code = \"ecom_product\"");
    let created = client
        .post(format!("{base_url}/blueprints"))
        .json(&json!({"definition": definition}))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);

    let conflict = create_plan(&client, &base_url, valid_archive(), "ecom").await;
    assert_eq!(conflict.status(), StatusCode::CREATED);
    let body = conflict.json::<Value>().await.unwrap();
    assert_eq!(body["ready"], false);
    assert_eq!(body["conflicts"][0]["logical_key"], "blueprints/product");
    assert_eq!(body["conflicts"][0]["action"], "conflict");
    assert_eq!(body["conflicts"][0]["reason_code"], "target_code_exists");

    let context = client
        .post(format!("{base_url}/contexts"))
        .json(&json!({"code": "cross_product", "data": {}}))
        .send()
        .await
        .unwrap();
    assert_eq!(context.status(), StatusCode::CREATED);
    let cross_kind = create_plan(&client, &base_url, valid_archive(), "cross").await;
    assert_eq!(cross_kind.status(), StatusCode::CREATED);
    let body = cross_kind.json::<Value>().await.unwrap();
    assert_eq!(body["ready"], false);
    assert_eq!(body["conflicts"][0]["logical_key"], "blueprints/product");
    assert_eq!(body["conflicts"][0]["reason_code"], "target_code_exists");
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn plan_routes_require_solution_pack_permission_and_isolate_workspaces(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let viewer_id = create_principal_with_role(
        &pool,
        "solution-pack-plan-viewer@example.test",
        "00000000-0000-4000-8000-000000000104",
    )
    .await;
    let denied = create_plan(
        &principal_client(viewer_id),
        &base_url,
        valid_archive(),
        "ecom",
    )
    .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let admin_id = create_principal_with_role(
        &pool,
        "solution-pack-plan-admin@example.test",
        "00000000-0000-4000-8000-000000000102",
    )
    .await;
    assert_eq!(
        create_plan(
            &principal_client(admin_id),
            &base_url,
            valid_archive(),
            "admin"
        )
        .await
        .status(),
        StatusCode::CREATED
    );

    let other_workspace = Uuid::new_v4();
    let hidden_plan = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO workspaces (id, slug, name, login_identifier) VALUES ($1, $2, 'Other', $3)",
    )
    .bind(other_workspace)
    .bind(format!("other-{other_workspace}"))
    .bind(format!("other-{other_workspace}.example.test"))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO solution_pack_plans (id, workspace_id, source_kind, source_metadata, archive_sha256, manifest_version, pack_id, pack_name, pack_version, pack_description, host_api, prefix, blueprint_publication, ready, expires_at) VALUES ($1, $2, 'local_archive', '{\"side_loaded\":true}'::jsonb, $3, 1, 'attricat.hidden', 'Hidden', '1.0.0', 'Hidden', '^1.0', 'hidden', 'draft', true, now() + interval '24 hours')")
        .bind(hidden_plan)
        .bind(other_workspace)
        .bind("0".repeat(64))
        .execute(&pool)
        .await
        .unwrap();
    let hidden = authenticated_client()
        .get(format!("{base_url}/solution-packs/plans/{hidden_plan}"))
        .send()
        .await
        .unwrap();
    assert_eq!(hidden.status(), StatusCode::NOT_FOUND);
    let hidden_apply =
        apply_plan(&authenticated_client(), &base_url, &hidden_plan.to_string()).await;
    assert_eq!(hidden_apply.status(), StatusCode::NOT_FOUND);

    let hidden_application = Uuid::new_v4();
    sqlx::query("INSERT INTO solution_pack_applications (id,workspace_id,plan_id,request_id,correlation_id,source_kind,source_metadata,archive_sha256,pack_id,pack_version,blueprint_publication,state,mapping_snapshot) VALUES ($1,$2,$3,$4,$4,'local_archive','{\"side_loaded\":true}'::jsonb,$5,'attricat.hidden','1.0.0','draft','running','[]'::jsonb)")
        .bind(hidden_application)
        .bind(other_workspace)
        .bind(hidden_plan)
        .bind(Uuid::new_v4())
        .bind("0".repeat(64))
        .execute(&pool)
        .await
        .unwrap();
    let hidden_show = authenticated_client()
        .get(format!(
            "{base_url}/solution-packs/applications/{hidden_application}"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(hidden_show.status(), StatusCode::NOT_FOUND);
    let visible_list = authenticated_client()
        .get(format!("{base_url}/solution-packs/applications"))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(visible_list, json!([]));
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn apply_creates_context_and_draft_blueprint_once_with_safe_history(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let plan = create_plan(
        &client,
        &base_url,
        archive_with_required_context(PRODUCT_BLUEPRINT),
        "starter",
    )
    .await;
    assert_eq!(plan.status(), StatusCode::CREATED);
    let plan = plan.json::<Value>().await.unwrap();
    assert_eq!(plan["ready"], true);
    let plan_id = plan["id"].as_str().unwrap();

    let applied = apply_plan(&client, &base_url, plan_id).await;
    assert_eq!(applied.status(), StatusCode::OK);
    let text = applied.text().await.unwrap();
    assert!(!text.contains("BLUEPRINT_SOURCE_SECRET"));
    assert!(!text.contains("CONTEXT_DATA_SECRET"));
    assert!(!text.contains("definition"));
    assert!(!text.contains("normalized_payload"));
    let application: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(application["state"], "completed");
    assert_eq!(application["mapping_snapshot"].as_array().unwrap().len(), 2);
    assert!(
        application["mapping_snapshot"]
            .as_array()
            .unwrap()
            .iter()
            .all(|mapping| mapping["logical_key"] != "system/default")
    );
    assert_eq!(application["steps"].as_array().unwrap().len(), 2);
    assert_eq!(application["steps"][0]["resource_kind"], "blueprint");
    assert_eq!(
        application["steps"][0]["result_snapshot"]["status"],
        "draft"
    );
    assert_eq!(application["steps"][1]["resource_kind"], "context");

    let repeated = apply_plan(&client, &base_url, plan_id).await;
    assert_eq!(repeated.status(), StatusCode::OK);
    assert_eq!(
        repeated.json::<Value>().await.unwrap()["id"],
        application["id"]
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM blueprints WHERE code='starter_product'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM attribute_contexts WHERE code='starter_web'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM solution_pack_applications WHERE plan_id=$1"
        )
        .bind(plan_id.parse::<Uuid>().unwrap())
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    let application_id = application["id"].as_str().unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM audit_events WHERE target->>'application_id'=$1"
        )
        .bind(application_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM audit_events WHERE target->>'id'=$1 AND target->>'type'='solution_pack_application'"
        )
        .bind(application_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );

    let listed = client
        .get(format!("{base_url}/solution-packs/applications"))
        .send()
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_text = listed.text().await.unwrap();
    assert!(!listed_text.contains("BLUEPRINT_SOURCE_SECRET"));
    assert!(!listed_text.contains("CONTEXT_DATA_SECRET"));
    assert!(!listed_text.contains("normalized_payload"));
    let listed: Value = serde_json::from_str(&listed_text).unwrap();
    assert_eq!(listed[0]["id"], application["id"]);
    assert!(listed[0].get("steps").is_none());
    assert!(listed[0].get("mapping_snapshot").is_none());
    let shown = client
        .get(format!(
            "{base_url}/solution-packs/applications/{}",
            application["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(shown.status(), StatusCode::OK);
    let shown_text = shown.text().await.unwrap();
    assert!(!shown_text.contains("BLUEPRINT_SOURCE_SECRET"));
    assert!(!shown_text.contains("CONTEXT_DATA_SECRET"));
    assert!(!shown_text.contains("normalized_payload"));
    assert_eq!(
        serde_json::from_str::<Value>(&shown_text).unwrap()["steps"],
        application["steps"]
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn application_read_contract_rejects_bad_inputs_and_keeps_max_pages_compact(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();

    for query in ["limit=0", "limit=101", "offset=-1", "offset=10001"] {
        let response = client
            .get(format!("{base_url}/solution-packs/applications?{query}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            response.json::<Value>().await.unwrap()["error"]["code"],
            "invalid_input"
        );
    }
    for path in [
        "/solution-packs/plans/not-a-uuid",
        "/solution-packs/applications/not-a-uuid",
    ] {
        let response = client
            .get(format!("{base_url}{path}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.json::<Value>().await.unwrap()["error"]["code"],
            "bad_request"
        );
    }

    let mappings = (0..192)
        .map(|index| {
            json!({
                "position": index,
                "resource_kind": if index < 64 { "blueprint" } else { "context" },
                "logical_key": format!("resources/{index:03}_{}", "x".repeat(96)),
                "target_id": Uuid::new_v4(),
                "target_code": format!("target_{index:03}_{}", "x".repeat(32)),
                "target_version": if index < 64 { Some(1) } else { None },
                "mapping_kind": "create"
            })
        })
        .collect::<Vec<_>>();
    let mapping_snapshot = serde_json::to_value(mappings).unwrap();
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    for index in 0..100 {
        let plan_id = Uuid::new_v4();
        sqlx::query("INSERT INTO solution_pack_plans (id,workspace_id,source_kind,source_metadata,archive_sha256,manifest_version,pack_id,pack_name,pack_version,pack_description,host_api,prefix,blueprint_publication,ready,expires_at) VALUES ($1,$2,'local_archive','{\"side_loaded\":true}'::jsonb,$3,1,$4,'Page fixture','1.0.0','Page fixture','^1.0',$5,'draft',true,clock_timestamp() + interval '24 hours')")
            .bind(plan_id)
            .bind(workspace_id)
            .bind(format!("{index:064x}"))
            .bind(format!("attricat.page.{index}"))
            .bind(format!("page_{index}"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO solution_pack_applications (id,workspace_id,plan_id,request_id,correlation_id,source_kind,source_metadata,archive_sha256,pack_id,pack_version,blueprint_publication,state,mapping_snapshot) VALUES ($1,$2,$3,$4,$4,'local_archive','{\"side_loaded\":true}'::jsonb,$5,$6,'1.0.0','draft','running',$7)")
            .bind(Uuid::new_v4())
            .bind(workspace_id)
            .bind(plan_id)
            .bind(Uuid::new_v4())
            .bind(format!("{index:064x}"))
            .bind(format!("attricat.page.{index}"))
            .bind(&mapping_snapshot)
            .execute(&pool)
            .await
            .unwrap();
    }

    let response = client
        .get(format!(
            "{base_url}/solution-packs/applications?limit=100&offset=0"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.bytes().await.unwrap();
    assert!(bytes.len() < 1024 * 1024);
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body.as_array().unwrap().len(), 100);
    assert!(body.as_array().unwrap().iter().all(|application| {
        application.get("mapping_snapshot").is_none()
            && application.get("steps").is_none()
            && application.get("normalized_payload").is_none()
    }));
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn apply_respects_publish_and_rejects_expired_blocked_and_stale_plans(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();

    let published_plan =
        create_plan_with_publication(&client, &base_url, valid_archive(), "published", "publish")
            .await
            .json::<Value>()
            .await
            .unwrap();
    let published_id = published_plan["id"].as_str().unwrap();
    let published = apply_plan(&client, &base_url, published_id).await;
    assert_eq!(published.status(), StatusCode::OK);
    let published = published.json::<Value>().await.unwrap();
    assert_eq!(
        published["steps"][0]["result_snapshot"]["status"],
        "published"
    );
    assert_eq!(published["mapping_snapshot"].as_array().unwrap().len(), 1);
    assert_eq!(
        published["mapping_snapshot"][0]["logical_key"],
        "blueprints/product"
    );

    let expired = create_plan(&client, &base_url, valid_archive(), "expired")
        .await
        .json::<Value>()
        .await
        .unwrap();
    let expired_id = expired["id"].as_str().unwrap().parse::<Uuid>().unwrap();
    sqlx::query("UPDATE solution_pack_plans SET expires_at=created_at + interval '1 millisecond' WHERE id=$1")
        .bind(expired_id).execute(&pool).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    let response = apply_plan(&client, &base_url, &expired_id.to_string()).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "solution_pack_plan_expired"
    );

    let blocked = create_plan(&client, &base_url, valid_archive(), "published")
        .await
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(blocked["ready"], false);
    let response = apply_plan(&client, &base_url, blocked["id"].as_str().unwrap()).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "solution_pack_plan_not_ready"
    );

    let stale = create_plan(&client, &base_url, valid_archive(), "stale")
        .await
        .json::<Value>()
        .await
        .unwrap();
    let created = client.post(format!("{base_url}/blueprints")).json(&json!({"definition": std::str::from_utf8(PRODUCT_BLUEPRINT).unwrap().replace("code = \"product\"", "code = \"stale_product\"")})).send().await.unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let response = apply_plan(&client, &base_url, stale["id"].as_str().unwrap()).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "solution_pack_plan_stale"
    );
    let invalid = apply_plan(&client, &base_url, stale["id"].as_str().unwrap()).await;
    assert_eq!(invalid.status(), StatusCode::CONFLICT);
    assert_eq!(
        invalid.json::<Value>().await.unwrap()["error"]["code"],
        "solution_pack_application_invalid"
    );

    let cross_blueprint = create_plan(&client, &base_url, valid_archive(), "crossblue")
        .await
        .json::<Value>()
        .await
        .unwrap();
    let created = client
        .post(format!("{base_url}/contexts"))
        .json(&json!({"code": "crossblue_product", "data": {}}))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let response = apply_plan(&client, &base_url, cross_blueprint["id"].as_str().unwrap()).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "solution_pack_plan_stale"
    );

    let cross_context = create_plan(&client, &base_url, context_only_archive(), "crossctx")
        .await
        .json::<Value>()
        .await
        .unwrap();
    let definition = std::str::from_utf8(PRODUCT_BLUEPRINT)
        .unwrap()
        .replace("code = \"product\"", "code = \"crossctx_web\"");
    let created = client
        .post(format!("{base_url}/blueprints"))
        .json(&json!({"definition": definition}))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let response = apply_plan(&client, &base_url, cross_context["id"].as_str().unwrap()).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "solution_pack_plan_stale"
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn target_created_while_a_step_is_executing_invalidates_the_application(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let plan = create_plan(
        &client,
        &base_url,
        archive_with_required_context(PRODUCT_BLUEPRINT),
        "racing",
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    let plan_id = plan["id"].as_str().unwrap().parse::<Uuid>().unwrap();
    let (target_id, target_code): (Uuid, String) = sqlx::query_as(
        "SELECT target_id,target_code FROM solution_pack_plan_mappings WHERE plan_id=$1 AND logical_key='contexts/web'",
    )
    .bind(plan_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let default_context: Uuid = sqlx::query_scalar(
        "SELECT id FROM attribute_contexts WHERE workspace_id=$1 AND code='default'",
    )
    .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();

    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE attribute_contexts IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let apply_client = client.clone();
    let apply_url = base_url.clone();
    let apply =
        tokio::spawn(
            async move { apply_plan(&apply_client, &apply_url, &plan_id.to_string()).await },
        );
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    sqlx::query("INSERT INTO attribute_contexts (id,workspace_id,code,data,parent_id) VALUES ($1,$2,$3,'{}'::jsonb,$4)")
        .bind(target_id)
        .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
        .bind(&target_code)
        .bind(default_context)
        .execute(&mut *blocker)
        .await
        .unwrap();
    blocker.commit().await.unwrap();

    let response = apply.await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "solution_pack_plan_stale"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM solution_pack_applications WHERE plan_id=$1"
        )
        .bind(plan_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "invalid"
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn concurrent_cross_kind_create_invalidates_the_application(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let plan = create_plan(&client, &base_url, context_only_archive(), "crossrace")
        .await
        .json::<Value>()
        .await
        .unwrap();
    let plan_id = plan["id"].as_str().unwrap().to_owned();
    let target_code = "crossrace_web";
    let definition = std::str::from_utf8(PRODUCT_BLUEPRINT)
        .unwrap()
        .replace("code = \"product\"", &format!("code = \"{target_code}\""));

    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE blueprints IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let create_client = client.clone();
    let create_url = base_url.clone();
    let create = tokio::spawn(async move {
        create_client
            .post(format!("{create_url}/blueprints"))
            .json(&json!({"definition": definition}))
            .send()
            .await
            .unwrap()
    });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let apply_client = client.clone();
    let apply_url = base_url.clone();
    let apply = tokio::spawn(async move { apply_plan(&apply_client, &apply_url, &plan_id).await });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    blocker.commit().await.unwrap();

    assert_eq!(create.await.unwrap().status(), StatusCode::CREATED);
    let response = apply.await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "solution_pack_plan_stale"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM attribute_contexts WHERE code='crossrace_web'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn concurrent_apply_requests_converge_without_duplicate_resources(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let plan = create_plan(&client, &base_url, valid_archive(), "parallel")
        .await
        .json::<Value>()
        .await
        .unwrap();
    let plan_id = plan["id"].as_str().unwrap().to_owned();
    let (first, second) = tokio::join!(
        apply_plan(&client, &base_url, &plan_id),
        apply_plan(&client, &base_url, &plan_id)
    );
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(second.status(), StatusCode::OK);
    let first = first.json::<Value>().await.unwrap();
    let second = second.json::<Value>().await.unwrap();
    assert_eq!(first["id"], second["id"]);
    assert_eq!(first["state"], "completed");
    assert_eq!(second["state"], "completed");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM blueprints WHERE code='parallel_product'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn failed_step_is_durable_and_retry_resumes_without_duplicate_completed_resources(
    pool: PgPool,
) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let plan = create_plan(
        &client,
        &base_url,
        archive_with_required_context(PRODUCT_BLUEPRINT),
        "resume",
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    let plan_id = plan["id"].as_str().unwrap().parse::<Uuid>().unwrap();
    let original_payload: Value = sqlx::query_scalar(
        "SELECT normalized_payload FROM solution_pack_plan_actions WHERE plan_id=$1 AND resource_kind='context'",
    )
    .bind(plan_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE solution_pack_plan_actions SET normalized_payload='{}'::jsonb WHERE plan_id=$1 AND resource_kind='context'")
        .bind(plan_id)
        .execute(&pool)
        .await
        .unwrap();

    let failed = apply_plan(&client, &base_url, &plan_id.to_string()).await;
    assert_eq!(failed.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM solution_pack_applications WHERE plan_id=$1"
        )
        .bind(plan_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "failed"
    );
    let failed_step: (String, Option<String>) = sqlx::query_as(
        "SELECT state, diagnostic_code FROM solution_pack_application_steps WHERE application_id=(SELECT id FROM solution_pack_applications WHERE plan_id=$1) AND logical_key='contexts/web'",
    )
    .bind(plan_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        failed_step,
        ("failed".to_owned(), Some("step_failed".to_owned()))
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM blueprints WHERE code='resume_product'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    sqlx::query("UPDATE solution_pack_plan_actions SET normalized_payload=$2 WHERE plan_id=$1 AND resource_kind='context'")
        .bind(plan_id)
        .bind(original_payload)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE solution_pack_plans SET created_at=clock_timestamp() - interval '25 hours', expires_at=clock_timestamp() - interval '1 hour' WHERE id=$1")
        .bind(plan_id)
        .execute(&pool)
        .await
        .unwrap();

    let application_id: Uuid =
        sqlx::query_scalar("SELECT id FROM solution_pack_applications WHERE plan_id=$1")
            .bind(plan_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let history_reads = async {
        let mut observations = Vec::new();
        for _ in 0..40 {
            let response = client
                .get(format!(
                    "{base_url}/solution-packs/applications/{application_id}"
                ))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            observations.push(response.json::<Value>().await.unwrap());
            tokio::task::yield_now().await;
        }
        observations
    };
    let plan_id_string = plan_id.to_string();
    let (resumed, observations) = tokio::join!(
        apply_plan(&client, &base_url, &plan_id_string),
        history_reads
    );
    for observation in observations {
        let step_states = observation["steps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|step| step["state"].as_str().unwrap())
            .collect::<Vec<_>>();
        match observation["state"].as_str().unwrap() {
            "failed" => assert!(step_states.contains(&"failed")),
            "running" => assert!(!step_states.contains(&"failed")),
            "completed" => assert!(step_states.iter().all(|state| *state == "completed")),
            state => panic!("unexpected application state {state}"),
        }
    }
    assert_eq!(resumed.status(), StatusCode::OK);
    let resumed = resumed.json::<Value>().await.unwrap();
    assert_eq!(resumed["state"], "completed");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM blueprints WHERE code='resume_product'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM attribute_contexts WHERE code='resume_web'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    server.abort();
}
