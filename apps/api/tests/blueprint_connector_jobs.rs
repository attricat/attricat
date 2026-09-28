mod support;

use api::{
    agent_tools::execute_read,
    extension_installer::ExtensionInstaller,
    extension_runtime::{ExtensionOperationTaskHandler, ExtensionRuntime, ExtensionRuntimeConfig},
    model::{CreateAttributeContext, CreateBlueprint},
    repository::CatalogRepository,
    storage::{FakeObjectStore, ObjectStore, StoredObject},
    task_worker::TaskHandler,
};
use catalog_domain::task_queue::TaskKind;
use sha2::{Digest, Sha256};
use std::{sync::Arc, time::Duration};
use support::json;
use uuid::Uuid;

const EXPORT_JOB: &str = r#"
[[connector_jobs]]
code = "csv_export"
direction = "export"
extension_id = "attricat-connector-csv"
operation_id = "export"
input = {profile = {version = 1, blueprint_id = "00000000-0000-4000-8000-000000000001", blueprint_version = 1, context_id = "00000000-0000-4000-8000-000000000001", columns = [{header = "ID", attribute = "external_id", kind = "string"}]}}
"#;

const BLUEPRINT: &str = r#"
format_version = 1
code = "connector_test"
name = "Connector Test"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["external_id"]
[[attributes]]
code = "external_id"
value_type = "string"
"#;

