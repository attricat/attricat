mod support;

use std::{io::Cursor, path::PathBuf, process::Command, sync::Arc, time::Duration};

use api::{
    extension_installer::ExtensionInstaller,
    extension_runtime::{ExtensionOperationTaskHandler, ExtensionRuntime, ExtensionRuntimeConfig},
    model::{CreateAttributeContext, CreateBlueprint, NewAttributeValue},
    repository::{
        CatalogRepository, ExtensionCatalogBatch, ExtensionCatalogIntent,
        ExtensionCatalogIntentStatus, RepositoryError,
    },
    storage::FakeObjectStore,
    task_worker::{TaskHandler, TaskOutcome},
};
use catalog_domain::task_queue::TaskKind;
use reqwest::{Client, StatusCode};
use support::{
    BOOTSTRAP_WORKSPACE_ID, Value, authenticated_client, json, start_server_with_object_store,
};
use uuid::Uuid;

const EXTENSION: &str = "acme.interactive";
const DOCUMENT_BLUEPRINT: &str = r#"
format_version = 1
code = "interactive_document_item"
name = "Interactive document item"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
[[attributes]]
code = "title"
value_type = "string"
"#;
const OTHER_BLUEPRINT: &str = r#"
format_version = 1
code = "interactive_other_item"
name = "Interactive other item"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
[[attributes]]
code = "title"
value_type = "string"
"#;
const VIEWER_ROLE: &str = "00000000-0000-4000-8000-000000000104";
const PERMISSIONS: &[&str] = &[
    "catalog.read",
    "catalog.annotations.write",
    "artifacts.write",
    "client.explorer_bulk_action",
    "client.explorer_row_action",
    "client.operations.start",
    "client.operations.read",
    "client.operations.cancel",
];

fn workspace() -> Uuid {
    BOOTSTRAP_WORKSPACE_ID.parse().unwrap()
}

fn interactive_component() -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .unwrap()
        .to_owned();
    assert!(
        Command::new("cargo")
            .current_dir(&root)
            .args([
                "build",
                "-p",
                "catalog-interactive-test-component",
                "--target",
                "wasm32-unknown-unknown",
                "--release",
            ])
            .status()
            .unwrap()
            .success()
    );
    let core =
        root.join("target/wasm32-unknown-unknown/release/catalog_interactive_test_component.wasm");
    let component = root.join("target/interactive-test.component.wasm");
    assert!(
        Command::new("wasm-tools")
            .args(["component", "new"])
            .arg(core)
            .args(["-o"])
            .arg(&component)
            .status()
            .unwrap()
            .success()
    );
    std::fs::read(component).unwrap()
}

fn append_file(tar: &mut tar::Builder<&mut Vec<u8>>, path: &str, bytes: &[u8]) {
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, path, bytes).unwrap();
}

fn archive(extension_id: &str, server: &[u8]) -> Vec<u8> {
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "name": "Interactive extension",
        "version": "1.0.0",
        "description": "interactive operation integration test",
        "icons": {"48": "icon.png"},
        "catalog": {"id": extension_id, "host_api": ">=1.5.0, <2.0.0"},
        "permissions": PERMISSIONS,
        "configuration": {"version": 1, "schema": {"type": "object", "additionalProperties": false}},
        "artifacts": [
            {"id": "server", "kind": "server_wasm", "path": "server.wasm"},
            {"id": "client", "kind": "client_component", "path": "client.js"}
        ],
        "server": {"operations": [{
            "id": "summarize", "handler": "summarize",
            "request_schema": {"type": "object", "required": ["template"], "properties": {"template": {"type": "string"}}, "additionalProperties": false},
            "max_request_bytes": 1024, "max_checkpoint_bytes": 8192,
            "interactive": {"version": 1, "max_selection": 50}
        }]},
        "ui": [
            {"id": "bulk", "version": 2, "kind": "action", "artifact": "client", "outlet": "explorer_bulk_action"},
            {"id": "row", "version": 1, "kind": "action", "artifact": "client", "outlet": "explorer_row_action"}
        ]
    }))
    .unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "manifest.json", &manifest);
        append_file(&mut tar, "server.wasm", server);
        append_file(&mut tar, "client.js", b"export const mount = () => {};");
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

