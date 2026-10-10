//! Repeatable database round-trip measurements for hot request and worker
//! paths. Counts come from `catalog_db_round_trips_*` metrics, which the
//! telemetry layer records for every completed SQL statement.
//!
//! ```sh
//! set -a; source .env; set +a
//! OTEL_EXPORTER_OTLP_TRACES_ENDPOINT= RUST_LOG=warn cargo test -p api --test round_trip_baseline -- --ignored --nocapture
//! ```
//!
//! Set `ROUND_TRIP_EXTENSION_ARCHIVE` to a packaged extension (for example the
//! sibling `attricat-extension-example` `dist/*.tar.zst`) to measure its event
//! handler; otherwise the unified test component is built and installed.

mod support;

use std::{
    collections::HashMap,
    io::Cursor,
    path::PathBuf,
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};

use api::{
    event_dispatcher::{self, DispatcherConfig, EventHandlerRegistry},
    extension_installer::ExtensionInstaller,
    extension_runtime::{
        self, ExtensionOperationTaskHandler, ExtensionRuntime, ExtensionRuntimeConfig,
        WasmExtensionTaskHandler,
    },
    repository::CatalogRepository,
    rule_runtime,
    storage::FakeObjectStore,
    task_worker::{self, TaskHandlerRegistry, TaskWorkerConfig},
    telemetry::{init_metrics, init_tracing},
    workflow_runtime,
};
use support::*;

const BLUEPRINT: &str = r#"
format_version = 1
code = "rt_product"
name = "Round-trip product"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[views.table]
type = "table"
columns = [
    { field = "name", label = "Name" },
    { field = "brand", label = "Brand" },
    { field = "price", label = "Price" },
    { field = "stock", label = "Stock" },
]

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "description"
value_type = "string"

[[attributes]]
code = "brand"
value_type = "string"

[[attributes]]
code = "color"
value_type = "string"

[[attributes]]
code = "material"
value_type = "string"

[[attributes]]
code = "sku"
value_type = "string"

[[attributes]]
code = "price"
value_type = "number"

[[attributes]]
code = "weight"
value_type = "number"

[[attributes]]
code = "stock"
value_type = "integer"

[[attributes]]
code = "available"
value_type = "boolean"
"#;

const UNIFIED_EXTENSION: &str = "acme.round_trips";

/// `(sum, count)` of the per-operation histogram and own-statement counters,
/// keyed by scope.
#[derive(Default)]
struct Snapshot {
    per_operation: HashMap<String, (f64, f64)>,
    totals: HashMap<String, f64>,
}

impl Snapshot {
    fn take() -> Self {
        let rendered = init_metrics().unwrap().render();
        let mut snapshot = Self::default();
        for line in rendered.lines() {
            let Some((series, value)) = line.rsplit_once(' ') else {
                continue;
            };
            let Ok(value) = value.parse::<f64>() else {
                continue;
            };
            let Some(scope) = series
                .split_once("scope=\"")
                .and_then(|(_, rest)| rest.split_once('"'))
                .map(|(scope, _)| scope.to_owned())
            else {
                continue;
            };
            if series.starts_with("catalog_db_round_trips_per_operation_sum") {
                snapshot.per_operation.entry(scope).or_default().0 = value;
            } else if series.starts_with("catalog_db_round_trips_per_operation_count") {
                snapshot.per_operation.entry(scope).or_default().1 = value;
            } else if series.starts_with("catalog_db_round_trips_total") {
                snapshot.totals.insert(scope, value);
            }
        }
        snapshot
    }

    /// Mean round trips per operation of `scope` since `before`.
    fn mean_since(&self, before: &Self, scope: &str) -> Option<f64> {
        let (sum, count) = self.per_operation.get(scope).copied()?;
        let (old_sum, old_count) = before.per_operation.get(scope).copied().unwrap_or_default();
        let operations = count - old_count;
        (operations > 0.0).then(|| (sum - old_sum) / operations)
    }

