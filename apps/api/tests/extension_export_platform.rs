//! The export platform of `attricat:host@1.1.0` (#424) against a real
//! database and the production Wasmtime handlers: typed handler outcomes,
//! the mapping store and its revision guard, connector page v2, the withdrawn
//! feed, deferred and failing batches, resyncs and the export status API.

mod support;

use std::{sync::Arc, time::Duration};

use api::{
    extension_runtime::{ExtensionRuntime, ExtensionRuntimeConfig, WasmExtensionTaskHandler},
    model::{CreateAttributeContext, CreateBlueprint, NewAttributeValue},
    repository::{
        AttricatRepository, ExportBegin, ExportMappingUpdate, ExportRetrySelection,
        ExtensionAttricatPageRequest, MaterializeScope, RejectionRecord,
    },
    storage::FakeObjectStore,
    task_queue::TaskKind,
    task_worker::{TaskHandler, TaskOutcome},
};
use support::*;
use uuid::Uuid;

const EXTENSION: &str = "acme.exporter";
const WASM_CONSUMER: &str = "attricat.extensions.wasm";
const PERMISSIONS: &[&str] = &[
    "attricat.read",
    "attricat.annotations.write",
    "events.subscribe",
    "storage.extension",
    "exports.mappings",
    "client.commands",
];