async fn install(
    repository: &CatalogRepository,
    store: Arc<FakeObjectStore>,
    server: &[u8],
) -> Uuid {
    ExtensionInstaller::new(repository.clone(), store)
        .install("test", &archive(EXTENSION, server))
        .await
        .unwrap();
    for capability in PERMISSIONS {
        repository
            .grant_extension(EXTENSION, "capability", capability)
            .await
            .unwrap();
    }
    repository.enable_extension(EXTENSION).await.unwrap();
    repository
        .installed_extension(EXTENSION)
        .await
        .unwrap()
        .installed_release_id
}

async fn published_blueprint(repository: &CatalogRepository, definition: &str) -> (Uuid, i64) {
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

async fn entity(repository: &CatalogRepository, blueprint: (Uuid, i64), title: &str) -> Uuid {
    repository
        .create_entity_with_values(
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

fn start_body(release: Uuid, key: &str, blueprint: (Uuid, i64), entities: &[Uuid]) -> Value {
    json!({
        "release_id": release,
        "operation_id": "summarize",
        "input": {"template": "summary"},
        "idempotency_key": key,
        "selection": {
            "blueprint_id": blueprint.0,
            "blueprint_version": blueprint.1,
            "context_id": null,
            "entity_ids": entities
        }
    })
}

/// Adds a member whose only grant is a viewer role scoped to one entity.
async fn entity_scoped_viewer(pool: &sqlx::PgPool, entity_id: Uuid) -> (Uuid, Uuid) {
    let user = Uuid::new_v4();
    let membership = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, $2)")
        .bind(user)
        .bind(format!("viewer-{user}@example.test"))
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(membership)
    .bind(workspace())
    .bind(user)
    .execute(pool)
    .await
    .unwrap();
    let grant = Uuid::new_v4();
    sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, $4::uuid, 'entity', $5)")
        .bind(grant)
        .bind(workspace())
        .bind(membership)
        .bind(VIEWER_ROLE)
        .bind(entity_id)
        .execute(pool)
        .await
        .unwrap();
    (user, grant)
}

fn client_for(user: Uuid) -> Client {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("x-catalog-user-id", user.to_string().parse().unwrap());
    headers.insert(
        "x-catalog-workspace-id",
        BOOTSTRAP_WORKSPACE_ID.parse().unwrap(),
    );
    Client::builder().default_headers(headers).build().unwrap()
}

async fn drain_operations(repository: &CatalogRepository, store: Arc<FakeObjectStore>) {
    let runtime = ExtensionRuntime::new(store, ExtensionRuntimeConfig::default()).unwrap();
    let handler = ExtensionOperationTaskHandler::new(repository.clone(), runtime);
    for _ in 0..10 {
        let Some(task) = repository
            .claim_task_for_kinds(
                "interactive-test",
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

#[sqlx::test(migrations = "./migrations")]
async fn interactive_run_reads_its_selection_and_annotates_through_the_v15_world(
    pool: sqlx::PgPool,
) {
    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(workspace())
        .await
        .unwrap();
    let store = Arc::new(FakeObjectStore::available());
    let release = install(&repository, store.clone(), &interactive_component()).await;
    let blueprint = published_blueprint(&repository, DOCUMENT_BLUEPRINT).await;
    let first = entity(&repository, blueprint, "First").await;
    let second = entity(&repository, blueprint, "Second").await;
    let (base, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let start_url = format!("{base}/extensions/{EXTENSION}/bulk/operations");

    let response = authenticated_client()
        .post(&start_url)
        .json(&start_body(
            release,
            "generate-1",
            blueprint,
            &[second, first],
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let run_id = response.json::<Value>().await.unwrap()["run_id"]
        .as_str()
        .unwrap()
        .to_owned();

    // An identical retry returns the same run; a changed request is rejected.
    let retry = authenticated_client()
        .post(&start_url)
        .json(&start_body(
            release,
            "generate-1",
            blueprint,
            &[second, first],
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(
        retry.json::<Value>().await.unwrap()["run_id"],
        run_id.as_str()
    );
    let reused = authenticated_client()
        .post(&start_url)
        .json(&start_body(release, "generate-1", blueprint, &[first]))
        .send()
        .await
        .unwrap();
    assert_eq!(reused.status(), StatusCode::CONFLICT);
    assert_eq!(
        reused.json::<Value>().await.unwrap()["error"]["code"],
        "idempotency_key_reused"
    );

    let detail = authenticated_client()
        .get(format!("{base}/extension-runs/{run_id}"))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(detail["status"], "queued");
    assert_eq!(detail["selection_count"], 2);
    assert_eq!(detail["artifacts"], json!([]));
    assert!(detail.get("input").is_none());

    drain_operations(&repository, store.clone()).await;

    let detail = authenticated_client()
        .get(format!("{base}/extension-runs/{run_id}"))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(detail["status"], "completed", "{detail}");
    assert_eq!(detail["progress"], json!({"completed": 2, "total": 2}));
    assert!(detail["outputs_expire_at"].is_string());
    let artifact = &detail["artifacts"][0];
    assert_eq!(artifact["name"], "summary.txt");
    let download = authenticated_client()
        .get(format!(
            "{base}/extension-runs/{run_id}/artifacts/{}/download",
            artifact["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(download.status(), StatusCode::OK);
    assert_eq!(
        download.headers()["content-disposition"],
        "attachment; filename=\"summary.txt\""
    );
    // Display order is preserved by the frozen membership.
    assert_eq!(
        download.text().await.unwrap(),
        format!("{second}\n{first}\n")
    );

    let checkpoint: Value =
        sqlx::query_scalar("SELECT checkpoint FROM extension_operation_runs WHERE id=$1::uuid")
            .bind(&run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(checkpoint["generic_read_rejected"], true);
    assert_eq!(checkpoint["outside_rejected"], true);

    let (tags, metadata): (Vec<String>, Value) =
        sqlx::query_as("SELECT system_tags, system_metadata FROM entities WHERE id=$1")
            .bind(first)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(tags.contains(&format!("{EXTENSION}:processed")));
    assert_eq!(metadata[EXTENSION]["run_id"], run_id.as_str());
    assert!(metadata[EXTENSION]["cleared"].is_null());
    assert!(
        metadata[EXTENSION]
            .as_object()
            .unwrap()
            .contains_key("cleared")
    );
    let events: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM domain_events WHERE event_type='entity.annotations_changed.v1' AND aggregate_id IN ($1,$2) AND source_name=$3",
    )
    .bind(first)
    .bind(second)
    .bind(format!("extension:{EXTENSION}"))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(events, 2);
    let audited: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_events WHERE action='catalog.extensions.operations.write' AND actor_user_id=$1 AND metadata->'annotations'->>'extension_id'=$2",
    )
    .bind(support::BOOTSTRAP_OWNER_ID.parse::<Uuid>().unwrap())
    .bind(EXTENSION)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audited, 2);

    let runs = authenticated_client()
        .get(format!("{base}/extension-runs?extension_id={EXTENSION}"))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(runs[0]["id"], run_id.as_str());
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn interactive_runs_are_authorized_per_entity_and_fail_closed(pool: sqlx::PgPool) {
    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(workspace())
        .await
        .unwrap();
    let store = Arc::new(FakeObjectStore::available());
    let release = install(&repository, store.clone(), &interactive_component()).await;
    let blueprint = published_blueprint(&repository, DOCUMENT_BLUEPRINT).await;
    let other_blueprint = published_blueprint(&repository, OTHER_BLUEPRINT).await;
    let visible = entity(&repository, blueprint, "Visible").await;
    let hidden = entity(&repository, blueprint, "Hidden").await;
    let foreign = entity(&repository, other_blueprint, "Foreign").await;
    let (viewer, grant) = entity_scoped_viewer(&pool, visible).await;
    let (base, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let start_url = format!("{base}/extensions/{EXTENSION}/bulk/operations");

    // One unreadable member rejects the whole selection.
    let denied = client_for(viewer)
        .post(&start_url)
        .json(&start_body(
            release,
            "viewer-1",
            blueprint,
            &[visible, hidden],
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let mixed = authenticated_client()
        .post(&start_url)
        .json(&start_body(
            release,
            "mixed",
            blueprint,
            &[visible, foreign],
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(mixed.status(), StatusCode::UNPROCESSABLE_ENTITY);
    // Version 1 contributions keep their old contract and cannot start runs.
    let legacy = authenticated_client()
        .post(format!("{base}/extensions/{EXTENSION}/row/operations"))
        .json(&start_body(release, "legacy", blueprint, &[visible]))
        .send()
        .await
        .unwrap();
    assert_eq!(legacy.status(), StatusCode::FORBIDDEN);

    let accepted = client_for(viewer)
        .post(&start_url)
        .json(&start_body(release, "viewer-1", blueprint, &[visible]))
        .send()
        .await
        .unwrap();
    assert_eq!(accepted.status(), StatusCode::ACCEPTED);
    let viewer_run = accepted.json::<Value>().await.unwrap()["run_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let owner_run = authenticated_client()
        .post(&start_url)
        .json(&start_body(release, "owner-1", blueprint, &[hidden]))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap()["run_id"]
        .as_str()
        .unwrap()
        .to_owned();

    // Other users do not inherit run access; operators do.
    let probe = client_for(viewer)
        .get(format!("{base}/extension-runs/{owner_run}"))
        .send()
        .await
        .unwrap();
    assert_eq!(probe.status(), StatusCode::NOT_FOUND);
    let forged_cancel = client_for(viewer)
        .post(format!("{base}/extension-runs/{owner_run}/cancel"))
        .send()
        .await
        .unwrap();
    assert_eq!(forged_cancel.status(), StatusCode::NOT_FOUND);
    let operator = authenticated_client()
        .get(format!("{base}/extension-runs/{viewer_run}"))
        .send()
        .await
        .unwrap();
    assert_eq!(operator.status(), StatusCode::OK);
    let own = client_for(viewer)
        .get(format!("{base}/extension-runs"))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(own.as_array().unwrap().len(), 1);

    // Initiator cancellation of queued work is immediately terminal.
    let cancelled = authenticated_client()
        .post(format!("{base}/extension-runs/{owner_run}/cancel"))
        .send()
        .await
        .unwrap();
    assert_eq!(cancelled.status(), StatusCode::NO_CONTENT);
    let detail = authenticated_client()
        .get(format!("{base}/extension-runs/{owner_run}"))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(detail["status"], "cancelled");
    assert_eq!(detail["can_cancel"], false);

    // A context cannot be deleted while an active run reads from it.
    let context = repository
        .create_context(CreateAttributeContext {
            code: "run_context".into(),
            data: json!({}),
            parent_id: None,
        })
        .await
        .unwrap();
    let mut body = start_body(release, "owner-context", blueprint, &[hidden]);
    body["selection"]["context_id"] = json!(context.id);
    let context_run = authenticated_client()
        .post(&start_url)
        .json(&body)
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap()["run_id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(matches!(
        repository.delete_context(context.id).await,
        Err(RepositoryError::ContextInUse)
    ));
    let cancelled = authenticated_client()
        .post(format!("{base}/extension-runs/{context_run}/cancel"))
        .send()
        .await
        .unwrap();
    assert_eq!(cancelled.status(), StatusCode::NO_CONTENT);
    repository.delete_context(context.id).await.unwrap();

    // Frozen membership is not permission: losing the entity grant hides the run.
    sqlx::query("DELETE FROM role_grants WHERE id=$1")
        .bind(grant)
        .execute(&pool)
        .await
        .unwrap();
    let revoked = client_for(viewer)
        .get(format!("{base}/extension-runs/{viewer_run}"))
        .send()
        .await
        .unwrap();
    assert_eq!(revoked.status(), StatusCode::FORBIDDEN);
    let listed = client_for(viewer)
        .get(format!("{base}/extension-runs"))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(listed, json!([]));

    // An initiator removed from the workspace fails the run closed.
    sqlx::query("UPDATE workspace_memberships SET state='inactive' WHERE user_id=$1")
        .bind(viewer)
        .execute(&pool)
        .await
        .unwrap();
    drain_operations(&repository, store).await;
    let run = repository
        .interactive_extension_run(viewer_run.parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(run.status, "failed");
    assert_eq!(run.failure, Some("access_revoked"));
    let annotated: bool =
        sqlx::query_scalar("SELECT system_metadata ? $2 FROM entities WHERE id=$1")
            .bind(visible)
            .bind(EXTENSION)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!annotated);
    server.abort();
}

fn annotate(key: &str, entity_id: Uuid, expected_revision: Option<i64>) -> ExtensionCatalogIntent {
    ExtensionCatalogIntent::Annotate {
        intent_key: key.into(),
        entity_id,
        add_tags: vec!["generated".into()],
        remove_tags: vec![],
        set_metadata: [("template".to_owned(), json!(2))].into(),
        remove_metadata: vec![],
        expected_revision,
    }
}

fn batch(key: &str, intents: Vec<ExtensionCatalogIntent>) -> ExtensionCatalogBatch {
    ExtensionCatalogBatch {
        batch_key: key.into(),
        dry_run: false,
        intents,
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn extension_annotation_namespaces_are_patched_and_protected_on_every_write_path(
    pool: sqlx::PgPool,
) {
    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(workspace())
        .await
        .unwrap();
    let extension = repository.for_extension("acme.docs");
    let blueprint = published_blueprint(&repository, DOCUMENT_BLUEPRINT).await;
    let first = entity(&repository, blueprint, "First").await;

    let applied = extension
        .execute_extension_catalog_batch(batch("b1", vec![annotate("a1", first, Some(0))]))
        .await
        .unwrap();
    assert_eq!(
        applied[0].status,
        ExtensionCatalogIntentStatus::Applied,
        "{applied:?}"
    );
    assert_eq!(applied[0].annotation_revision, Some(1));
    // A retry is recognized before its now-stale expected revision is tested.
    let replay = extension
        .execute_extension_catalog_batch(batch("b1", vec![annotate("a1", first, Some(0))]))
        .await
        .unwrap();
    assert_eq!(
        replay[0].status,
        ExtensionCatalogIntentStatus::AlreadyApplied
    );
    let stale = extension
        .execute_extension_catalog_batch(batch("b2", vec![annotate("a2", first, Some(0))]))
        .await
        .unwrap();
    assert_eq!(stale[0].status, ExtensionCatalogIntentStatus::Rejected);
    assert!(
        stale[0]
            .error
            .as_deref()
            .unwrap()
            .contains("expected revision 0")
    );
    let contradictory = extension
        .execute_extension_catalog_batch(batch(
            "b3",
            vec![ExtensionCatalogIntent::Annotate {
                intent_key: "a3".into(),
                entity_id: first,
                add_tags: vec!["x".into()],
                remove_tags: vec!["x".into()],
                set_metadata: Default::default(),
                remove_metadata: vec![],
                expected_revision: None,
            }],
        ))
        .await
        .unwrap();
    assert_eq!(
        contradictory[0].status,
        ExtensionCatalogIntentStatus::Rejected
    );

    // A disjoint writer keeps the first namespace untouched.
    let other = repository.for_extension("acme.other");
    let applied = other
        .execute_extension_catalog_batch(batch("o1", vec![annotate("o1", first, None)]))
        .await
        .unwrap();
    assert_eq!(applied[0].status, ExtensionCatalogIntentStatus::Applied);
    let (tags, metadata): (Vec<String>, Value) =
        sqlx::query_as("SELECT system_tags, system_metadata FROM entities WHERE id=$1")
            .bind(first)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(tags.contains(&"acme.docs:generated".to_owned()));
    assert!(tags.contains(&"acme.other:generated".to_owned()));
    assert_eq!(metadata["acme.docs"]["template"], 2);

    // Legacy create fields cannot write a claimed namespace.
    let create = extension
        .execute_extension_catalog_batch(batch(
            "c1",
            vec![ExtensionCatalogIntent::Create {
                intent_key: "c1".into(),
                blueprint_id: blueprint.0,
                blueprint_version: blueprint.1,
                values: vec![],
                system_tags: vec!["acme.docs:forged".into()],
                system_metadata: json!({}),
            }],
        ))
        .await
        .unwrap();
    assert_eq!(create[0].status, ExtensionCatalogIntentStatus::Rejected);

    let (base, server) =
        start_server_with_object_store(pool.clone(), Arc::new(FakeObjectStore::available())).await;
    let entity_url = format!("{base}/v1/entities/{first}");
    // Whole-field writes that drop or alter a protected namespace are rejected.
    let dropped = authenticated_client()
        .put(&entity_url)
        .json(&json!({"system_tags": ["editor"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(dropped.status(), StatusCode::CONFLICT);
    assert_eq!(
        dropped.json::<Value>().await.unwrap()["error"]["code"],
        "protected_annotation_namespace"
    );
    let forged = authenticated_client()
        .put(&entity_url)
        .json(&json!({"system_metadata": {"acme.docs": {"template": 9}, "acme.other": {"template": 2}}}))
        .send()
        .await
        .unwrap();
    assert_eq!(forged.status(), StatusCode::CONFLICT);
    // Unrelated edits that round-trip the namespaces still succeed.
    let mut kept = tags.clone();
    kept.push("editor".into());
    let mut metadata_with_note = metadata.clone();
    metadata_with_note["note"] = json!("ok");
    let unrelated = authenticated_client()
        .put(&entity_url)
        .json(&json!({"system_tags": kept, "system_metadata": metadata_with_note}))
        .send()
        .await
        .unwrap();
    assert_eq!(unrelated.status(), StatusCode::OK);

    // A duplicate does not inherit facts about the source entity.
    let copy = repository.duplicate_entity(first).await.unwrap();
    assert!(copy.system_tags.contains(&"editor".to_owned()));
    assert!(!copy.system_tags.iter().any(|tag| tag.starts_with("acme.")));
    assert!(copy.system_metadata.get("acme.docs").is_none());
    assert_eq!(copy.system_metadata["note"], "ok");

    // Operator repair uses the same audited patch path.
    let repaired = authenticated_client()
        .post(format!(
            "{base}/extensions/acme.docs/annotation-namespace/entities/{first}"
        ))
        .json(&json!({"remove_tags": ["generated"], "remove_metadata": ["template"], "expected_revision": 1}))
        .send()
        .await
        .unwrap();
    assert_eq!(repaired.status(), StatusCode::OK);
    let repaired = repaired.json::<Value>().await.unwrap();
    assert_eq!(repaired, json!({"tags": [], "metadata": {}, "revision": 2}));
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn legacy_annotations_require_explicit_namespace_adoption(pool: sqlx::PgPool) {
    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(workspace())
        .await
        .unwrap();
    let store = Arc::new(FakeObjectStore::available());
    install(&repository, store.clone(), b"server").await;
    let blueprint = published_blueprint(&repository, DOCUMENT_BLUEPRINT).await;
    let legacy = repository
        .create_entity_with_values(
            blueprint.0,
            blueprint.1,
            vec![],
            vec![format!("{EXTENSION}:old"), format!("{EXTENSION}:Old Tag")],
            json!({EXTENSION: "not an object"}),
        )
        .await
        .unwrap()
        .id;
    let extension = repository.for_extension(EXTENSION);
    let rejected = extension
        .execute_extension_catalog_batch(batch("l1", vec![annotate("l1", legacy, None)]))
        .await
        .unwrap();
    assert_eq!(rejected[0].status, ExtensionCatalogIntentStatus::Rejected);
    assert!(rejected[0].error.as_deref().unwrap().contains("adopt"));

    let (base, server) = start_server_with_object_store(pool.clone(), store).await;
    let inventory = authenticated_client()
        .get(format!(
            "{base}/extensions/{EXTENSION}/annotation-namespace"
        ))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(inventory["claimed"], false);
    assert_eq!(inventory["annotated_entities"], 1);
    let adopted = authenticated_client()
        .post(format!(
            "{base}/extensions/{EXTENSION}/annotation-namespace"
        ))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(adopted["claimed"], true);
    assert_eq!(adopted["adopted_legacy"], true);

    // Adoption preserves data; a non-object value must be repaired, not merged.
    let not_object = extension
        .execute_extension_catalog_batch(batch("l2", vec![annotate("l2", legacy, None)]))
        .await
        .unwrap();
    assert_eq!(not_object[0].status, ExtensionCatalogIntentStatus::Rejected);
    let tags: Vec<String> = sqlx::query_scalar("SELECT system_tags FROM entities WHERE id=$1")
        .bind(legacy)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        tags,
        vec![format!("{EXTENSION}:old"), format!("{EXTENSION}:Old Tag")]
    );
    // Operator repair can remove malformed legacy tags and replace the
    // non-object value, after which the extension can write again.
    let repaired = authenticated_client()
        .post(format!(
            "{base}/extensions/{EXTENSION}/annotation-namespace/entities/{legacy}"
        ))
        .json(&json!({"remove_tags": ["Old Tag"], "set_metadata": {"migrated": true}}))
        .send()
        .await
        .unwrap();
    assert_eq!(repaired.status(), StatusCode::OK);
    assert_eq!(
        repaired.json::<Value>().await.unwrap(),
        json!({"tags": ["old"], "metadata": {"migrated": true}, "revision": 1})
    );
    let audited: Value = sqlx::query_scalar(
        "SELECT metadata->'annotations'->'replaced_namespace_value' FROM audit_events WHERE metadata->'annotations'->>'extension_id'=$1",
    )
    .bind(EXTENSION)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audited, json!("not an object"));
    let accepted = extension
        .execute_extension_catalog_batch(batch("l3", vec![annotate("l3", legacy, Some(1))]))
        .await
        .unwrap();
    assert_eq!(accepted[0].status, ExtensionCatalogIntentStatus::Applied);
    // Reserved Core names can never be claimed.
    let reserved = repository
        .for_extension("attricat.sample")
        .execute_extension_catalog_batch(batch("r1", vec![annotate("r1", legacy, None)]))
        .await
        .unwrap();
    assert_eq!(reserved[0].status, ExtensionCatalogIntentStatus::Rejected);
    server.abort();
}
