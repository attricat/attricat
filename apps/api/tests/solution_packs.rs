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

const SAMPLE: &[u8] = br#"{"format_version":1,"kind":"solution_pack_sample_data","classification":"synthetic","entities":[{"key":"sample-entities/navy-shirt","blueprint":"blueprints/product","facts":[{"attribute":"blueprints/product/attributes/name","value":"Sample Navy Shirt"}],"relationships":[]}]}"#;
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
const CATEGORY_BLUEPRINT: &[u8] = br#"
format_version = 1
code = "category"
name = "Category"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"
"#;
const CHANGED_CATEGORY_BLUEPRINT: &[u8] = br#"
format_version = 1
code = "category"
name = "Changed category"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "description"
value_type = "string"
"#;
const LEGACY_BLUEPRINT: &[u8] = br#"
format_version = 1
code = "legacy"
name = "Legacy"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"
"#;
const ACCESSORY_BLUEPRINT: &[u8] = br#"
format_version = 1
code = "accessory"
name = "Accessory"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"
"#;

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn valid_archive() -> Vec<u8> {
    archive_with_blueprint(PRODUCT_BLUEPRINT)
}

fn archive_with_sample_data() -> Vec<u8> {
    archive_with_sample_blueprint(PRODUCT_BLUEPRINT)
}

fn archive_with_sample_blueprint(blueprint: &[u8]) -> Vec<u8> {
    let manifest = serde_json::to_vec(&json!({
        "manifest_version":1,
        "id":"attricat.samples",
        "name":"Sample pack",
        "version":"1.0.0",
        "description":"Explicit synthetic samples",
        "catalog":{"host_api":"^1.0"},
        "resources":{
            "blueprints":[{"key":"blueprints/product","path":"blueprints/product.toml","required":true,"sha256":digest(blueprint)}],
            "sample_data":{"key":"sample-data/default","path":"sample-data/sample-data.json","sha256":digest(SAMPLE)}
        }
    })).unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "solution-pack.json", &manifest);
        append_file(&mut tar, "blueprints/product.toml", blueprint);
        append_file(&mut tar, "sample-data/sample-data.json", SAMPLE);
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn archive_with_sample(version: &str, blueprint: &[u8], sample: &[u8]) -> Vec<u8> {
    let manifest = serde_json::to_vec(&json!({
        "manifest_version":1,
        "id":"attricat.samples",
        "name":"Sample pack",
        "version":version,
        "description":"Explicit synthetic samples",
        "catalog":{"host_api":"^1.0"},
        "resources":{
            "blueprints":[{"key":"blueprints/product","path":"blueprints/product.toml","required":true,"sha256":digest(blueprint)}],
            "sample_data":{"key":"sample-data/default","path":"sample-data/sample-data.json","sha256":digest(sample)}
        }
    })).unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "solution-pack.json", &manifest);
        append_file(&mut tar, "blueprints/product.toml", blueprint);
        append_file(&mut tar, "sample-data/sample-data.json", sample);
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn archive_with_rejected_context_resource() -> Vec<u8> {
    const CONTEXT: &[u8] =
        br#"{"format_version":1,"code":"web","data":{},"parent":"system/default"}"#;
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "id": "attricat.rejected-context",
        "name": "Rejected context",
        "version": "1.0.0",
        "description": "Contexts are not pack resources",
        "catalog": {"host_api": "^1.0"},
        "resources": {
            "blueprints": [{"key":"blueprints/product","path":"blueprints/product.toml","required":true,"sha256":digest(PRODUCT_BLUEPRINT)}],
            "contexts": [{"key":"contexts/web","path":"contexts/web.json","required":true,"sha256":digest(CONTEXT)}]
        }
    }))
    .unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "solution-pack.json", &manifest);
        append_file(&mut tar, "blueprints/product.toml", PRODUCT_BLUEPRINT);
        append_file(&mut tar, "contexts/web.json", CONTEXT);
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn archive_with_guidance_and_checks() -> Vec<u8> {
    const README: &[u8] = b"# Guided setup\n\nSee [publishing](#publishing).";
    const CHECKLIST: &[u8] = br#"{"format_version":1,"items":[{"key":"checklist/publish","title":"Publish product","markdown":"Publish the product.","check":"checks/product-published"}]}"#;
    const CHECKS: &[u8] = br#"{"format_version":1,"checks":[{"key":"checks/product-published","title":"Product is published","predicate":{"type":"blueprint_published","blueprint":"blueprints/product"}}]}"#;
    let manifest = serde_json::to_vec(&json!({
        "manifest_version":1,
        "id":"attricat.guidance",
        "name":"Guided pack",
        "version":"1.0.0",
        "description":"Guidance and informational checks",
        "catalog":{"host_api":"^1.0"},
        "resources":{"blueprints":[{"key":"blueprints/product","path":"blueprints/product.toml","required":true,"sha256":digest(PRODUCT_BLUEPRINT)}]},
        "documentation":{
            "readme":{"path":"README.md","sha256":digest(README)},
            "setup_checklist":{"path":"setup/checklist.json","sha256":digest(CHECKLIST)}
        },
        "checks":{"path":"checks/checks.json","sha256":digest(CHECKS)}
    })).unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "solution-pack.json", &manifest);
        append_file(&mut tar, "blueprints/product.toml", PRODUCT_BLUEPRINT);
        append_file(&mut tar, "README.md", README);
        append_file(&mut tar, "setup/checklist.json", CHECKLIST);
        append_file(&mut tar, "checks/checks.json", CHECKS);
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn archive_with_blueprint(product_blueprint: &[u8]) -> Vec<u8> {
    archive_with_options(product_blueprint)
}

fn archive_with_two_blueprints() -> Vec<u8> {
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "id": "attricat.two-blueprints",
        "name": "Two blueprints",
        "version": "1.0.0",
        "description": "Retry fixture",
        "catalog": {"host_api": "^1.0"},
        "resources": {"blueprints": [
            {"key":"blueprints/product","path":"blueprints/product.toml","required":true,"sha256":digest(PRODUCT_BLUEPRINT)},
            {"key":"blueprints/category","path":"blueprints/category.toml","required":true,"sha256":digest(CATEGORY_BLUEPRINT)}
        ]}
    }))
    .unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "solution-pack.json", &manifest);
        append_file(&mut tar, "blueprints/product.toml", PRODUCT_BLUEPRINT);
        append_file(&mut tar, "blueprints/category.toml", CATEGORY_BLUEPRINT);
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn archive_with_reverse_order_dependency() -> Vec<u8> {
    const DEPENDENT_PRODUCT: &[u8] = br#"
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

[[attributes]]
code = "category"
value_type = "relationship"
target_blueprint = "blueprints/category"
"#;
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "id": "attricat.reverse-order",
        "name": "Reverse order",
        "version": "1.0.0",
        "description": "Declaration order differs from dependency order",
        "catalog": {"host_api": "^1.0"},
        "resources": {"blueprints": [
            {"key":"blueprints/product","path":"blueprints/product.toml","required":true,"sha256":digest(DEPENDENT_PRODUCT)},
            {"key":"blueprints/category","path":"blueprints/category.toml","required":true,"sha256":digest(CATEGORY_BLUEPRINT)}
        ]}
    }))
    .unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "solution-pack.json", &manifest);
        append_file(&mut tar, "blueprints/product.toml", DEPENDENT_PRODUCT);
        append_file(&mut tar, "blueprints/category.toml", CATEGORY_BLUEPRINT);
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn archive_with_all_checks() -> Vec<u8> {
    const TEMPLATE: &[u8] = br#"{"nested":{"enabled":true},"list":[1,2]}"#;
    let navigation = serde_json::to_vec(&json!({
        "format_version":1,
        "kind":"explore_navigation",
        "entries":[{"blueprint":"blueprints/product"}]
    }))
    .unwrap();
    let layout = serde_json::to_vec(&json!({
        "format_version":1,
        "kind":"extension_layout",
        "entries":[{"contribution":"acme.layout:nav","outlet":"navigation","promoted":true,"required":true}]
    })).unwrap();
    let checks = serde_json::to_vec(&json!({
        "format_version":1,
        "checks":[
            {"key":"checks/published","title":"Published","predicate":{"type":"blueprint_published","blueprint":"blueprints/product"}},
            {"key":"checks/installed","title":"Installed","predicate":{"type":"extension_installed","extension":"extensions/layout"}},
            {"key":"checks/enabled","title":"Enabled","predicate":{"type":"extension_enabled","extension":"extensions/layout"}},
            {"key":"checks/configured","title":"Configured","predicate":{"type":"extension_configuration_matches","extension":"extensions/layout"}},
            {"key":"checks/navigation","title":"Navigation","predicate":{"type":"explore_navigation_entry_present","blueprint":"blueprints/product"}},
            {"key":"checks/layout","title":"Layout","predicate":{"type":"workspace_extension_layout_placement_present","contribution":"acme.layout:nav"}}
        ]
    })).unwrap();
    let manifest = serde_json::to_vec(&json!({
        "manifest_version":1,
        "id":"attricat.all-checks",
        "name":"All checks",
        "version":"1.0.0",
        "description":"All informational check predicates",
        "catalog":{"host_api":"^1.0"},
        "resources":{
            "blueprints":[{"key":"blueprints/product","path":"blueprints/product.toml","required":true,"sha256":digest(PRODUCT_BLUEPRINT)}],
            "workspace_settings":[
                {"key":"workspace/explore-navigation","path":"workspace/explore-navigation.json","required":true,"sha256":digest(&navigation)},
                {"key":"workspace/extension-layout","path":"workspace/extension-layout.json","required":true,"sha256":digest(&layout)}
            ]
        },
        "extensions":[{"key":"extensions/layout","id":"acme.layout","version":"^1.0","required":true,"configuration_template":{"path":"extensions/layout.json","sha256":digest(TEMPLATE)}}],
        "checks":{"path":"checks/checks.json","sha256":digest(&checks)}
    })).unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "solution-pack.json", &manifest);
        append_file(&mut tar, "blueprints/product.toml", PRODUCT_BLUEPRINT);
        append_file(&mut tar, "workspace/explore-navigation.json", &navigation);
        append_file(&mut tar, "workspace/extension-layout.json", &layout);
        append_file(&mut tar, "extensions/layout.json", TEMPLATE);
        append_file(&mut tar, "checks/checks.json", &checks);
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn archive_with_checklist(checklist: &[u8]) -> Vec<u8> {
    let manifest = serde_json::to_vec(&json!({
        "manifest_version":1,
        "id":"attricat.checklist",
        "name":"Checklist",
        "version":"1.0.0",
        "description":"Checklist boundary",
        "catalog":{"host_api":"^1.0"},
        "resources":{"blueprints":[{"key":"blueprints/product","path":"blueprints/product.toml","required":true,"sha256":digest(PRODUCT_BLUEPRINT)}]},
        "documentation":{"setup_checklist":{"path":"setup/checklist.json","sha256":digest(checklist)}}
    })).unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "solution-pack.json", &manifest);
        append_file(&mut tar, "blueprints/product.toml", PRODUCT_BLUEPRINT);
        append_file(&mut tar, "setup/checklist.json", checklist);
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
            "blueprints": [{
                "key": "blueprints/product",
                "path": "blueprints/product.toml",
                "required": true,
                "sha256": digest(PRODUCT_BLUEPRINT)
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
        append_file(&mut tar, "blueprints/product.toml", PRODUCT_BLUEPRINT);
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

fn archive_with_options(product_blueprint: &[u8]) -> Vec<u8> {
    archive_with_pack_id(product_blueprint, "attricat.ecommerce")
}

fn archive_with_pack_id(product_blueprint: &[u8], pack_id: &str) -> Vec<u8> {
    release_archive(
        pack_id,
        "1.2.0",
        &[("blueprints/product", product_blueprint)],
    )
}

fn release_archive(pack_id: &str, version: &str, blueprints: &[(&str, &[u8])]) -> Vec<u8> {
    let resources = blueprints
        .iter()
        .map(|(key, definition)| {
            json!({
                "key": key,
                "path": format!("{key}.toml"),
                "required": true,
                "sha256": digest(definition),
            })
        })
        .collect::<Vec<_>>();
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "id": pack_id,
        "name": "Release test pack",
        "version": version,
        "description": "Later-release fixture",
        "catalog": {"host_api": ">=1.0.0, <2.0.0"},
        "resources": {"blueprints": resources},
    }))
    .unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "solution-pack.json", &manifest);
        for (key, definition) in blueprints {
            append_file(&mut tar, &format!("{key}.toml"), definition);
        }
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

async fn create_plan_from_application(
    client: &Client,
    base_url: &str,
    archive: Vec<u8>,
    prefix: &str,
    publication: &str,
    application_id: Uuid,
) -> reqwest::Response {
    client
        .post(format!(
            "{base_url}/solution-packs/plans?prefix={prefix}&blueprint_publication={publication}&from_application={application_id}"
        ))
        .header("content-type", "application/zstd")
        .body(archive)
        .send()
        .await
        .unwrap()
}

