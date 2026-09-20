mod support;

use std::{io::Cursor, sync::Arc};

use api::{
    extension_installer::{ExtensionInstaller, installed_artifact_key},
    model::CreateBlueprint,
    repository::CatalogRepository,
    storage::{FakeObjectStore, ObjectStore},
};
use support::{Value, authenticated_client, json, start_server_with_object_store};
use uuid::Uuid;

const ARTIFACT_BYTES: &[u8] = b"server bytes";

fn release_archive(version: &str, dependencies: Value, artifact_bytes: &[u8]) -> Vec<u8> {
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "name": "Acme extension",
        "version": version,
        "description": "extension integration test",
        "icons": {"48": "icon.png"},
        "catalog": {"id": "acme.extension", "host_api": "^1.0"},
        "permissions": ["network.request"],
        "host_permissions": [{
            "id": "api-read",
            "matches": ["https://api.acme.example/v1/*"],
            "methods": ["GET"]
        }],
        "artifacts": [{
            "id": "server",
            "kind": "server_wasm",
            "path": "server.wasm"
        }],
        "configuration": {
            "version": 1,
            "schema": {"type": "object", "required": ["endpoint"], "properties": {"endpoint": {"type": "string"}}, "additionalProperties": false}
        },
        "dependencies": dependencies
    }))
    .unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "manifest.json", &manifest);
        append_file(&mut tar, "server.wasm", artifact_bytes);
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn storage_client_release_archive(version: &str) -> Vec<u8> {
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "name": "Storage client extension",
        "version": version,
        "description": "extension storage integration test",
        "icons": {"48": "icon.png"},
        "catalog": {"id": "acme.storage", "host_api": "^1.0"},
        "permissions": ["storage.extension"],
        "configuration": {"version": 1, "schema": {"type": "object", "additionalProperties": false}},
        "artifacts": [{"id": "client", "kind": "client_component", "path": "client.js"}],
        "ui": [{"id": "panel", "version": 1, "kind": "embedded", "artifact": "client", "outlet": "entity_preview_panel"}]
    }))
    .unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "manifest.json", &manifest);
        append_file(&mut tar, "client.js", b"export {}");
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn client_release_archive_for_outlet(
    extension_id: &str,
    contribution_id: &str,
    outlet: &str,
) -> Vec<u8> {
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "name": "Client extension",
        "version": "1.0.0",
        "description": "client runtime integration test",
        "icons": {"48": "icon.png"},
        "catalog": {"id": extension_id, "host_api": "^1.0"},
        "permissions": ["configuration.read"],
        "configuration": {"version": 1, "schema": {"type": "object", "additionalProperties": false}},
        "artifacts": [{
            "id": "client",
            "kind": "client_component",
            "path": "client.js"
        }],
        "ui": [{
            "id": contribution_id,
            "version": 1,
            "kind": "embedded",
            "artifact": "client",
            "outlet": outlet
        }]
    }))
    .unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "manifest.json", &manifest);
        append_file(&mut tar, "client.js", b"export {}");
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn client_release_archive(extension_id: &str) -> Vec<u8> {
    client_release_archive_for_outlet(extension_id, "panel", "entity_preview_panel")
}