    fn statements_since(&self, before: &Self) -> f64 {
        self.totals
            .iter()
            .map(|(scope, value)| value - before.totals.get(scope).copied().unwrap_or_default())
            .sum()
    }
}

fn unified_component() -> Vec<u8> {
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
                "catalog-unified-test-component",
                "--target",
                "wasm32-unknown-unknown",
                "--release",
            ])
            .status()
            .unwrap()
            .success()
    );
    let core =
        root.join("target/wasm32-unknown-unknown/release/catalog_unified_test_component.wasm");
    let component = root.join("target/round-trip-unified.component.wasm");
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

fn unified_archive() -> (String, Vec<(&'static str, String)>, Vec<u8>) {
    let permissions = vec!["catalog.read".to_owned(), "events.subscribe".to_owned()];
    let manifest = json!({
        "manifest_version": 1,
        "name": "Round-trip probe",
        "version": "1.0.0",
        "description": "round-trip measurement",
        "icons": {"48": "icon.png"},
        "catalog": {"id": UNIFIED_EXTENSION, "host_api": ">=1.0.0, <2.0.0"},
        "permissions": permissions,
        "configuration": {"version": 1, "schema": {"type": "object", "additionalProperties": false}},
        "artifacts": [{"id": "server", "kind": "server_wasm", "path": "server.wasm"}],
        "server": {
            "event_handlers": [{"id": "on-update", "event_types": ["record.updated.v1"], "handler": "handle-event"}]
        }
    });
    let manifest = serde_json::to_vec(&manifest).unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "manifest.json", &manifest);
        append_file(&mut tar, "server.wasm", &unified_component());
        tar.finish().unwrap();
    }
    let archive = zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap();
    let grants = permissions
        .into_iter()
        .map(|permission| ("capability", permission))
        .collect();
    (UNIFIED_EXTENSION.to_owned(), grants, archive)
}