async fn create_sample_plan_from_application(
    client: &Client,
    base_url: &str,
    archive: Vec<u8>,
    prefix: &str,
    application_id: Uuid,
) -> reqwest::Response {
    client
        .post(format!(
            "{base_url}/solution-packs/plans?prefix={prefix}&blueprint_publication=publish&include_sample_data=true&from_application={application_id}"
        ))
        .header("content-type", "application/zstd")
        .body(archive)
        .send()
        .await
        .unwrap()
}

async fn create_plan_with_maps(
    client: &Client,
    base_url: &str,
    archive: Vec<u8>,
    prefix: &str,
    publication: &str,
    mappings: &[(&str, &str)],
) -> reqwest::Response {
    let archive = reqwest::multipart::Part::bytes(archive)
        .file_name("pack.tar.zst")
        .mime_str("application/zstd")
        .unwrap();
    let mut form = reqwest::multipart::Form::new().part("archive", archive);
    for (key, code) in mappings {
        form = form.text(
            "blueprint_map",
            json!({"key": key, "code": code}).to_string(),
        );
    }
    client
        .post(format!(
            "{base_url}/solution-packs/plans?prefix={prefix}&blueprint_publication={publication}"
        ))
        .multipart(form)
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
    let application = apply_plan(&bearer_client(&permitted_secret), &base_url, plan_id).await;
    assert_eq!(application.status(), StatusCode::OK);
    let application = application.json::<Value>().await.unwrap();
    let checks_url = format!(
        "{base_url}/solution-packs/applications/{}/checks",
        application["id"].as_str().unwrap()
    );
    assert_eq!(
        bearer_client(&denied_secret)
            .post(&checks_url)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        bearer_client(&permitted_secret)
            .post(&checks_url)
            .send()
            .await
            .unwrap()
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
    let multipart_form = || {
        reqwest::multipart::Form::new().part(
            "archive",
            reqwest::multipart::Part::bytes(valid_archive())
                .mime_str("application/zstd")
                .unwrap(),
        )
    };
    let missing_multipart_csrf = client
        .post(format!(
            "{base_url}/solution-packs/plans?prefix=csrf_multi&blueprint_publication=draft"
        ))
        .header("cookie", &cookie)
        .multipart(multipart_form())
        .send()
        .await
        .unwrap();
    assert_eq!(missing_multipart_csrf.status(), StatusCode::FORBIDDEN);
    let valid_multipart_csrf = client
        .post(format!(
            "{base_url}/solution-packs/plans?prefix=csrf_multi&blueprint_publication=draft"
        ))
        .header("cookie", &cookie)
        .header("x-catalog-csrf", csrf_value)
        .multipart(multipart_form())
        .send()
        .await
        .unwrap();
    assert_eq!(valid_multipart_csrf.status(), StatusCode::CREATED);

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
    let application = valid_apply_csrf.json::<Value>().await.unwrap();
    let checks_url = format!(
        "{base_url}/solution-packs/applications/{}/checks",
        application["id"].as_str().unwrap()
    );
    let missing_checks_csrf = client
        .post(&checks_url)
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(missing_checks_csrf.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        missing_checks_csrf.json::<Value>().await.unwrap()["error"]["code"],
        "csrf_failed"
    );
    assert_eq!(
        client
            .post(checks_url)
            .header("cookie", &cookie)
            .header("x-catalog-csrf", csrf_value)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CREATED
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn sample_matcher_rejects_an_omitted_effective_default(pool: PgPool) {
    const BLUEPRINT: &[u8] = br#"
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
[[attributes]]
code = "contact"
value_type = "string"
default_value = "Jane@example.com"
"#;
    let (base_url, server) = start_server(pool).await;
    let response = inspect(
        &authenticated_client(),
        &base_url,
        archive_with_sample_blueprint(BLUEPRINT),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(response.text().await.unwrap().contains("prohibited-v1"));
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn matcher_substrings_cover_explicit_defaulted_and_encoded_values(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let base64 = format!("x{}", "QUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFB");
    for prohibited in [
        "dead192.0.2.1beef".to_owned(),
        base64,
        "ſabcdefabcdefabcdefabcdefſ".to_owned(),
        "000000002026-09-18".to_owned(),
        "x2026-09-18y".to_owned(),
        "ſ2026-09-18€".to_owned(),
        "000000002026-09-18T09:00:00Z9".to_owned(),
    ] {
        let encoded = prohibited
            .as_bytes()
            .iter()
            .map(|byte| format!("%{byte:02X}"))
            .collect::<String>();
        let variants = [
            prohibited.clone(),
            encoded.clone(),
            encoded.replace('%', "%25"),
        ];
        for candidate in variants {
            let default_json = serde_json::to_string(&candidate).unwrap();
            let blueprint = format!(
                "format_version = 1\ncode = \"product\"\nname = \"Product\"\nkind = \"entity\"\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"contact\"]\n[[attributes]]\ncode = \"contact\"\nvalue_type = \"string\"\ndefault_value = {default_json}\n"
            );
            let omitted = br#"{"format_version":1,"kind":"solution_pack_sample_data","classification":"synthetic","entities":[{"key":"sample-entities/item","blueprint":"blueprints/product","facts":[],"relationships":[]}] }"#;
            assert_eq!(
                inspect(
                    &client,
                    &base_url,
                    archive_with_sample("1.0.0", blueprint.as_bytes(), omitted)
                )
                .await
                .status(),
                StatusCode::UNPROCESSABLE_ENTITY,
                "accepted prohibited default variant {candidate} derived from {prohibited}"
            );

            let sample = serde_json::to_vec(&json!({
                "format_version":1,
                "kind":"solution_pack_sample_data",
                "classification":"synthetic",
                "entities":[{"key":"sample-entities/item","blueprint":"blueprints/product","facts":[{"attribute":"blueprints/product/attributes/name","value":candidate}],"relationships":[]}]
            })).unwrap();
            assert_eq!(
                inspect(
                    &client,
                    &base_url,
                    archive_with_sample("1.0.0", PRODUCT_BLUEPRINT, &sample)
                )
                .await
                .status(),
                StatusCode::UNPROCESSABLE_ENTITY,
                "accepted prohibited explicit variant derived from {prohibited}"
            );
        }
    }

    let prohibited_default_blueprint = br#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["contact"]
[[attributes]]
code = "contact"
value_type = "string"
default_value = "Jane@example.com"
"#;
    let safe_override = br#"{"format_version":1,"kind":"solution_pack_sample_data","classification":"synthetic","entities":[{"key":"sample-entities/item","blueprint":"blueprints/product","facts":[{"attribute":"blueprints/product/attributes/contact","value":"Sample contact"}],"relationships":[]}]}"#;
    assert_eq!(
        inspect(
            &client,
            &base_url,
            archive_with_sample("1.0.0", prohibited_default_blueprint, safe_override)
        )
        .await
        .status(),
        StatusCode::OK
    );
    let plan = client
        .post(format!("{base_url}/solution-packs/plans?prefix=override&blueprint_publication=publish&include_sample_data=true"))
        .header("content-type", "application/zstd")
        .body(archive_with_sample("1.0.0", prohibited_default_blueprint, safe_override))
        .send().await.unwrap();
    assert_eq!(
        plan.status(),
        StatusCode::CREATED,
        "{}",
        plan.text().await.unwrap()
    );
    let canonical: Value = sqlx::query_scalar(
        "SELECT canonical_input FROM solution_pack_plan_sample_entities LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(canonical["effective_defaults"], json!([]));
    assert_eq!(canonical["entity"]["facts"][0]["value"], "Sample contact");
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn sample_temporal_values_are_type_aware_and_strict(pool: PgPool) {
    const DATE_BLUEPRINT: &[u8] = br#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["available_on"]
[[attributes]]
code = "available_on"
value_type = "date"
"#;
    const VALID_DATE: &[u8] = br#"{"format_version":1,"kind":"solution_pack_sample_data","classification":"synthetic","entities":[{"key":"sample-entities/item","blueprint":"blueprints/product","facts":[{"attribute":"blueprints/product/attributes/available_on","value":"2026-09-18"}],"relationships":[]}]}"#;
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let valid_date = inspect(
        &client,
        &base_url,
        archive_with_sample("1.0.0", DATE_BLUEPRINT, VALID_DATE),
    )
    .await;
    assert_eq!(
        valid_date.status(),
        StatusCode::OK,
        "{}",
        valid_date.text().await.unwrap()
    );
    let invalid_adjacent_native_date = br#"{"format_version":1,"kind":"solution_pack_sample_data","classification":"synthetic","entities":[{"key":"sample-entities/item","blueprint":"blueprints/product","facts":[{"attribute":"blueprints/product/attributes/available_on","value":"000000002026-09-18"}],"relationships":[]}] }"#;
    assert_eq!(
        inspect(
            &client,
            &base_url,
            archive_with_sample("1.0.0", DATE_BLUEPRINT, invalid_adjacent_native_date),
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "native-date exemption must follow strict native parsing"
    );

    let time_blueprint = b"format_version = 1\ncode = \"product\"\nname = \"Product\"\nkind = \"entity\"\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"temporal\"]\n[[attributes]]\ncode = \"temporal\"\nvalue_type = \"time\"\n";
    let valid_time = serde_json::to_vec(&json!({
        "format_version":1,"kind":"solution_pack_sample_data","classification":"synthetic",
        "entities":[{"key":"sample-entities/item","blueprint":"blueprints/product","facts":[{"attribute":"blueprints/product/attributes/temporal","value":{"time":"12:34:56","time_zone":"America/New_York"}}],"relationships":[]}]
    })).unwrap();
    let response = inspect(
        &client,
        &base_url,
        archive_with_sample("1.0.0", time_blueprint, &valid_time),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        response.text().await.unwrap()
    );
    let time_plan = client
        .post(format!("{base_url}/solution-packs/plans?prefix=native_time&blueprint_publication=publish&include_sample_data=true"))
        .header("content-type", "application/zstd")
        .body(archive_with_sample("1.0.0", time_blueprint, &valid_time))
        .send().await.unwrap();
    assert_eq!(time_plan.status(), StatusCode::CREATED);
    let time_plan = time_plan.json::<Value>().await.unwrap();
    assert_eq!(
        apply_plan(&client, &base_url, time_plan["id"].as_str().unwrap())
            .await
            .status(),
        StatusCode::OK
    );
    for invalid_time in [
        json!({"time":"25:00:00","time_zone":"UTC"}),
        json!({"time":"12:34:56","time_zone":"Mars/Olympus"}),
        json!({"time":"12:34:56","time_zone":"UTC","extra":"rejected"}),
        json!({"time":"12:34:56","time_zone":"xQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFB"}),
    ] {
        let sample = serde_json::to_vec(&json!({
            "format_version":1,"kind":"solution_pack_sample_data","classification":"synthetic",
            "entities":[{"key":"sample-entities/item","blueprint":"blueprints/product","facts":[{"attribute":"blueprints/product/attributes/temporal","value":invalid_time}],"relationships":[]}]
        })).unwrap();
        assert_eq!(
            inspect(
                &client,
                &base_url,
                archive_with_sample("1.0.0", time_blueprint, &sample)
            )
            .await
            .status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    let time_on_string = serde_json::to_vec(&json!({
        "format_version":1,"kind":"solution_pack_sample_data","classification":"synthetic",
        "entities":[{"key":"sample-entities/item","blueprint":"blueprints/product","facts":[{"attribute":"blueprints/product/attributes/name","value":{"time":"12:34:56","time_zone":"UTC"}}],"relationships":[]}]
    })).unwrap();
    assert_eq!(
        inspect(
            &client,
            &base_url,
            archive_with_sample("1.0.0", PRODUCT_BLUEPRINT, &time_on_string)
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let string_date = br#"{"format_version":1,"kind":"solution_pack_sample_data","classification":"synthetic","entities":[{"key":"sample-entities/item","blueprint":"blueprints/product","facts":[{"attribute":"blueprints/product/attributes/name","value":"2026-09-18"}],"relationships":[]}]}"#;
    assert_eq!(
        inspect(
            &client,
            &base_url,
            archive_with_sample("1.0.0", PRODUCT_BLUEPRINT, string_date)
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    for (value_type, invalid) in [
        ("date", "2026-02-30"),
        ("time", "25:00:00"),
        ("datetime", "not-a-datetime"),
    ] {
        let blueprint = format!(
            "format_version = 1\ncode = \"product\"\nname = \"Product\"\nkind = \"entity\"\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"temporal\"]\n[[attributes]]\ncode = \"temporal\"\nvalue_type = \"{value_type}\"\n"
        );
        let sample = serde_json::to_vec(&json!({
            "format_version":1,
            "kind":"solution_pack_sample_data",
            "classification":"synthetic",
            "entities":[{"key":"sample-entities/item","blueprint":"blueprints/product","facts":[{"attribute":"blueprints/product/attributes/temporal","value":invalid}],"relationships":[]}]
        })).unwrap();
        assert_eq!(
            inspect(
                &client,
                &base_url,
                archive_with_sample("1.0.0", blueprint.as_bytes(), &sample)
            )
            .await
            .status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "accepted invalid {value_type}"
        );
    }

    for (value_type, default_value) in [
        ("date", "\"2026-02-30\""),
        ("time", "{ time = \"25:00:00\", time_zone = \"UTC\" }"),
        ("datetime", "\"not-a-datetime\""),
    ] {
        let blueprint = format!(
            "format_version = 1\ncode = \"product\"\nname = \"Product\"\nkind = \"entity\"\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"temporal\"]\n[[attributes]]\ncode = \"temporal\"\nvalue_type = \"{value_type}\"\ndefault_value = {default_value}\n"
        );
        let empty = br#"{"format_version":1,"kind":"solution_pack_sample_data","classification":"synthetic","entities":[{"key":"sample-entities/item","blueprint":"blueprints/product","facts":[],"relationships":[]}]}"#;
        assert_eq!(
            inspect(
                &client,
                &base_url,
                archive_with_sample("1.0.0", blueprint.as_bytes(), empty)
            )
            .await
            .status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "accepted invalid defaulted {value_type}"
        );
    }
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn sample_data_requires_opt_in_and_applies_with_ordinary_audit_and_event(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();

    let inspection = inspect(&client, &base_url, archive_with_sample_data()).await;
    assert_eq!(inspection.status(), StatusCode::OK);
    let inspection = inspection.json::<Value>().await.unwrap();
    assert_eq!(inspection["sample_data"]["entity_count"], 1);
    assert!(
        inspection["warnings"][0]
            .as_str()
            .unwrap()
            .contains("automation")
    );
    assert!(!inspection.to_string().contains("Sample Navy Shirt"));

    let unselected = create_plan_with_publication(
        &client,
        &base_url,
        archive_with_sample_data(),
        "samples_none",
        "publish",
    )
    .await;
    assert_eq!(unselected.status(), StatusCode::CREATED);
    let unselected = unselected.json::<Value>().await.unwrap();
    assert_eq!(unselected["sample_data_selected"], false);
    assert!(
        unselected["actions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|action| action["resource_kind"] != "sample_entity")
    );

    let selected = client.post(format!("{base_url}/solution-packs/plans?prefix=samples&blueprint_publication=publish&include_sample_data=true"))
        .header("content-type", "application/zstd").body(archive_with_sample_data()).send().await.unwrap();
    assert_eq!(selected.status(), StatusCode::CREATED);
    let selected = selected.json::<Value>().await.unwrap();
    assert_eq!(selected["sample_data_selected"], true);
    assert!(
        selected["sample_automation_warning"]
            .as_str()
            .unwrap()
            .contains("entity.created.v1")
    );
    assert!(!selected.to_string().contains("Sample Navy Shirt"));
    let duplicate = client.post(format!("{base_url}/solution-packs/plans?prefix=samples_again&blueprint_publication=publish&include_sample_data=true"))
        .header("content-type", "application/zstd").body(archive_with_sample_data()).send().await.unwrap();
    assert_eq!(duplicate.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let applied = apply_plan(&client, &base_url, selected["id"].as_str().unwrap()).await;
    assert_eq!(applied.status(), StatusCode::OK);
    let applied = applied.json::<Value>().await.unwrap();
    assert!(!applied.to_string().contains("Sample Navy Shirt"));
    let started_at =
        chrono::DateTime::parse_from_rfc3339(applied["started_at"].as_str().unwrap()).unwrap();
    let resumable_until =
        chrono::DateTime::parse_from_rfc3339(applied["resumable_until"].as_str().unwrap()).unwrap();
    assert_eq!(resumable_until - started_at, chrono::Duration::days(30));
    let entity_id: Uuid =
        sqlx::query_scalar("SELECT id FROM entities WHERE 'attricat.sample'=ANY(system_tags)")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM audit_events WHERE target->>'type'='entity' AND target->>'id'=$1"
        )
        .bind(entity_id.to_string())
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM domain_events WHERE aggregate_id=$1 AND event_type='entity.created.v1'")
        .bind(entity_id).fetch_one(&pool).await.unwrap(), 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM solution_pack_plan_sample_entities")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let retained_audit_value: Value = sqlx::query_scalar(
        "SELECT after_value FROM audit_event_changes WHERE entity_id=$1 AND attribute_code='name'",
    )
    .bind(entity_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(retained_audit_value, json!("Sample Navy Shirt"));
    let retained_event: Value = sqlx::query_scalar(
        "SELECT payload FROM domain_events WHERE aggregate_id=$1 AND event_type='entity.created.v1'",
    )
    .bind(entity_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        retained_event["facts"][0]["after_value"],
        "Sample Navy Shirt"
    );
    let detail = client
        .get(format!("{base_url}/v1/entities/{entity_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(detail.status(), StatusCode::OK);
    assert_eq!(
        detail.json::<Value>().await.unwrap()["entity"]["is_sample"],
        true
    );
    let search = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({"blueprint":{"code":"samples_product"},"page":{"size":25}}))
        .send()
        .await
        .unwrap();
    assert_eq!(search.status(), StatusCode::OK);
    assert_eq!(
        search.json::<Value>().await.unwrap()["items"][0]["is_sample"],
        true
    );
    let edited = client
        .put(format!("{base_url}/v1/entities/{entity_id}"))
        .json(&json!({"values":[],"relationships":[],"remove_values":[],"system_tags":[]}))
        .send()
        .await
        .unwrap();
    assert_eq!(edited.status(), StatusCode::OK);
    assert_eq!(edited.json::<Value>().await.unwrap()["is_sample"], false);

    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn sample_dataset_reservation_survives_every_plan_and_application_state(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    for (index, state) in [
        "ready",
        "running",
        "failed",
        "invalid",
        "abandoned",
        "completed",
        "expired",
    ]
    .into_iter()
    .enumerate()
    {
        let version = format!("2.{}.0", index + 1);
        let archive = archive_with_sample(&version, PRODUCT_BLUEPRINT, SAMPLE);
        let first = client
            .post(format!("{base_url}/solution-packs/plans?prefix=reservation_{index}&blueprint_publication=publish&include_sample_data=true"))
            .header("content-type", "application/zstd")
            .body(archive.clone())
            .send().await.unwrap();
        assert_eq!(
            first.status(),
            StatusCode::CREATED,
            "first plan for {state}"
        );
        let first = first.json::<Value>().await.unwrap();
        let plan_id = Uuid::parse_str(first["id"].as_str().unwrap()).unwrap();
        if state == "expired" {
            sqlx::query("UPDATE solution_pack_plans SET created_at=clock_timestamp()-interval '25 hours',expires_at=clock_timestamp()-interval '1 hour' WHERE id=$1")
                .bind(plan_id).execute(&pool).await.unwrap();
        } else if state != "ready" {
            let application = apply_plan(&client, &base_url, first["id"].as_str().unwrap()).await;
            assert_eq!(application.status(), StatusCode::OK, "apply for {state}");
            let application = application.json::<Value>().await.unwrap();
            if state != "completed" {
                let application_id = Uuid::parse_str(application["id"].as_str().unwrap()).unwrap();
                sqlx::query("UPDATE solution_pack_applications SET state=$2,completed_at=NULL,abandoned_at=CASE WHEN $2='abandoned' THEN clock_timestamp() ELSE NULL END WHERE id=$1")
                    .bind(application_id).bind(state).execute(&pool).await.unwrap();
            }
        }
        let duplicate = client
            .post(format!("{base_url}/solution-packs/plans?prefix=reservation_again_{index}&blueprint_publication=publish&include_sample_data=true"))
            .header("content-type", "application/zstd")
            .body(archive)
            .send().await.unwrap();
        assert_eq!(
            duplicate.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "reservation was released in {state} state"
        );
    }
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn later_sample_releases_create_reuse_conflict_and_record_removals(pool: PgPool) {
    let sample = |entities: Value| {
        serde_json::to_vec(&json!({
            "format_version":1,
            "kind":"solution_pack_sample_data",
            "classification":"synthetic",
            "entities":entities
        }))
        .unwrap()
    };
    let entity = |key: &str, name: &str| json!({"key":format!("sample-entities/{key}"),"blueprint":"blueprints/product","facts":[{"attribute":"blueprints/product/attributes/name","value":name}],"relationships":[]});
    let initial_sample = sample(json!([
        entity("navy-shirt", "Sample Navy Shirt"),
        entity("cap", "Sample Cap")
    ]));
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let initial = client
        .post(format!("{base_url}/solution-packs/plans?prefix=sample_lineage&blueprint_publication=publish&include_sample_data=true"))
        .header("content-type", "application/zstd")
        .body(archive_with_sample("1.0.0", PRODUCT_BLUEPRINT, &initial_sample))
        .send()
        .await
        .unwrap();
    assert_eq!(initial.status(), StatusCode::CREATED);
    let initial = initial.json::<Value>().await.unwrap();
    let applied = apply_plan(&client, &base_url, initial["id"].as_str().unwrap()).await;
    assert_eq!(applied.status(), StatusCode::OK);
    let application = applied.json::<Value>().await.unwrap();
    let prior_application_id = application["id"].as_str().unwrap().parse::<Uuid>().unwrap();
    let navy_id = application["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["logical_key"] == "sample-entities/navy-shirt")
        .unwrap()["target_id"]
        .as_str()
        .unwrap()
        .parse::<Uuid>()
        .unwrap();
    let entity_count_before: i64 = sqlx::query_scalar("SELECT count(*) FROM entities")
        .fetch_one(&pool)
        .await
        .unwrap();
    let edited = client
        .put(format!("{base_url}/v1/entities/{navy_id}"))
        .json(&json!({"values":[{"kind":"scalar","attribute_code":"name","value":"Locally edited sample"}],"relationships":[],"remove_values":[],"system_tags":[]}))
        .send()
        .await
        .unwrap();
    assert_eq!(edited.status(), StatusCode::OK);
    assert_eq!(edited.json::<Value>().await.unwrap()["is_sample"], false);

    let unchanged = create_sample_plan_from_application(
        &client,
        &base_url,
        archive_with_sample("1.1.0", PRODUCT_BLUEPRINT, &initial_sample),
        "sample_lineage_next",
        prior_application_id,
    )
    .await;
    let unchanged_status = unchanged.status();
    let unchanged_body = unchanged.text().await.unwrap();
    assert_eq!(unchanged_status, StatusCode::CREATED, "{unchanged_body}");
    let unchanged = serde_json::from_str::<Value>(&unchanged_body).unwrap();
    assert_eq!(unchanged["ready"], true);
    let reuse = unchanged["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["logical_key"] == "sample-entities/navy-shirt")
        .unwrap();
    assert_eq!(reuse["action"], "map");
    assert_eq!(reuse["reason_code"], "unchanged_from_prior_application");
    let reapplied = apply_plan(&client, &base_url, unchanged["id"].as_str().unwrap()).await;
    assert_eq!(reapplied.status(), StatusCode::OK);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM entities")
            .fetch_one(&pool)
            .await
            .unwrap(),
        entity_count_before
    );
    let (name, tags): (String, Vec<String>) = sqlx::query_as(
        "SELECT av.value_text,e.system_tags FROM entities e JOIN attributes a ON a.blueprint_id=e.blueprint_id AND a.blueprint_version=e.blueprint_version AND a.code='name' JOIN attribute_values av ON av.entity_id=e.id AND av.attribute_id=a.id AND av.active WHERE e.id=$1",
    )
    .bind(navy_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(name, "Locally edited sample");
    assert!(!tags.contains(&"attricat.sample".to_owned()));

    let changed_sample = sample(json!([
        entity("navy-shirt", "Changed Sample Shirt"),
        entity("cap", "Sample Cap")
    ]));
    let changed = create_sample_plan_from_application(
        &client,
        &base_url,
        archive_with_sample("1.2.0", PRODUCT_BLUEPRINT, &changed_sample),
        "sample_lineage_changed",
        prior_application_id,
    )
    .await;
    assert_eq!(changed.status(), StatusCode::CREATED);
    let changed = changed.json::<Value>().await.unwrap();
    assert_eq!(changed["ready"], false);
    assert!(changed["actions"].as_array().unwrap().iter().any(|action| {
        action["logical_key"] == "sample-entities/navy-shirt"
            && action["action"] == "conflict"
            && action["reason_code"] == "update_not_supported"
    }));

    let added_sample = sample(json!([
        entity("navy-shirt", "Sample Navy Shirt"),
        entity("cap", "Sample Cap"),
        entity("belt", "Sample Belt")
    ]));
    let added = create_sample_plan_from_application(
        &client,
        &base_url,
        archive_with_sample("1.3.0", PRODUCT_BLUEPRINT, &added_sample),
        "sample_lineage_added",
        prior_application_id,
    )
    .await;
    assert_eq!(added.status(), StatusCode::CREATED);
    let added = added.json::<Value>().await.unwrap();
    assert!(
        added["release_changes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|change| {
                change["logical_key"] == "sample-entities/belt" && change["change_kind"] == "added"
            })
    );

    let removed_sample = sample(json!([entity("navy-shirt", "Sample Navy Shirt")]));
    let removed = create_sample_plan_from_application(
        &client,
        &base_url,
        archive_with_sample("1.4.0", PRODUCT_BLUEPRINT, &removed_sample),
        "sample_lineage_removed",
        prior_application_id,
    )
    .await;
    assert_eq!(removed.status(), StatusCode::CREATED);
    let removed = removed.json::<Value>().await.unwrap();
    assert!(
        removed["release_changes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|change| {
                change["logical_key"] == "sample-entities/cap" && change["change_kind"] == "removed"
            })
    );

    sqlx::query("UPDATE entities SET deleted_at=clock_timestamp() WHERE id=$1")
        .bind(navy_id)
        .execute(&pool)
        .await
        .unwrap();
    let missing = create_sample_plan_from_application(
        &client,
        &base_url,
        archive_with_sample("1.5.0", PRODUCT_BLUEPRINT, &initial_sample),
        "sample_lineage_missing",
        prior_application_id,
    )
    .await;
    assert_eq!(missing.status(), StatusCode::CREATED);
    let missing = missing.json::<Value>().await.unwrap();
    assert_eq!(missing["ready"], false);
    assert!(missing["actions"].as_array().unwrap().iter().any(|action| {
        action["logical_key"] == "sample-entities/navy-shirt"
            && action["reason_code"] == "prior_sample_target_missing_or_changed"
    }));
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn guidance_and_checks_persist_and_reruns_are_informational_and_immutable(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let inspection = inspect(&client, &base_url, archive_with_guidance_and_checks()).await;
    assert_eq!(inspection.status(), StatusCode::OK);
    let inspection = inspection.json::<Value>().await.unwrap();
    assert_eq!(inspection["guidance"]["readme_bytes"], 46);
    assert_eq!(inspection["guidance"]["setup_checklist_items"], 1);
    assert_eq!(
        inspection["guidance"]["checks"][0]["predicate_type"],
        "blueprint_published"
    );
    assert!(inspection.to_string().find("blueprints/product").is_some());

    let plan_response = create_plan_with_publication(
        &client,
        &base_url,
        archive_with_guidance_and_checks(),
        "guided",
        "publish",
    )
    .await;
    assert_eq!(plan_response.status(), StatusCode::CREATED);
    let plan = plan_response.json::<Value>().await.unwrap();
    assert_eq!(
        plan["readme_markdown"],
        "# Guided setup\n\nSee [publishing](#publishing)."
    );
    assert_eq!(plan["checks"][0]["predicate_type"], "blueprint_published");
    assert!(plan.get("predicate").is_none());

    let applied = apply_plan(&client, &base_url, plan["id"].as_str().unwrap()).await;
    assert_eq!(applied.status(), StatusCode::OK);
    let application = applied.json::<Value>().await.unwrap();
    let application_id = application["id"].as_str().unwrap();
    let first_run_id = application["latest_check_run"]["id"].as_str().unwrap();
    assert_eq!(application["latest_check_run"]["passed_count"], 1);
    assert_eq!(application["state"], "completed");
    let checks_url = format!("{base_url}/solution-packs/applications/{application_id}/checks");
    assert_eq!(
        client
            .post(&checks_url)
            .body("{}")
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    for query in ["limit=0", "limit=101", "offset=10001"] {
        assert_eq!(
            client
                .get(format!("{checks_url}?{query}"))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM solution_pack_check_runs WHERE application_id=$1"
        )
        .bind(application_id.parse::<Uuid>().unwrap())
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );

    sqlx::query(
        "UPDATE blueprints SET status='draft' WHERE workspace_id=$1 AND code='guided_product'",
    )
    .bind(Uuid::from_u128(0x00000000000040008000000000000002))
    .execute(&pool)
    .await
    .unwrap();
    let rerun = client
        .post(format!(
            "{base_url}/solution-packs/applications/{application_id}/checks"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(rerun.status(), StatusCode::CREATED);
    let rerun = rerun.json::<Value>().await.unwrap();
    assert_eq!(rerun["failed_count"], 1);
    assert_eq!(rerun["results"][0]["passed"], false);
    assert_eq!(rerun["results"][0]["reason_code"], "not_published");
    assert_eq!(
        rerun["results"][0]["evidence"],
        json!({"blueprint_code":"guided_product"})
    );

    let original = client
        .get(format!(
            "{base_url}/solution-packs/applications/{application_id}/checks/{first_run_id}"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(original.status(), StatusCode::OK);
    assert_eq!(
        original.json::<Value>().await.unwrap()["results"][0]["passed"],
        true
    );
    let history = client
        .get(format!(
            "{base_url}/solution-packs/applications/{application_id}/checks?limit=2&offset=0"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(history.status(), StatusCode::OK);
    assert_eq!(
        history
            .json::<Value>()
            .await
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM solution_pack_check_runs WHERE application_id=$1"
        )
        .bind(application_id.parse::<Uuid>().unwrap())
        .fetch_one(&pool)
        .await
        .unwrap(),
        2
    );
    let (concurrent_first, concurrent_second) = tokio::join!(
        client.post(&checks_url).send(),
        client.post(&checks_url).send()
    );
    let concurrent_first = concurrent_first.unwrap();
    let concurrent_second = concurrent_second.unwrap();
    assert_eq!(concurrent_first.status(), StatusCode::CREATED);
    assert_eq!(concurrent_second.status(), StatusCode::CREATED);
    let concurrent_first = concurrent_first.json::<Value>().await.unwrap();
    let concurrent_second = concurrent_second.json::<Value>().await.unwrap();
    assert_ne!(concurrent_first["id"], concurrent_second["id"]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM solution_pack_check_runs WHERE application_id=$1"
        )
        .bind(application_id.parse::<Uuid>().unwrap())
        .fetch_one(&pool)
        .await
        .unwrap(),
        4
    );

    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn near_limit_checklist_persists_without_null_optional_checks(pool: PgPool) {
    let items = (0..16)
        .map(|position| {
            json!({
                "key":format!("checklist/item_{position}"),
                "title":format!("Item {position}"),
                "markdown":"x".repeat(4000)
            })
        })
        .collect::<Vec<_>>();
    let checklist = serde_json::to_vec(&json!({"format_version":1,"items":items})).unwrap();
    assert!(checklist.len() > 64_000 && checklist.len() <= 65_536);
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let plan_response = create_plan(
        &client,
        &base_url,
        archive_with_checklist(&checklist),
        "bounded",
    )
    .await;
    assert_eq!(plan_response.status(), StatusCode::CREATED);
    let plan = plan_response.json::<Value>().await.unwrap();
    assert!(plan["setup_checklist"]["items"][0].get("check").is_none());
    let applied = apply_plan(&client, &base_url, plan["id"].as_str().unwrap()).await;
    assert_eq!(applied.status(), StatusCode::OK);
    let application = applied.json::<Value>().await.unwrap();
    assert!(
        application["setup_checklist"]["items"][0]
            .get("check")
            .is_none()
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn all_check_predicates_evaluate_current_state_and_fail_closed_atomically(pool: PgPool) {
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let release_id = install_layout_extension(&pool).await;
    sqlx::query("UPDATE extension_installations SET state='enabled',configuration=$2 WHERE workspace_id=$1 AND extension_id='acme.layout'")
        .bind(workspace_id)
        .bind(json!({"nested":{"enabled":true,"extra":"safe"},"list":[1,2],"unrelated":true}))
        .execute(&pool).await.unwrap();
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let plan = create_plan_with_publication(
        &client,
        &base_url,
        archive_with_all_checks(),
        "all",
        "publish",
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    assert_eq!(plan["ready"], true);
    let applied = apply_plan(&client, &base_url, plan["id"].as_str().unwrap()).await;
    assert_eq!(applied.status(), StatusCode::OK);
    let application = applied.json::<Value>().await.unwrap();
    let application_id = application["id"].as_str().unwrap();
    let initial_run_id = application["latest_check_run"]["id"].as_str().unwrap();
    let initial = client
        .get(format!(
            "{base_url}/solution-packs/applications/{application_id}/checks/{initial_run_id}"
        ))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(initial["total_count"], 6);
    assert_eq!(initial["passed_count"], 6);
    assert!(!initial.to_string().contains("safe"));
    assert!(!initial.to_string().contains("unrelated"));

    sqlx::query("UPDATE workspaces SET extensions_enabled=false WHERE id=$1")
        .bind(workspace_id)
        .execute(&pool)
        .await
        .unwrap();
    let disabled = client
        .post(format!(
            "{base_url}/solution-packs/applications/{application_id}/checks"
        ))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let enabled_result = disabled["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["key"] == "checks/enabled")
        .unwrap();
    assert_eq!(enabled_result["passed"], false);
    assert_eq!(
        enabled_result["reason_code"],
        "workspace_extensions_disabled"
    );
    assert_eq!(disabled["failed_count"], 1);
    sqlx::query("UPDATE workspaces SET extensions_enabled=true WHERE id=$1")
        .bind(workspace_id)
        .execute(&pool)
        .await
        .unwrap();

    let original_navigation: Value =
        sqlx::query_scalar("SELECT settings->'explore_navigation' FROM workspaces WHERE id=$1")
            .bind(workspace_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("UPDATE workspaces SET settings=jsonb_set(settings,'{explore_navigation}','{}'::jsonb,true) WHERE id=$1")
        .bind(workspace_id).execute(&pool).await.unwrap();
    let malformed_navigation = client
        .post(format!(
            "{base_url}/solution-packs/applications/{application_id}/checks"
        ))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let navigation_result = malformed_navigation["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["key"] == "checks/navigation")
        .unwrap();
    assert_eq!(navigation_result["passed"], false);
    assert_eq!(
        navigation_result["reason_code"],
        "invalid_workspace_navigation"
    );
    sqlx::query("UPDATE workspaces SET settings=jsonb_set(settings,'{explore_navigation}',$2,true) WHERE id=$1")
        .bind(workspace_id).bind(original_navigation).execute(&pool).await.unwrap();

    sqlx::query("UPDATE extension_installations SET configuration=$2 WHERE workspace_id=$1 AND extension_id='acme.layout'")
        .bind(workspace_id)
        .bind(json!({"nested":{"enabled":false},"list":[1,2]}))
        .execute(&pool).await.unwrap();
    let mismatch = client
        .post(format!(
            "{base_url}/solution-packs/applications/{application_id}/checks"
        ))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let configuration_result = mismatch["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["key"] == "checks/configured")
        .unwrap();
    assert_eq!(configuration_result["passed"], false);
    assert_eq!(
        configuration_result["reason_code"],
        "configuration_mismatch"
    );

    let original_layout: Value =
        sqlx::query_scalar("SELECT settings->'extension_layout' FROM workspaces WHERE id=$1")
            .bind(workspace_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("UPDATE workspaces SET settings=jsonb_set(settings,'{extension_layout}','[]'::jsonb,true) WHERE id=$1")
        .bind(workspace_id).execute(&pool).await.unwrap();
    let malformed_layout = client
        .post(format!(
            "{base_url}/solution-packs/applications/{application_id}/checks"
        ))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let layout_result = malformed_layout["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["key"] == "checks/layout")
        .unwrap();
    assert_eq!(layout_result["passed"], false);
    assert_eq!(layout_result["reason_code"], "placement_mismatch");
    sqlx::query("UPDATE workspaces SET settings=jsonb_set(settings,'{extension_layout}',$2,true) WHERE id=$1")
        .bind(workspace_id).bind(original_layout).execute(&pool).await.unwrap();

    sqlx::query("UPDATE extension_installations SET state='quarantined' WHERE workspace_id=$1 AND extension_id='acme.layout'")
        .bind(workspace_id).execute(&pool).await.unwrap();
    let quarantined = client
        .post(format!(
            "{base_url}/solution-packs/applications/{application_id}/checks"
        ))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let installed_result = quarantined["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["key"] == "checks/installed")
        .unwrap();
    assert_eq!(installed_result["passed"], false);
    assert_eq!(installed_result["reason_code"], "quarantined");
    sqlx::query("UPDATE extension_installations SET state='enabled' WHERE workspace_id=$1 AND extension_id='acme.layout'")
        .bind(workspace_id).execute(&pool).await.unwrap();

    let incompatible_release_id = Uuid::new_v4();
    sqlx::query("INSERT INTO installed_extension_releases (id,workspace_id,extension_id,version,manifest,manifest_sha256,source) SELECT $1,workspace_id,extension_id,'2.0.0',jsonb_set(manifest,'{version}','\"2.0.0\"'::jsonb),$2,source FROM installed_extension_releases WHERE id=$3")
        .bind(incompatible_release_id).bind("9".repeat(64)).bind(release_id)
        .execute(&pool).await.unwrap();
    sqlx::query("UPDATE extension_installations SET installed_release_id=$2 WHERE workspace_id=$1 AND extension_id='acme.layout'")
        .bind(workspace_id).bind(incompatible_release_id).execute(&pool).await.unwrap();
    let incompatible = client
        .post(format!(
            "{base_url}/solution-packs/applications/{application_id}/checks"
        ))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let installed_result = incompatible["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["key"] == "checks/installed")
        .unwrap();
    assert_eq!(installed_result["passed"], false);
    assert_eq!(installed_result["reason_code"], "incompatible_version");
    sqlx::query("UPDATE extension_installations SET installed_release_id=$2 WHERE workspace_id=$1 AND extension_id='acme.layout'")
        .bind(workspace_id).bind(release_id).execute(&pool).await.unwrap();

    let runs_before: i64 =
        sqlx::query_scalar("SELECT count(*) FROM solution_pack_check_runs WHERE application_id=$1")
            .bind(application_id.parse::<Uuid>().unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    let results_before: i64 = sqlx::query_scalar("SELECT count(*) FROM solution_pack_check_results WHERE run_id IN (SELECT id FROM solution_pack_check_runs WHERE application_id=$1)")
        .bind(application_id.parse::<Uuid>().unwrap()).fetch_one(&pool).await.unwrap();
    let audits_before: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_events WHERE target->>'type'='solution_pack_check_run' AND target->>'application_id'=$1")
        .bind(application_id).fetch_one(&pool).await.unwrap();
    sqlx::query("UPDATE solution_pack_application_check_definitions SET predicate=$2 WHERE application_id=$1 AND position=0")
        .bind(application_id.parse::<Uuid>().unwrap())
        .bind(json!({"type":"unknown"}))
        .execute(&pool).await.unwrap();
    let failed = client
        .post(format!(
            "{base_url}/solution-packs/applications/{application_id}/checks"
        ))
        .send()
        .await
        .unwrap();
    assert!(!failed.status().is_success());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM solution_pack_check_runs WHERE application_id=$1"
        )
        .bind(application_id.parse::<Uuid>().unwrap())
        .fetch_one(&pool)
        .await
        .unwrap(),
        runs_before
    );
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM solution_pack_check_results WHERE run_id IN (SELECT id FROM solution_pack_check_runs WHERE application_id=$1)")
        .bind(application_id.parse::<Uuid>().unwrap()).fetch_one(&pool).await.unwrap(), results_before);
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM audit_events WHERE target->>'type'='solution_pack_check_run' AND target->>'application_id'=$1")
        .bind(application_id).fetch_one(&pool).await.unwrap(), audits_before);

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
async fn context_resources_are_rejected_by_inspection_and_planning(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let contexts_before: i64 = sqlx::query_scalar("SELECT count(*) FROM attribute_contexts")
        .fetch_one(&pool)
        .await
        .unwrap();
    let plans_before: i64 = sqlx::query_scalar("SELECT count(*) FROM solution_pack_plans")
        .fetch_one(&pool)
        .await
        .unwrap();

    let inspection = inspect(&client, &base_url, archive_with_rejected_context_resource()).await;
    assert_eq!(inspection.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        inspection.json::<Value>().await.unwrap()["error"]["code"],
        "invalid_input"
    );
    let plan = create_plan(
        &client,
        &base_url,
        archive_with_rejected_context_resource(),
        "rejected",
    )
    .await;
    assert_eq!(plan.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        plan.json::<Value>().await.unwrap()["error"]["code"],
        "invalid_input"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attribute_contexts")
            .fetch_one(&pool)
            .await
            .unwrap(),
        contexts_before
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM solution_pack_plans")
            .fetch_one(&pool)
            .await
            .unwrap(),
        plans_before
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
    let body: Value = serde_json::from_str(&body_text).unwrap();
    assert_eq!(
        body.as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from([
            "archive_sha256",
            "extensions",
            "guidance",
            "manifest",
            "resources",
            "sample_data",
            "warnings",
        ])
    );
    assert_eq!(body["guidance"]["setup_checklist_items"], 0);
    assert_eq!(body["guidance"]["checks"], json!([]));
    assert_eq!(body["archive_sha256"], archive_sha256);
    assert_eq!(body["manifest"]["id"], "attricat.ecommerce");
    assert_eq!(body["manifest"]["version"], "1.2.0");
    assert_eq!(
        body["resources"]["blueprints"][0]["key"],
        "blueprints/product"
    );
    assert_eq!(body["resources"]["blueprints"][0]["code"], "product");
    assert_eq!(body["resources"]["blueprints"][0]["includes"], json!([]));
    assert!(body["resources"].get("contexts").is_none());
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
            "SELECT count(*) FROM blueprints WHERE workspace_id=$1 AND code='extok_product'"
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
    assert_eq!(body["mappings"][0]["logical_key"], "blueprints/product");
    assert_eq!(body["mappings"][0]["target_code"], "ecom_product");

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
async fn explicit_prior_application_classifies_and_persists_later_release_evidence(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let prior_archive = release_archive(
        "attricat.release-test",
        "1.0.0",
        &[
            ("blueprints/product", PRODUCT_BLUEPRINT),
            ("blueprints/category", CATEGORY_BLUEPRINT),
            ("blueprints/legacy", LEGACY_BLUEPRINT),
        ],
    );
    let prior_plan = create_plan_with_publication(
        &client,
        &base_url,
        prior_archive.clone(),
        "release",
        "publish",
    )
    .await;
    assert_eq!(prior_plan.status(), StatusCode::CREATED);
    let prior_plan = prior_plan.json::<Value>().await.unwrap();
    let prior_application =
        apply_plan(&client, &base_url, prior_plan["id"].as_str().unwrap()).await;
    assert_eq!(prior_application.status(), StatusCode::OK);
    let prior_application = prior_application.json::<Value>().await.unwrap();
    let prior_application_id = prior_application["id"]
        .as_str()
        .unwrap()
        .parse::<Uuid>()
        .unwrap();

    let changed_archive = release_archive(
        "attricat.release-test",
        "1.1.0",
        &[
            ("blueprints/product", PRODUCT_BLUEPRINT),
            ("blueprints/category", CHANGED_CATEGORY_BLUEPRINT),
            ("blueprints/accessory", ACCESSORY_BLUEPRINT),
        ],
    );
    let changed = create_plan_from_application(
        &client,
        &base_url,
        changed_archive,
        "release_next",
        "publish",
        prior_application_id,
    )
    .await;
    assert_eq!(changed.status(), StatusCode::CREATED);
    let changed = changed.json::<Value>().await.unwrap();
    assert_eq!(
        changed["prior_application_id"],
        prior_application_id.to_string()
    );
    assert_eq!(changed["ready"], false);
    assert_eq!(
        changed["release_changes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|change| (
                change["logical_key"].as_str().unwrap(),
                change["change_kind"].as_str().unwrap(),
                change["reason_code"].as_str().unwrap(),
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "blueprints/product",
                "unchanged",
                "unchanged_from_prior_application",
            ),
            ("blueprints/category", "changed", "update_not_supported",),
            ("blueprints/accessory", "added", "new_blueprint"),
            ("blueprints/legacy", "removed", "removed_from_release",),
        ]
    );
    assert_eq!(
        changed["actions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|action| action["logical_key"] == "blueprints/product")
            .unwrap()["action"],
        "map"
    );
    assert_eq!(
        changed["actions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|action| action["logical_key"] == "blueprints/category")
            .unwrap()["reason_code"],
        "update_not_supported"
    );
    assert!(
        changed["actions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|action| action["logical_key"] != "blueprints/legacy")
    );
    let changed_plan_id = changed["id"].as_str().unwrap().parse::<Uuid>().unwrap();
    sqlx::query("UPDATE solution_pack_plans SET ready=true WHERE id=$1")
        .bind(changed_plan_id)
        .execute(&pool)
        .await
        .unwrap();
    let forged_ready = apply_plan(&client, &base_url, &changed_plan_id.to_string()).await;
    assert_eq!(forged_ready.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM solution_pack_applications WHERE plan_id=$1"
        )
        .bind(changed_plan_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM blueprints WHERE code='release_next_accessory'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );

    let compatible_archive = release_archive(
        "attricat.release-test",
        "1.1.0",
        &[
            ("blueprints/product", PRODUCT_BLUEPRINT),
            ("blueprints/category", CATEGORY_BLUEPRINT),
            ("blueprints/accessory", ACCESSORY_BLUEPRINT),
        ],
    );
    let compatible = create_plan_from_application(
        &client,
        &base_url,
        compatible_archive.clone(),
        "release_next",
        "publish",
        prior_application_id,
    )
    .await;
    assert_eq!(compatible.status(), StatusCode::CREATED);
    let compatible = compatible.json::<Value>().await.unwrap();
    assert_eq!(compatible["ready"], true);
    let application = apply_plan(&client, &base_url, compatible["id"].as_str().unwrap()).await;
    assert_eq!(application.status(), StatusCode::OK);
    let application = application.json::<Value>().await.unwrap();
    assert_eq!(
        application["prior_application_id"],
        prior_application_id.to_string()
    );
    assert_eq!(
        application["release_change_snapshot"],
        compatible["release_changes"]
    );
    assert_eq!(application["steps"].as_array().unwrap().len(), 3);

    let create_only_lineage = create_plan_from_application(
        &client,
        &base_url,
        release_archive(
            "attricat.release-test",
            "1.2.0",
            &[("blueprints/accessory", ACCESSORY_BLUEPRINT)],
        ),
        "release_create_only",
        "publish",
        prior_application_id,
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    assert_eq!(create_only_lineage["ready"], true);
    assert!(
        create_only_lineage["mappings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|mapping| mapping["mapping_kind"] == "create")
    );
    let create_only_plan_id = create_only_lineage["id"]
        .as_str()
        .unwrap()
        .parse::<Uuid>()
        .unwrap();
    sqlx::query("UPDATE solution_pack_plans SET resource_evidence_format=NULL,resource_evidence_sha256=NULL WHERE id=$1")
        .bind(create_only_plan_id)
        .execute(&pool)
        .await
        .unwrap();
    let downgraded = apply_plan(&client, &base_url, &create_only_plan_id.to_string()).await;
    assert_eq!(downgraded.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM solution_pack_applications WHERE plan_id=$1"
        )
        .bind(create_only_plan_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );

    let tampered = create_plan_from_application(
        &client,
        &base_url,
        compatible_archive.clone(),
        "release_tampered",
        "publish",
        prior_application_id,
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    let tampered_plan_id = tampered["id"].as_str().unwrap().parse::<Uuid>().unwrap();
    sqlx::query("UPDATE solution_pack_plan_release_changes SET reason_code='tampered_evidence' WHERE plan_id=$1 AND position=0")
        .bind(tampered_plan_id)
        .execute(&pool)
        .await
        .unwrap();
    let rejected = apply_plan(&client, &base_url, &tampered_plan_id.to_string()).await;
    assert_eq!(rejected.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        rejected.json::<Value>().await.unwrap()["error"]["code"],
        "invalid_input"
    );

    let equal_release = create_plan_from_application(
        &client,
        &base_url,
        prior_archive,
        "release_equal",
        "publish",
        prior_application_id,
    )
    .await;
    assert_eq!(equal_release.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        equal_release.json::<Value>().await.unwrap()["error"]["code"],
        "invalid_input"
    );
    for version in ["0.9.0", "1.0.0+different-build"] {
        let non_increasing = create_plan_from_application(
            &client,
            &base_url,
            release_archive(
                "attricat.release-test",
                version,
                &[("blueprints/product", PRODUCT_BLUEPRINT)],
            ),
            "release_non_increasing",
            "publish",
            prior_application_id,
        )
        .await;
        assert_eq!(non_increasing.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
    let missing = create_plan_from_application(
        &client,
        &base_url,
        compatible_archive.clone(),
        "release_missing",
        "publish",
        Uuid::new_v4(),
    )
    .await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    let malformed = client
        .post(format!(
            "{base_url}/solution-packs/plans?prefix=release_malformed&blueprint_publication=publish&from_application=not-a-uuid"
        ))
        .header("content-type", "application/zstd")
        .body(compatible_archive.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        malformed.json::<Value>().await.unwrap()["error"]["code"],
        "bad_request"
    );
    let wrong_pack = create_plan_from_application(
        &client,
        &base_url,
        release_archive(
            "attricat.different-pack",
            "2.0.0",
            &[("blueprints/product", PRODUCT_BLUEPRINT)],
        ),
        "release_wrong_pack",
        "publish",
        prior_application_id,
    )
    .await;
    assert_eq!(wrong_pack.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let mixed = client
        .post(format!(
            "{base_url}/solution-packs/plans?prefix=release_mixed&blueprint_publication=publish&from_application={prior_application_id}"
        ))
        .multipart(
            reqwest::multipart::Form::new()
                .part(
                    "archive",
                    reqwest::multipart::Part::bytes(compatible_archive.clone())
                        .mime_str("application/zstd")
                        .unwrap(),
                )
                .text(
                    "blueprint_map",
                    json!({"key":"blueprints/product","code":"release_product"}).to_string(),
                ),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(mixed.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let prior_product_id = compatible["release_changes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|change| change["logical_key"] == "blueprints/product")
        .unwrap()["prior_target_id"]
        .as_str()
        .unwrap();
    let revision_definition = std::str::from_utf8(PRODUCT_BLUEPRINT)
        .unwrap()
        .replace("code = \"product\"", "code = \"release_product\"");
    let revision = client
        .post(format!("{base_url}/blueprints/{prior_product_id}/versions"))
        .json(&json!({"definition":revision_definition}))
        .send()
        .await
        .unwrap();
    assert_eq!(revision.status(), StatusCode::CREATED);
    let published = client
        .post(format!(
            "{base_url}/blueprints/{prior_product_id}/versions/2/publish"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(published.status(), StatusCode::OK);
    let drifted = create_plan_from_application(
        &client,
        &base_url,
        release_archive(
            "attricat.release-test",
            "1.2.0",
            &[("blueprints/product", PRODUCT_BLUEPRINT)],
        ),
        "release_drifted",
        "publish",
        prior_application_id,
    )
    .await;
    assert_eq!(drifted.status(), StatusCode::CREATED);
    let drifted = drifted.json::<Value>().await.unwrap();
    assert_eq!(drifted["ready"], false);
    assert_eq!(
        drifted["release_changes"][0]["reason_code"],
        "prior_target_revision_drifted"
    );

    assert_eq!(
        sqlx::query_scalar::<_, Option<Uuid>>(
            "SELECT prior_application_id FROM solution_pack_applications WHERE id=$1"
        )
        .bind(application["id"].as_str().unwrap().parse::<Uuid>().unwrap())
        .fetch_one(&pool)
        .await
        .unwrap(),
        Some(prior_application_id)
    );

    let prior_plan_id = prior_plan["id"].as_str().unwrap().parse::<Uuid>().unwrap();
    sqlx::query("UPDATE solution_pack_plan_actions SET summary=jsonb_set(summary,'{tampered}','true'::jsonb) WHERE plan_id=$1 AND logical_key='blueprints/product'")
        .bind(prior_plan_id)
        .execute(&pool)
        .await
        .unwrap();
    let forged_prior = create_plan_from_application(
        &client,
        &base_url,
        release_archive(
            "attricat.release-test",
            "1.3.0",
            &[("blueprints/product", PRODUCT_BLUEPRINT)],
        ),
        "release_forged_prior",
        "publish",
        prior_application_id,
    )
    .await;
    assert_eq!(forged_prior.status(), StatusCode::UNPROCESSABLE_ENTITY);

    sqlx::query(
        "UPDATE solution_pack_applications SET state='running',completed_at=NULL WHERE id=$1",
    )
    .bind(prior_application_id)
    .execute(&pool)
    .await
    .unwrap();
    let incomplete_prior = create_plan_from_application(
        &client,
        &base_url,
        release_archive(
            "attricat.release-test",
            "1.4.0",
            &[("blueprints/product", PRODUCT_BLUEPRINT)],
        ),
        "release_incomplete_prior",
        "publish",
        prior_application_id,
    )
    .await;
    assert_eq!(incomplete_prior.status(), StatusCode::UNPROCESSABLE_ENTITY);
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn draft_created_prior_blueprint_is_not_reused_by_a_later_release(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let prior = release_archive(
        "attricat.draft-release-test",
        "1.0.0",
        &[("blueprints/product", PRODUCT_BLUEPRINT)],
    );
    let plan = create_plan(&client, &base_url, prior, "draft_release").await;
    let plan = plan.json::<Value>().await.unwrap();
    let application = apply_plan(&client, &base_url, plan["id"].as_str().unwrap()).await;
    assert_eq!(application.status(), StatusCode::OK);
    let application_id = application.json::<Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .parse::<Uuid>()
        .unwrap();
    let later = release_archive(
        "attricat.draft-release-test",
        "1.1.0",
        &[("blueprints/product", PRODUCT_BLUEPRINT)],
    );
    let response = create_plan_from_application(
        &client,
        &base_url,
        later,
        "draft_release_next",
        "publish",
        application_id,
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let plan = response.json::<Value>().await.unwrap();
    assert_eq!(plan["ready"], false);
    assert_eq!(plan["release_changes"][0]["change_kind"], "unchanged");
    assert_eq!(
        plan["release_changes"][0]["reason_code"],
        "prior_target_unpublished"
    );
    assert_eq!(plan["actions"][0]["action"], "conflict");
    assert_eq!(
        plan["actions"][0]["reason_code"],
        "prior_target_unpublished"
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn later_release_reports_missing_deleted_and_hash_drifted_prior_targets(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let prior = release_archive(
        "attricat.target-drift-test",
        "1.0.0",
        &[
            ("blueprints/product", PRODUCT_BLUEPRINT),
            ("blueprints/category", CATEGORY_BLUEPRINT),
            ("blueprints/legacy", LEGACY_BLUEPRINT),
        ],
    );
    let plan = create_plan_with_publication(&client, &base_url, prior, "lineage_drift", "publish")
        .await
        .json::<Value>()
        .await
        .unwrap();
    let applied = apply_plan(&client, &base_url, plan["id"].as_str().unwrap()).await;
    assert_eq!(applied.status(), StatusCode::OK);
    let application_id = applied.json::<Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .parse::<Uuid>()
        .unwrap();

    sqlx::query("DELETE FROM attributes WHERE workspace_id=$1 AND blueprint_id=(SELECT id FROM blueprints WHERE workspace_id=$1 AND code='lineage_drift_product')")
        .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM blueprints WHERE workspace_id=$1 AND code='lineage_drift_product'")
        .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE blueprints SET deleted_at=clock_timestamp() WHERE workspace_id=$1 AND code='lineage_drift_category'")
        .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let drifted_definition = std::str::from_utf8(LEGACY_BLUEPRINT)
        .unwrap()
        .replace("code = \"legacy\"", "code = \"lineage_drift_legacy\"")
        .replace("name = \"Legacy\"", "name = \"Drifted legacy\"");
    sqlx::query(
        "UPDATE blueprints SET definition=$2 WHERE workspace_id=$1 AND code='lineage_drift_legacy'",
    )
    .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
    .bind(drifted_definition)
    .execute(&pool)
    .await
    .unwrap();

    let later = release_archive(
        "attricat.target-drift-test",
        "1.1.0",
        &[
            ("blueprints/product", PRODUCT_BLUEPRINT),
            ("blueprints/category", CATEGORY_BLUEPRINT),
            ("blueprints/legacy", LEGACY_BLUEPRINT),
        ],
    );
    let response = create_plan_from_application(
        &client,
        &base_url,
        later,
        "lineage_drift_next",
        "publish",
        application_id,
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let plan = response.json::<Value>().await.unwrap();
    assert_eq!(plan["ready"], false);
    assert_eq!(
        plan["release_changes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|change| change["reason_code"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec![
            "prior_target_missing",
            "prior_target_deleted",
            "prior_target_drifted",
        ]
    );
    assert!(
        plan["actions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|action| action["action"] == "conflict")
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
async fn apply_joins_actions_to_mappings_by_logical_key_not_position(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let plan = create_plan(
        &client,
        &base_url,
        archive_with_reverse_order_dependency(),
        "ordered",
    )
    .await;
    assert_eq!(plan.status(), StatusCode::CREATED);
    let plan = plan.json::<Value>().await.unwrap();
    let plan_id = plan["id"].as_str().unwrap().parse::<Uuid>().unwrap();
    assert_eq!(
        plan["actions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|action| action["logical_key"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["blueprints/category", "blueprints/product"]
    );
    // Simulate a legacy create-only plan, which predates the versioned digest.
    // Its mappings preserve an independent position space; reverse that space
    // to prove legacy apply joins evidence by logical key.
    sqlx::query("UPDATE solution_pack_plans SET resource_evidence_format=NULL,resource_evidence_sha256=NULL WHERE id=$1")
        .bind(plan_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE solution_pack_plan_mappings SET position=position+10 WHERE plan_id=$1")
        .bind(plan_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE solution_pack_plan_mappings SET position=11-position WHERE plan_id=$1")
        .bind(plan_id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        apply_plan(&client, &base_url, plan["id"].as_str().unwrap())
            .await
            .status(),
        StatusCode::OK
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn mapped_plan_digest_rejects_kind_downgrade_and_resource_row_removal(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let source =
        create_plan_with_publication(&client, &base_url, valid_archive(), "shared", "publish")
            .await
            .json::<Value>()
            .await
            .unwrap();
    assert_eq!(
        apply_plan(&client, &base_url, source["id"].as_str().unwrap())
            .await
            .status(),
        StatusCode::OK
    );
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let blueprint_count_before =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM blueprints WHERE workspace_id=$1")
            .bind(workspace_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    let downgraded = create_plan_with_maps(
        &client,
        &base_url,
        valid_archive(),
        "shared",
        "publish",
        &[("blueprints/product", "shared_product")],
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    let downgraded_id = downgraded["id"].as_str().unwrap().parse::<Uuid>().unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, Option<String>>(
            "SELECT resource_evidence_sha256 FROM solution_pack_plans WHERE id=$1"
        )
        .bind(downgraded_id)
        .fetch_one(&pool)
        .await
        .unwrap()
        .map(|digest| digest.len()),
        Some(64)
    );
    let create_payload = json!({
        "definition": std::str::from_utf8(PRODUCT_BLUEPRINT)
            .unwrap()
            .replace("code = \"product\"", "code = \"shared_product\""),
        "version": 1,
        "publication": "publish",
        "extension_contributions": []
    });
    sqlx::query("UPDATE solution_pack_plan_mappings SET mapping_kind='create' WHERE plan_id=$1 AND logical_key='blueprints/product'")
        .bind(downgraded_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE solution_pack_plan_actions SET action='create',reason_code='target_absent',normalized_payload=$2,preconditions=$3 WHERE plan_id=$1 AND logical_key='blueprints/product'")
        .bind(downgraded_id)
        .bind(create_payload)
        .bind(json!([{"kind":"target_absent","resource_kind":"blueprint","code":"shared_product"}]))
        .execute(&pool)
        .await
        .unwrap();
    let response = apply_plan(&client, &base_url, &downgraded_id.to_string()).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "invalid_input"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM solution_pack_applications WHERE plan_id=$1"
        )
        .bind(downgraded_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );

    let missing_digest = create_plan_with_maps(
        &client,
        &base_url,
        valid_archive(),
        "shared",
        "publish",
        &[("blueprints/product", "shared_product")],
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    let missing_digest_id = missing_digest["id"]
        .as_str()
        .unwrap()
        .parse::<Uuid>()
        .unwrap();
    sqlx::query("UPDATE solution_pack_plans SET resource_evidence_format=NULL,resource_evidence_sha256=NULL WHERE id=$1")
        .bind(missing_digest_id)
        .execute(&pool)
        .await
        .unwrap();
    let response = apply_plan(&client, &base_url, &missing_digest_id.to_string()).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "invalid_input"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM solution_pack_applications WHERE plan_id=$1"
        )
        .bind(missing_digest_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );

    let removed = create_plan_with_maps(
        &client,
        &base_url,
        valid_archive(),
        "shared",
        "publish",
        &[("blueprints/product", "shared_product")],
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    let removed_id = removed["id"].as_str().unwrap().parse::<Uuid>().unwrap();
    sqlx::query("DELETE FROM solution_pack_plan_actions WHERE plan_id=$1")
        .bind(removed_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM solution_pack_plan_mappings WHERE plan_id=$1")
        .bind(removed_id)
        .execute(&pool)
        .await
        .unwrap();
    let response = apply_plan(&client, &base_url, &removed_id.to_string()).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "invalid_input"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM solution_pack_applications WHERE plan_id=$1"
        )
        .bind(removed_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM blueprints WHERE workspace_id=$1")
            .bind(workspace_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        blueprint_count_before
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn legacy_create_only_plan_without_resource_digest_still_applies(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let plan = create_plan(&client, &base_url, valid_archive(), "legacy").await;
    assert_eq!(plan.status(), StatusCode::CREATED);
    let plan = plan.json::<Value>().await.unwrap();
    let plan_id = plan["id"].as_str().unwrap().parse::<Uuid>().unwrap();
    assert_eq!(
        sqlx::query_as::<_, (Option<String>, Option<i16>)>(
            "SELECT resource_evidence_sha256,resource_evidence_format FROM solution_pack_plans WHERE id=$1"
        )
        .bind(plan_id)
        .fetch_one(&pool)
        .await
        .unwrap()
        .1,
        Some(2)
    );
    sqlx::query("UPDATE solution_pack_plans SET resource_evidence_format=NULL,resource_evidence_sha256=NULL WHERE id=$1")
        .bind(plan_id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        apply_plan(&client, &base_url, &plan_id.to_string())
            .await
            .status(),
        StatusCode::OK
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn digestless_completed_application_cannot_be_a_lineage_source(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let prior = create_plan_with_publication(
        &client,
        &base_url,
        valid_archive(),
        "digestless_source",
        "publish",
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    let prior_plan_id = prior["id"].as_str().unwrap().parse::<Uuid>().unwrap();
    let applied = apply_plan(&client, &base_url, prior["id"].as_str().unwrap()).await;
    assert_eq!(applied.status(), StatusCode::OK);
    let prior_application_id = applied.json::<Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .parse::<Uuid>()
        .unwrap();
    sqlx::query("UPDATE solution_pack_plans SET resource_evidence_format=NULL,resource_evidence_sha256=NULL WHERE id=$1")
        .bind(prior_plan_id)
        .execute(&pool)
        .await
        .unwrap();
    let plan_count: i64 = sqlx::query_scalar("SELECT count(*) FROM solution_pack_plans")
        .fetch_one(&pool)
        .await
        .unwrap();
    let blueprint_count: i64 = sqlx::query_scalar("SELECT count(*) FROM blueprints")
        .fetch_one(&pool)
        .await
        .unwrap();

    let response = create_plan_from_application(
        &client,
        &base_url,
        release_archive(
            "attricat.ecommerce",
            "1.3.0",
            &[("blueprints/product", PRODUCT_BLUEPRINT)],
        ),
        "digestless_later",
        "publish",
        prior_application_id,
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "invalid_input"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM solution_pack_plans")
            .fetch_one(&pool)
            .await
            .unwrap(),
        plan_count
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM blueprints")
            .fetch_one(&pool)
            .await
            .unwrap(),
        blueprint_count
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn later_release_reuses_target_from_prior_explicit_mapping_without_mutation(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let seed = create_plan_with_publication(
        &client,
        &base_url,
        release_archive(
            "attricat.mapping-seed",
            "1.0.0",
            &[("blueprints/product", PRODUCT_BLUEPRINT)],
        ),
        "mapped_target",
        "publish",
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    let target_id = seed["mappings"][0]["target_id"]
        .as_str()
        .unwrap()
        .parse::<Uuid>()
        .unwrap();
    assert_eq!(
        apply_plan(&client, &base_url, seed["id"].as_str().unwrap())
            .await
            .status(),
        StatusCode::OK
    );

    let mapped = create_plan_with_maps(
        &client,
        &base_url,
        release_archive(
            "attricat.mapped-lineage",
            "1.0.0",
            &[("blueprints/product", PRODUCT_BLUEPRINT)],
        ),
        "mapped_v1",
        "publish",
        &[("blueprints/product", "mapped_target_product")],
    )
    .await;
    assert_eq!(mapped.status(), StatusCode::CREATED);
    let mapped = mapped.json::<Value>().await.unwrap();
    assert_eq!(mapped["mappings"][0]["mapping_kind"], "existing");
    assert_eq!(mapped["mappings"][0]["target_id"], target_id.to_string());
    let mapped_application = apply_plan(&client, &base_url, mapped["id"].as_str().unwrap()).await;
    assert_eq!(mapped_application.status(), StatusCode::OK);
    let mapped_application = mapped_application.json::<Value>().await.unwrap();
    let mapped_application_id = mapped_application["id"]
        .as_str()
        .unwrap()
        .parse::<Uuid>()
        .unwrap();
    assert_eq!(
        mapped_application["steps"][0]["result_snapshot"]["outcome"],
        "reused"
    );

    let blueprint_count: i64 = sqlx::query_scalar("SELECT count(*) FROM blueprints")
        .fetch_one(&pool)
        .await
        .unwrap();
    let later = create_plan_from_application(
        &client,
        &base_url,
        release_archive(
            "attricat.mapped-lineage",
            "1.1.0",
            &[("blueprints/product", PRODUCT_BLUEPRINT)],
        ),
        "mapped_v2",
        "publish",
        mapped_application_id,
    )
    .await;
    assert_eq!(later.status(), StatusCode::CREATED);
    let later = later.json::<Value>().await.unwrap();
    assert_eq!(later["ready"], true);
    assert_eq!(
        later["prior_application_id"],
        mapped_application_id.to_string()
    );
    assert_eq!(later["release_changes"][0]["change_kind"], "unchanged");
    assert_eq!(
        later["release_changes"][0]["reason_code"],
        "unchanged_from_prior_application"
    );
    assert_eq!(
        later["release_changes"][0]["prior_target_id"],
        target_id.to_string()
    );
    assert_eq!(
        later["release_changes"][0]["prior_target_code"],
        "mapped_target_product"
    );
    assert_eq!(later["release_changes"][0]["prior_target_version"], 1);
    assert_eq!(later["mappings"][0]["mapping_kind"], "existing");
    assert_eq!(later["mappings"][0]["target_id"], target_id.to_string());
    assert_eq!(later["mappings"][0]["target_code"], "mapped_target_product");
    assert_eq!(later["mappings"][0]["target_version"], 1);
    assert_eq!(later["actions"][0]["action"], "map");

    let later_application = apply_plan(&client, &base_url, later["id"].as_str().unwrap()).await;
    assert_eq!(later_application.status(), StatusCode::OK);
    let later_application = later_application.json::<Value>().await.unwrap();
    assert_eq!(
        later_application["prior_application_id"],
        mapped_application_id.to_string()
    );
    assert_eq!(
        later_application["release_change_snapshot"],
        later["release_changes"]
    );
    assert_eq!(
        later_application["steps"][0]["result_snapshot"]["outcome"],
        "reused"
    );
    assert_eq!(
        later_application["steps"][0]["result_snapshot"]["id"],
        target_id.to_string()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM blueprints")
            .fetch_one(&pool)
            .await
            .unwrap(),
        blueprint_count
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn explicit_exact_blueprint_mapping_reuses_without_mutation_and_revalidates(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let first =
        create_plan_with_publication(&client, &base_url, valid_archive(), "shared", "publish")
            .await;
    assert_eq!(first.status(), StatusCode::CREATED);
    let first = first.json::<Value>().await.unwrap();
    let applied = apply_plan(&client, &base_url, first["id"].as_str().unwrap()).await;
    assert_eq!(applied.status(), StatusCode::OK);

    let differently_formatted = format!(
        "# administrator-authored formatting\n\n{}\n# another harmless comment\n",
        std::str::from_utf8(PRODUCT_BLUEPRINT)
            .unwrap()
            .replace("code = \"product\"", "code = \"semantic_product\"")
    );
    let semantic_created = client
        .post(format!("{base_url}/blueprints"))
        .json(&json!({"definition": differently_formatted}))
        .send()
        .await
        .unwrap();
    assert_eq!(semantic_created.status(), StatusCode::CREATED);
    let semantic_created = semantic_created.json::<Value>().await.unwrap();
    let semantic_id = semantic_created["blueprint"]["id"].as_str().unwrap();
    let semantic_published = client
        .post(format!(
            "{base_url}/blueprints/{semantic_id}/versions/1/publish"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(semantic_published.status(), StatusCode::OK);
    let semantic = create_plan_with_maps(
        &client,
        &base_url,
        valid_archive(),
        "semantic",
        "publish",
        &[("blueprints/product", "semantic_product")],
    )
    .await;
    assert_eq!(semantic.status(), StatusCode::CREATED);
    let semantic = semantic.json::<Value>().await.unwrap();
    assert_eq!(semantic["ready"], true);
    assert_eq!(semantic["actions"][0]["action"], "map");

    let draft_only = create_plan(&client, &base_url, valid_archive(), "draftonly")
        .await
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(
        apply_plan(&client, &base_url, draft_only["id"].as_str().unwrap())
            .await
            .status(),
        StatusCode::OK
    );
    let unpublished = create_plan_with_maps(
        &client,
        &base_url,
        valid_archive(),
        "draftonly",
        "draft",
        &[("blueprints/product", "draftonly_product")],
    )
    .await;
    assert_eq!(unpublished.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let deleted_plan =
        create_plan_with_publication(&client, &base_url, valid_archive(), "deleted", "publish")
            .await
            .json::<Value>()
            .await
            .unwrap();
    assert_eq!(
        apply_plan(&client, &base_url, deleted_plan["id"].as_str().unwrap())
            .await
            .status(),
        StatusCode::OK
    );
    sqlx::query("UPDATE blueprints SET deleted_at=clock_timestamp() WHERE workspace_id=$1 AND code='deleted_product'")
        .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let deleted = create_plan_with_maps(
        &client,
        &base_url,
        valid_archive(),
        "deleted",
        "publish",
        &[("blueprints/product", "deleted_product")],
    )
    .await;
    assert_eq!(deleted.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let foreign_workspace = Uuid::new_v4();
    sqlx::query("INSERT INTO workspaces (id,slug,name,login_identifier) VALUES ($1,$2,'Foreign mapping',$3)")
        .bind(foreign_workspace)
        .bind(format!("foreign-map-{foreign_workspace}"))
        .bind(format!("foreign-map-{foreign_workspace}.example.test"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO blueprints (id,workspace_id,code,name,kind,version,includes,views,entity_schema,status,published_at,definition,definition_hash) SELECT $1,$2,'foreign_product',name,kind,version,includes,views,entity_schema,status,published_at,definition,definition_hash FROM blueprints WHERE workspace_id=$3 AND code='shared_product' AND version=1")
        .bind(Uuid::new_v4())
        .bind(foreign_workspace)
        .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let foreign = create_plan_with_maps(
        &client,
        &base_url,
        valid_archive(),
        "foreign",
        "publish",
        &[("blueprints/product", "foreign_product")],
    )
    .await;
    assert_eq!(foreign.status(), StatusCode::UNPROCESSABLE_ENTITY);

    for invalid_maps in [
        vec![
            ("blueprints/product", "shared_product"),
            ("blueprints/product", "shared_product"),
        ],
        vec![("blueprints/unknown", "shared_product")],
        vec![("blueprints/product", "Unsafe-code")],
    ] {
        let invalid = create_plan_with_maps(
            &client,
            &base_url,
            valid_archive(),
            "shared",
            "publish",
            &invalid_maps,
        )
        .await;
        assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
    let plan_url =
        format!("{base_url}/solution-packs/plans?prefix=shared&blueprint_publication=publish");
    let malformed = client
        .post(&plan_url)
        .multipart(reqwest::multipart::Form::new().text("unexpected", "value"))
        .send()
        .await
        .unwrap();
    assert_eq!(malformed.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let missing_archive = client
        .post(&plan_url)
        .multipart(reqwest::multipart::Form::new().text(
            "blueprint_map",
            json!({"key":"blueprints/product","code":"shared_product"}).to_string(),
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(missing_archive.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let wrong_archive_type = client
        .post(&plan_url)
        .multipart(
            reqwest::multipart::Form::new().part(
                "archive",
                reqwest::multipart::Part::bytes(valid_archive())
                    .mime_str("application/octet-stream")
                    .unwrap(),
            ),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(
        wrong_archive_type.status(),
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
    let duplicate_archive = client
        .post(&plan_url)
        .multipart(
            reqwest::multipart::Form::new()
                .part(
                    "archive",
                    reqwest::multipart::Part::bytes(valid_archive())
                        .mime_str("application/zstd")
                        .unwrap(),
                )
                .part(
                    "archive",
                    reqwest::multipart::Part::bytes(valid_archive())
                        .mime_str("application/zstd")
                        .unwrap(),
                ),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(duplicate_archive.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let oversized_mapping = client
        .post(&plan_url)
        .multipart(
            reqwest::multipart::Form::new()
                .part(
                    "archive",
                    reqwest::multipart::Part::bytes(valid_archive())
                        .mime_str("application/zstd")
                        .unwrap(),
                )
                .text("blueprint_map", "x".repeat(64 * 1024 + 1)),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(oversized_mapping.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        oversized_mapping.json::<Value>().await.unwrap()["error"]["code"],
        "invalid_input"
    );
    let mut too_many_form = reqwest::multipart::Form::new().part(
        "archive",
        reqwest::multipart::Part::bytes(valid_archive())
            .mime_str("application/zstd")
            .unwrap(),
    );
    for index in 0..=64 {
        too_many_form = too_many_form.text(
            "blueprint_map",
            json!({"key":format!("blueprints/{index}"),"code":format!("code_{index}")}).to_string(),
        );
    }
    let too_many = client
        .post(&plan_url)
        .multipart(too_many_form)
        .send()
        .await
        .unwrap();
    assert_eq!(too_many.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let oversized_archive = client
        .post(&plan_url)
        .multipart(
            reqwest::multipart::Form::new().part(
                "archive",
                reqwest::multipart::Part::bytes(vec![0; 32 * 1024 * 1024 + 1])
                    .mime_str("application/zstd")
                    .unwrap(),
            ),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(oversized_archive.status(), StatusCode::PAYLOAD_TOO_LARGE);

    let before_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM blueprints WHERE workspace_id=$1 AND code='shared_product'",
    )
    .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    let mapped = create_plan_with_maps(
        &client,
        &base_url,
        valid_archive(),
        "shared",
        "publish",
        &[("blueprints/product", "shared_product")],
    )
    .await;
    assert_eq!(mapped.status(), StatusCode::CREATED);
    let mapped = mapped.json::<Value>().await.unwrap();
    assert_eq!(mapped["ready"], true);
    assert_eq!(mapped["mappings"][0]["mapping_kind"], "existing");
    assert_eq!(mapped["mappings"][0]["target_version"], 1);
    assert_eq!(mapped["mappings"][0]["snapshot"]["status"], "published");
    assert_eq!(mapped["actions"][0]["action"], "map");
    assert_eq!(mapped["actions"][0]["reason_code"], "exact_blueprint_match");

    let application = apply_plan(&client, &base_url, mapped["id"].as_str().unwrap()).await;
    assert_eq!(application.status(), StatusCode::OK);
    let application = application.json::<Value>().await.unwrap();
    assert_eq!(
        application["steps"][0]["result_snapshot"]["outcome"],
        "reused"
    );
    assert_eq!(
        application["steps"][0]["result_snapshot"]["status"],
        "published"
    );
    let after_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM blueprints WHERE workspace_id=$1 AND code='shared_product'",
    )
    .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after_count, before_count);

    let distinct_pack = create_plan_with_maps(
        &client,
        &base_url,
        archive_with_pack_id(PRODUCT_BLUEPRINT, "attricat.distinct-pack"),
        "shared",
        "publish",
        &[("blueprints/product", "shared_product")],
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    assert_eq!(distinct_pack["ready"], true);
    assert_eq!(
        apply_plan(&client, &base_url, distinct_pack["id"].as_str().unwrap())
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM blueprints WHERE workspace_id=$1 AND code='shared_product'"
        )
        .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
        .fetch_one(&pool)
        .await
        .unwrap(),
        before_count
    );

    let shared_id = first["mappings"][0]["target_id"].as_str().unwrap();
    let revision_definition = std::str::from_utf8(PRODUCT_BLUEPRINT)
        .unwrap()
        .replace("code = \"product\"", "code = \"shared_product\"");
    let revision = client
        .post(format!("{base_url}/blueprints/{shared_id}/versions"))
        .json(&json!({"definition":revision_definition}))
        .send()
        .await
        .unwrap();
    assert_eq!(revision.status(), StatusCode::CREATED);
    let racing_plan = create_plan_with_maps(
        &client,
        &base_url,
        valid_archive(),
        "shared",
        "publish",
        &[("blueprints/product", "shared_product")],
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    let mut publication = pool.begin().await.unwrap();
    sqlx::query("SELECT version FROM blueprints WHERE workspace_id=$1 AND code='shared_product' AND version=2 FOR UPDATE")
        .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
        .fetch_one(&mut *publication)
        .await
        .unwrap();
    let racing_client = client.clone();
    let racing_url = base_url.clone();
    let racing_plan_id = racing_plan["id"].as_str().unwrap().to_owned();
    let racing_apply =
        tokio::spawn(async move { apply_plan(&racing_client, &racing_url, &racing_plan_id).await });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    sqlx::query("UPDATE blueprints SET status='published',published_at=clock_timestamp() WHERE workspace_id=$1 AND code='shared_product' AND version=2")
        .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
        .execute(&mut *publication)
        .await
        .unwrap();
    publication.commit().await.unwrap();
    let racing_apply = racing_apply.await.unwrap();
    assert_eq!(racing_apply.status(), StatusCode::CONFLICT);
    assert_eq!(
        racing_apply.json::<Value>().await.unwrap()["error"]["code"],
        "solution_pack_plan_stale"
    );

    let incompatible_source = std::str::from_utf8(PRODUCT_BLUEPRINT)
        .unwrap()
        .replace("name = \"Product\"", "name = \"Similar Product\"");
    let incompatible = create_plan_with_maps(
        &client,
        &base_url,
        archive_with_blueprint(incompatible_source.as_bytes()),
        "shared",
        "publish",
        &[("blueprints/product", "shared_product")],
    )
    .await;
    assert_eq!(incompatible.status(), StatusCode::CREATED);
    let incompatible = incompatible.json::<Value>().await.unwrap();
    assert_eq!(incompatible["ready"], false);
    assert_eq!(incompatible["actions"][0]["action"], "conflict");
    assert_eq!(
        incompatible["actions"][0]["reason_code"],
        "existing_blueprint_incompatible"
    );

    let other_plan =
        create_plan_with_publication(&client, &base_url, valid_archive(), "other", "publish")
            .await
            .json::<Value>()
            .await
            .unwrap();
    assert_eq!(
        apply_plan(&client, &base_url, other_plan["id"].as_str().unwrap())
            .await
            .status(),
        StatusCode::OK
    );
    let forged = create_plan_with_maps(
        &client,
        &base_url,
        valid_archive(),
        "shared",
        "publish",
        &[("blueprints/product", "shared_product")],
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    let forged_plan_id = forged["id"].as_str().unwrap().parse::<Uuid>().unwrap();
    let (other_id, other_version, other_hash, other_kind): (Uuid, i64, String, String) =
        sqlx::query_as("SELECT id,version,definition_hash,kind FROM blueprints WHERE workspace_id=$1 AND code='other_product' AND status='published'")
            .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("UPDATE solution_pack_plan_mappings SET target_id=$2,target_code='other_product',target_version=$3 WHERE plan_id=$1 AND logical_key='blueprints/product'")
        .bind(forged_plan_id)
        .bind(other_id)
        .bind(other_version)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE solution_pack_plan_actions SET preconditions=$2 WHERE plan_id=$1 AND logical_key='blueprints/product'")
        .bind(forged_plan_id)
        .bind(json!([{
            "kind":"existing_blueprint",
            "id":other_id,
            "code":"other_product",
            "version":other_version,
            "definition_hash":other_hash,
            "blueprint_kind":other_kind,
            "status":"published",
            "deleted":false
        }]))
        .execute(&pool)
        .await
        .unwrap();
    let forged_apply = apply_plan(&client, &base_url, &forged_plan_id.to_string()).await;
    assert_eq!(forged_apply.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        forged_apply.json::<Value>().await.unwrap()["error"]["code"],
        "invalid_input"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM solution_pack_applications WHERE plan_id=$1"
        )
        .bind(forged_plan_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );

    for mutation in ["snapshot", "precondition", "action"] {
        let forged = create_plan_with_maps(
            &client,
            &base_url,
            valid_archive(),
            "shared",
            "publish",
            &[("blueprints/product", "shared_product")],
        )
        .await
        .json::<Value>()
        .await
        .unwrap();
        let plan_id = forged["id"].as_str().unwrap().parse::<Uuid>().unwrap();
        let query = match mutation {
            "snapshot" => {
                "UPDATE solution_pack_plan_mappings SET snapshot=jsonb_set(snapshot,'{status}','\"draft\"'::jsonb) WHERE plan_id=$1 AND logical_key='blueprints/product'"
            }
            "precondition" => {
                "UPDATE solution_pack_plan_actions SET preconditions=jsonb_set(preconditions,'{0,status}','\"draft\"'::jsonb) WHERE plan_id=$1 AND logical_key='blueprints/product'"
            }
            "action" => {
                "UPDATE solution_pack_plan_actions SET reason_code='target_absent' WHERE plan_id=$1 AND logical_key='blueprints/product'"
            }
            _ => unreachable!(),
        };
        sqlx::query(query)
            .bind(plan_id)
            .execute(&pool)
            .await
            .unwrap();
        let response = apply_plan(&client, &base_url, &plan_id.to_string()).await;
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "{mutation}"
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM solution_pack_applications WHERE plan_id=$1"
            )
            .bind(plan_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            0,
            "{mutation}"
        );
    }

    let stale = create_plan_with_maps(
        &client,
        &base_url,
        valid_archive(),
        "shared",
        "publish",
        &[("blueprints/product", "shared_product")],
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    sqlx::query(
        "UPDATE blueprints SET status='draft' WHERE workspace_id=$1 AND code='shared_product'",
    )
    .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
    .execute(&pool)
    .await
    .unwrap();
    let stale_apply = apply_plan(&client, &base_url, stale["id"].as_str().unwrap()).await;
    assert_eq!(stale_apply.status(), StatusCode::CONFLICT);
    assert_eq!(
        stale_apply.json::<Value>().await.unwrap()["error"]["code"],
        "solution_pack_plan_stale"
    );
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
    let hidden_lineage = create_plan_from_application(
        &authenticated_client(),
        &base_url,
        release_archive(
            "attricat.hidden",
            "1.1.0",
            &[("blueprints/product", PRODUCT_BLUEPRINT)],
        ),
        "hidden_lineage",
        "publish",
        hidden_application,
    )
    .await;
    assert_eq!(hidden_lineage.status(), StatusCode::NOT_FOUND);
    let hidden_run = Uuid::new_v4();
    let hidden_request = Uuid::new_v4();
    sqlx::query("INSERT INTO solution_pack_check_runs (id,workspace_id,application_id,request_id,correlation_id,trigger,total_count,passed_count,failed_count,started_at,completed_at) VALUES ($1,$2,$3,$4,$4,'manual',0,0,0,now(),now())")
        .bind(hidden_run).bind(other_workspace).bind(hidden_application).bind(hidden_request)
        .execute(&pool).await.unwrap();
    let checks_url = format!("{base_url}/solution-packs/applications/{hidden_application}/checks");
    assert_eq!(
        authenticated_client()
            .get(&checks_url)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        authenticated_client()
            .post(&checks_url)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        authenticated_client()
            .get(format!("{checks_url}/{hidden_run}"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
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
    let ordinary_repository = CatalogRepository::new(
        pool.clone(),
        Uuid::from_u128(0x00000000000040008000000000000002),
    );
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
    // Simulate a legacy create/workspace-only plan without a versioned digest,
    // then make only the persisted satisfied setting step executable to exercise
    // its application/retry contract without existing-resource adoption.
    sqlx::query("UPDATE solution_pack_plans SET ready=true,resource_evidence_format=NULL,resource_evidence_sha256=NULL WHERE id=$1")
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
async fn apply_creates_draft_blueprint_once_with_safe_history(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let plan = create_plan(&client, &base_url, valid_archive(), "starter").await;
    assert_eq!(plan.status(), StatusCode::CREATED);
    let plan = plan.json::<Value>().await.unwrap();
    assert_eq!(plan["ready"], true);
    let plan_id = plan["id"].as_str().unwrap();

    let applied = apply_plan(&client, &base_url, plan_id).await;
    assert_eq!(applied.status(), StatusCode::OK);
    let text = applied.text().await.unwrap();
    assert!(!text.contains("BLUEPRINT_SOURCE_SECRET"));
    assert!(!text.contains("definition"));
    assert!(!text.contains("normalized_payload"));
    let application: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(application["state"], "completed");
    assert_eq!(application["mapping_snapshot"].as_array().unwrap().len(), 1);
    assert_eq!(application["steps"].as_array().unwrap().len(), 1);
    assert_eq!(application["steps"][0]["resource_kind"], "blueprint");
    assert_eq!(
        application["steps"][0]["result_snapshot"]["status"],
        "draft"
    );

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
            "SELECT count(*) FROM solution_pack_check_runs WHERE application_id=$1 AND trigger='post_apply'"
        )
        .bind(application_id.parse::<Uuid>().unwrap())
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
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
                "resource_kind": "blueprint",
                "logical_key": format!("resources/{index:03}_{}", "x".repeat(96)),
                "target_id": Uuid::new_v4(),
                "target_code": format!("target_{index:03}_{}", "x".repeat(32)),
                "target_version": Some(1),
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

    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn cross_kind_target_created_while_a_step_is_executing_invalidates_the_application(
    pool: PgPool,
) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let plan = create_plan(&client, &base_url, valid_archive(), "racing")
        .await
        .json::<Value>()
        .await
        .unwrap();
    let plan_id = plan["id"].as_str().unwrap().parse::<Uuid>().unwrap();
    let (target_id, target_code): (Uuid, String) = sqlx::query_as(
        "SELECT target_id,target_code FROM solution_pack_plan_mappings WHERE plan_id=$1 AND logical_key='blueprints/product'",
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
async fn concurrent_apply_requests_converge_without_duplicate_resources(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let plan = create_plan(
        &client,
        &base_url,
        archive_with_guidance_and_checks(),
        "parallel",
    )
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
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM solution_pack_check_runs WHERE application_id=$1 AND trigger='post_apply'")
            .bind(first["id"].as_str().unwrap().parse::<Uuid>().unwrap())
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM solution_pack_check_results WHERE run_id=(SELECT id FROM solution_pack_check_runs WHERE application_id=$1 AND trigger='post_apply')")
            .bind(first["id"].as_str().unwrap().parse::<Uuid>().unwrap())
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
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
    let plan = create_plan(&client, &base_url, archive_with_two_blueprints(), "resume")
        .await
        .json::<Value>()
        .await
        .unwrap();
    let plan_id = plan["id"].as_str().unwrap().parse::<Uuid>().unwrap();
    let original_payload: Value = sqlx::query_scalar(
        "SELECT normalized_payload FROM solution_pack_plan_actions WHERE plan_id=$1 AND logical_key='blueprints/product'",
    )
    .bind(plan_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    // Exercise durable step failure compatibility through the legacy
    // create-only path, which intentionally has no resource digest.
    sqlx::query("UPDATE solution_pack_plans SET resource_evidence_format=NULL,resource_evidence_sha256=NULL WHERE id=$1")
        .bind(plan_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE solution_pack_plan_actions SET normalized_payload='{}'::jsonb WHERE plan_id=$1 AND logical_key='blueprints/product'")
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
        "SELECT state, diagnostic_code FROM solution_pack_application_steps WHERE application_id=(SELECT id FROM solution_pack_applications WHERE plan_id=$1) AND logical_key='blueprints/product'",
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
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM blueprints WHERE code='resume_category'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    sqlx::query("UPDATE solution_pack_plan_actions SET normalized_payload=$2 WHERE plan_id=$1 AND logical_key='blueprints/product'")
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
    server.abort();
}