#[sqlx::test(migrations = "./migrations")]
async fn scoped_export_fans_out_to_channels_via_existing_task_queue(pool: sqlx::PgPool) {
    let Ok(path) = std::env::var("ATTRICAT_CONNECTOR_CSV_ARCHIVE") else {
        return;
    };
    let archive = std::fs::read(path).unwrap();
    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(workspace)
        .await
        .unwrap();
    let store = Arc::new(FakeObjectStore::available());
    let channels = ["connector_web", "connector_market"];
    let mut channel_ids = Vec::new();
    for code in channels {
        let context = repository
            .create_context(CreateAttributeContext {
                code: code.into(),
                data: json!({}),
                parent_id: None,
            })
            .await
            .unwrap();
        repository
            .set_publication_channel(context.id, true)
            .await
            .unwrap();
        channel_ids.push(context.id);
    }
    ExtensionInstaller::new(repository.clone(), store.clone())
        .install("test", &archive)
        .await
        .unwrap();
    for capability in [
        "catalog.read",
        "catalog.write",
        "artifacts.read",
        "artifacts.write",
    ] {
        repository
            .grant_extension("attricat-connector-csv", "capability", capability)
            .await
            .unwrap();
    }
    repository
        .enable_extension("attricat-connector-csv")
        .await
        .unwrap();
    let blueprint = repository
        .create_blueprint(CreateBlueprint {
            definition: format!("{BLUEPRINT}\n{EXPORT_JOB}"),
        })
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(blueprint.blueprint.id, blueprint.blueprint.version)
        .await
        .unwrap();
    let job = repository
        .list_blueprint_connector_jobs(blueprint.blueprint.id)
        .await
        .unwrap()
        .remove(0);
    let (jobs_page, has_more) = repository
        .blueprint_connector_jobs_page(blueprint.blueprint.id, 1, 0)
        .await
        .unwrap();
    assert_eq!(jobs_page.len(), 1);
    assert_eq!(jobs_page[0].id, job.id);
    assert!(!has_more);
    let (_, server) = support::start_server(pool.clone()).await;
    let jobs = execute_read(
        &repository,
        support::BOOTSTRAP_OWNER_ID.parse().unwrap(),
        support::BOOTSTRAP_WORKSPACE_ID.parse().unwrap(),
        "list_blueprint_connector_jobs",
        json!({"blueprint_id":blueprint.blueprint.id,"limit":1}),
    )
    .await
    .unwrap();
    assert_eq!(jobs["items"][0]["id"], job.id.to_string());
    assert!(jobs["items"][0].get("input").is_none());
    assert!(jobs["items"][0].get("input_file_id").is_none());
    server.abort();
    let runs = repository
        .run_blueprint_connector_job(job.id, "first")
        .await
        .unwrap();
    assert_eq!(runs.len(), 2);
    assert_eq!(
        repository
            .run_blueprint_connector_job(job.id, "first")
            .await
            .unwrap(),
        runs
    );
    for run in &runs {
        let task = repository
            .claim_task_for_kinds(
                "scoped-csv",
                Duration::from_secs(60),
                &[TaskKind::ExtensionOperationRunV1],
            )
            .await
            .unwrap()
            .unwrap();
        let runtime =
            ExtensionRuntime::new(store.clone(), ExtensionRuntimeConfig::default()).unwrap();
        ExtensionOperationTaskHandler::new(repository.clone(), runtime)
            .handle(task)
            .await
            .unwrap();
        let status: String =
            sqlx::query_scalar("SELECT status FROM extension_operation_runs WHERE id=$1")
                .bind(run)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, "completed");
        let (artifact_id,key):(Uuid,String)=sqlx::query_as("SELECT id,object_key FROM extension_operation_artifacts WHERE operation_run_id=$1 AND direction='output' AND state='completed'")
            .bind(run).fetch_one(&pool).await.unwrap();
        repository
            .completed_extension_operation_artifact(*run, artifact_id)
            .await
            .unwrap();
        assert_eq!(store.get(&key).await.unwrap().bytes.as_ref(), b"ID\n");
    }
    let selected:Vec<Uuid>=sqlx::query_scalar("SELECT connector_channel_id FROM extension_operation_runs WHERE connector_job_id=$1 ORDER BY connector_channel_id")
        .bind(job.id).fetch_all(&pool).await.unwrap();
    channel_ids.sort();
    assert_eq!(selected, channel_ids);
    // Imports use the same queue, but the host pins a single write context
    // and a ready input artifact instead of fanning out across channels.
    let csv = b"ID\nSKU-1\n";
    let file = Uuid::new_v4();
    let key = format!("files/{file}");
    store
        .put(
            &key,
            StoredObject {
                bytes: csv.to_vec().into(),
                content_type: Some("text/csv".into()),
            },
        )
        .await
        .unwrap();
    sqlx::query("INSERT INTO files(id,workspace_id,original_filename,display_filename,mime_type,byte_size,sha256,original_key,status) VALUES($1,$2,'source.csv','source.csv','text/csv',$3,$4,$5,'ready')")
        .bind(file).bind(workspace).bind(csv.len() as i64).bind(format!("{:x}",Sha256::digest(csv))).bind(&key)
        .execute(&pool).await.unwrap();
    let context_code: String =
        sqlx::query_scalar("SELECT code FROM attribute_contexts WHERE id=$1")
            .bind(channel_ids[0])
            .fetch_one(&pool)
            .await
            .unwrap();
    let revision = repository.create_blueprint_revision(blueprint.blueprint.id, CreateBlueprint {
        definition: format!(r#"{BLUEPRINT}
{EXPORT_JOB}
[[connector_jobs]]
code = "csv_import"
direction = "import"
extension_id = "attricat-connector-csv"
operation_id = "import"
context = "{context_code}"
input_file_id = "{file}"
input = {{profile = {{version = 1, blueprint_id = "00000000-0000-4000-8000-000000000001", blueprint_version = 1, context_id = "00000000-0000-4000-8000-000000000001", business_key = "external_id", columns = [{{header = "ID", attribute = "external_id", kind = "string"}}]}}}}
[[connector_jobs]]
code = "scheduled_export"
direction = "export"
extension_id = "attricat-connector-csv"
operation_id = "export"
interval_seconds = 60
input = {{profile = {{version = 1, blueprint_id = "00000000-0000-4000-8000-000000000001", blueprint_version = 1, context_id = "00000000-0000-4000-8000-000000000001", columns = [{{header = "ID", attribute = "external_id", kind = "string"}}]}}}}
"#),
    }).await.unwrap();
    repository
        .publish_blueprint_revision(blueprint.blueprint.id, revision.blueprint.version)
        .await
        .unwrap();
    let jobs = repository
        .list_blueprint_connector_jobs(blueprint.blueprint.id)
        .await
        .unwrap();
    assert_eq!(jobs.len(), 3);
    assert_eq!(
        jobs.iter()
            .find(|item| item.code.as_deref() == Some("csv_export"))
            .unwrap()
            .id,
        job.id
    );
    let import_job = jobs
        .iter()
        .find(|item| item.code.as_deref() == Some("csv_import"))
        .unwrap();
    let import_runs = repository
        .run_blueprint_connector_job(import_job.id, "import-once")
        .await
        .unwrap();
    assert_eq!(import_runs.len(), 1);
    let task = repository
        .claim_task_for_kinds(
            "scoped-import",
            Duration::from_secs(60),
            &[TaskKind::ExtensionOperationRunV1],
        )
        .await
        .unwrap()
        .unwrap();
    let runtime = ExtensionRuntime::new(store.clone(), ExtensionRuntimeConfig::default()).unwrap();
    ExtensionOperationTaskHandler::new(repository.clone(), runtime)
        .handle(task)
        .await
        .unwrap();
    let imported: i64 = sqlx::query_scalar("SELECT count(*) FROM entities WHERE workspace_id=$1 AND blueprint_id=$2 AND deleted_at IS NULL")
        .bind(workspace).bind(blueprint.blueprint.id).fetch_one(&pool).await.unwrap();
    assert_eq!(imported, 1);
    let entity: Uuid =
        sqlx::query_scalar("SELECT id FROM entities WHERE workspace_id=$1 AND blueprint_id=$2")
            .bind(workspace)
            .bind(blueprint.blueprint.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let publisher = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,email) VALUES($1,$2)")
        .bind(publisher)
        .bind(format!("{publisher}@example.test"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO entity_channel_publications(workspace_id,entity_id,context_id,published_at,published_by_user_id) VALUES($1,$2,$3,now(),$4)")
        .bind(workspace).bind(entity).bind(channel_ids[0]).bind(publisher).execute(&pool).await.unwrap();
    repository
        .set_publication_channel(channel_ids[1], false)
        .await
        .unwrap();
    let second = repository
        .run_blueprint_connector_job(job.id, "second")
        .await
        .unwrap();
    assert_eq!(second.len(), 1);
    let task = repository
        .claim_task_for_kinds(
            "published-export",
            Duration::from_secs(60),
            &[TaskKind::ExtensionOperationRunV1],
        )
        .await
        .unwrap()
        .unwrap();
    let runtime = ExtensionRuntime::new(store.clone(), ExtensionRuntimeConfig::default()).unwrap();
    ExtensionOperationTaskHandler::new(repository.clone(), runtime)
        .handle(task)
        .await
        .unwrap();
    let (artifact_id,key):(Uuid,String)=sqlx::query_as("SELECT id,object_key FROM extension_operation_artifacts WHERE operation_run_id=$1 AND direction='output' AND state='completed'")
        .bind(second[0]).fetch_one(&pool).await.unwrap();
    repository
        .completed_extension_operation_artifact(second[0], artifact_id)
        .await
        .unwrap();
    assert_eq!(
        store.get(&key).await.unwrap().bytes.as_ref(),
        b"ID\nSKU-1\n"
    );
    let scheduled = jobs
        .iter()
        .find(|item| item.code.as_deref() == Some("scheduled_export"))
        .unwrap();
    sqlx::query("UPDATE blueprint_connector_jobs SET next_at=clock_timestamp()-interval '1 second' WHERE id=$1")
        .bind(scheduled.id).execute(&pool).await.unwrap();
    assert_eq!(
        repository
            .produce_due_blueprint_connector_jobs()
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        repository
            .produce_due_blueprint_connector_jobs()
            .await
            .unwrap(),
        0
    );
    let scheduled_runs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM extension_operation_runs WHERE connector_job_id=$1",
    )
    .bind(scheduled.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(scheduled_runs, 1);
    // Publication, not draft creation, validates the installed connector.
    // A failed publish leaves the prior job declarations unchanged.
    let invalid = repository
        .create_blueprint_revision(
            blueprint.blueprint.id,
            CreateBlueprint {
                definition: format!(
                    "{BLUEPRINT}\n{}",
                    EXPORT_JOB.replace("attricat-connector-csv", "missing-connector")
                ),
            },
        )
        .await
        .unwrap();
    assert!(
        repository
            .publish_blueprint_revision(blueprint.blueprint.id, invalid.blueprint.version)
            .await
            .is_err()
    );
    assert!(
        repository
            .list_blueprint_connector_jobs(blueprint.blueprint.id)
            .await
            .unwrap()
            .iter()
            .any(|item| item.code.as_deref() == Some("csv_import") && item.enabled)
    );
    // A later revision can remove and pause declarations without losing run history.
    let replacement = repository
        .create_blueprint_revision(
            blueprint.blueprint.id,
            CreateBlueprint {
                definition: format!(
                    "{BLUEPRINT}\n{}",
                    EXPORT_JOB.replace(
                        "code = \"csv_export\"",
                        "code = \"csv_export\"\nenabled = false"
                    )
                ),
            },
        )
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(blueprint.blueprint.id, replacement.blueprint.version)
        .await
        .unwrap();
    let retained = repository
        .list_blueprint_connector_jobs(blueprint.blueprint.id)
        .await
        .unwrap();
    assert_eq!(retained.len(), 3);
    assert!(retained.iter().all(|item| !item.enabled));
    assert_eq!(
        retained
            .iter()
            .find(|item| item.code.as_deref() == Some("csv_export"))
            .unwrap()
            .id,
        job.id
    );
}