/// Reads the extension id and its required grants from a packaged archive.
fn packaged_archive(path: &str) -> (String, Vec<(&'static str, String)>, Vec<u8>) {
    let archive = std::fs::read(path).unwrap();
    let tar_bytes = zstd::stream::decode_all(Cursor::new(&archive)).unwrap();
    let mut tar = tar::Archive::new(Cursor::new(tar_bytes));
    let manifest: Value = tar
        .entries()
        .unwrap()
        .map(Result::unwrap)
        .find(|entry| entry.path().unwrap().to_str() == Some("manifest.json"))
        .map(|entry| serde_json::from_reader(entry).unwrap())
        .expect("archive has a manifest");
    let id = manifest["catalog"]["id"].as_str().unwrap().to_owned();
    let ids = |pointer: &str, field: Option<&str>| {
        manifest
            .pointer(pointer)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|item| {
                field
                    .map_or(item, |field| &item[field])
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect::<Vec<_>>()
    };
    let mut grants = Vec::new();
    grants.extend(
        ids("/permissions", None)
            .into_iter()
            .map(|id| ("capability", id)),
    );
    grants.extend(
        ids("/host_permissions", Some("id"))
            .into_iter()
            .map(|id| ("host_permission", id)),
    );
    grants.extend(
        ids("/event_contracts/exports", Some("id"))
            .into_iter()
            .map(|id| ("event_publish", id)),
    );
    (id, grants, archive)
}

fn bearer_client(secret: &str) -> Client {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "authorization",
        reqwest::header::HeaderValue::from_str(&format!("Bearer {secret}")).unwrap(),
    );
    Client::builder().default_headers(headers).build().unwrap()
}

fn values(seed: usize) -> Value {
    json!([
        { "kind": "scalar", "attribute_code": "name", "value": format!("Product {seed}") },
        { "kind": "scalar", "attribute_code": "description", "value": format!("Description {seed}") },
        { "kind": "scalar", "attribute_code": "brand", "value": if seed.is_multiple_of(2) { "Acme" } else { "Globex" } },
        { "kind": "scalar", "attribute_code": "color", "value": (["red", "green", "blue"][seed % 3]) },
        { "kind": "scalar", "attribute_code": "material", "value": "steel" },
        { "kind": "scalar", "attribute_code": "sku", "value": format!("SKU-{seed}") },
        { "kind": "scalar", "attribute_code": "price", "value": 10.5 * seed as f64 },
        { "kind": "scalar", "attribute_code": "weight", "value": 1.25 },
        { "kind": "scalar", "attribute_code": "stock", "value": seed as i64 },
        { "kind": "scalar", "attribute_code": "available", "value": seed.is_multiple_of(2) }
    ])
}

async fn wait_until(pool: &PgPool, sql: &str, timeout: Duration) -> bool {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if sqlx::query_scalar::<_, bool>(sql)
            .fetch_one(pool)
            .await
            .unwrap()
        {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

#[sqlx::test(migrations = "./migrations")]
#[ignore = "measurement harness; run explicitly with --ignored --nocapture"]
async fn database_round_trips(pool: PgPool) {
    init_tracing("round-trip-baseline").unwrap();
    let store = Arc::new(FakeObjectStore::available());
    let (base_url, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let owner = authenticated_client();
    let blueprint = create_blueprint(&owner, &base_url, BLUEPRINT).await;

    let permissions: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT permission_code FROM role_permissions WHERE role_id = '00000000-0000-4000-8000-000000000101' ORDER BY 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let token: Value = owner
        .post(format!("{base_url}/personal-access-tokens"))
        .json(&json!({ "label": "round trips", "permissions": permissions }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let client = bearer_client(token["secret"].as_str().unwrap());

    let mut record_ids = Vec::new();
    for seed in 0..30 {
        let record = create_record(&owner, &base_url, &blueprint).await;
        let id = record["id"].as_str().unwrap().to_owned();
        owner
            .put(format!("{base_url}/v1/records/{id}"))
            .json(&json!({ "values": values(seed) }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
        record_ids.push(id);
    }

    let mut report = Vec::<(String, String)>::new();

    // Explorer record search with three filters.
    let search = json!({
        "blueprint": { "code": "rt_product" },
        "filters": [
            { "field": "brand", "operator": "eq", "value": "Acme" },
            { "field": "price", "operator": "gte", "value": 20 },
            { "field": "available", "operator": "eq", "value": true }
        ],
        "sort": { "field": "price", "direction": "asc" },
        "page": { "size": 25, "cursor": null },
        "include_total": true
    });
    for run in ["cold", "warm"] {
        let before = Snapshot::take();
        let response = client
            .post(format!("{base_url}/v1/records/search"))
            .json(&search)
            .send()
            .await
            .unwrap();
        let status = response.status();
        assert!(
            status.is_success(),
            "{status}: {}",
            response.text().await.unwrap()
        );
        let after = Snapshot::take();
        report.push((
            format!("explorer search, 3 filters ({run})"),
            format!(
                "{:.0}",
                after
                    .mean_since(&before, "POST /v1/records/search")
                    .unwrap()
            ),
        ));
    }

    // Record update with ten values.
    for (run, seed) in [("cold", 100), ("warm", 101)] {
        let before = Snapshot::take();
        client
            .put(format!("{base_url}/v1/records/{}", record_ids[0]))
            .json(&json!({ "values": values(seed) }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
        let after = Snapshot::take();
        report.push((
            format!("record update, 10 values ({run})"),
            format!(
                "{:.0}",
                after
                    .mean_since(&before, "PUT /v1/records/{record_id}")
                    .unwrap()
            ),
        ));
    }

    // Extension event delivery through the real coordinator and task worker.
    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(BOOTSTRAP_WORKSPACE_ID.parse().unwrap())
        .await
        .unwrap();
    let (extension_id, grants, archive) = match std::env::var("ROUND_TRIP_EXTENSION_ARCHIVE") {
        Ok(path) => packaged_archive(&path),
        Err(_) => unified_archive(),
    };
    ExtensionInstaller::new(repository.clone(), store.clone())
        .install("round-trips", &archive)
        .await
        .unwrap();
    for (kind, grant) in &grants {
        repository
            .grant_extension(&extension_id, kind, grant)
            .await
            .unwrap();
    }
    repository.enable_extension(&extension_id).await.unwrap();

    let (shutdown_sender, shutdown) = tokio::sync::watch::channel(());
    let system = CatalogRepository::system(pool.clone());
    let runtime = ExtensionRuntime::new(store.clone(), ExtensionRuntimeConfig::default()).unwrap();
    let mut workers = vec![
        extension_runtime::start_event_delivery_coordinator(
            system.clone(),
            store.clone(),
            shutdown.clone(),
        ),
        workflow_runtime::start_schedule_coordinator(system.clone(), shutdown.clone()),
        rule_runtime::start_schedule_coordinator(system.clone(), shutdown.clone()),
    ];
    workers.extend(event_dispatcher::start(
        system.clone(),
        rule_runtime::add_to_registry(workflow_runtime::add_to_registry(
            EventHandlerRegistry::default_handlers(),
        )),
        DispatcherConfig::from_env().unwrap(),
        shutdown.clone(),
    ));
    let task_worker = task_worker::start(
        system.clone(),
        TaskHandlerRegistry::new(vec![
            Arc::new(WasmExtensionTaskHandler::new(
                system.clone(),
                runtime.clone(),
            )),
            Arc::new(ExtensionOperationTaskHandler::new(
                system.clone(),
                runtime.clone(),
            )),
            workflow_runtime::task_handler(system.clone()),
            rule_runtime::task_handler(system.clone()),
        ])
        .unwrap(),
        TaskWorkerConfig::from_env().unwrap(),
        shutdown.clone(),
    );
    // Let startup work and the first coordinator ticks settle.
    tokio::time::sleep(Duration::from_secs(2)).await;

    for (expected, run) in [(1, "cold"), (2, "warm")] {
        let before = Snapshot::take();
        client
            .put(format!("{base_url}/v1/records/{}", record_ids[1]))
            .json(&json!({ "values": [
                { "kind": "scalar", "attribute_code": "name", "value": format!("Event {run}") }
            ] }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
        assert!(
            wait_until(
                &pool,
                &format!(
                    "SELECT count(*) >= {expected} FROM tasks WHERE kind = 'event_delivery.v1' AND status = 'succeeded'"
                ),
                Duration::from_secs(20),
            )
            .await,
            "extension event was not delivered"
        );
        tokio::time::sleep(Duration::from_millis(300)).await;
        let after = Snapshot::take();
        report.push((
            format!("extension event handler task ({run})"),
            after
                .mean_since(&before, "task:event_delivery.v1")
                .map_or_else(|| "n/a".to_owned(), |mean| format!("{mean:.0}")),
        ));
    }

    // Idle background load with every coordinator running.
    let workspaces: i64 =
        sqlx::query_scalar("SELECT count(*) FROM workspaces WHERE deleted_at IS NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
    let window = Duration::from_secs(10);
    let before = Snapshot::take();
    tokio::time::sleep(window).await;
    let after = Snapshot::take();
    let mut idle_scopes = after
        .totals
        .iter()
        .map(|(scope, value)| {
            (
                scope.clone(),
                (value - before.totals.get(scope).copied().unwrap_or_default())
                    / window.as_secs_f64(),
            )
        })
        .filter(|(_, rate)| *rate > 0.0)
        .collect::<Vec<_>>();
    idle_scopes.sort_by(|left, right| right.1.total_cmp(&left.1));
    report.push((
        format!("idle round trips per second ({workspaces} workspace)"),
        format!(
            "{:.1}",
            after.statements_since(&before) / window.as_secs_f64()
        ),
    ));
    for (scope, rate) in idle_scopes {
        report.push((format!("  idle: {scope}"), format!("{rate:.1}")));
    }

    let _ = shutdown_sender.send(());
    for worker in workers {
        worker.await.unwrap();
    }
    task_worker.await.unwrap().unwrap();
    server.abort();

    println!("\n{:<55} {:>10}", "scenario", "round trips");
    for (scenario, value) in report {
        println!("{scenario:<55} {value:>10}");
    }
}