fn append_file(tar: &mut tar::Builder<&mut Vec<u8>>, path: &str, bytes: &[u8]) {
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, path, bytes).unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn sideload_installs_a_validated_local_archive(pool: sqlx::PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base, server) = start_server_with_object_store(pool, store.clone()).await;
    let response = authenticated_client()
        .post(format!("{base}/extensions/sideload"))
        .header("content-type", "application/zstd")
        .body(release_archive("1.0.0", json!([]), ARTIFACT_BYTES))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::CREATED);
    let installation: Value = response.json().await.unwrap();
    assert_eq!(installation["extension_id"], "acme.extension");
    assert_eq!(installation["state"], "disabled");
    assert_eq!(store.object_count().await, 1);
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn enabled_client_contributions_are_hidden_after_state_changes(pool: sqlx::PgPool) {
    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::new(pool)
        .for_workspace(workspace)
        .await
        .unwrap();
    let installer =
        ExtensionInstaller::new(repository.clone(), Arc::new(FakeObjectStore::available()));

    installer
        .install(
            "github:acme/client@v1.0.0",
            &client_release_archive("acme.client"),
        )
        .await
        .unwrap();
    assert!(
        repository
            .client_extension_contributions()
            .await
            .unwrap()
            .is_empty()
    );

    repository
        .grant_extension("acme.client", "capability", "configuration.read")
        .await
        .unwrap();
    repository.enable_extension("acme.client").await.unwrap();
    let contributions = repository.client_extension_contributions().await.unwrap();
    assert_eq!(contributions.len(), 1);
    assert_eq!(contributions[0].id, "panel");
    assert_eq!(
        contributions[0].artifact_key,
        format!(
            "extensions/{}/client",
            contributions[0].installed_release_id
        )
    );
    assert_eq!(contributions[0].contribution_key, "acme.client:panel");

    // Layout hides are evaluated after the enabled/grant runtime gate. The
    // stored key survives removal so toggling visibility restores the same
    // contribution without an installation-time ordering contract.
    repository
        .update_workspace_extension_layout(json!({
            "version": 1,
            "outlets": {
                "entity_preview_panel": {
                    "order": [],
                    "hidden": ["acme.client:panel"]
                }
            }
        }))
        .await
        .unwrap();
    assert!(
        repository
            .client_extension_contributions()
            .await
            .unwrap()
            .is_empty()
    );
    repository
        .update_workspace_extension_layout(json!({
            "version": 1,
            "outlets": {
                "entity_preview_panel": { "order": [], "hidden": [] }
            }
        }))
        .await
        .unwrap();
    assert_eq!(
        repository
            .client_extension_contributions()
            .await
            .unwrap()
            .len(),
        1
    );

    repository.disable_extension("acme.client").await.unwrap();
    assert!(
        repository
            .client_extension_contributions()
            .await
            .unwrap()
            .is_empty()
    );
    repository.enable_extension("acme.client").await.unwrap();
    assert_eq!(
        repository.client_extension_contributions().await.unwrap()[0].contribution_key,
        "acme.client:panel"
    );
    repository
        .quarantine_extension("acme.client", "test")
        .await
        .unwrap();
    assert!(
        repository
            .client_extension_contributions()
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn extension_layout_replacement_rejects_non_object_roots_and_repairs_malformed_layout(
    pool: sqlx::PgPool,
) {
    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::new(pool.clone());
    for malformed in [json!("scalar"), json!([])] {
        sqlx::query("UPDATE workspaces SET settings=$2 WHERE id=$1")
            .bind(workspace)
            .bind(malformed.clone())
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            repository
                .update_workspace_extension_layout(json!({"version":1,"outlets":{}}))
                .await
                .is_err()
        );
        let stored: Value = sqlx::query_scalar("SELECT settings FROM workspaces WHERE id=$1")
            .bind(workspace)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(stored, malformed);
    }
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events WHERE workspace_id=$1 AND action='workspace.extension_layout.set'",
    )
    .bind(workspace)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 0);

    sqlx::query("UPDATE workspaces SET settings=$2 WHERE id=$1")
        .bind(workspace)
        .bind(json!({"theme":"dark","extension_layout":{"malformed":true}}))
        .execute(&pool)
        .await
        .unwrap();
    let replacement = json!({
        "version":1,
        "outlets": {
            "navigation": {"order":[],"hidden":[],"promoted":[]}
        }
    });
    repository
        .update_workspace_extension_layout(replacement.clone())
        .await
        .unwrap();
    let repaired: Value = sqlx::query_scalar("SELECT settings FROM workspaces WHERE id=$1")
        .bind(workspace)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(repaired["theme"], "dark");
    assert_eq!(repaired["extension_layout"], replacement);
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events WHERE workspace_id=$1 AND action='workspace.extension_layout.set'",
    )
    .bind(workspace)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn extension_layout_order_is_stable_and_host_owned(pool: sqlx::PgPool) {
    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::new(pool.clone())
        .for_workspace(workspace)
        .await
        .unwrap();
    let installer =
        ExtensionInstaller::new(repository.clone(), Arc::new(FakeObjectStore::available()));
    for extension_id in ["acme.zebra", "acme.alpha"] {
        installer
            .install("test", &client_release_archive(extension_id))
            .await
            .unwrap();
        repository
            .grant_extension(extension_id, "capability", "configuration.read")
            .await
            .unwrap();
        repository.enable_extension(extension_id).await.unwrap();
    }
    let keys = || async {
        repository
            .client_extension_contributions()
            .await
            .unwrap()
            .into_iter()
            .map(|item| item.contribution_key)
            .collect::<Vec<_>>()
    };
    assert_eq!(keys().await, ["acme.alpha:panel", "acme.zebra:panel"]);
    repository
        .update_workspace_extension_layout(json!({
            "version": 1,
            "outlets": {
                "entity_preview_panel": {
                    "order": ["acme.zebra:panel"],
                    "hidden": []
                }
            }
        }))
        .await
        .unwrap();
    assert_eq!(keys().await, ["acme.zebra:panel", "acme.alpha:panel"]);
    let audited: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events WHERE workspace_id = $1 AND action = 'workspace.extension_layout.set' AND metadata ? 'extension_layout'",
    )
    .bind(workspace)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audited, 1);
    for invalid in [
        json!({
            "version": 1,
            "outlets": {
                "entity_preview_panel": {
                    "order": ["acme.alpha:panel"],
                    "hidden": ["acme.alpha:panel"]
                }
            }
        }),
        json!({
            "version": 1,
            "outlets": {
                "navigation": {"order": [], "hidden": []}
            }
        }),
        json!({
            "version": 1,
            "outlets": {
                "entity_action": {"order": ["malformed"], "hidden": []}
            }
        }),
        json!({
            "version": 1,
            "outlets": {
                "entity_action": {"order": ["acme.shared:item"], "hidden": []},
                "entity_preview_panel": {"order": ["acme.shared:item"], "hidden": []}
            }
        }),
        json!({
            "version": 1,
            "outlets": {
                "navigation": {
                    "order": [],
                    "hidden": [],
                    "promoted": ["acme.shared:item"]
                }
            }
        }),
        json!({
            "version": 1,
            "outlets": {
                "navigation": {
                    "order": [],
                    "hidden": [],
                    "promoted": ["acme.shared:item"]
                },
                "entity_action": {
                    "order": ["acme.shared:item"],
                    "hidden": []
                }
            }
        }),
    ] {
        assert!(
            repository
                .update_workspace_extension_layout(invalid)
                .await
                .is_err()
        );
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn blueprint_layout_overlays_owned_outlets_and_preserves_global_workspace_rules(
    pool: sqlx::PgPool,
) {
    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::new(pool)
        .for_workspace(workspace)
        .await
        .unwrap();
    let installer =
        ExtensionInstaller::new(repository.clone(), Arc::new(FakeObjectStore::available()));
    for extension_id in ["acme.alpha", "acme.zebra"] {
        installer
            .install("test", &client_release_archive(extension_id))
            .await
            .unwrap();
        repository
            .grant_extension(extension_id, "capability", "configuration.read")
            .await
            .unwrap();
        repository.enable_extension(extension_id).await.unwrap();
    }
    installer
        .install(
            "test",
            &client_release_archive_for_outlet("acme.navigation", "entry", "navigation"),
        )
        .await
        .unwrap();
    repository
        .grant_extension("acme.navigation", "capability", "configuration.read")
        .await
        .unwrap();
    repository
        .enable_extension("acme.navigation")
        .await
        .unwrap();
    repository
        .update_workspace_extension_layout(json!({
            "version": 1,
            "outlets": {
                "navigation": {
                    "order": ["acme.navigation:entry"],
                    "hidden": [],
                    "promoted": ["acme.navigation:entry"]
                }
            }
        }))
        .await
        .unwrap();
    assert_eq!(
        repository
            .client_extension_contributions()
            .await
            .unwrap()
            .into_iter()
            .find(|item| item.contribution_key == "acme.navigation:entry")
            .unwrap()
            .navigation_group
            .as_deref(),
        Some("promoted")
    );
    repository
        .update_workspace_extension_layout(json!({
            "version": 1,
            "outlets": {
                "entity_preview_panel": {
                    "order": ["acme.alpha:panel", "acme.zebra:panel"],
                    "hidden": []
                },
                "navigation": {
                    "order": [],
                    "hidden": ["acme.navigation:entry"],
                    "promoted": []
                }
            }
        }))
        .await
        .unwrap();
    let definition = r#"
format_version = 1
code = "extension_layout_product"
name = "Extension layout product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[views.extension_layout]
type = "extension_layout"
version = 1

[views.extension_layout.outlets.entity_preview_panel]
order = ["acme.zebra:panel"]
hidden = ["acme.alpha:panel"]

[[attributes]]
code = "title"
value_type = "string"
"#;
    let blueprint = repository
        .create_blueprint(CreateBlueprint {
            definition: definition.to_owned(),
        })
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(blueprint.blueprint.id, 1)
        .await
        .unwrap();

    let scoped = repository
        .client_extension_contributions_for_blueprint(Some((blueprint.blueprint.id, 1)))
        .await
        .unwrap();
    assert_eq!(
        scoped
            .iter()
            .map(|item| item.contribution_key.as_str())
            .collect::<Vec<_>>(),
        ["acme.zebra:panel"]
    );
    assert!(
        scoped
            .iter()
            .all(|item| item.contribution_key != "acme.navigation:entry")
    );
    assert_eq!(
        repository
            .client_extension_contributions()
            .await
            .unwrap()
            .into_iter()
            .map(|item| item.contribution_key)
            .collect::<Vec<_>>(),
        ["acme.alpha:panel", "acme.zebra:panel"]
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn hidden_contributions_remain_authorized_and_are_validated_on_publish(pool: sqlx::PgPool) {
    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::new(pool)
        .for_workspace(workspace)
        .await
        .unwrap();
    let installer =
        ExtensionInstaller::new(repository.clone(), Arc::new(FakeObjectStore::available()));
    installer
        .install("test", &client_release_archive("acme.client"))
        .await
        .unwrap();
    repository
        .grant_extension("acme.client", "capability", "configuration.read")
        .await
        .unwrap();
    repository.enable_extension("acme.client").await.unwrap();
    repository
        .update_workspace_extension_layout(json!({
            "version": 1,
            "outlets": {
                "entity_preview_panel": {
                    "order": [],
                    "hidden": ["acme.client:panel"]
                }
            }
        }))
        .await
        .unwrap();
    assert!(
        repository
            .client_extension_contributions()
            .await
            .unwrap()
            .is_empty()
    );
    let contribution = repository
        .client_extension_contribution("acme.client", "panel")
        .await
        .unwrap();
    assert!(contribution.artifact_key.ends_with("/client"));

    let incompatible = r#"
format_version = 1
code = "invalid_extension_layout"
name = "Invalid extension layout"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[views.extension_layout]
type = "extension_layout"
version = 1

[views.extension_layout.outlets.entity_action]
order = ["acme.client:panel"]
hidden = []

[[attributes]]
code = "title"
value_type = "string"
"#;
    let blueprint = repository
        .create_blueprint(CreateBlueprint {
            definition: incompatible.to_owned(),
        })
        .await
        .unwrap();
    assert!(
        repository
            .publish_blueprint_revision(blueprint.blueprint.id, 1)
            .await
            .is_err()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn workspace_safe_mode_blocks_runtime_descriptors_and_storage_without_mutating_installations(
    pool: sqlx::PgPool,
) {
    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::new(pool)
        .for_workspace(workspace)
        .await
        .unwrap();
    let installer =
        ExtensionInstaller::new(repository.clone(), Arc::new(FakeObjectStore::available()));
    let installed = installer
        .install("test", &storage_client_release_archive("1.0.0"))
        .await
        .unwrap();
    repository
        .grant_extension("acme.storage", "capability", "storage.extension")
        .await
        .unwrap();
    repository.enable_extension("acme.storage").await.unwrap();

    repository
        .set_workspace_extensions_enabled(false)
        .await
        .unwrap();
    assert!(
        repository
            .client_extension_contributions()
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        repository
            .runtime_extension_installation("acme.storage", installed.installed_release_id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        repository
            .extension_storage_get("acme.storage", installed.installed_release_id, "state")
            .await
            .is_err()
    );
    assert_eq!(
        repository
            .installed_extension("acme.storage")
            .await
            .unwrap()
            .state,
        "enabled"
    );

    repository
        .set_workspace_extensions_enabled(true)
        .await
        .unwrap();
    assert_eq!(
        repository
            .client_extension_contributions()
            .await
            .unwrap()
            .len(),
        1
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn extension_storage_enforces_cas_bounds_quota_and_workspace_namespace(pool: sqlx::PgPool) {
    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::new(pool)
        .for_workspace(workspace)
        .await
        .unwrap();
    let installer =
        ExtensionInstaller::new(repository.clone(), Arc::new(FakeObjectStore::available()));
    let installed = installer
        .install("test", &storage_client_release_archive("1.0.0"))
        .await
        .unwrap();
    repository
        .grant_extension("acme.storage", "capability", "storage.extension")
        .await
        .unwrap();
    repository.enable_extension("acme.storage").await.unwrap();

    let first = repository
        .extension_storage_set(
            "acme.storage",
            installed.installed_release_id,
            "state",
            json!({"enabled": true}),
            None,
        )
        .await
        .unwrap();
    assert_eq!(first, 1);
    assert_eq!(
        repository
            .extension_storage_get("acme.storage", installed.installed_release_id, "state")
            .await
            .unwrap()
            .unwrap()
            .value,
        json!({"enabled": true})
    );
    assert_eq!(
        repository
            .extension_storage_set(
                "acme.storage",
                installed.installed_release_id,
                "state",
                json!(2),
                Some(first)
            )
            .await
            .unwrap(),
        2
    );
    assert!(matches!(
        repository
            .extension_storage_set(
                "acme.storage",
                installed.installed_release_id,
                "state",
                json!(3),
                Some(first)
            )
            .await,
        Err(api::repository::ExtensionStorageError::Conflict)
    ));
    assert!(
        repository
            .extension_storage_set(
                "acme.storage",
                installed.installed_release_id,
                &"x".repeat(257),
                json!(null),
                None
            )
            .await
            .is_err()
    );
    assert!(
        repository
            .extension_storage_set(
                "acme.storage",
                installed.installed_release_id,
                "large",
                json!("x".repeat(65_535)),
                None
            )
            .await
            .is_err()
    );

    let quota_value = json!("x".repeat(65_534));
    for index in 0..79 {
        repository
            .extension_storage_set(
                "acme.storage",
                installed.installed_release_id,
                &format!("quota-{index}"),
                quota_value.clone(),
                None,
            )
            .await
            .unwrap();
    }
    assert!(matches!(
        repository
            .extension_storage_set(
                "acme.storage",
                installed.installed_release_id,
                "quota-overflow",
                quota_value,
                None
            )
            .await,
        Err(api::repository::ExtensionStorageError::QuotaExceeded)
    ));
    let page = repository
        .extension_storage_list(
            "acme.storage",
            installed.installed_release_id,
            Some("quota-"),
            None,
            1,
        )
        .await
        .unwrap();
    assert_eq!(page.entries.len(), 1);
    assert!(page.cursor.is_some());
    repository.disable_extension("acme.storage").await.unwrap();
    assert!(matches!(
        repository
            .extension_storage_get("acme.storage", installed.installed_release_id, "state")
            .await,
        Err(api::repository::ExtensionStorageError::Denied)
    ));
}

#[sqlx::test(migrations = "./migrations")]
async fn extension_storage_list_treats_like_characters_as_literal_prefixes(pool: sqlx::PgPool) {
    let workspace = Uuid::from_u128(0x00000000000040008000000000000009);
    sqlx::query("INSERT INTO workspaces (id, slug, name, login_identifier) VALUES ($1, 'extension-prefix', 'Extension prefix', 'extension-prefix.local')")
        .bind(workspace)
        .execute(&pool)
        .await
        .unwrap();
    let repository = CatalogRepository::new(pool)
        .for_workspace(workspace)
        .await
        .unwrap();
    let installer =
        ExtensionInstaller::new(repository.clone(), Arc::new(FakeObjectStore::available()));
    let installed = installer
        .install("test", &storage_client_release_archive("1.0.0"))
        .await
        .unwrap();
    repository
        .grant_extension("acme.storage", "capability", "storage.extension")
        .await
        .unwrap();
    repository.enable_extension("acme.storage").await.unwrap();
    for key in ["literal%key", "literal_key", "literalXkey"] {
        repository
            .extension_storage_set(
                "acme.storage",
                installed.installed_release_id,
                key,
                json!(true),
                None,
            )
            .await
            .unwrap();
    }

    let page = repository
        .extension_storage_list(
            "acme.storage",
            installed.installed_release_id,
            Some("literal%"),
            None,
            10,
        )
        .await
        .unwrap();
    assert_eq!(
        page.entries
            .into_iter()
            .map(|entry| entry.key)
            .collect::<Vec<_>>(),
        vec!["literal%key"]
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn lifecycle_installs_validated_archive_artifacts_and_retains_history(pool: sqlx::PgPool) {
    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::new(pool.clone())
        .for_workspace(workspace)
        .await
        .unwrap();
    let store = Arc::new(FakeObjectStore::available());
    let installer = ExtensionInstaller::new(repository.clone(), store.clone());

    assert!(
        installer
            .install("github:acme/extension@v1.0.0", b"not a zstd archive")
            .await
            .is_err()
    );
    assert_eq!(store.object_count().await, 0);

    let installed = installer
        .install(
            "github:acme/extension@v1.0.0",
            &release_archive("1.0.0", json!([]), ARTIFACT_BYTES),
        )
        .await
        .unwrap();
    assert_eq!(installed.state, "disabled");
    assert_eq!(
        store
            .get(&installed_artifact_key(
                installed.installed_release_id,
                "server"
            ))
            .await
            .unwrap()
            .bytes
            .as_ref(),
        ARTIFACT_BYTES
    );

    // A repository failure after S3 staging cleans up the newly staged object.
    assert!(
        installer
            .install(
                "github:acme/extension@v1.0.0",
                &release_archive("1.0.0", json!([]), ARTIFACT_BYTES),
            )
            .await
            .is_err()
    );
    assert_eq!(store.object_count().await, 1);

    assert!(repository.enable_extension("acme.extension").await.is_err());
    repository
        .configure_extension(
            "acme.extension",
            json!({"endpoint": "https://api.acme.example"}),
        )
        .await
        .unwrap();
    repository
        .grant_extension("acme.extension", "capability", "network.request")
        .await
        .unwrap();
    repository
        .grant_extension("acme.extension", "host_permission", "api-read")
        .await
        .unwrap();
    assert_eq!(
        repository
            .enable_extension("acme.extension")
            .await
            .unwrap()
            .state,
        "enabled"
    );

    // A runtime candidate is not authorization: the invocation-time lookup
    // must observe configuration and release changes made after selection.
    let authorized = repository
        .runtime_extension_installation("acme.extension", installed.installed_release_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        authorized.configuration["endpoint"],
        "https://api.acme.example"
    );
    repository
        .configure_extension(
            "acme.extension",
            json!({"endpoint": "https://changed.acme.example"}),
        )
        .await
        .unwrap();
    let refreshed = repository
        .runtime_extension_installation("acme.extension", installed.installed_release_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        refreshed.configuration["endpoint"],
        "https://changed.acme.example"
    );
    let upgraded = installer
        .upgrade(
            "github:acme/extension@v2.0.0",
            &release_archive("2.0.0", json!([]), ARTIFACT_BYTES),
        )
        .await
        .unwrap();
    assert_ne!(
        upgraded.installed_release_id,
        installed.installed_release_id
    );
    assert!(
        repository
            .runtime_extension_installation("acme.extension", installed.installed_release_id)
            .await
            .unwrap()
            .is_none()
    );
    repository
        .configure_extension(
            "acme.extension",
            json!({"endpoint": "https://upgraded.acme.example"}),
        )
        .await
        .unwrap();

    assert_eq!(
        repository
            .quarantine_extension("acme.extension", "timeout")
            .await
            .unwrap()
            .state,
        "quarantined"
    );
    repository
        .grant_extension("acme.extension", "capability", "network.request")
        .await
        .unwrap();
    repository
        .grant_extension("acme.extension", "host_permission", "api-read")
        .await
        .unwrap();
    assert_eq!(
        repository
            .enable_extension("acme.extension")
            .await
            .unwrap()
            .state,
        "enabled"
    );
    repository
        .disable_extension("acme.extension")
        .await
        .unwrap();
    repository.remove_extension("acme.extension").await.unwrap();

    let history = repository
        .extension_lifecycle_history("acme.extension")
        .await
        .unwrap();
    assert_eq!(history.last().unwrap().operation, "remove");
    assert!(
        history
            .iter()
            .any(|record| record.operation == "quarantine")
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM audit_events WHERE workspace_id = $1 AND target->>'type' = 'extension'",
        )
        .bind(workspace)
        .fetch_one(&pool)
        .await
        .unwrap(),
        history.len() as i64
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM extension_installations")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}
