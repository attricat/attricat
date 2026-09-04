mod support;

use std::{io::Cursor, sync::Arc};

use api::{
    extension_installer::{ExtensionInstaller, installed_artifact_key},
    repository::CatalogRepository,
    storage::{FakeObjectStore, ObjectStore},
};
use support::{Value, json};
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

fn append_file(tar: &mut tar::Builder<&mut Vec<u8>>, path: &str, bytes: &[u8]) {
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, path, bytes).unwrap();
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
    assert_eq!(
        repository
            .quarantine_extension("acme.extension", "timeout")
            .await
            .unwrap()
            .state,
        "quarantined"
    );
    assert!(repository.enable_extension("acme.extension").await.is_err());
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
