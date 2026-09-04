mod support;

use std::collections::BTreeMap;

use api::{extensions::Manifest, repository::CatalogRepository};
use sha2::{Digest, Sha256};
use support::{Value, json};
use uuid::Uuid;

fn manifest(version: &str, dependencies: Value) -> Manifest {
    let artifact_hash = format!("{:x}", Sha256::digest(b"server bytes"));
    serde_json::from_value(json!({
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
            "path": "server.wasm",
            "sha256": artifact_hash
        }],
        "configuration": {
            "version": 1,
            "schema": {"type": "object", "required": ["endpoint"], "properties": {"endpoint": {"type": "string"}}, "additionalProperties": false}
        },
        "dependencies": dependencies
    })).unwrap()
}

fn package_files() -> BTreeMap<String, Vec<u8>> {
    BTreeMap::from([("server.wasm".to_owned(), b"server bytes".to_vec())])
}

#[sqlx::test(migrations = "./migrations")]
async fn lifecycle_requires_configuration_and_grants_and_retains_history(pool: sqlx::PgPool) {
    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::new(pool.clone())
        .for_workspace(workspace)
        .await
        .unwrap();
    let mut corrupted_files = package_files();
    corrupted_files.insert("server.wasm".to_owned(), b"tampered".to_vec());
    assert!(
        repository
            .install_extension(&manifest("1.0.0", json!([])), "test", &corrupted_files)
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM extension_installations")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );

    let installed = repository
        .install_extension(&manifest("1.0.0", json!([])), "test", &package_files())
        .await
        .unwrap();
    assert_eq!(installed.state, "disabled");
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
