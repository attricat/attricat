mod support;

use std::{collections::BTreeSet, io::Cursor};

use api::{
    account::{Password, hash_password},
    repository::CatalogRepository,
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

fn archive_with_extension_requirement(required: bool) -> Vec<u8> {
    const TEMPLATE: &[u8] = br#"{"endpoint":"TEMPLATE_VALUE_SENTINEL"}"#;
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "id": "attricat.extensions",
        "name": "Extension requirements",
        "version": "1.0.0",
        "description": "Extension requirement test",
        "catalog": {"host_api": "^1.0"},
        "resources": {
            "contexts": [{
                "key": "contexts/web",
                "path": "contexts/web.json",
                "required": true,
                "sha256": digest(WEB_CONTEXT)
            }]
        },
        "extensions": [{
            "key": "extensions/shopify",
            "id": "acme.shopify",
            "version": ">=2.1.0 <3.0.0",
            "required": required,
            "configuration_template": {
                "path": "extensions/shopify.json",
                "sha256": digest(TEMPLATE)
            }
        }]
    }))
    .unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "solution-pack.json", &manifest);
        append_file(&mut tar, "contexts/web.json", WEB_CONTEXT);
        append_file(&mut tar, "extensions/shopify.json", TEMPLATE);
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn archive_with_blueprint_extension_layout() -> Vec<u8> {
    let blueprint = PRODUCT_BLUEPRINT
        .iter()
        .copied()
        .chain(
            br#"
[views.extension_layout]
type = "extension_layout"
version = 1
[views.extension_layout.outlets.entity_action]
order = ["acme.layout:action"]
hidden = []
"#
            .iter()
            .copied(),
        )
        .collect::<Vec<_>>();
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "id": "attricat.blueprint-layout",
        "name": "Blueprint extension layout",
        "version": "1.0.0",
        "description": "Entity blueprint contribution defaults",
        "catalog": {"host_api": "^1.0"},
        "resources": {
            "blueprints": [{
                "key": "blueprints/product",
                "path": "blueprints/product.toml",
                "required": true,
                "sha256": digest(&blueprint)
            }]
        },
        "extensions": [{
            "key": "extensions/layout",
            "id": "acme.layout",
            "version": "^1.0",
            "required": true
        }]
    }))
    .unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "solution-pack.json", &manifest);
        append_file(&mut tar, "blueprints/product.toml", &blueprint);
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn archive_with_extension_layout() -> Vec<u8> {
    let layout = serde_json::to_vec(&json!({
        "format_version": 1,
        "kind": "extension_layout",
        "entries": [{
            "contribution": "acme.layout:nav",
            "outlet": "navigation",
            "promoted": true,
            "required": true,
        }],
    }))
    .unwrap();
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "id": "attricat.extension-layout",
        "name": "Extension layout",
        "version": "1.0.0",
        "description": "Extension contribution defaults",
        "catalog": {"host_api": "^1.0"},
        "resources": {
            "workspace_settings": [{
                "key": "workspace/extension-layout",
                "path": "workspace/extension-layout.json",
                "required": true,
                "sha256": digest(&layout)
            }]
        },
        "extensions": [{
            "key": "extensions/layout",
            "id": "acme.layout",
            "version": "^1.0",
            "required": true
        }]
    }))
    .unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "solution-pack.json", &manifest);
        append_file(&mut tar, "workspace/extension-layout.json", &layout);
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn archive_with_both_workspace_settings(extension_layout_first: bool) -> Vec<u8> {
    let navigation = serde_json::to_vec(&json!({
        "format_version": 1,
        "kind": "explore_navigation",
        "entries": [{"blueprint": "blueprints/product"}],
    }))
    .unwrap();
    let layout = serde_json::to_vec(&json!({
        "format_version": 1,
        "kind": "extension_layout",
        "entries": [{
            "contribution": "acme.layout:nav",
            "outlet": "navigation",
            "required": true,
        }],
    }))
    .unwrap();
    let navigation_resource = json!({
        "key": "workspace/explore-navigation",
        "path": "workspace/explore-navigation.json",
        "required": true,
        "sha256": digest(&navigation),
    });
    let layout_resource = json!({
        "key": "workspace/extension-layout",
        "path": "workspace/extension-layout.json",
        "required": true,
        "sha256": digest(&layout),
    });
    let workspace_settings = if extension_layout_first {
        vec![layout_resource, navigation_resource]
    } else {
        vec![navigation_resource, layout_resource]
    };
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "id": "attricat.both-settings",
        "name": "Both settings",
        "version": "1.0.0",
        "description": "Both bounded workspace settings",
        "catalog": {"host_api": "^1.0"},
        "resources": {
            "blueprints": [{
                "key": "blueprints/product",
                "path": "blueprints/product.toml",
                "required": true,
                "sha256": digest(PRODUCT_BLUEPRINT)
            }],
            "workspace_settings": workspace_settings
        },
        "extensions": [{
            "key": "extensions/layout",
            "id": "acme.layout",
            "version": "^1.0",
            "required": true
        }]
    }))
    .unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "solution-pack.json", &manifest);
        append_file(&mut tar, "blueprints/product.toml", PRODUCT_BLUEPRINT);
        append_file(&mut tar, "workspace/explore-navigation.json", &navigation);
        append_file(&mut tar, "workspace/extension-layout.json", &layout);
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn archive_with_explore_navigation() -> Vec<u8> {
    archive_with_explore_navigation_roles(&[])
}

