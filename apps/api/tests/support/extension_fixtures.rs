//! Archives, WebAssembly test components and task draining for extension and
//! solution-pack integration tests.

use std::{io::Cursor, path::PathBuf, process::Command, sync::Arc, time::Duration};

use api::{
    extension_installer::ExtensionInstaller,
    extension_runtime::{ExtensionOperationTaskHandler, ExtensionRuntime, ExtensionRuntimeConfig},
    model::{CreateBlueprint, NewAttributeValue},
    repository::CatalogRepository,
    storage::FakeObjectStore,
    task_queue::TaskKind,
    task_worker::{TaskHandler, TaskOutcome},
};
use serde_json::json;
use uuid::Uuid;

pub fn append_file(tar: &mut tar::Builder<&mut Vec<u8>>, path: &str, bytes: &[u8]) {
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, path, bytes).unwrap();
}

/// A zstd-compressed tar archive of `files` in order.
pub fn tar_zst(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        for (path, bytes) in files {
            append_file(&mut tar, path, bytes);
        }
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

/// Builds the workspace crate `package` for `wasm32-unknown-unknown` and wraps
/// it as a component at `target/<output>`, returning the component bytes.
pub fn build_test_component(package: &str, output: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("API crate is below workspace root")
        .to_owned();
    assert!(
        Command::new("cargo")
            .current_dir(&root)
            .args([
                "build",
                "-p",
                package,
                "--target",
                "wasm32-unknown-unknown",
                "--release",
            ])
            .status()
            .expect("cargo must be available for the component fixture")
            .success()
    );
    let core = root.join(format!(
        "target/wasm32-unknown-unknown/release/{}.wasm",
        package.replace('-', "_")
    ));
    let component = root.join("target").join(output);
    assert!(
        Command::new("wasm-tools")
            .args(["component", "new"])
            .arg(core)
            .args(["-o"])
            .arg(&component)
            .status()
            .expect("wasm-tools must be available for the component fixture")
            .success()
    );
    std::fs::read(component).expect("component fixture must be readable")
}

/// Runs queued extension operation tasks until none is left, immediately
/// requeueing rescheduled ones. Bounded so a run that never settles cannot
/// hang the test.
pub async fn drain_extension_operations(
    repository: &CatalogRepository,
    store: Arc<FakeObjectStore>,
) {
    let runtime = ExtensionRuntime::new(store, ExtensionRuntimeConfig::default()).unwrap();
    let handler = ExtensionOperationTaskHandler::new(repository.clone(), runtime);
    for _ in 0..10 {
        let Some(task) = repository
            .claim_task_for_kinds(
                "extension-operations-test",
                Duration::from_secs(30),
                &[TaskKind::ExtensionOperationRunV1],
            )
            .await
            .unwrap()
        else {
            return;
        };
        match handler.handle(task.clone()).await.unwrap() {
            TaskOutcome::Reschedule { .. } => repository
                .reschedule_task_at(
                    task.id,
                    &task.lease_owner,
                    task.lease_token,
                    chrono::Utc::now(),
                )
                .await
                .unwrap(),
            TaskOutcome::Complete => repository
                .complete_task(task.id, &task.lease_owner, task.lease_token)
                .await
                .unwrap(),
            _ => {}
        }
    }
}

/// Side-loads `archive`, grants every capability in `permissions`, enables
/// the extension and returns its installed release id.
pub async fn install_extension(
    repository: &CatalogRepository,
    store: Arc<FakeObjectStore>,
    extension_id: &str,
    archive: &[u8],
    permissions: &[&str],
) -> Uuid {
    ExtensionInstaller::new(repository.clone(), store)
        .install("test", archive)
        .await
        .unwrap();
    for capability in permissions {
        repository
            .grant_extension(extension_id, "capability", capability)
            .await
            .unwrap();
    }
    repository.enable_extension(extension_id).await.unwrap();
    repository
        .installed_extension(extension_id)
        .await
        .unwrap()
        .installed_release_id
}

/// Creates and publishes a blueprint, returning its `(id, version)`.
pub async fn published_blueprint(repository: &CatalogRepository, definition: &str) -> (Uuid, i64) {
    let blueprint = repository
        .create_blueprint(CreateBlueprint {
            definition: definition.into(),
        })
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(blueprint.blueprint.id, blueprint.blueprint.version)
        .await
        .unwrap();
    (blueprint.blueprint.id, blueprint.blueprint.version)
}

/// Creates a record whose `title` attribute is `title`.
pub async fn titled_record(
    repository: &CatalogRepository,
    blueprint: (Uuid, i64),
    title: &str,
) -> Uuid {
    repository
        .create_record_with_values(
            blueprint.0,
            blueprint.1,
            vec![NewAttributeValue::Scalar {
                attribute_id: None,
                attribute_code: Some("title".into()),
                context_id: None,
                value: json!(title),
            }],
            vec![],
            json!({}),
        )
        .await
        .unwrap()
        .id
}
