mod support;

use std::{sync::Arc, time::Duration};

use api::{
    model::{CreateAttributeContext, RelationshipTargets},
    repository::{
        AttricatRepository, AuthorizationActor, ExtensionAttricatBatch, ExtensionAttricatIntent,
        ExtensionAttricatIntentStatus, InteractiveRunFailure, InteractiveRunScope,
        InteractiveRunStatus, RepositoryError, StartExtensionOperation,
    },
    storage::FakeObjectStore,
};
use support::*;

const EXTENSION: &str = "acme.interactive";
const DOCUMENT_BLUEPRINT: &str = r#"
format_version = 1
code = "interactive_document_item"
name = "Interactive document item"
kind = "record"
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
kind = "record"
[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
[[attributes]]
code = "title"
value_type = "string"
"#;
const PERMISSIONS: &[&str] = &[
    "attricat.read",
    "attricat.annotations.write",
    "artifacts.write",
    "client.explorer_bulk_action",
    "client.explorer_row_action",
    "client.operations.start",
    "client.operations.read",
    "client.operations.cancel",
];

fn interactive_component() -> Vec<u8> {
    build_test_component(
        "attricat-interactive-test-component",
        "interactive-test.component.wasm",
    )
}

fn archive(extension_id: &str, server: &[u8]) -> Vec<u8> {
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "name": "Interactive extension",
        "version": "1.0.0",
        "description": "interactive operation integration test",
        "icons": {"48": "icon.png"},
        "attricat": {"id": extension_id, "host_api": ">=1.0.0, <2.0.0"},
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
        }, {
            "id": "summarize-again", "handler": "summarize",
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
    tar_zst(&[
        ("manifest.json", &manifest),
        ("server.wasm", server),
        ("client.js", b"export const mount = () => {};"),
    ])
}

async fn install(
    repository: &AttricatRepository,
    store: Arc<FakeObjectStore>,
    server: &[u8],
) -> Uuid {
    install_extension(
        repository,
        store,
        EXTENSION,
        &archive(EXTENSION, server),
        PERMISSIONS,
    )
    .await
}

fn start_body(release: Uuid, key: &str, blueprint: (Uuid, i64), records: &[Uuid]) -> Value {
    json!({
        "release_id": release,
        "operation_id": "summarize",
        "input": {"template": "summary"},
        "idempotency_key": key,
        "selection": {
            "blueprint_id": blueprint.0,
            "blueprint_version": blueprint.1,
            "context_id": null,
            "record_ids": records
        }
    })
}

struct ScopedViewer {
    user: Uuid,
    membership: Uuid,
    grant: Uuid,
}

/// Adds a member whose only grant is a viewer role scoped to one record.
async fn record_scoped_viewer(pool: &sqlx::PgPool, record_id: Uuid) -> ScopedViewer {
    let (user, membership) = add_workspace_user(pool).await;
    let grant = grant_role(
        pool,
        membership,
        VIEWER_ROLE_ID,
        GrantScope::Record(record_id),
    )
    .await;
    ScopedViewer {
        user,
        membership,
        grant,
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn interactive_run_reads_its_selection_and_annotates_through_the_v15_world(
    pool: sqlx::PgPool,
) {
    let repository = AttricatRepository::system(pool.clone())
        .for_workspace(bootstrap_workspace_id())
        .await
        .unwrap();
    let store = Arc::new(FakeObjectStore::available());
    let release = install(&repository, store.clone(), &interactive_component()).await;
    let blueprint = published_blueprint(&repository, DOCUMENT_BLUEPRINT).await;
    let first = titled_record(&repository, blueprint, "First").await;
    let second = titled_record(&repository, blueprint, "Second").await;
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
    // The key names the request, not the operation.
    let mut other_operation = start_body(release, "generate-1", blueprint, &[second, first]);
    other_operation["operation_id"] = json!("summarize-again");
    let reused = authenticated_client()
        .post(&start_url)
        .json(&other_operation)
        .send()
        .await
        .unwrap();
    assert_eq!(reused.status(), StatusCode::CONFLICT);
    assert_eq!(
        reused.json::<Value>().await.unwrap()["error"]["code"],
        "idempotency_key_reused"
    );
    // Administrative keys are free-form, so one can equal a stored interactive
    // key. The collision is reported instead of retried forever.
    repository
        .start_extension_operation(StartExtensionOperation {
            extension_id: EXTENSION.into(),
            expected_release_id: release,
            operation_id: "summarize".into(),
            input: json!({"template": "summary"}),
            source_reference: json!({}),
            destination_reference: json!({}),
            idempotency_key: format!("interactive:{BOOTSTRAP_OWNER_ID}:collide"),
            schedule_id: None,
            configuration_snapshot: None,
        })
        .await
        .unwrap();
    let collided = tokio::time::timeout(
        Duration::from_secs(30),
        authenticated_client()
            .post(&start_url)
            .json(&start_body(release, "collide", blueprint, &[first]))
            .send(),
    )
    .await
    .expect("colliding start must not retry forever")
    .unwrap();
    assert_eq!(collided.status(), StatusCode::CONFLICT);
    assert_eq!(
        collided.json::<Value>().await.unwrap()["error"]["code"],
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

    drain_extension_operations(&repository, store.clone()).await;

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
        sqlx::query_as("SELECT system_tags, system_metadata FROM records WHERE id=$1")
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
        "SELECT COUNT(*) FROM domain_events WHERE event_type='record.annotations_changed.v1' AND aggregate_id IN ($1,$2) AND source_name=$3",
    )
    .bind(first)
    .bind(second)
    .bind(format!("extension:{EXTENSION}"))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(events, 2);
    let audited: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_events WHERE action='attricat.extensions.operations.write' AND actor_user_id=$1 AND metadata->'annotations'->>'extension_id'=$2",
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
async fn interactive_runs_are_authorized_per_record_and_fail_closed(pool: sqlx::PgPool) {
    let repository = AttricatRepository::system(pool.clone())
        .for_workspace(bootstrap_workspace_id())
        .await
        .unwrap();
    let store = Arc::new(FakeObjectStore::available());
    let release = install(&repository, store.clone(), &interactive_component()).await;
    let blueprint = published_blueprint(&repository, DOCUMENT_BLUEPRINT).await;
    let other_blueprint = published_blueprint(&repository, OTHER_BLUEPRINT).await;
    let visible = titled_record(&repository, blueprint, "Visible").await;
    let hidden = titled_record(&repository, blueprint, "Hidden").await;
    let foreign = titled_record(&repository, other_blueprint, "Foreign").await;
    let scoped_viewer = record_scoped_viewer(&pool, visible).await;
    let viewer = scoped_viewer.user;
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
    assert_eq!(
        operator.json::<Value>().await.unwrap()["initiated_by_me"],
        false
    );
    // Extension frames ask for `scope=own`: an operator's frame cannot read,
    // cancel or download another user's run.
    for request in [
        authenticated_client().get(format!("{base}/extension-runs/{viewer_run}?scope=own")),
        authenticated_client().post(format!(
            "{base}/extension-runs/{viewer_run}/cancel?scope=own"
        )),
        authenticated_client().get(format!(
            "{base}/extension-runs/{viewer_run}/artifacts/{}/download?scope=own",
            Uuid::new_v4()
        )),
    ] {
        assert_eq!(
            request.send().await.unwrap().status(),
            StatusCode::NOT_FOUND
        );
    }
    let own_scoped = client_for(viewer)
        .get(format!("{base}/extension-runs/{viewer_run}?scope=own"))
        .send()
        .await
        .unwrap();
    assert_eq!(own_scoped.status(), StatusCode::OK);
    assert_eq!(
        own_scoped.json::<Value>().await.unwrap()["initiated_by_me"],
        true
    );
    let own = client_for(viewer)
        .get(format!("{base}/extension-runs"))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(own.as_array().unwrap().len(), 1);
    let cancelled_run = client_for(viewer)
        .post(&start_url)
        .json(&start_body(release, "viewer-2", blueprint, &[visible]))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap()["run_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let stoppable_run = client_for(viewer)
        .post(&start_url)
        .json(&start_body(release, "viewer-3", blueprint, &[visible]))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap()["run_id"]
        .as_str()
        .unwrap()
        .to_owned();

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

    // Frozen membership is not permission: losing the record grant hides the run.
    sqlx::query("DELETE FROM role_grants WHERE id=$1")
        .bind(scoped_viewer.grant)
        .execute(&pool)
        .await
        .unwrap();
    let revoked = client_for(viewer)
        .get(format!("{base}/extension-runs/{viewer_run}"))
        .send()
        .await
        .unwrap();
    assert_eq!(revoked.status(), StatusCode::FORBIDDEN);
    // The initiator can still stop the run they can no longer inspect.
    let stopped = client_for(viewer)
        .post(format!("{base}/extension-runs/{stoppable_run}/cancel"))
        .send()
        .await
        .unwrap();
    assert_eq!(stopped.status(), StatusCode::NO_CONTENT);
    let listed = client_for(viewer)
        .get(format!("{base}/extension-runs"))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(listed, json!([]));
    // An initiator who is also an operator keeps operator access to the run.
    let operator_role = create_role(&pool, "run-operator", &["extensions.manage"]).await;
    grant_role(
        &pool,
        scoped_viewer.membership,
        operator_role,
        GrantScope::Workspace,
    )
    .await;
    let as_operator = client_for(viewer)
        .get(format!("{base}/extension-runs/{viewer_run}"))
        .send()
        .await
        .unwrap();
    assert_eq!(as_operator.status(), StatusCode::OK);

    // A cancellation requested before the initiator was removed is still
    // delivered rather than turned into an access failure.
    sqlx::query("UPDATE extension_operation_runs SET cancellation_requested=true WHERE id=$1")
        .bind(cancelled_run.parse::<Uuid>().unwrap())
        .execute(&pool)
        .await
        .unwrap();

    // An initiator removed from the workspace fails the run closed.
    sqlx::query("UPDATE workspace_memberships SET state='inactive' WHERE user_id=$1")
        .bind(viewer)
        .execute(&pool)
        .await
        .unwrap();
    drain_extension_operations(&repository, store).await;
    let run = repository
        .interactive_extension_run(viewer_run.parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(run.status, InteractiveRunStatus::Failed);
    assert_eq!(run.failure, Some(InteractiveRunFailure::AccessRevoked));
    let cancelled = repository
        .interactive_extension_run(cancelled_run.parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cancelled.status, InteractiveRunStatus::Cancelled);
    let annotated: bool =
        sqlx::query_scalar("SELECT system_metadata ? $2 FROM records WHERE id=$1")
            .bind(visible)
            .bind(EXTENSION)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!annotated);
    server.abort();
}

fn annotate(key: &str, record_id: Uuid, expected_revision: Option<i64>) -> ExtensionAttricatIntent {
    ExtensionAttricatIntent::Annotate {
        intent_key: key.into(),
        record_id,
        add_tags: vec!["generated".into()],
        remove_tags: vec![],
        set_metadata: [("template".to_owned(), json!(2))].into(),
        remove_metadata: vec![],
        expected_revision,
    }
}

fn batch(key: &str, intents: Vec<ExtensionAttricatIntent>) -> ExtensionAttricatBatch {
    ExtensionAttricatBatch {
        batch_key: key.into(),
        dry_run: false,
        intents,
    }
}

const TAG_CHECKED_BLUEPRINT: &str = r#"
format_version = 1
code = "tag_checked_item"
name = "Tag checked item"
kind = "record"
record_schema = '''{
  "x-attricat-checks": [
    {"code": "not-blocked", "predicate": {"type": "missing_tag", "tag": "acme.docs:blocked"}}
  ]
}'''
[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
[[attributes]]
code = "title"
value_type = "string"
"#;

#[sqlx::test(migrations = "./migrations")]
async fn annotation_tag_changes_run_tag_checks(pool: sqlx::PgPool) {
    let repository = AttricatRepository::system(pool.clone())
        .for_workspace(bootstrap_workspace_id())
        .await
        .unwrap();
    let extension = repository.for_extension("acme.docs");
    let blueprint = published_blueprint(&repository, TAG_CHECKED_BLUEPRINT).await;
    let item = titled_record(&repository, blueprint, "Item").await;
    let tag = |key: &str, tag: &str| ExtensionAttricatIntent::Annotate {
        intent_key: key.into(),
        record_id: item,
        add_tags: vec![tag.into()],
        remove_tags: vec![],
        set_metadata: Default::default(),
        remove_metadata: vec![],
        expected_revision: None,
    };
    let allowed = extension
        .execute_extension_attricat_batch(batch("t1", vec![tag("t1", "reviewed")]))
        .await
        .unwrap();
    assert_eq!(
        allowed[0].status,
        ExtensionAttricatIntentStatus::Applied,
        "{allowed:?}"
    );
    let blocked = extension
        .execute_extension_attricat_batch(batch("t2", vec![tag("t2", "blocked")]))
        .await
        .unwrap();
    assert_eq!(blocked[0].status, ExtensionAttricatIntentStatus::Rejected);
    let tags: Vec<String> = sqlx::query_scalar("SELECT system_tags FROM records WHERE id = $1")
        .bind(item)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tags, ["acme.docs:reviewed"]);
}

#[sqlx::test(migrations = "./migrations")]
async fn extension_annotation_namespaces_are_patched_and_protected_on_every_write_path(
    pool: sqlx::PgPool,
) {
    let repository = AttricatRepository::system(pool.clone())
        .for_workspace(bootstrap_workspace_id())
        .await
        .unwrap();
    let extension = repository.for_extension("acme.docs");
    let blueprint = published_blueprint(&repository, DOCUMENT_BLUEPRINT).await;
    let first = titled_record(&repository, blueprint, "First").await;

    let applied = extension
        .execute_extension_attricat_batch(batch("b1", vec![annotate("a1", first, Some(0))]))
        .await
        .unwrap();
    assert_eq!(
        applied[0].status,
        ExtensionAttricatIntentStatus::Applied,
        "{applied:?}"
    );
    assert_eq!(applied[0].annotation_revision, Some(1));
    // A retry is recognized before its now-stale expected revision is tested.
    let replay = extension
        .execute_extension_attricat_batch(batch("b1", vec![annotate("a1", first, Some(0))]))
        .await
        .unwrap();
    assert_eq!(
        replay[0].status,
        ExtensionAttricatIntentStatus::AlreadyApplied
    );
    let stale = extension
        .execute_extension_attricat_batch(batch("b2", vec![annotate("a2", first, Some(0))]))
        .await
        .unwrap();
    assert_eq!(stale[0].status, ExtensionAttricatIntentStatus::Rejected);
    assert!(
        stale[0]
            .error
            .as_deref()
            .unwrap()
            .contains("expected revision 0")
    );
    let contradictory = extension
        .execute_extension_attricat_batch(batch(
            "b3",
            vec![ExtensionAttricatIntent::Annotate {
                intent_key: "a3".into(),
                record_id: first,
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
        ExtensionAttricatIntentStatus::Rejected
    );

    // A disjoint writer keeps the first namespace untouched.
    let other = repository.for_extension("acme.other");
    let applied = other
        .execute_extension_attricat_batch(batch("o1", vec![annotate("o1", first, None)]))
        .await
        .unwrap();
    assert_eq!(applied[0].status, ExtensionAttricatIntentStatus::Applied);
    let (tags, metadata): (Vec<String>, Value) =
        sqlx::query_as("SELECT system_tags, system_metadata FROM records WHERE id=$1")
            .bind(first)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(tags.contains(&"acme.docs:generated".to_owned()));
    assert!(tags.contains(&"acme.other:generated".to_owned()));
    assert_eq!(metadata["acme.docs"]["template"], 2);

    // Legacy create fields cannot write a claimed namespace.
    let create = extension
        .execute_extension_attricat_batch(batch(
            "c1",
            vec![ExtensionAttricatIntent::Create {
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
    assert_eq!(create[0].status, ExtensionAttricatIntentStatus::Rejected);

    let (base, server) =
        start_server_with_object_store(pool.clone(), Arc::new(FakeObjectStore::available())).await;
    let record_url = format!("{base}/v1/records/{first}");
    // Whole-field writes that drop or alter a protected namespace are rejected.
    let dropped = authenticated_client()
        .put(&record_url)
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
        .put(&record_url)
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
        .put(&record_url)
        .json(&json!({"system_tags": kept, "system_metadata": metadata_with_note}))
        .send()
        .await
        .unwrap();
    assert_eq!(unrelated.status(), StatusCode::OK);

    // A duplicate does not inherit facts about the source record.
    let copy = repository.duplicate_record(first).await.unwrap();
    assert!(copy.system_tags.contains(&"editor".to_owned()));
    assert!(!copy.system_tags.iter().any(|tag| tag.starts_with("acme.")));
    assert!(copy.system_metadata.get("acme.docs").is_none());
    assert_eq!(copy.system_metadata["note"], "ok");

    // Operator repair uses the same audited patch path.
    let repaired = authenticated_client()
        .post(format!(
            "{base}/extensions/acme.docs/annotation-namespace/records/{first}"
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
    let repository = AttricatRepository::system(pool.clone())
        .for_workspace(bootstrap_workspace_id())
        .await
        .unwrap();
    let store = Arc::new(FakeObjectStore::available());
    install(&repository, store.clone(), b"server").await;
    let blueprint = published_blueprint(&repository, DOCUMENT_BLUEPRINT).await;
    let legacy = repository
        .create_record_with_values(
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
        .execute_extension_attricat_batch(batch("l1", vec![annotate("l1", legacy, None)]))
        .await
        .unwrap();
    assert_eq!(rejected[0].status, ExtensionAttricatIntentStatus::Rejected);
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
    assert_eq!(inventory["annotated_records"], 1);
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
        .execute_extension_attricat_batch(batch("l2", vec![annotate("l2", legacy, None)]))
        .await
        .unwrap();
    assert_eq!(
        not_object[0].status,
        ExtensionAttricatIntentStatus::Rejected
    );
    let tags: Vec<String> = sqlx::query_scalar("SELECT system_tags FROM records WHERE id=$1")
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
            "{base}/extensions/{EXTENSION}/annotation-namespace/records/{legacy}"
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
        .execute_extension_attricat_batch(batch("l3", vec![annotate("l3", legacy, Some(1))]))
        .await
        .unwrap();
    assert_eq!(accepted[0].status, ExtensionAttricatIntentStatus::Applied);
    // Reserved Core names can never be claimed.
    let reserved = repository
        .for_extension("attricat.sample")
        .execute_extension_attricat_batch(batch("r1", vec![annotate("r1", legacy, None)]))
        .await
        .unwrap();
    assert_eq!(reserved[0].status, ExtensionAttricatIntentStatus::Rejected);
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn generic_writes_serialize_with_namespace_claims(pool: sqlx::PgPool) {
    let repository = AttricatRepository::system(pool.clone())
        .for_workspace(bootstrap_workspace_id())
        .await
        .unwrap();
    let blueprint = published_blueprint(&repository, DOCUMENT_BLUEPRINT).await;
    let record = titled_record(&repository, blueprint, "Race").await;
    // Stand in for a first claim of `acme.race` that is still counting
    // existing data under that name.
    let mut claim = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!(
            "extension-annotation-namespace:{}:acme.race",
            bootstrap_workspace_id()
        ))
        .execute(&mut *claim)
        .await
        .unwrap();

    // Writes that do not touch the name are not held up.
    tokio::time::timeout(
        Duration::from_secs(10),
        repository.update_record_with_values(
            record,
            vec![],
            vec![],
            vec![],
            None,
            Some(json!({"plain": true})),
        ),
    )
    .await
    .expect("an unrelated write must not wait for the claim")
    .unwrap();

    // A write of data under the name waits until the claim finishes.
    let writer = repository.clone();
    let touching = tokio::spawn(async move {
        writer
            .update_record_with_values(
                record,
                vec![],
                vec![],
                vec![],
                None,
                Some(json!({"plain": true, "acme.race": {"x": 1}})),
            )
            .await
    });
    let blocked = wait_until(|| async {
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM pg_locks WHERE locktype = 'advisory' AND NOT granted)",
        )
        .fetch_one(&pool)
        .await
        .unwrap()
    })
    .await;
    assert!(
        blocked,
        "the touching write must wait for the namespace claim"
    );
    assert!(!touching.is_finished());
    claim.rollback().await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), touching)
        .await
        .expect("the write proceeds once the claim ends")
        .unwrap()
        .unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn replay_is_refused_after_the_run_context_is_deleted(pool: sqlx::PgPool) {
    let repository = AttricatRepository::system(pool.clone())
        .for_workspace(bootstrap_workspace_id())
        .await
        .unwrap();
    let store = Arc::new(FakeObjectStore::available());
    let release = install(&repository, store.clone(), &interactive_component()).await;
    let blueprint = published_blueprint(&repository, DOCUMENT_BLUEPRINT).await;
    let member = titled_record(&repository, blueprint, "Replay").await;
    let viewer = record_scoped_viewer(&pool, member).await.user;
    let context = repository
        .create_context(CreateAttributeContext {
            code: "replay_context".into(),
            data: json!({}),
            parent_id: None,
        })
        .await
        .unwrap();
    let (base, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let mut body = start_body(release, "replay-1", blueprint, &[member]);
    body["selection"]["context_id"] = json!(context.id);
    let run_id: Uuid = client_for(viewer)
        .post(format!("{base}/extensions/{EXTENSION}/bulk/operations"))
        .json(&body)
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap()["run_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    // The run fails closed while its initiator is suspended; a failed run no
    // longer blocks deleting its context.
    sqlx::query("UPDATE workspace_memberships SET state='inactive' WHERE user_id=$1")
        .bind(viewer)
        .execute(&pool)
        .await
        .unwrap();
    drain_extension_operations(&repository, store).await;
    let failed = repository
        .interactive_extension_run(run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed.status, InteractiveRunStatus::Failed);
    repository.delete_context(context.id).await.unwrap();

    let replay = authenticated_client()
        .post(format!("{base}/extension-operation-runs/{run_id}/replay"))
        .send()
        .await
        .unwrap();
    assert_eq!(replay.status(), StatusCode::CONFLICT);
    let run = repository
        .interactive_extension_run(run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(run.status, InteractiveRunStatus::Failed);
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn selection_pages_hold_at_most_one_pool_connection(pool: sqlx::PgPool) {
    let repository = AttricatRepository::system(pool.clone())
        .for_workspace(bootstrap_workspace_id())
        .await
        .unwrap();
    let blueprint = published_blueprint(&repository, DOCUMENT_BLUEPRINT).await;
    let first = titled_record(&repository, blueprint, "First").await;
    let second = titled_record(&repository, blueprint, "Second").await;
    let viewer = record_scoped_viewer(&pool, first).await.user;
    // With a single connection, holding one while acquiring another would
    // wait for the acquire timeout and fail.
    let bounded = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(2))
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap();
    let bounded_repository = AttricatRepository::system(bounded)
        .for_workspace(bootstrap_workspace_id())
        .await
        .unwrap();
    let scope = InteractiveRunScope {
        actor: AuthorizationActor {
            user_id: viewer,
            token_id: None,
        },
        record_ids: vec![first, second],
        blueprint_id: blueprint.0,
        blueprint_version: blueprint.1,
        context_id: None,
    };
    let page = bounded_repository
        .interactive_selection_page(EXTENSION, &scope, "", 10)
        .await
        .unwrap();
    let statuses: Vec<&str> = page["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| record["status"].as_str().unwrap())
        .collect();
    assert_eq!(statuses, ["available", "unavailable"]);
}

const LINKING_BLUEPRINT: &str = r#"
format_version = 1
code = "interactive_linking_item"
name = "Interactive linking item"
kind = "record"
[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
[[attributes]]
code = "title"
value_type = "string"
[[attributes]]
code = "related"
value_type = "relationship"
target_blueprint = "interactive_other_item"
cardinality = "many"
target_cardinality = "many"
"#;
#[sqlx::test(migrations = "./migrations")]
async fn interactive_links_require_read_access_to_every_target(pool: sqlx::PgPool) {
    let repository = AttricatRepository::system(pool.clone())
        .for_workspace(bootstrap_workspace_id())
        .await
        .unwrap();
    let store = Arc::new(FakeObjectStore::available());
    install(&repository, store, b"server").await;
    let targets = published_blueprint(&repository, OTHER_BLUEPRINT).await;
    let linking = published_blueprint(&repository, LINKING_BLUEPRINT).await;
    let source = titled_record(&repository, linking, "Source").await;
    let readable = titled_record(&repository, targets, "Readable").await;
    let hidden = titled_record(&repository, targets, "Hidden").await;
    let ScopedViewer {
        user, membership, ..
    } = record_scoped_viewer(&pool, source).await;
    grant_role(
        &pool,
        membership,
        EDITOR_ROLE_ID,
        GrantScope::Record(source),
    )
    .await;
    grant_role(
        &pool,
        membership,
        VIEWER_ROLE_ID,
        GrantScope::Record(readable),
    )
    .await;
    let scope = InteractiveRunScope {
        actor: AuthorizationActor {
            user_id: user,
            token_id: None,
        },
        record_ids: vec![source],
        blueprint_id: linking.0,
        blueprint_version: linking.1,
        context_id: None,
    };
    let run = repository.for_interactive_run(EXTENSION, Uuid::new_v4(), &scope);
    let link = |key: &str, target: Uuid| ExtensionAttricatIntent::Relationships {
        intent_key: key.into(),
        record_id: source,
        relationships: vec![RelationshipTargets {
            attribute_id: None,
            attribute_code: Some("related".into()),
            context_id: None,
            target_record_ids: vec![target],
        }],
    };

    // The initiator cannot read the target, even though the source is selected.
    let denied = run
        .execute_extension_attricat_batch(batch("link-1", vec![link("hidden", hidden)]))
        .await
        .unwrap();
    assert_eq!(denied[0].status, ExtensionAttricatIntentStatus::Rejected);
    let allowed = run
        .execute_extension_attricat_batch(batch("link-2", vec![link("readable", readable)]))
        .await
        .unwrap();
    assert_eq!(allowed[0].status, ExtensionAttricatIntentStatus::Applied);
}