fn archive_with_explore_navigation_roles(role_codes: &[&str]) -> Vec<u8> {
    let navigation = serde_json::to_vec(&json!({
        "format_version": 1,
        "kind": "explore_navigation",
        "entries": [{
            "blueprint": "blueprints/product",
            "visible_to_role_codes": role_codes,
        }],
    }))
    .unwrap();
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "id": "attricat.navigation",
        "name": "Navigation",
        "version": "1.0.0",
        "description": "Explore navigation defaults",
        "catalog": {"host_api": "^1.0"},
        "resources": {
            "blueprints": [{
                "key": "blueprints/product",
                "path": "blueprints/product.toml",
                "required": true,
                "sha256": digest(PRODUCT_BLUEPRINT)
            }],
            "workspace_settings": [{
                "key": "workspace/explore-navigation",
                "path": "workspace/explore-navigation.json",
                "required": true,
                "sha256": digest(&navigation)
            }]
        }
    }))
    .unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "solution-pack.json", &manifest);
        append_file(&mut tar, "blueprints/product.toml", PRODUCT_BLUEPRINT);
        append_file(&mut tar, "workspace/explore-navigation.json", &navigation);
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

async fn install_layout_extension(pool: &PgPool) -> Uuid {
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let release_id = Uuid::new_v4();
    let extension_manifest = json!({
        "manifest_version": 1,
        "name": "Layout extension",
        "version": "1.0.0",
        "description": "Layout extension test",
        "icons": {"48": "icon.png"},
        "catalog": {"id": "acme.layout", "host_api": "^1.0"},
        "permissions": [],
        "artifacts": [{"id":"client","kind":"client_component","path":"client.js"}],
        "ui": [
            {
                "id": "nav",
                "version": 1,
                "kind": "embedded",
                "artifact": "client",
                "outlet": "navigation"
            },
            {
                "id": "action",
                "version": 1,
                "kind": "action",
                "artifact": "client",
                "outlet": "entity_action"
            }
        ]
    });
    sqlx::query("INSERT INTO installed_extension_releases (id,workspace_id,extension_id,version,manifest,manifest_sha256,source) VALUES ($1,$2,'acme.layout','1.0.0',$3,$4,'side_load')")
        .bind(release_id)
        .bind(workspace_id)
        .bind(extension_manifest)
        .bind("2".repeat(64))
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO extension_installations (id,workspace_id,extension_id,installed_release_id,state,configuration) VALUES ($1,$2,'acme.layout',$3,'disabled','{}'::jsonb)")
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(release_id)
        .execute(pool)
        .await
        .unwrap();
    release_id
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
        std::collections::BTreeSet::from(["archive_sha256", "extensions", "manifest", "resources"])
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
async fn extension_requirements_are_safe_visible_plan_evidence(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let archive = archive_with_extension_requirement(true);

    let inspection = inspect(&client, &base_url, archive.clone()).await;
    assert_eq!(inspection.status(), StatusCode::OK);
    let inspection_text = inspection.text().await.unwrap();
    assert!(!inspection_text.contains("TEMPLATE_VALUE_SENTINEL"));
    let inspection: Value = serde_json::from_str(&inspection_text).unwrap();
    assert_eq!(inspection["extensions"][0]["id"], "acme.shopify");
    assert_eq!(
        inspection["extensions"][0]["configuration_template"]["path"],
        "extensions/shopify.json"
    );

    let response = create_plan(&client, &base_url, archive, "ext").await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let plan_text = response.text().await.unwrap();
    assert!(!plan_text.contains("TEMPLATE_VALUE_SENTINEL"));
    let plan: Value = serde_json::from_str(&plan_text).unwrap();
    assert_eq!(plan["ready"], false);
    assert_eq!(plan["extension_requirements"][0]["status"], "blocked");
    assert_eq!(plan["extension_requirements"][0]["reason_code"], "missing");
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT evaluation_template->>'endpoint' FROM solution_pack_plan_extension_requirements WHERE plan_id=$1"
        )
        .bind(Uuid::parse_str(plan["id"].as_str().unwrap()).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap(),
        "TEMPLATE_VALUE_SENTINEL"
    );

    let optional = create_plan(
        &client,
        &base_url,
        archive_with_extension_requirement(false),
        "extoptional",
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    assert_eq!(optional["ready"], true);
    assert_eq!(optional["extension_requirements"][0]["status"], "skipped");
    assert_eq!(
        optional["extension_requirements"][0]["reason_code"],
        "missing"
    );

    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn compatible_disabled_extension_satisfies_plan_and_is_revalidated_on_apply(pool: PgPool) {
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let release_id = Uuid::new_v4();
    sqlx::query("INSERT INTO installed_extension_releases (id,workspace_id,extension_id,version,manifest,manifest_sha256,source) VALUES ($1,$2,'acme.shopify','2.2.0','{}'::jsonb,$3,'side_load')")
        .bind(release_id)
        .bind(workspace_id)
        .bind("1".repeat(64))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO extension_installations (id,workspace_id,extension_id,installed_release_id,state,configuration) VALUES ($1,$2,'acme.shopify',$3,'disabled',$4)")
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(release_id)
        .bind(json!({"endpoint":"TEMPLATE_VALUE_SENTINEL","unrelated_secret":"INSTALLED_SECRET_SENTINEL"}))
        .execute(&pool)
        .await
        .unwrap();

    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let plan_response = create_plan(
        &client,
        &base_url,
        archive_with_extension_requirement(true),
        "extok",
    )
    .await;
    assert_eq!(plan_response.status(), StatusCode::CREATED);
    let plan_text = plan_response.text().await.unwrap();
    assert!(!plan_text.contains("TEMPLATE_VALUE_SENTINEL"));
    assert!(!plan_text.contains("INSTALLED_SECRET_SENTINEL"));
    let plan: Value = serde_json::from_str(&plan_text).unwrap();
    assert_eq!(plan["ready"], true);
    assert_eq!(plan["extension_requirements"][0]["status"], "satisfied");
    assert_eq!(
        plan["extension_requirements"][0]["installed_state"],
        "disabled"
    );

    let plan_id = Uuid::parse_str(plan["id"].as_str().unwrap()).unwrap();
    let interrupted_application_id = Uuid::new_v4();
    let request_id = Uuid::new_v4();
    sqlx::query("INSERT INTO solution_pack_applications (id,workspace_id,plan_id,request_id,correlation_id,source_kind,source_metadata,archive_sha256,pack_id,pack_version,blueprint_publication,state,diagnostic_code,diagnostic_message,mapping_snapshot) VALUES ($1,$2,$3,$4,$4,'local_archive','{\"side_loaded\":true}'::jsonb,$5,'attricat.extensions','1.0.0','draft','failed','step_failed','synthetic interrupted application','[]'::jsonb)")
        .bind(interrupted_application_id)
        .bind(workspace_id)
        .bind(plan_id)
        .bind(request_id)
        .bind("3".repeat(64))
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("UPDATE extension_installations SET configuration=$2 WHERE workspace_id=$1 AND extension_id='acme.shopify'")
        .bind(workspace_id)
        .bind(json!({"endpoint":"changed"}))
        .execute(&pool)
        .await
        .unwrap();
    let response = apply_plan(&client, &base_url, plan["id"].as_str().unwrap()).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "solution_pack_plan_stale"
    );
    let (application_state, diagnostic_code): (String, Option<String>) =
        sqlx::query_as("SELECT state,diagnostic_code FROM solution_pack_applications WHERE id=$1")
            .bind(interrupted_application_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(application_state, "invalid");
    assert_eq!(diagnostic_code.as_deref(), Some("plan_stale"));
    let repeated = apply_plan(&client, &base_url, plan["id"].as_str().unwrap()).await;
    assert_eq!(repeated.status(), StatusCode::CONFLICT);
    assert_eq!(
        repeated.json::<Value>().await.unwrap()["error"]["code"],
        "solution_pack_application_invalid"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM attribute_contexts WHERE workspace_id=$1 AND code='extok_web'"
        )
        .bind(workspace_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );

    let mismatch = create_plan(
        &client,
        &base_url,
        archive_with_extension_requirement(true),
        "extmismatch",
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    assert_eq!(mismatch["ready"], false);
    assert_eq!(
        mismatch["extension_requirements"][0]["reason_code"],
        "configuration_mismatch"
    );

    let incompatible_release_id = Uuid::new_v4();
    sqlx::query("INSERT INTO installed_extension_releases (id,workspace_id,extension_id,version,manifest,manifest_sha256,source) VALUES ($1,$2,'acme.shopify','3.0.0','{}'::jsonb,$3,'side_load')")
        .bind(incompatible_release_id)
        .bind(workspace_id)
        .bind("2".repeat(64))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE extension_installations SET installed_release_id=$2, configuration=$3 WHERE workspace_id=$1 AND extension_id='acme.shopify'")
        .bind(workspace_id)
        .bind(incompatible_release_id)
        .bind(json!({"endpoint":"TEMPLATE_VALUE_SENTINEL"}))
        .execute(&pool)
        .await
        .unwrap();
    for (required, expected_status, expected_ready, prefix) in [
        (true, "blocked", false, "extversionrequired"),
        (false, "skipped", true, "extversionoptional"),
    ] {
        let plan = create_plan(
            &client,
            &base_url,
            archive_with_extension_requirement(required),
            prefix,
        )
        .await
        .json::<Value>()
        .await
        .unwrap();
        assert_eq!(plan["ready"], expected_ready);
        assert_eq!(plan["extension_requirements"][0]["status"], expected_status);
        assert_eq!(
            plan["extension_requirements"][0]["reason_code"],
            "incompatible_version"
        );
    }

    sqlx::query("UPDATE extension_installations SET installed_release_id=$2, state='quarantined' WHERE workspace_id=$1 AND extension_id='acme.shopify'")
        .bind(workspace_id)
        .bind(release_id)
        .execute(&pool)
        .await
        .unwrap();
    for (required, expected_status, expected_ready, prefix) in [
        (true, "blocked", false, "extquarantinedrequired"),
        (false, "skipped", true, "extquarantinedoptional"),
    ] {
        let plan = create_plan(
            &client,
            &base_url,
            archive_with_extension_requirement(required),
            prefix,
        )
        .await
        .json::<Value>()
        .await
        .unwrap();
        assert_eq!(plan["ready"], expected_ready);
        assert_eq!(plan["extension_requirements"][0]["status"], expected_status);
        assert_eq!(
            plan["extension_requirements"][0]["reason_code"],
            "quarantined"
        );
    }
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
async fn apply_merges_extension_layout_without_enabling_or_replacing_settings(pool: PgPool) {
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    install_layout_extension(&pool).await;
    sqlx::query("UPDATE workspaces SET settings=$2 WHERE id=$1")
        .bind(workspace_id)
        .bind(json!({
            "theme":"dark",
            "extension_layout": {
                "version":1,
                "outlets": {
                    "entity_action": {
                        "order":["other.extension:action"],
                        "hidden":[]
                    }
                }
            }
        }))
        .execute(&pool)
        .await
        .unwrap();

    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let inspected = inspect(&client, &base_url, archive_with_extension_layout()).await;
    assert_eq!(inspected.status(), StatusCode::OK);
    assert_eq!(
        inspected.json::<Value>().await.unwrap()["resources"]["workspace_settings"][0]["kind"],
        "extension_layout"
    );
    let plan = create_plan(
        &client,
        &base_url,
        archive_with_extension_layout(),
        "layout",
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    assert_eq!(plan["ready"], true);
    let action = plan["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["logical_key"] == "workspace/extension-layout")
        .unwrap();
    assert_eq!(action["action"], "append");

    let plan_id = Uuid::parse_str(plan["id"].as_str().unwrap()).unwrap();
    let application_id = Uuid::new_v4();
    let request_id = Uuid::new_v4();
    sqlx::query("INSERT INTO solution_pack_applications (id,workspace_id,plan_id,request_id,correlation_id,source_kind,source_metadata,archive_sha256,pack_id,pack_version,blueprint_publication,state,diagnostic_code,diagnostic_message,mapping_snapshot) VALUES ($1,$2,$3,$4,$4,'local_archive','{\"side_loaded\":true}'::jsonb,$5,'attricat.extension-layout','1.0.0','draft','failed','step_failed','synthetic layout failure','[]'::jsonb)")
        .bind(application_id)
        .bind(workspace_id)
        .bind(plan_id)
        .bind(request_id)
        .bind("5".repeat(64))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO solution_pack_application_steps (application_id,workspace_id,position,plan_id,resource_kind,logical_key,target_id,target_code,target_version,state,diagnostic_code,diagnostic_message) SELECT $1,a.workspace_id,a.position,a.plan_id,a.resource_kind,a.logical_key,m.target_id,m.target_code,m.target_version,'failed','step_failed','synthetic layout failure' FROM solution_pack_plan_actions a JOIN solution_pack_plan_mappings m ON m.plan_id=a.plan_id AND m.logical_key=a.logical_key WHERE a.plan_id=$2 AND a.logical_key='workspace/extension-layout'")
        .bind(application_id)
        .bind(plan_id)
        .execute(&pool)
        .await
        .unwrap();

    let applied = apply_plan(&client, &base_url, plan["id"].as_str().unwrap()).await;
    assert_eq!(applied.status(), StatusCode::OK);
    let application = applied.json::<Value>().await.unwrap();
    assert_eq!(
        application["steps"][0]["result_snapshot"]["outcome"],
        "appended"
    );
    let settings: Value = sqlx::query_scalar("SELECT settings FROM workspaces WHERE id=$1")
        .bind(workspace_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(settings["theme"], "dark");
    assert_eq!(
        settings["extension_layout"]["outlets"]["entity_action"]["order"][0],
        "other.extension:action"
    );
    assert_eq!(
        settings["extension_layout"]["outlets"]["navigation"]["order"][0],
        "acme.layout:nav"
    );
    assert_eq!(
        settings["extension_layout"]["outlets"]["navigation"]["promoted"][0],
        "acme.layout:nav"
    );
    let state: String = sqlx::query_scalar(
        "SELECT state FROM extension_installations WHERE workspace_id=$1 AND extension_id='acme.layout'",
    )
    .bind(workspace_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(state, "disabled");

    let repeated = apply_plan(&client, &base_url, plan["id"].as_str().unwrap()).await;
    assert_eq!(repeated.status(), StatusCode::OK);
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn blueprint_extension_layout_applies_for_draft_and_publish_with_disabled_extension(
    pool: PgPool,
) {
    install_layout_extension(&pool).await;
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();

    for (prefix, publication, expected_status) in [
        ("layoutdraft", "draft", "draft"),
        ("layoutpublish", "publish", "published"),
    ] {
        let plan = create_plan_with_publication(
            &client,
            &base_url,
            archive_with_blueprint_extension_layout(),
            prefix,
            publication,
        )
        .await
        .json::<Value>()
        .await
        .unwrap();
        assert_eq!(plan["ready"], true);
        let applied = apply_plan(&client, &base_url, plan["id"].as_str().unwrap()).await;
        assert_eq!(applied.status(), StatusCode::OK);
        let (status, definition): (String, String) = sqlx::query_as(
            "SELECT status,definition FROM blueprints WHERE workspace_id=$1 AND code=$2",
        )
        .bind(workspace_id)
        .bind(format!("{prefix}_product"))
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(status, expected_status);
        assert!(definition.contains("acme.layout:action"));
    }
    let installation_state: String = sqlx::query_scalar(
        "SELECT state FROM extension_installations WHERE workspace_id=$1 AND extension_id='acme.layout'",
    )
    .bind(workspace_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(installation_state, "disabled");
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn layout_apply_rejects_compatible_release_change_after_planning(pool: PgPool) {
    let prior_release_id = install_layout_extension(&pool).await;
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let plan = create_plan(
        &client,
        &base_url,
        archive_with_extension_layout(),
        "layoutstale",
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    assert_eq!(plan["ready"], true);

    let replacement_release_id = Uuid::new_v4();
    sqlx::query("INSERT INTO installed_extension_releases (id,workspace_id,extension_id,version,manifest,manifest_sha256,source) SELECT $1,workspace_id,extension_id,'1.1.0',jsonb_set(manifest,'{version}','\"1.1.0\"'::jsonb),$2,source FROM installed_extension_releases WHERE id=$3")
        .bind(replacement_release_id)
        .bind("4".repeat(64))
        .bind(prior_release_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE extension_installations SET installed_release_id=$2 WHERE workspace_id=$1 AND extension_id='acme.layout'")
        .bind(workspace_id)
        .bind(replacement_release_id)
        .execute(&pool)
        .await
        .unwrap();

    let applied = apply_plan(&client, &base_url, plan["id"].as_str().unwrap()).await;
    assert_eq!(applied.status(), StatusCode::CONFLICT);
    assert_eq!(
        applied.json::<Value>().await.unwrap()["error"]["code"],
        "solution_pack_plan_stale"
    );
    let layout: Option<Value> =
        sqlx::query_scalar("SELECT settings->'extension_layout' FROM workspaces WHERE id=$1")
            .bind(workspace_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(layout.is_none());
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn layout_apply_rejects_malformed_current_layout_and_preserves_it(pool: PgPool) {
    install_layout_extension(&pool).await;
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let plan = create_plan(
        &client,
        &base_url,
        archive_with_extension_layout(),
        "layoutmalformedapply",
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    assert_eq!(plan["ready"], true);
    let plan_id = Uuid::parse_str(plan["id"].as_str().unwrap()).unwrap();

    let malformed = json!({
        "theme": "dark",
        "extension_layout": {
            "version": 1,
            "outlets": {
                "navigation": {
                    "order": [],
                    "hidden": [],
                    "promoted": ["acme.layout:nav"]
                }
            }
        }
    });
    sqlx::query("UPDATE workspaces SET settings=$2 WHERE id=$1")
        .bind(workspace_id)
        .bind(&malformed)
        .execute(&pool)
        .await
        .unwrap();

    let applied = apply_plan(&client, &base_url, plan["id"].as_str().unwrap()).await;
    assert_eq!(applied.status(), StatusCode::CONFLICT);
    assert_eq!(
        applied.json::<Value>().await.unwrap()["error"]["code"],
        "solution_pack_plan_stale"
    );
    let (state, diagnostic_code): (String, Option<String>) = sqlx::query_as(
        "SELECT state,diagnostic_code FROM solution_pack_applications WHERE plan_id=$1",
    )
    .bind(plan_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(state, "invalid");
    assert_eq!(diagnostic_code.as_deref(), Some("plan_stale"));
    let preserved: Value = sqlx::query_scalar("SELECT settings FROM workspaces WHERE id=$1")
        .bind(workspace_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(preserved, malformed);

    let repeated = apply_plan(&client, &base_url, plan["id"].as_str().unwrap()).await;
    assert_eq!(repeated.status(), StatusCode::CONFLICT);
    assert_eq!(
        repeated.json::<Value>().await.unwrap()["error"]["code"],
        "solution_pack_application_invalid"
    );
    let application_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM solution_pack_applications WHERE plan_id=$1")
            .bind(plan_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(application_count, 1);
    let still_preserved: Value = sqlx::query_scalar("SELECT settings FROM workspaces WHERE id=$1")
        .bind(workspace_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(still_preserved, malformed);
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn both_workspace_settings_plan_and_apply_in_either_manifest_order(pool: PgPool) {
    install_layout_extension(&pool).await;
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();

    for (index, extension_layout_first) in [false, true].into_iter().enumerate() {
        let plan_response = create_plan_with_publication(
            &client,
            &base_url,
            archive_with_both_workspace_settings(extension_layout_first),
            &format!("both{index}"),
            "publish",
        )
        .await;
        assert_eq!(plan_response.status(), StatusCode::CREATED);
        let plan = plan_response.json::<Value>().await.unwrap();
        assert_eq!(plan["ready"], true);
        let setting_keys = plan["actions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|action| action["resource_kind"] == "workspace_setting")
            .map(|action| action["logical_key"].as_str().unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            setting_keys,
            BTreeSet::from(["workspace/explore-navigation", "workspace/extension-layout",])
        );
        let applied = apply_plan(&client, &base_url, plan["id"].as_str().unwrap()).await;
        assert_eq!(applied.status(), StatusCode::OK);
    }

    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let settings: Value = sqlx::query_scalar("SELECT settings FROM workspaces WHERE id=$1")
        .bind(workspace_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(settings["explore_navigation"].as_array().unwrap().len(), 2);
    assert_eq!(
        settings["extension_layout"]["outlets"]["navigation"]["order"],
        json!(["acme.layout:nav"])
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn malformed_workspace_settings_root_fails_closed_before_layout_application(pool: PgPool) {
    install_layout_extension(&pool).await;
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();

    for (index, malformed) in [
        json!("scalar"),
        json!([]),
        json!({
            "extension_layout": {
                "version": 1,
                "outlets": {"navigation": {"order": []}}
            }
        }),
    ]
    .into_iter()
    .enumerate()
    {
        sqlx::query("UPDATE workspaces SET settings=$2 WHERE id=$1")
            .bind(workspace_id)
            .bind(malformed)
            .execute(&pool)
            .await
            .unwrap();
        let response = create_plan(
            &client,
            &base_url,
            archive_with_extension_layout(),
            &format!("malformed{index}"),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let plan = response.json::<Value>().await.unwrap();
        assert_eq!(plan["ready"], false);
        assert_eq!(
            plan["actions"]
                .as_array()
                .unwrap()
                .iter()
                .find(|action| action["logical_key"] == "workspace/extension-layout")
                .unwrap()["reason_code"],
            "invalid_current_extension_layout"
        );
        let apply = apply_plan(&client, &base_url, plan["id"].as_str().unwrap()).await;
        assert_eq!(apply.status(), StatusCode::CONFLICT);
    }
    let applications: i64 =
        sqlx::query_scalar("SELECT count(*) FROM solution_pack_applications WHERE workspace_id=$1")
            .bind(workspace_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(applications, 0);
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn apply_appends_explore_navigation_and_preserves_unrelated_settings(pool: PgPool) {
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    sqlx::query("UPDATE workspaces SET settings = '{\"theme\":\"dark\",\"explore_navigation\":[]}'::jsonb WHERE id=$1")
        .bind(workspace_id)
        .execute(&pool)
        .await
        .unwrap();
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();

    let inspected = inspect(&client, &base_url, archive_with_explore_navigation()).await;
    assert_eq!(inspected.status(), StatusCode::OK);
    let inspected = inspected.json::<Value>().await.unwrap();
    assert_eq!(
        inspected["resources"]["workspace_settings"][0]["entry_count"],
        1
    );

    let plan = create_plan_with_publication(
        &client,
        &base_url,
        archive_with_explore_navigation(),
        "nav",
        "publish",
    )
    .await;
    assert_eq!(plan.status(), StatusCode::CREATED);
    let plan = plan.json::<Value>().await.unwrap();
    assert_eq!(plan["ready"], true);
    assert_eq!(
        plan["actions"].as_array().unwrap().last().unwrap()["action"],
        "append"
    );

    let applied = apply_plan(&client, &base_url, plan["id"].as_str().unwrap()).await;
    assert_eq!(applied.status(), StatusCode::OK);
    let application = applied.json::<Value>().await.unwrap();
    assert_eq!(application["state"], "completed");
    let navigation_step = application["steps"].as_array().unwrap().last().unwrap();
    assert_eq!(navigation_step["resource_kind"], "workspace_setting");
    assert_eq!(navigation_step["result_snapshot"]["outcome"], "appended");
    let settings: Value = sqlx::query_scalar("SELECT settings FROM workspaces WHERE id=$1")
        .bind(workspace_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(settings["theme"], "dark");
    assert_eq!(
        settings["explore_navigation"][0]["blueprint_code"],
        "nav_product"
    );

    let repeated = apply_plan(&client, &base_url, plan["id"].as_str().unwrap()).await;
    assert_eq!(repeated.status(), StatusCode::OK);
    let entry_count: i32 = sqlx::query_scalar(
        "SELECT jsonb_array_length(settings->'explore_navigation') FROM workspaces WHERE id=$1",
    )
    .bind(workspace_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(entry_count, 1);
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn malformed_or_duplicate_existing_navigation_is_conflict_and_is_preserved(pool: PgPool) {
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();

    for (index, navigation) in [
        json!({"malformed": true}),
        json!([
            {"blueprint_code":"nav_product"},
            {"blueprint_code":"nav_product","visible_to_role_codes":["viewer"]}
        ]),
        json!([{"blueprint_code":"legacy","legacy_label":"preserve me"}]),
    ]
    .into_iter()
    .enumerate()
    {
        let settings = json!({"theme":"dark", "explore_navigation":navigation});
        sqlx::query("UPDATE workspaces SET settings=$1 WHERE id=$2")
            .bind(&settings)
            .bind(workspace_id)
            .execute(&pool)
            .await
            .unwrap();
        let before: String = sqlx::query_scalar(
            "SELECT (settings->'explore_navigation')::text FROM workspaces WHERE id=$1",
        )
        .bind(workspace_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        let plan = create_plan_with_publication(
            &client,
            &base_url,
            archive_with_explore_navigation(),
            &format!("nav{index}"),
            "publish",
        )
        .await;
        assert_eq!(plan.status(), StatusCode::CREATED);
        let plan = plan.json::<Value>().await.unwrap();
        assert_eq!(plan["ready"], false);
        let setting_action = plan["actions"].as_array().unwrap().last().unwrap();
        assert_eq!(setting_action["action"], "conflict");
        assert_eq!(setting_action["reason_code"], "invalid_current_navigation");
        let apply = apply_plan(&client, &base_url, plan["id"].as_str().unwrap()).await;
        assert_eq!(apply.status(), StatusCode::CONFLICT);

        let after: String = sqlx::query_scalar(
            "SELECT (settings->'explore_navigation')::text FROM workspaces WHERE id=$1",
        )
        .bind(workspace_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(after, before);
    }
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn concurrent_ordinary_navigation_replacement_and_pack_append_do_not_lose_updates(
    pool: PgPool,
) {
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let seed =
        create_plan_with_publication(&client, &base_url, valid_archive(), "ordinary", "publish")
            .await
            .json::<Value>()
            .await
            .unwrap();
    assert_eq!(
        apply_plan(&client, &base_url, seed["id"].as_str().unwrap())
            .await
            .status(),
        StatusCode::OK
    );
    let plan = create_plan_with_publication(
        &client,
        &base_url,
        archive_with_explore_navigation(),
        "race",
        "publish",
    )
    .await
    .json::<Value>()
    .await
    .unwrap();

    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM workspaces WHERE id=$1 FOR UPDATE")
        .bind(workspace_id)
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let ordinary_client = client.clone();
    let ordinary_url = base_url.clone();
    let ordinary = tokio::spawn(async move {
        ordinary_client
            .put(format!("{ordinary_url}/workspace/navigation"))
            .json(&json!({"explore_navigation":[{"blueprint_code":"ordinary_product"}]}))
            .send()
            .await
            .unwrap()
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let apply_client = client.clone();
    let apply_url = base_url.clone();
    let plan_id = plan["id"].as_str().unwrap().to_owned();
    let apply = tokio::spawn(async move { apply_plan(&apply_client, &apply_url, &plan_id).await });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    blocker.commit().await.unwrap();

    assert_eq!(ordinary.await.unwrap().status(), StatusCode::NO_CONTENT);
    assert_eq!(apply.await.unwrap().status(), StatusCode::OK);
    let navigation: Value =
        sqlx::query_scalar("SELECT settings->'explore_navigation' FROM workspaces WHERE id=$1")
            .bind(workspace_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        navigation,
        json!([
            {"blueprint_code":"ordinary_product"},
            {"blueprint_code":"race_product"}
        ])
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn concurrent_ordinary_layout_replacement_and_pack_append_do_not_lose_updates(pool: PgPool) {
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    install_layout_extension(&pool).await;
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let plan = create_plan(
        &client,
        &base_url,
        archive_with_extension_layout(),
        "layoutrace",
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    assert_eq!(plan["ready"], true);

    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM workspaces WHERE id=$1 FOR UPDATE")
        .bind(workspace_id)
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let ordinary_repository = CatalogRepository::new(pool.clone());
    let ordinary = tokio::spawn(async move {
        ordinary_repository
            .update_workspace_extension_layout(json!({
                "version":1,
                "outlets": {
                    "entity_action": {
                        "order":["other.extension:action"],
                        "hidden":[]
                    }
                }
            }))
            .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let apply_client = client.clone();
    let apply_url = base_url.clone();
    let plan_id = plan["id"].as_str().unwrap().to_owned();
    let apply = tokio::spawn(async move { apply_plan(&apply_client, &apply_url, &plan_id).await });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    blocker.commit().await.unwrap();

    ordinary.await.unwrap().unwrap();
    assert_eq!(apply.await.unwrap().status(), StatusCode::OK);
    let layout: Value =
        sqlx::query_scalar("SELECT settings->'extension_layout' FROM workspaces WHERE id=$1")
            .bind(workspace_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        layout["outlets"]["entity_action"]["order"],
        json!(["other.extension:action"])
    );
    assert_eq!(
        layout["outlets"]["navigation"]["order"],
        json!(["acme.layout:nav"])
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn satisfied_navigation_step_is_durable_and_visibility_change_is_stale(pool: PgPool) {
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();

    let seed =
        create_plan_with_publication(&client, &base_url, valid_archive(), "satisfied", "publish")
            .await
            .json::<Value>()
            .await
            .unwrap();
    assert_eq!(
        apply_plan(&client, &base_url, seed["id"].as_str().unwrap())
            .await
            .status(),
        StatusCode::OK
    );
    sqlx::query("UPDATE workspaces SET settings=jsonb_set(settings,'{explore_navigation}',$1::jsonb,true) WHERE id=$2")
        .bind(json!([{"blueprint_code":"satisfied_product"}]))
        .bind(workspace_id)
        .execute(&pool)
        .await
        .unwrap();
    let satisfied = create_plan_with_publication(
        &client,
        &base_url,
        archive_with_explore_navigation(),
        "satisfied",
        "publish",
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    assert_eq!(
        satisfied["actions"].as_array().unwrap().last().unwrap()["action"],
        "satisfied"
    );
    // The blueprint collision correctly makes the ordinary plan not ready. Make
    // only the persisted satisfied setting step executable to exercise its
    // application/retry contract without introducing existing-resource adoption.
    sqlx::query("UPDATE solution_pack_plans SET ready=true WHERE id=$1")
        .bind(satisfied["id"].as_str().unwrap().parse::<Uuid>().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let applied = apply_plan(&client, &base_url, satisfied["id"].as_str().unwrap()).await;
    assert_eq!(applied.status(), StatusCode::OK);
    let applied = applied.json::<Value>().await.unwrap();
    assert_eq!(applied["steps"].as_array().unwrap().len(), 1);
    assert_eq!(
        applied["steps"][0]["result_snapshot"]["outcome"],
        "satisfied"
    );

    sqlx::query("UPDATE workspaces SET settings=jsonb_set(settings,'{explore_navigation}','[]'::jsonb,true) WHERE id=$1")
        .bind(workspace_id)
        .execute(&pool)
        .await
        .unwrap();
    let stale = create_plan_with_publication(
        &client,
        &base_url,
        archive_with_explore_navigation(),
        "stalevis",
        "publish",
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    sqlx::query("UPDATE workspaces SET settings=jsonb_set(settings,'{explore_navigation}',$1::jsonb,true) WHERE id=$2")
        .bind(json!([{"blueprint_code":"stalevis_product","visible_to_role_codes":["viewer"]}]))
        .bind(workspace_id)
        .execute(&pool)
        .await
        .unwrap();
    let response = apply_plan(&client, &base_url, stale["id"].as_str().unwrap()).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "solution_pack_plan_stale"
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn duplicate_system_and_workspace_role_codes_remain_valid_for_navigation(pool: PgPool) {
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    sqlx::query("INSERT INTO roles (id,code,workspace_id,is_system) VALUES ($1,'viewer',$2,false)")
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .execute(&pool)
        .await
        .unwrap();
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let plan = create_plan_with_publication(
        &client,
        &base_url,
        archive_with_explore_navigation_roles(&["viewer"]),
        "roles",
        "publish",
    )
    .await;
    assert_eq!(plan.status(), StatusCode::CREATED);
    let plan = plan.json::<Value>().await.unwrap();
    assert_eq!(plan["ready"], true);
    let applied = apply_plan(&client, &base_url, plan["id"].as_str().unwrap()).await;
    assert_eq!(applied.status(), StatusCode::OK);
    let roles: Value = sqlx::query_scalar(
        "SELECT settings->'explore_navigation'->0->'visible_to_role_codes' FROM workspaces WHERE id=$1",
    )
    .bind(workspace_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(roles, json!(["viewer"]));
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