const VARIANT_BLUEPRINT: &str = r#"
format_version = 1
code = "export_variant"
name = "Export variant"
kind = "record"
[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
[[attributes]]
code = "title"
value_type = "string"
"#;

fn product_blueprint(job: &str) -> String {
    format!(
        r#"
format_version = 1
code = "export_product"
name = "Export product"
kind = "record"
[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
[[attributes]]
code = "title"
value_type = "string"
[[attributes]]
code = "variants"
value_type = "relationship"
target_blueprint = "export_variant"
cardinality = "many"
{job}
"#
    )
}

const EXPORT_JOB: &str = r#"
[[connector_jobs]]
code = "marketplace"
direction = "export"
extension_id = "acme.exporter"
operation_id = "export"
input = {expand = ["variants"]}
"#;

fn manifest() -> Value {
    json!({
        "manifest_version": 1,
        "name": "Export platform fixture",
        "version": "1.0.0",
        "description": "attricat:host 1.1 export platform test",
        "icons": {"48": "icon.png"},
        "attricat": {"id": EXTENSION, "host_api": "^1.1"},
        "permissions": PERMISSIONS,
        "configuration": {"version": 1, "schema": {"type": "object", "additionalProperties": false}},
        "artifacts": [{"id": "server", "kind": "server_wasm", "path": "server.wasm"}],
        "server": {
            "event_handlers": [{
                "id": "push",
                "event_types": ["record.created.v1", "export.retry_requested.v1"],
                "handler": "handle-event"
            }, {
                "id": "coalesced",
                "event_types": ["attribute_value.changed.v1"],
                "handler": "handle-event",
                "debounce_ms": 300
            }],
            "commands": [
                {"id": "preview", "handler": "preview", "request_schema": {"type": "object"}, "response_schema": {"type": "object"}},
                {"id": "clock", "handler": "clock", "request_schema": {"type": "object"}, "response_schema": {"type": "object"}}
            ],
            "operations": [{
                "id": "export", "handler": "export",
                "request_schema": {"type": "object"},
                "max_checkpoint_bytes": 65536
            }],
            "exports": [{"id": "marketplace", "title": "Marketplace", "preview_command": "preview", "resync_operation": "export"}]
        }
    })
}

/// The component is built once per test binary; parallel tests would race
/// on the same build output.
fn archive() -> Vec<u8> {
    static COMPONENT: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    let component = COMPONENT.get_or_init(|| {
        build_test_component(
            "attricat-export-test-component",
            "export-test.component.wasm",
        )
    });
    let manifest = serde_json::to_vec(&manifest()).unwrap();
    tar_zst(&[("manifest.json", &manifest), ("server.wasm", component)])
}

async fn workspace_repository(pool: &sqlx::PgPool) -> AttricatRepository {
    AttricatRepository::system(pool.clone())
        .for_workspace(bootstrap_workspace_id())
        .await
        .unwrap()
}

/// Delivers every pending extension event like the production coordinator
/// and task worker, making rescheduled deliveries due at once.
async fn deliver_events(repository: &AttricatRepository, store: Arc<FakeObjectStore>) {
    let runtime = ExtensionRuntime::new(store, ExtensionRuntimeConfig::default()).unwrap();
    let handler = WasmExtensionTaskHandler::new(repository.clone(), runtime);
    let event_types = repository.enabled_extension_event_types().await.unwrap();
    repository
        .materialize_event_delivery_tasks_with(WASM_CONSUMER, &event_types, MaterializeScope::Full)
        .await
        .unwrap();
    for _ in 0..200 {
        let Some(task) = repository
            .claim_task_for_kinds(
                "export-events-test",
                Duration::from_secs(30),
                &[TaskKind::EventDeliveryV1],
            )
            .await
            .unwrap()
        else {
            return;
        };
        match handler.handle(task.clone()).await {
            Ok(TaskOutcome::Reschedule { at }) => {
                // Wait out debounce windows; backoffs in tests are tiny.
                if let Ok(wait) = (at - chrono::Utc::now()).to_std() {
                    tokio::time::sleep(wait.min(Duration::from_secs(2))).await;
                }
                repository
                    .reschedule_task_at(
                        task.id,
                        &task.lease_owner,
                        task.lease_token,
                        chrono::Utc::now() - chrono::Duration::seconds(60),
                    )
                    .await
                    .unwrap()
            }
            Ok(_) => repository
                .complete_task(task.id, &task.lease_owner, task.lease_token)
                .await
                .unwrap(),
            // A faulted handler leaves its task to the failure budget.
            Err(_) => repository
                .complete_task(task.id, &task.lease_owner, task.lease_token)
                .await
                .unwrap(),
        }
    }
    panic!("event deliveries did not settle");
}

async fn installation_state(pool: &sqlx::PgPool) -> String {
    sqlx::query_scalar("SELECT state FROM extension_installations WHERE extension_id=$1")
        .bind(EXTENSION)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn mapping_status(pool: &sqlx::PgPool, record_id: Uuid) -> Option<(String, Option<Value>)> {
    sqlx::query_as(
        "SELECT status,error FROM extension_export_mappings WHERE record_id=$1 AND extension_id=$2",
    )
    .bind(record_id)
    .bind(EXTENSION)
    .fetch_optional(pool)
    .await
    .unwrap()
}

async fn content_revision(pool: &sqlx::PgPool, record_id: Uuid) -> i64 {
    workspace_repository(pool)
        .await
        .get_record_with_revision(record_id)
        .await
        .unwrap()
        .unwrap()
        .content_revision
        .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn handler_outcomes_reject_and_retry_records_without_quarantine(pool: sqlx::PgPool) {
    let repository = workspace_repository(&pool).await;
    repository
        .ensure_event_consumer(WASM_CONSUMER, &[])
        .await
        .unwrap();
    let store = Arc::new(FakeObjectStore::available());
    install_extension(
        &repository,
        store.clone(),
        EXTENSION,
        &archive(),
        PERMISSIONS,
    )
    .await;
    let blueprint = published_blueprint(&repository, VARIANT_BLUEPRINT).await;
    let synced = titled_record(&repository, blueprint, "Blue shirt").await;
    let rejected = titled_record(&repository, blueprint, "reject: no price").await;
    let retried = titled_record(&repository, blueprint, "retry later").await;

    deliver_events(&repository, store.clone()).await;

    // A rejected record and a temporarily failing destination never switch
    // off the installation; only a fault does.
    assert_eq!(installation_state(&pool).await, "enabled");
    let (status, _) = mapping_status(&pool, synced).await.unwrap();
    assert_eq!(status, "synced");
    let (external_id, revision): (String, i64) = sqlx::query_as(
        "SELECT external_id,record_revision FROM extension_export_mappings WHERE record_id=$1",
    )
    .bind(synced)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(external_id, format!("ext-{synced}"));
    assert_eq!(revision, content_revision(&pool, synced).await);
    let (status, error) = mapping_status(&pool, rejected).await.unwrap();
    assert_eq!(status, "rejected");
    assert_eq!(error.unwrap()["code"], "invalid_title");
    let outcome: (String, i32, Option<String>) = sqlx::query_as(
        "SELECT status,attempts,error_code FROM extension_handler_attempts WHERE record_id=$1",
    )
    .bind(rejected)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        outcome,
        ("rejected".into(), 1, Some("invalid_title".into()))
    );
    // The retried record succeeded on its third attempt; the attempt count
    // reached the component, and transient outcomes were cleaned up.
    assert_eq!(mapping_status(&pool, retried).await.unwrap().0, "synced");
    let attempt: i64 = sqlx::query_scalar(
        "SELECT (value->>'attempt')::bigint FROM extension_storage_entries WHERE extension_id=$1 AND value->>'record_revision' = $2",
    )
    .bind(EXTENSION)
    .bind(content_revision(&pool, retried).await.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(attempt, 3);
    assert_eq!(
        count(&pool, "SELECT count(*) FROM extension_handler_attempts WHERE status NOT IN ('rejected','exhausted')").await,
        0
    );
    let failures = repository
        .extension_handler_failures(EXTENSION, 10)
        .await
        .unwrap();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].event_type, "record.created.v1");

    // An operator retry of the rejected record reaches only this extension.
    let selected = repository
        .request_export_retry(
            EXTENSION,
            api::repository::ExportRetrySelection {
                connection: None,
                record_ids: None,
                status: Some("rejected".into()),
                request_key: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(selected, vec![rejected]);
    assert_eq!(mapping_status(&pool, rejected).await.unwrap().0, "pending");
    deliver_events(&repository, store.clone()).await;
    // Still rejected by the destination, recorded again.
    assert_eq!(mapping_status(&pool, rejected).await.unwrap().0, "rejected");

    titled_record(&repository, blueprint, "fault").await;
    deliver_events(&repository, store).await;
    assert_eq!(installation_state(&pool).await, "quarantined");
}

#[sqlx::test(migrations = "./migrations")]
async fn content_revisions_guard_the_mapping_store_against_stale_pushes(pool: sqlx::PgPool) {
    let repository = workspace_repository(&pool).await;
    let store = Arc::new(FakeObjectStore::available());
    install_extension(&repository, store, EXTENSION, &archive(), PERMISSIONS).await;
    let blueprint = published_blueprint(&repository, VARIANT_BLUEPRINT).await;
    let record = titled_record(&repository, blueprint, "Lamp").await;
    let created = content_revision(&pool, record).await;
    let created_event: i64 = sqlx::query_scalar(
        "SELECT sequence FROM domain_events WHERE aggregate_id=$1 AND event_type='record.created.v1'",
    )
    .bind(record)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(created, created_event);
    repository
        .append_values(
            record,
            api::model::AppendAttributeValues {
                expected_updated_at: None,
                values: vec![NewAttributeValue::Scalar {
                    attribute_id: None,
                    attribute_code: Some("title".into()),
                    context_id: None,
                    value: json!("Desk lamp"),
                }],
            },
        )
        .await
        .unwrap();
    let updated = content_revision(&pool, record).await;
    assert!(updated > created);
    // An exporter's own annotations never advance the revision.
    let annotation = repository.for_extension(EXTENSION);
    api::attricat_service::AttricatMutationService::new(&annotation)
        .execute_extension_attricat_batch(
            serde_json::from_value(json!({
                "batch_key": "annotate",
                "dry_run": false,
                "intents": [{"kind": "annotate", "intent_key": "synced", "record_id": record, "add_tags": ["synced"]}]
            }))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM domain_events WHERE event_type='record.annotations_changed.v1'"
        )
        .await,
        1
    );
    assert_eq!(content_revision(&pool, record).await, updated);

    let begin = |revision| {
        let repository = repository.clone();
        async move {
            repository
                .export_mapping_begin(EXTENSION, None, record, revision, None, false, None, None)
                .await
                .unwrap()
        }
    };
    let ExportBegin::Proceed { lease_token, .. } = begin(updated).await else {
        panic!("a first push proceeds");
    };
    // A concurrent worker is held off while the lease is live.
    assert!(matches!(begin(updated).await, ExportBegin::Busy { .. }));
    repository
        .export_mapping_commit(
            EXTENSION,
            None,
            record,
            Some(lease_token),
            ExportMappingUpdate {
                status: "synced".into(),
                record_revision: Some(updated),
                external_id: Some(Some("SKU-1".into())),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    // Reordered or duplicated events with an older or equal revision are
    // stale and never push again.
    assert!(matches!(begin(created).await, ExportBegin::Stale { .. }));
    assert!(matches!(begin(updated).await, ExportBegin::Stale { .. }));

    let begin_as = |holder: &'static str, force: bool, resync_since| {
        let repository = repository.clone();
        async move {
            repository
                .export_mapping_begin(
                    EXTENSION,
                    None,
                    record,
                    updated,
                    None,
                    force,
                    Some(holder),
                    resync_since,
                )
                .await
                .unwrap()
        }
    };
    // A resync leases a record settled before it started, and still does
    // after its own lease was released without a result.
    // The resync starts just after the record settled, by the database's
    // clock, which this process's clock can trail.
    let settled_at = repository
        .export_mapping_get(EXTENSION, None, api::repository::MappingKey::Record(record))
        .await
        .unwrap()
        .unwrap()
        .settled_at
        .unwrap();
    let resync_since = Some(settled_at + chrono::TimeDelta::milliseconds(1));
    let ExportBegin::Proceed { lease_token, .. } =
        begin_as("run:resync", false, resync_since).await
    else {
        panic!("a resync pushes a record settled before it");
    };
    assert!(
        repository
            .export_mapping_release(EXTENSION, None, record, lease_token)
            .await
            .unwrap()
    );
    assert!(matches!(
        begin_as("run:resync", false, resync_since).await,
        ExportBegin::Proceed { .. }
    ));
    // A retried invocation that finds its record stale ends its own lease.
    assert!(matches!(
        begin_as("run:resync", false, None).await,
        ExportBegin::Stale { .. }
    ));
    assert!(matches!(
        begin_as("handler:other", false, None).await,
        ExportBegin::Stale { .. }
    ));
    let mapping = repository
        .export_mapping_get(EXTENSION, None, api::repository::MappingKey::Record(record))
        .await
        .unwrap()
        .unwrap();
    assert!(mapping.lease_token.is_none());
    // A result the resync settles makes the record stale for it.
    let ExportBegin::Proceed { lease_token, .. } =
        begin_as("run:resync", false, resync_since).await
    else {
        panic!("the resync leases the record again");
    };
    repository
        .export_mapping_commit(
            EXTENSION,
            None,
            record,
            Some(lease_token),
            ExportMappingUpdate {
                status: "synced".into(),
                record_revision: Some(updated),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        begin_as("run:resync", false, resync_since).await,
        ExportBegin::Stale { .. }
    ));
    // A handler that gives up marks the records it still leases as retrying
    // and ends its leases.
    let ExportBegin::Proceed { .. } = begin_as("handler:gave-up", true, None).await else {
        panic!("a forced push leases the record");
    };
    assert_eq!(
        repository
            .fail_export_leases(EXTENSION, "handler:gave-up", "retries_exhausted", "busy")
            .await
            .unwrap(),
        1
    );
    let mapping = repository
        .export_mapping_get(EXTENSION, None, api::repository::MappingKey::Record(record))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(mapping.status, "retrying");
    assert!(mapping.lease_token.is_none());
    repository
        .export_mapping_commit(
            EXTENSION,
            None,
            record,
            None,
            ExportMappingUpdate {
                status: "synced".into(),
                record_revision: Some(updated),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    // A late result for an older revision cannot replace the newer one.
    let late = repository
        .export_mapping_commit(
            EXTENSION,
            None,
            record,
            None,
            ExportMappingUpdate {
                status: "synced".into(),
                record_revision: Some(created),
                ..Default::default()
            },
        )
        .await;
    assert!(late.is_err());
    // Nor can a late rejection of an older revision.
    repository
        .record_export_rejection(
            EXTENSION,
            None,
            record,
            Some(created),
            None,
            "missing_gtin",
            "GTIN is required",
        )
        .await
        .unwrap();
    assert_eq!(mapping_status(&pool, record).await.unwrap().0, "synced");
    // A late push cannot replace a newer withdrawal either.
    repository
        .export_mapping_commit(
            EXTENSION,
            None,
            record,
            None,
            ExportMappingUpdate {
                status: "withdrawn".into(),
                record_revision: Some(updated),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let late = repository
        .export_mapping_commit(
            EXTENSION,
            None,
            record,
            None,
            ExportMappingUpdate {
                status: "synced".into(),
                record_revision: Some(created),
                ..Default::default()
            },
        )
        .await;
    assert!(late.is_err());
    assert_eq!(mapping_status(&pool, record).await.unwrap().0, "withdrawn");
    // Lookups by external ID serve reconciliation.
    let found = repository
        .export_mapping_get(
            EXTENSION,
            None,
            api::repository::MappingKey::ExternalId("SKU-1"),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.record_id, record);

    // A newer revision still being retried holds off an older one, whose
    // commit would be refused after it pushed.
    repository
        .export_mapping_commit(
            EXTENSION,
            None,
            record,
            None,
            ExportMappingUpdate {
                status: "retrying".into(),
                record_revision: Some(updated),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(matches!(begin(created).await, ExportBegin::Stale { .. }));
    assert!(matches!(begin(updated).await, ExportBegin::Proceed { .. }));

    // A deleted record is never selected for a retry, since no run would
    // push it.
    repository.delete_record(record).await.unwrap();
    let retried = repository
        .request_export_retry(
            EXTENSION,
            api::repository::ExportRetrySelection {
                connection: None,
                record_ids: None,
                status: Some("retrying".into()),
                request_key: None,
            },
        )
        .await
        .unwrap();
    assert!(retried.is_empty());
    assert_eq!(mapping_status(&pool, record).await.unwrap().0, "retrying");
    // A deleted record is never pushed again: only its withdrawal is
    // recorded.
    assert!(matches!(begin(updated).await, ExportBegin::Stale { .. }));
    let commit = |status: &'static str| {
        let repository = repository.clone();
        async move {
            repository
                .export_mapping_commit(
                    EXTENSION,
                    None,
                    record,
                    None,
                    ExportMappingUpdate {
                        status: status.into(),
                        record_revision: Some(updated),
                        ..Default::default()
                    },
                )
                .await
        }
    };
    assert!(commit("synced").await.is_err());
    // The lease an earlier step left ends first.
    sqlx::query("UPDATE extension_export_mappings SET lease_until=now()-interval '1 second' WHERE record_id=$1")
        .bind(record)
        .execute(&pool)
        .await
        .unwrap();
    commit("withdrawn").await.unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn leases_yield_to_rejections_and_keep_mid_push_retries(pool: sqlx::PgPool) {
    let repository = workspace_repository(&pool).await;
    let store = Arc::new(FakeObjectStore::available());
    install_extension(&repository, store, EXTENSION, &archive(), PERMISSIONS).await;
    let blueprint = published_blueprint(&repository, VARIANT_BLUEPRINT).await;
    let record = titled_record(&repository, blueprint, "Lamp").await;
    let revision = content_revision(&pool, record).await;
    let begin = |holder: &'static str| {
        let repository = repository.clone();
        async move {
            match repository
                .export_mapping_begin(
                    EXTENSION,
                    None,
                    record,
                    revision,
                    None,
                    false,
                    Some(holder),
                    None,
                )
                .await
                .unwrap()
            {
                ExportBegin::Proceed { lease_token, .. } => Some(lease_token),
                _ => None,
            }
        }
    };
    let reject = |holder: &'static str| {
        let repository = repository.clone();
        async move {
            repository
                .record_export_rejection(
                    EXTENSION,
                    None,
                    record,
                    None,
                    Some(holder),
                    "missing_gtin",
                    "GTIN is required",
                )
                .await
                .unwrap()
        }
    };
    // Another invocation's rejection leaves the record to the lease holder.
    let lease = begin("run:a").await.expect("a first push proceeds");
    // Another invocation is held off; the same one takes its lease back.
    assert!(begin("handler:b").await.is_none(), "another holder is busy");
    let lease = begin("run:a")
        .await
        .inspect(|retaken| assert_eq!(*retaken, lease, "it keeps its token"))
        .expect("a retried holder takes its lease back");
    reject("handler:b").await;
    assert_eq!(mapping_status(&pool, record).await.unwrap().0, "pending");
    // The holder's own rejection settles the revision it leased and ends
    // the lease, although it names no revision.
    reject("run:a").await;
    assert_eq!(mapping_status(&pool, record).await.unwrap().0, "rejected");
    let (rejected_at, leased): (i64, Option<Uuid>) = sqlx::query_as(
        "SELECT record_revision,lease_token FROM extension_export_mappings WHERE record_id=$1",
    )
    .bind(record)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((rejected_at, leased), (revision, None));
    assert!(
        repository
            .export_mapping_commit(
                EXTENSION,
                None,
                record,
                Some(lease),
                ExportMappingUpdate {
                    status: "synced".into(),
                    ..Default::default()
                }
            )
            .await
            .is_err(),
        "the ended lease cannot commit"
    );

    // A retry requested while a push is under way outlives that push.
    repository
        .request_export_retry(
            EXTENSION,
            ExportRetrySelection {
                connection: None,
                record_ids: Some(vec![record]),
                status: None,
                request_key: None,
            },
        )
        .await
        .unwrap();
    let lease = begin("run:a").await.expect("a retried record proceeds");
    repository
        .request_export_retry(
            EXTENSION,
            ExportRetrySelection {
                connection: None,
                record_ids: Some(vec![record]),
                status: None,
                request_key: None,
            },
        )
        .await
        .unwrap();
    repository
        .export_mapping_commit(
            EXTENSION,
            None,
            record,
            Some(lease),
            ExportMappingUpdate {
                status: "synced".into(),
                record_revision: Some(revision),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(
        begin("run:a").await.is_some(),
        "the retry requested mid-push is still pending"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn leases_survive_a_retried_begin_and_withdrawals_can_be_undone(pool: sqlx::PgPool) {
    let repository = workspace_repository(&pool).await;
    let store = Arc::new(FakeObjectStore::available());
    install_extension(&repository, store, EXTENSION, &archive(), PERMISSIONS).await;
    let blueprint = published_blueprint(&repository, VARIANT_BLUEPRINT).await;
    let record = titled_record(&repository, blueprint, "Lamp").await;
    let revision = content_revision(&pool, record).await;
    let begin = |holder: &'static str| {
        let repository = repository.clone();
        async move {
            match repository
                .export_mapping_begin(
                    EXTENSION,
                    None,
                    record,
                    revision,
                    None,
                    false,
                    Some(holder),
                    None,
                )
                .await
                .unwrap()
            {
                ExportBegin::Proceed { lease_token, .. } => Some(lease_token),
                _ => None,
            }
        }
    };
    // The same invocation beginning again keeps its token, so an earlier
    // attempt still pushing can commit.
    let first = begin("run:a").await.unwrap();
    assert_eq!(begin("run:a").await, Some(first));
    repository
        .export_mapping_commit(
            EXTENSION,
            None,
            record,
            Some(first),
            ExportMappingUpdate {
                status: "withdrawn".into(),
                record_revision: Some(revision),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    // A record published again unchanged comes back at the revision it was
    // withdrawn at, and is pushed again.
    assert!(begin("handler:published").await.is_some());
}

#[sqlx::test(migrations = "./migrations")]
async fn a_keyed_retry_sent_again_retries_nothing_twice(pool: sqlx::PgPool) {
    let repository = workspace_repository(&pool).await;
    let store = Arc::new(FakeObjectStore::available());
    install_extension(&repository, store, EXTENSION, &archive(), PERMISSIONS).await;
    let blueprint = published_blueprint(&repository, VARIANT_BLUEPRINT).await;
    let record = titled_record(&repository, blueprint, "Lamp").await;
    repository
        .record_export_rejection(EXTENSION, None, record, None, None, "gtin", "GTIN missing")
        .await
        .unwrap();
    let retry = |request_key: Option<&'static str>| {
        let repository = repository.clone();
        async move {
            repository
                .request_export_retry(
                    EXTENSION,
                    ExportRetrySelection {
                        connection: None,
                        record_ids: None,
                        status: Some("rejected".into()),
                        request_key: request_key.map(str::to_owned),
                    },
                )
                .await
                .unwrap()
        }
    };
    let events = || {
        count(
            &pool,
            "SELECT count(*) FROM domain_events WHERE event_type='export.retry_requested.v1'",
        )
    };
    assert_eq!(retry(Some("k")).await, vec![record]);
    assert_eq!(events().await, 1);
    // The record is pending now, so the status selects nothing; the request
    // sent again still finds what it retried, and emits nothing new.
    assert_eq!(retry(Some("k")).await, vec![record]);
    assert_eq!(events().await, 1);
    assert_eq!(retry(None).await, Vec::<Uuid>::new());
    // A withdrawal is closed out, not retried.
    assert!(
        repository
            .request_export_retry(
                EXTENSION,
                ExportRetrySelection {
                    connection: None,
                    record_ids: None,
                    status: Some("withdrawn".into()),
                    request_key: None,
                },
            )
            .await
            .is_err()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn catalog_pages_pair_snapshot_values_with_snapshot_revisions(pool: sqlx::PgPool) {
    let repository = workspace_repository(&pool).await;
    let store = Arc::new(FakeObjectStore::available());
    install_extension(&repository, store, EXTENSION, &archive(), PERMISSIONS).await;
    let blueprint = published_blueprint(&repository, VARIANT_BLUEPRINT).await;
    titled_record(&repository, blueprint, "Lamp").await;
    let chair = titled_record(&repository, blueprint, "Chair").await;
    let created = content_revision(&pool, chair).await;
    let request = |cursor| ExtensionAttricatPageRequest {
        blueprint_id: blueprint.0,
        blueprint_version: blueprint.1,
        context_id: None,
        publication_context_id: None,
        cursor,
        limit: 1,
    };
    let first = repository
        .extension_attricat_page(request(None))
        .await
        .unwrap();
    // The chair is edited while the exporter is between pages.
    set_title(&repository, chair, "Armchair").await;
    let edited = content_revision(&pool, chair).await;
    assert!(edited > created);
    let second = repository
        .extension_attricat_page(request(first.next_cursor))
        .await
        .unwrap();
    assert_eq!(second.records[0].id, chair);
    // The page's values are the snapshot's, so its revision is too: the
    // edit's revision stays unexported until the edit itself is.
    assert_eq!(second.records[0].content_revision, Some(created));
    let values = repository
        .extension_attricat_values_at(chair, second.snapshot_at)
        .await
        .unwrap();
    assert!(values.iter().any(|value| value.value == json!("Chair")));
    // A run that started before the edit reports rejections at the revision
    // it can have read.
    assert_eq!(
        repository
            .record_content_revision_at(chair, first.snapshot_at, None)
            .await
            .unwrap(),
        created
    );
    // Now by the database's clock, which this process's clock can trail.
    let now: chrono::DateTime<chrono::Utc> = sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        repository
            .record_content_revision_at(chair, now, None)
            .await
            .unwrap(),
        edited
    );
}

/// Creates an enabled publication channel below the default context.
async fn child_channel(repository: &AttricatRepository, pool: &sqlx::PgPool, code: &str) -> Uuid {
    let default: Uuid = sqlx::query_scalar(
        "SELECT id FROM attribute_contexts WHERE workspace_id=$1 AND parent_id IS NULL ORDER BY code LIMIT 1",
    )
    .bind(bootstrap_workspace_id())
    .fetch_one(pool)
    .await
    .unwrap();
    let context = repository
        .create_context(CreateAttributeContext {
            code: code.into(),
            data: json!({}),
            parent_id: Some(default),
        })
        .await
        .unwrap();
    repository
        .set_publication_channel(context.id, true)
        .await
        .unwrap();
    context.id
}

async fn publish(pool: &sqlx::PgPool, record_id: Uuid, channel: Uuid) {
    let publisher = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,email) VALUES($1,$2)")
        .bind(publisher)
        .bind(format!("{publisher}@example.test"))
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO record_channel_publications(workspace_id,record_id,context_id,published_at,published_by_user_id) VALUES($1,$2,$3,now(),$4)")
        .bind(bootstrap_workspace_id())
        .bind(record_id)
        .bind(channel)
        .bind(publisher)
        .execute(pool)
        .await
        .unwrap();
}

/// Drains operation tasks until no run is pending, waiting out retries
/// scheduled slightly in the future.
async fn drain_operations(
    repository: &AttricatRepository,
    pool: &sqlx::PgPool,
    store: Arc<FakeObjectStore>,
) {
    for _ in 0..50 {
        drain_extension_operations(repository, store.clone()).await;
        if count(
            pool,
            "SELECT count(*) FROM extension_operation_runs WHERE status IN ('pending','leased')",
        )
        .await
            == 0
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn run_status(pool: &sqlx::PgPool, run: Uuid) -> (String, Value, Option<String>) {
    sqlx::query_as(
        "SELECT status,checkpoint,last_error_code FROM extension_operation_runs WHERE id=$1",
    )
    .bind(run)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn connector_runs_read_resolved_rows_close_out_withdrawals_and_resync(pool: sqlx::PgPool) {
    let repository = workspace_repository(&pool).await;
    let store = Arc::new(FakeObjectStore::available());
    install_extension(
        &repository,
        store.clone(),
        EXTENSION,
        &archive(),
        PERMISSIONS,
    )
    .await;
    let channel = child_channel(&repository, &pool, "marketplace").await;
    let variants = published_blueprint(&repository, VARIANT_BLUEPRINT).await;
    let products = published_blueprint(&repository, &product_blueprint(EXPORT_JOB)).await;
    let variant = titled_record(&repository, variants, "Blue / M").await;
    publish(&pool, variant, channel).await;
    let mut product_ids = Vec::new();
    for title in ["Shirt", "Trousers"] {
        let product = repository
            .create_record_with_values(
                products.0,
                products.1,
                vec![
                    NewAttributeValue::Scalar {
                        attribute_id: None,
                        attribute_code: Some("title".into()),
                        context_id: None,
                        value: json!(title),
                    },
                    NewAttributeValue::Relationship {
                        attribute_id: None,
                        attribute_code: Some("variants".into()),
                        context_id: None,
                        target_record_id: variant,
                    },
                ],
                vec![],
                json!({}),
            )
            .await
            .unwrap()
            .id;
        publish(&pool, product, channel).await;
        product_ids.push(product);
    }
    let job = repository
        .list_blueprint_connector_jobs(products.0)
        .await
        .unwrap()
        .remove(0);

    let runs = repository
        .run_blueprint_connector_job(job.id, "first")
        .await
        .unwrap();
    assert_eq!(runs.len(), 1);
    drain_operations(&repository, &pool, store.clone()).await;
    let (status, checkpoint, _) = run_status(&pool, runs[0]).await;
    assert_eq!(status, "completed", "{checkpoint}");
    // The first batch deferred itself once; the run still completed.
    assert_eq!(checkpoint["deferred"], true);
    let rows = checkpoint["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    for row in rows {
        let row = &row["row"];
        // Values set in the default context resolve in the child channel.
        assert!(["Shirt", "Trousers"].contains(&row["values"]["title"].as_str().unwrap()));
        assert_eq!(row["relationships"]["variants"], json!([variant]));
        assert_eq!(row["related"]["variants"][0]["record_id"], json!(variant));
        assert_eq!(row["related"]["variants"][0]["values"]["title"], "Blue / M");
        assert_eq!(row["related"]["variants"][0]["published"], true);
        assert!(row["content_revision"].as_i64().unwrap() > 0);
    }
    for product in &product_ids {
        assert_eq!(mapping_status(&pool, *product).await.unwrap().0, "synced");
        // Scoped runs record sync status as annotations of their own namespace.
        let tags: Vec<String> = sqlx::query_scalar("SELECT system_tags FROM records WHERE id=$1")
            .bind(product)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(tags.contains(&format!("{EXTENSION}:exported")));
    }

    // Unpublishing is visible to the next run, which withdraws the item.
    repository
        .unpublish_record(product_ids[0], channel)
        .await
        .unwrap();
    let runs = repository
        .run_blueprint_connector_job(job.id, "second")
        .await
        .unwrap();
    drain_operations(&repository, &pool, store.clone()).await;
    let (status, checkpoint, _) = run_status(&pool, runs[0]).await;
    assert_eq!(status, "completed");
    assert_eq!(checkpoint["rows"].as_array().unwrap().len(), 1);
    assert_eq!(
        checkpoint["withdrawn"][0]["record_id"],
        json!(product_ids[0])
    );
    assert_eq!(checkpoint["withdrawn"][0]["reason"], "unpublished");
    assert_eq!(
        mapping_status(&pool, product_ids[0]).await.unwrap().0,
        "withdrawn"
    );
    // The unchanged product was not pushed again.
    assert_eq!(checkpoint["rows"][0]["outcome"], "stale");

    // A resync of one record re-reads only it and skips it by content hash.
    let runs = repository
        .resync_blueprint_connector_job(job.id, "resync", None, Some(vec![product_ids[1]]))
        .await
        .unwrap();
    let mode: String =
        sqlx::query_scalar("SELECT connector_mode FROM extension_operation_runs WHERE id=$1")
            .bind(runs[0])
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(mode, "resync");
    drain_operations(&repository, &pool, store).await;
    let (status, checkpoint, _) = run_status(&pool, runs[0]).await;
    assert_eq!(status, "completed");
    let rows = checkpoint["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["record_id"], json!(product_ids[1]));
    // The synced record was leased again rather than found stale, and the
    // component skipped it by its unchanged content hash.
    assert_eq!(rows[0]["outcome"], "unchanged");
    // A resync leaves nothing behind for later runs: once settled, the record
    // is stale again outside a resync.
    let revision = content_revision(&pool, product_ids[1]).await;
    assert!(matches!(
        repository
            .export_mapping_begin(
                EXTENSION,
                None,
                product_ids[1],
                revision,
                None,
                false,
                None,
                None
            )
            .await
            .unwrap(),
        ExportBegin::Stale { .. }
    ));

    // A run confined to selected records reads each one's withdrawal, a page
    // at a time, and keeps the channel's position, so a full run still sees
    // the rest.
    let run = Uuid::new_v4();
    let withdrawn = |selection: Vec<Uuid>, run: Uuid, cursor: Option<String>| {
        let repository = repository.clone();
        async move {
            repository
                .extension_withdrawn_records(
                    products.0,
                    Some(channel),
                    Some((&selection, run)),
                    cursor.as_deref(),
                    1,
                )
                .await
                .unwrap()
        }
    };
    let unselected = withdrawn(vec![product_ids[1]], run, None).await;
    assert_eq!(unselected["records"], json!([]));
    assert_eq!(unselected["more"], false);
    let selected = withdrawn(vec![product_ids[0]], run, None).await;
    assert_eq!(selected["records"][0]["record_id"], json!(product_ids[0]));
    assert_eq!(selected["more"], true);
    let cursor = selected["next_cursor"].as_str().map(str::to_owned);
    let rest = withdrawn(vec![product_ids[0]], run, cursor.clone()).await;
    assert_eq!(rest["records"], json!([]));
    assert_eq!(rest["more"], false);
    // Another run starts its selection again.
    let again = withdrawn(vec![product_ids[0]], Uuid::new_v4(), cursor.clone()).await;
    assert_eq!(again["records"][0]["record_id"], json!(product_ids[0]));
    let full = repository
        .extension_withdrawn_records(products.0, Some(channel), None, cursor.as_deref(), 16)
        .await
        .unwrap();
    assert_eq!(full["records"][0]["record_id"], json!(product_ids[0]));

    // An idempotency key names one selection: reusing it for another
    // selection starts another resync, and repeating a request finds its run.
    let resync = |selection: &'static str| {
        let repository = repository.clone();
        let records = Some(vec![product_ids[1]]);
        async move {
            repository
                .resync_blueprint_connector_job(job.id, "key", Some(selection), records)
                .await
                .unwrap()
        }
    };
    let first = resync("status:synced").await;
    assert_eq!(resync("status:synced").await, first);
    assert_ne!(resync("status:rejected").await, first);
}

#[sqlx::test(migrations = "./migrations")]
async fn batch_failures_retry_reject_or_fault_by_their_kind(pool: sqlx::PgPool) {
    let repository = workspace_repository(&pool).await;
    let store = Arc::new(FakeObjectStore::available());
    install_extension(
        &repository,
        store.clone(),
        EXTENSION,
        &archive(),
        PERMISSIONS,
    )
    .await;
    let channel = child_channel(&repository, &pool, "marketplace").await;
    published_blueprint(&repository, VARIANT_BLUEPRINT).await;
    let products = published_blueprint(&repository, &product_blueprint("")).await;
    let job_for = |fail: &str| {
        format!(
            r#"
[[connector_jobs]]
code = "fail_{fail}"
direction = "export"
extension_id = "acme.exporter"
operation_id = "export"
input = {{fail = "{fail}"}}
"#
        )
    };
    let revision = repository
        .create_blueprint_revision(
            products.0,
            CreateBlueprint {
                definition: product_blueprint(&format!(
                    "{}{}{}{}{}{}",
                    job_for("none"),
                    job_for("retry"),
                    job_for("reject"),
                    job_for("prepare"),
                    job_for("fault"),
                    job_for("retry_always")
                )),
            },
        )
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(products.0, revision.blueprint.version)
        .await
        .unwrap();
    let jobs = repository
        .list_blueprint_connector_jobs(products.0)
        .await
        .unwrap();
    let job = |code: &str| {
        jobs.iter()
            .find(|job| job.code.as_deref() == Some(code))
            .unwrap()
            .id
    };

    let retried = repository
        .run_blueprint_connector_job(job("fail_retry"), "retry")
        .await
        .unwrap();
    drain_operations(&repository, &pool, store.clone()).await;
    assert_eq!(run_status(&pool, retried[0]).await.0, "completed");

    // A retry delay defers the batch without spending the failure budget,
    // but only so many times in a row.
    let throttled = repository
        .run_blueprint_connector_job(job("fail_retry_always"), "retry_always")
        .await
        .unwrap();
    drain_extension_operations(&repository, store.clone()).await;
    let deferrals = || async {
        sqlx::query_as::<_, (i32, Option<String>)>(
            "SELECT deferrals,last_error_code FROM extension_operation_runs WHERE id=$1",
        )
        .bind(throttled[0])
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    let (deferred, code) = deferrals().await;
    assert!(deferred >= 1, "{deferred}");
    assert_eq!(code.as_deref(), Some("retry"));
    sqlx::query("UPDATE extension_operation_runs SET deferrals=100 WHERE id=$1")
        .bind(throttled[0])
        .execute(&pool)
        .await
        .unwrap();
    // The deferral made the task due by this process's clock, which the
    // database clock can trail; make it due on both.
    sqlx::query("UPDATE tasks SET available_at=now()-interval '1 minute' WHERE subject_id=$1 AND status='queued'")
        .bind(throttled[0])
        .execute(&pool)
        .await
        .unwrap();
    drain_extension_operations(&repository, store.clone()).await;
    assert_eq!(deferrals().await, (100, Some("operation".to_owned())));
    // Settle it so later drains do not wait on its backoff.
    sqlx::query("UPDATE tasks SET status='dead_letter',failed_at=now() WHERE subject_id=$1")
        .bind(throttled[0])
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE extension_operation_runs SET status='dead_letter' WHERE id=$1")
        .bind(throttled[0])
        .execute(&pool)
        .await
        .unwrap();

    let rejected = repository
        .run_blueprint_connector_job(job("fail_reject"), "reject")
        .await
        .unwrap();
    drain_operations(&repository, &pool, store.clone()).await;
    let (status, _, code) = run_status(&pool, rejected[0]).await;
    assert_eq!(
        (status.as_str(), code.as_deref()),
        ("dead_letter", Some("rejected"))
    );
    assert_eq!(installation_state(&pool).await, "enabled");
    // A replayed run gets a fresh deferral budget.
    sqlx::query("UPDATE extension_operation_runs SET deferrals=100 WHERE id=$1")
        .bind(rejected[0])
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        repository
            .replay_extension_operation(rejected[0])
            .await
            .unwrap()
    );
    let replayed: (i32, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as("SELECT deferrals,resume_at FROM extension_operation_runs WHERE id=$1")
            .bind(rejected[0])
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(replayed, (0, None));
    drain_operations(&repository, &pool, store.clone()).await;

    // A rejection of one record settles that record; the run continues.
    let current = (products.0, revision.blueprint.version);
    let invalid = titled_record(&repository, current, "reject: no GTIN").await;
    let valid = titled_record(&repository, current, "Shirt").await;
    publish(&pool, invalid, channel).await;
    publish(&pool, valid, channel).await;
    let partial = repository
        .run_blueprint_connector_job(job("fail_none"), "partial")
        .await
        .unwrap();
    drain_operations(&repository, &pool, store.clone()).await;
    let (status, checkpoint, _) = run_status(&pool, partial[0]).await;
    assert_eq!(status, "completed", "{checkpoint}");
    assert_eq!(mapping_status(&pool, invalid).await.unwrap().0, "rejected");
    assert_eq!(mapping_status(&pool, valid).await.unwrap().0, "synced");

    // An error returned by a lifecycle call fails the run without
    // quarantining the extension.
    let unprepared = repository
        .run_blueprint_connector_job(job("fail_prepare"), "prepare")
        .await
        .unwrap();
    drain_operations(&repository, &pool, store.clone()).await;
    let (status, _, _) = run_status(&pool, unprepared[0]).await;
    assert!(
        matches!(status.as_str(), "failed" | "dead_letter"),
        "{status}"
    );
    assert_eq!(installation_state(&pool).await, "enabled");

    let faulted = repository
        .run_blueprint_connector_job(job("fail_fault"), "fault")
        .await
        .unwrap();
    drain_operations(&repository, &pool, store).await;
    assert_ne!(run_status(&pool, faulted[0]).await.0, "completed");
    assert_eq!(installation_state(&pool).await, "quarantined");
}

#[sqlx::test(migrations = "./migrations")]
async fn export_status_preview_and_retry_are_served_over_http(pool: sqlx::PgPool) {
    let repository = workspace_repository(&pool).await;
    let store = Arc::new(FakeObjectStore::available());
    install_extension(
        &repository,
        store.clone(),
        EXTENSION,
        &archive(),
        PERMISSIONS,
    )
    .await;
    let blueprint = published_blueprint(&repository, VARIANT_BLUEPRINT).await;
    let record = titled_record(&repository, blueprint, "Chair").await;
    titled_record(&repository, blueprint, "Table").await;
    let rejection = |connection: Option<&'static str>| {
        let repository = repository.clone();
        async move {
            repository
                .record_export_rejection(
                    EXTENSION,
                    connection,
                    record,
                    None,
                    None,
                    "missing_gtin",
                    "GTIN is required",
                )
                .await
                .unwrap()
        }
    };
    assert_eq!(rejection(None).await, RejectionRecord::Settled);
    // A repeat changes nothing, and an unknown connection cannot be recorded.
    assert_eq!(rejection(None).await, RejectionRecord::Unchanged);
    // The repeat wrote nothing: still one attempt.
    let attempts: i32 =
        sqlx::query_scalar("SELECT attempts FROM extension_export_mappings WHERE record_id=$1")
            .bind(record)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(attempts, 1);
    assert_eq!(rejection(Some("gone")).await, RejectionRecord::Refused);
    // Retrying needs an extension that keeps export mappings.
    assert!(matches!(
        repository
            .request_export_retry(
                "acme.unknown",
                ExportRetrySelection {
                    connection: None,
                    record_ids: Some(vec![record]),
                    status: None,
                    request_key: None,
                },
            )
            .await,
        Err(api::repository::RepositoryError::NotFound(_))
    ));
    let (base_url, server) = start_server_with_object_store(pool.clone(), store).await;
    let client = authenticated_client();

    // The extension's status page names records across the workspace, so
    // `extensions.read` alone is not enough.
    let reader = member_with_role(
        &pool,
        create_role(&pool, "extension_reader", &["extensions.read"]).await,
    )
    .await;
    expect_status(
        client_for(reader)
            .get(format!("{base_url}/extensions/{EXTENSION}/export-status"))
            .send()
            .await
            .unwrap(),
        reqwest::StatusCode::FORBIDDEN,
    )
    .await;

    // The Explorer filters by export status.
    let found = expect_status(
        client
            .post(format!("{base_url}/v1/records/search"))
            .json(&json!({
                "blueprint": {"code": "export_variant"},
                "export_status": {"extension_id": EXTENSION, "status": "rejected"},
            }))
            .send()
            .await
            .unwrap(),
        reqwest::StatusCode::OK,
    )
    .await;
    let ids: Vec<&str> = found["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec![record.to_string()]);

    let record_status = get_json(
        &client,
        format!("{base_url}/v1/records/{record}/export-status"),
    )
    .await;
    let statuses = &record_status["statuses"];
    assert_eq!(statuses[0]["extension_id"], EXTENSION);
    assert_eq!(statuses[0]["status"], "rejected");
    assert_eq!(statuses[0]["error"]["code"], "missing_gtin");
    assert!(statuses[0].get("lease_token").is_none());
    assert_eq!(record_status["destinations"][0]["export_id"], "marketplace");
    assert_eq!(record_status["destinations"][0]["preview"], true);

    let overview = get_json(&client, format!("{base_url}/export-status")).await;
    assert_eq!(overview["counts"][0]["status"], "rejected");
    assert_eq!(overview["counts"][0]["count"], 1);

    let page = get_json(
        &client,
        format!("{base_url}/extensions/{EXTENSION}/export-status?status=rejected"),
    )
    .await;
    assert_eq!(page["mappings"][0]["record_id"], json!(record));

    let preview = expect_status(
        client
            .post(format!(
                "{base_url}/extensions/{EXTENSION}/exports/marketplace/preview"
            ))
            .json(&json!({"record_id": record}))
            .send()
            .await
            .unwrap(),
        reqwest::StatusCode::OK,
    )
    .await;
    assert_eq!(preview["payload"]["title"], "Chair");
    assert_eq!(preview["issues"], json!([]));

    let retry = expect_status(
        client
            .post(format!(
                "{base_url}/extensions/{EXTENSION}/export-status/retry"
            ))
            .json(&json!({"record_ids": [record]}))
            .send()
            .await
            .unwrap(),
        reqwest::StatusCode::ACCEPTED,
    )
    .await;
    assert_eq!(retry["record_ids"], json!([record]));
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM domain_events WHERE event_type='export.retry_requested.v1'"
        )
        .await,
        1
    );
    assert_eq!(mapping_status(&pool, record).await.unwrap().0, "pending");
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM audit_events WHERE action LIKE '%export_status.retry%'"
        )
        .await,
        1
    );
    server.abort();
}

async fn set_title(repository: &AttricatRepository, record: Uuid, title: &str) {
    repository
        .append_values(
            record,
            api::model::AppendAttributeValues {
                expected_updated_at: None,
                values: vec![NewAttributeValue::Scalar {
                    attribute_id: None,
                    attribute_code: Some("title".into()),
                    context_id: None,
                    value: json!(title),
                }],
            },
        )
        .await
        .unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn debounced_handlers_push_a_burst_of_edits_once(pool: sqlx::PgPool) {
    let repository = workspace_repository(&pool).await;
    repository
        .ensure_event_consumer(WASM_CONSUMER, &[])
        .await
        .unwrap();
    let store = Arc::new(FakeObjectStore::available());
    install_extension(
        &repository,
        store.clone(),
        EXTENSION,
        &archive(),
        PERMISSIONS,
    )
    .await;
    let blueprint = published_blueprint(&repository, VARIANT_BLUEPRINT).await;
    let record = titled_record(&repository, blueprint, "v1").await;
    for title in ["v2", "v3", "v4"] {
        set_title(&repository, record, title).await;
    }
    deliver_events(&repository, store).await;
    // One push for the creation, one for the whole burst of edits: the
    // earlier edits were superseded by the latest one.
    let deliveries: Vec<Value> = sqlx::query_scalar(
        "SELECT value FROM extension_storage_entries WHERE extension_id=$1 AND key LIKE 'delivered:%' ORDER BY (value->>'sequence')::bigint",
    )
    .bind(EXTENSION)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(deliveries.len(), 2, "{deliveries:?}");
    let latest: i64 = sqlx::query_scalar(
        "SELECT max(sequence) FROM domain_events WHERE aggregate_id=$1 AND event_type='attribute_value.changed.v1'",
    )
    .bind(record)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(deliveries[1]["sequence"], latest);
    let (revision,): (i64,) =
        sqlx::query_as("SELECT record_revision FROM extension_export_mappings WHERE record_id=$1")
            .bind(record)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(revision, content_revision(&pool, record).await);
    assert_eq!(installation_state(&pool).await, "enabled");
}

#[sqlx::test(migrations = "./migrations")]
async fn related_record_changes_trigger_and_advance_the_linking_row(pool: sqlx::PgPool) {
    let repository = workspace_repository(&pool).await;
    let store = Arc::new(FakeObjectStore::available());
    install_extension(
        &repository,
        store.clone(),
        EXTENSION,
        &archive(),
        PERMISSIONS,
    )
    .await;
    let channel = child_channel(&repository, &pool, "marketplace").await;
    let variants = published_blueprint(&repository, VARIANT_BLUEPRINT).await;
    let products = published_blueprint(
        &repository,
        &product_blueprint(
            r#"
[[connector_jobs]]
code = "near_real_time"
direction = "export"
extension_id = "acme.exporter"
operation_id = "export"
input = {expand = ["variants"]}
trigger = {events = ["attribute_value.changed.v1"], debounce_seconds = 1}
"#,
        ),
    )
    .await;
    let variant = titled_record(&repository, variants, "Blue / M").await;
    publish(&pool, variant, channel).await;
    let product = repository
        .create_record_with_values(
            products.0,
            products.1,
            vec![
                NewAttributeValue::Scalar {
                    attribute_id: None,
                    attribute_code: Some("title".into()),
                    context_id: None,
                    value: json!("Shirt"),
                },
                NewAttributeValue::Relationship {
                    attribute_id: None,
                    attribute_code: Some("variants".into()),
                    context_id: None,
                    target_record_id: variant,
                },
            ],
            vec![],
            json!({}),
        )
        .await
        .unwrap()
        .id;
    publish(&pool, product, channel).await;
    // Editing only the variant triggers a run of the product that links it.
    set_title(&repository, variant, "Navy / M").await;
    sqlx::query("UPDATE record_channel_publications SET published_at=now(),published_by_user_id=(SELECT id FROM users LIMIT 1) WHERE record_id=$1")
        .bind(variant)
        .execute(&pool)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(1100)).await;
    repository
        .produce_due_blueprint_connector_jobs()
        .await
        .unwrap();
    let (run, filter): (Uuid, Vec<Uuid>) =
        sqlx::query_as("SELECT id,connector_record_ids FROM extension_operation_runs")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(filter, vec![product]);
    drain_operations(&repository, &pool, store.clone()).await;
    let (status, checkpoint, _) = run_status(&pool, run).await;
    assert_eq!(status, "completed", "{checkpoint}");
    let row = &checkpoint["rows"][0]["row"];
    assert_eq!(row["related"]["variants"][0]["values"]["title"], "Navy / M");
    // The row embeds the variant, so its revision is the variant's newer one.
    let variant_revision = content_revision(&pool, variant).await;
    assert!(variant_revision > content_revision(&pool, product).await);
    assert_eq!(row["content_revision"], json!(variant_revision));
    assert_eq!(checkpoint["rows"][0]["outcome"], "pushed");

    // Deleting the variant leaves the link in place but drops it from the
    // row, so the deletion still makes the row newer than what was pushed.
    repository.delete_record(variant).await.unwrap();
    let deleted: i64 = sqlx::query_scalar(
        "SELECT sequence FROM domain_events WHERE aggregate_id=$1 AND event_type='record.deleted.v1'",
    )
    .bind(variant)
    .fetch_one(&pool)
    .await
    .unwrap();
    let job: Uuid = sqlx::query_scalar("SELECT id FROM blueprint_connector_jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    let runs = repository
        .run_blueprint_connector_job(job, "after-delete")
        .await
        .unwrap();
    drain_operations(&repository, &pool, store).await;
    let (status, checkpoint, _) = run_status(&pool, runs[0]).await;
    assert_eq!(status, "completed", "{checkpoint}");
    let row = &checkpoint["rows"][0];
    assert_eq!(row["row"]["related"]["variants"], json!([]));
    assert_eq!(row["row"]["content_revision"], json!(deleted));
    assert_eq!(row["outcome"], "pushed");

    // Mappings keep one status per record and connection, so a job of an
    // extension with mappings exports one channel: with several enabled it
    // must name its channel as `context`.
    child_channel(&repository, &pool, "web").await;
    let refused = repository
        .run_blueprint_connector_job(job, "two-channels")
        .await
        .unwrap_err();
    assert!(refused.to_string().contains("context"), "{refused}");
}

#[sqlx::test(migrations = "./migrations")]
async fn change_triggered_jobs_export_only_the_changed_records(pool: sqlx::PgPool) {
    let repository = workspace_repository(&pool).await;
    let store = Arc::new(FakeObjectStore::available());
    install_extension(
        &repository,
        store.clone(),
        EXTENSION,
        &archive(),
        PERMISSIONS,
    )
    .await;
    let channel = child_channel(&repository, &pool, "marketplace").await;
    published_blueprint(&repository, VARIANT_BLUEPRINT).await;
    let products = published_blueprint(
        &repository,
        &product_blueprint(
            r#"
[[connector_jobs]]
code = "near_real_time"
direction = "export"
extension_id = "acme.exporter"
operation_id = "export"
input = {}
trigger = {events = ["attribute_value.changed.v1"], debounce_seconds = 1}
"#,
        ),
    )
    .await;
    let mut ids = Vec::new();
    for title in ["Hat", "Scarf"] {
        let id = titled_record(&repository, products, title).await;
        publish(&pool, id, channel).await;
        ids.push(id);
    }
    // Creation events are not triggers; nothing runs yet.
    repository
        .produce_due_blueprint_connector_jobs()
        .await
        .unwrap();
    assert_eq!(
        count(&pool, "SELECT count(*) FROM extension_operation_runs").await,
        0
    );
    set_title(&repository, ids[0], "Wool hat").await;
    // An edit withdraws publication approval; approve the new content again.
    sqlx::query("UPDATE record_channel_publications SET published_at=now(),published_by_user_id=(SELECT id FROM users LIMIT 1) WHERE record_id=$1")
        .bind(ids[0])
        .execute(&pool)
        .await
        .unwrap();
    // The change waits out its debounce.
    repository
        .produce_due_blueprint_connector_jobs()
        .await
        .unwrap();
    assert_eq!(
        count(&pool, "SELECT count(*) FROM extension_operation_runs").await,
        0
    );
    tokio::time::sleep(Duration::from_millis(1100)).await;
    repository
        .produce_due_blueprint_connector_jobs()
        .await
        .unwrap();
    let (run, mode, filter): (Uuid, String, Vec<Uuid>) = sqlx::query_as(
        "SELECT id,connector_mode,connector_record_ids FROM extension_operation_runs",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((mode.as_str(), filter), ("triggered", vec![ids[0]]));
    drain_operations(&repository, &pool, store).await;
    let (status, checkpoint, _) = run_status(&pool, run).await;
    assert_eq!(status, "completed");
    let rows = checkpoint["rows"]
        .as_array()
        .unwrap_or_else(|| panic!("{checkpoint}"));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["row"]["values"]["title"], "Wool hat");
    // Published again, the record is not reported as withdrawn.
    assert!(checkpoint["withdrawn"].is_null(), "{checkpoint}");
    // The watermark moved past the change: no second run.
    repository
        .produce_due_blueprint_connector_jobs()
        .await
        .unwrap();
    assert_eq!(
        count(&pool, "SELECT count(*) FROM extension_operation_runs").await,
        1
    );

    // A change whose run cannot be queued keeps the watermark and is
    // exported once the job can run again.
    let watermark = || async {
        sqlx::query_scalar::<_, i64>(
            "SELECT trigger_watermark FROM blueprint_connector_jobs WHERE code='near_real_time'",
        )
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    let before = watermark().await;
    repository.disable_extension(EXTENSION).await.unwrap();
    set_title(&repository, ids[1], "Silk scarf").await;
    tokio::time::sleep(Duration::from_millis(1100)).await;
    repository
        .produce_due_blueprint_connector_jobs()
        .await
        .unwrap();
    assert_eq!(watermark().await, before);
    repository.enable_extension(EXTENSION).await.unwrap();
    // The failed attempt backs the job off, leaving the tick to other jobs.
    let runs = count(&pool, "SELECT count(*) FROM extension_operation_runs").await;
    repository
        .produce_due_blueprint_connector_jobs()
        .await
        .unwrap();
    assert_eq!(
        count(&pool, "SELECT count(*) FROM extension_operation_runs").await,
        runs
    );
    sqlx::query(
        "UPDATE blueprint_connector_jobs SET trigger_blocked_until=now()-interval '1 second'",
    )
    .execute(&pool)
    .await
    .unwrap();
    repository
        .produce_due_blueprint_connector_jobs()
        .await
        .unwrap();
    let filter: Vec<Uuid> = sqlx::query_scalar(
        "SELECT connector_record_ids FROM extension_operation_runs ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(filter, vec![ids[1]]);
    assert!(watermark().await > before);

    // Steady edits cannot hold a run back forever: once the oldest pending
    // change has waited ten debounces, the job runs although the newest is
    // still inside its debounce.
    set_title(&repository, ids[0], "Felt hat").await;
    sqlx::query(
        "UPDATE domain_events SET occurred_at=now()-interval '20 seconds' WHERE sequence>$1",
    )
    .bind(watermark().await)
    .execute(&pool)
    .await
    .unwrap();
    set_title(&repository, ids[1], "Linen scarf").await;
    let runs = count(&pool, "SELECT count(*) FROM extension_operation_runs").await;
    repository
        .produce_due_blueprint_connector_jobs()
        .await
        .unwrap();
    assert_eq!(
        count(&pool, "SELECT count(*) FROM extension_operation_runs").await,
        runs + 1
    );
}
