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
                "required": false,
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
    client
        .post(format!(
            "{base_url}/solution-packs/plans?prefix={prefix}&blueprint_publication=draft"
        ))
        .header("content-type", "application/zstd")
        .body(archive)
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
    assert_eq!(
        create_plan(
            &bearer_client(&permitted_secret),
            &base_url,
            valid_archive(),
            "pat"
        )
        .await
        .status(),
        StatusCode::CREATED
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
    server.abort();
}
