use api::{
    blueprint_migration_worker::{
        BlueprintMigrationBatchConfig, BlueprintMigrationBatchTaskHandler,
    },
    model::{CreateBlueprint, NewAttributeValue},
    repository::CatalogRepository,
    task_worker::TaskHandler,
};
use serde_json::json;
use sqlx::PgPool;
use std::time::Instant;
use uuid::Uuid;

#[derive(Clone, Copy)]
enum Workload {
    Scalar,
    File,
    Relationship,
    Mixed,
}

impl Workload {
    fn name(self) -> &'static str {
        match self {
            Self::Scalar => "scalar",
            Self::File => "file",
            Self::Relationship => "relationship",
            Self::Mixed => "mixed",
        }
    }

    fn definition(self) -> String {
        let code = format!("migration_bench_{}", self.name());
        let fields = if matches!(self, Self::File) {
            "image"
        } else {
            "title"
        };
        let mut definition = format!(
            r#"format_version = 1
code = "{code}"
name = "Record migration {code}"
kind = "record"
[views.dropdown_option]
type = "dropdown_option"
fields = ["{fields}"]
"#,
        );
        if !matches!(self, Self::File) {
            definition.push_str(
                r#"[[attributes]]
code = "title"
value_type = "string"
"#,
            );
        }
        if matches!(self, Self::Relationship | Self::Mixed) {
            definition.push_str(
                r#"[[attributes]]
code = "target"
value_type = "relationship"
target_blueprint = "migration_bench_target"
cardinality = "one"
"#,
            );
        }
        if matches!(self, Self::File | Self::Mixed) {
            definition.push_str(
                r#"[[attributes]]
code = "image"
value_type = "file"
cardinality = "many"
allowed_mime_groups = ["image"]
allowed_extensions = ["png"]
max_bytes = 1048576
image_only = true
"#,
            );
        }
        definition
    }
}

/// Reproducible write-amplification/throughput benchmark for issue #244.
///
/// Run with:
/// `RECORD_MIGRATION_BENCH_RECORDS=100 cargo test -p api --test record_migration_benchmarks -- --ignored --nocapture`
///
/// For a recorded pre-change comparison, also set the per-workload baseline
/// variables printed by this test, for example
/// `RECORD_MIGRATION_BENCH_BASELINE_SCALAR_EPS=12.5`. Explicit baselines are
/// asserted so a benchmark run supplies reproducible throughput evidence
/// without making shared CI timing a pass/fail threshold.
#[sqlx::test]
#[ignore = "benchmark; run explicitly with a controlled PostgreSQL instance"]
async fn preserved_value_workloads_report_writes_and_throughput(pool: PgPool) {
    let repository = CatalogRepository::new(
        pool.clone(),
        Uuid::from_u128(0x00000000000040008000000000000002),
    );
    let target_blueprint = repository
        .create_blueprint(CreateBlueprint {
            definition: r#"format_version = 1
code = "migration_bench_target"
name = "Migration benchmark target"
kind = "record"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
"#
            .to_owned(),
        })
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(target_blueprint.blueprint.id, 1)
        .await
        .unwrap();
    let target = repository
        .create_record_with_values(
            target_blueprint.blueprint.id,
            1,
            vec![NewAttributeValue::Scalar {
                attribute_id: None,
                attribute_code: Some("name".to_owned()),
                context_id: None,
                value: json!("target"),
            }],
            Vec::new(),
            json!({}),
        )
        .await
        .unwrap();
    let record_count = std::env::var("RECORD_MIGRATION_BENCH_RECORDS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(50);

    for workload in [
        Workload::Scalar,
        Workload::File,
        Workload::Relationship,
        Workload::Mixed,
    ] {
        run_workload(&repository, &pool, target.id, workload, record_count).await;
    }
}

async fn run_workload(
    repository: &CatalogRepository,
    pool: &PgPool,
    target_id: Uuid,
    workload: Workload,
    record_count: usize,
) {
    let definition = workload.definition();
    let blueprint = repository
        .create_blueprint(CreateBlueprint {
            definition: definition.clone(),
        })
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(blueprint.blueprint.id, 1)
        .await
        .unwrap();
    let mut record_ids = Vec::with_capacity(record_count);
    for index in 0..record_count {
        let mut values = Vec::new();
        if !matches!(workload, Workload::File) {
            values.push(NewAttributeValue::Scalar {
                attribute_id: None,
                attribute_code: Some("title".to_owned()),
                context_id: None,
                value: json!(format!("record-{index}")),
            });
        }
        if matches!(workload, Workload::Relationship | Workload::Mixed) {
            values.push(NewAttributeValue::Relationship {
                attribute_id: None,
                attribute_code: Some("target".to_owned()),
                context_id: None,
                target_record_id: target_id,
            });
        }
        let record = repository
            .create_record_with_values(blueprint.blueprint.id, 1, values, Vec::new(), json!({}))
            .await
            .unwrap();
        record_ids.push(record.id);
        if matches!(workload, Workload::File | Workload::Mixed) {
            insert_benchmark_files(pool, blueprint.blueprint.id, record.id, workload).await;
        }
    }
    let source_value_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM attribute_values WHERE record_id = ANY($1)")
            .bind(&record_ids)
            .fetch_one(pool)
            .await
            .unwrap();
    let source_file_reference_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM attribute_file_references r JOIN attribute_values av ON av.id = r.attribute_value_id WHERE av.record_id = ANY($1)",
    )
    .bind(&record_ids)
    .fetch_one(pool)
    .await
    .unwrap();
    repository
        .create_blueprint_revision(
            blueprint.blueprint.id,
            CreateBlueprint {
                definition: format!("{definition}\n# benchmark revision two\n"),
            },
        )
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(blueprint.blueprint.id, 2)
        .await
        .unwrap();

    let batch = repository
        .start_safe_blueprint_migration_batch(blueprint.blueprint.id, 2)
        .await
        .unwrap();
    let task = repository
        .claim_task(
            &format!("migration-benchmark-{}", workload.name()),
            std::time::Duration::from_secs(300),
        )
        .await
        .unwrap()
        .expect("batch creation enqueues a task");
    assert_eq!(task.subject_id, batch.id);
    let handler = BlueprintMigrationBatchTaskHandler::with_config(
        repository.clone(),
        BlueprintMigrationBatchConfig {
            page_size: 100,
            concurrency: 8,
        },
    );
    let started = Instant::now();
    handler.handle(task).await.unwrap();
    let elapsed = started.elapsed();
    let history_writes: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM attribute_value_history WHERE record_id = ANY($1)",
    )
    .bind(&record_ids)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(history_writes, 0);
    let file_reference_history_writes: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM attribute_file_reference_history r JOIN attribute_value_history h ON h.id = r.attribute_value_history_id AND h.archived_at = r.attribute_value_history_archived_at WHERE h.record_id = ANY($1)",
    )
    .bind(&record_ids)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(file_reference_history_writes, 0);
    let current_value_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM attribute_values WHERE record_id = ANY($1)")
            .bind(&record_ids)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(current_value_count, source_value_count);
    let records_per_second = record_count as f64 / elapsed.as_secs_f64();
    let baseline_name = format!(
        "RECORD_MIGRATION_BENCH_BASELINE_{}_EPS",
        workload.name().to_ascii_uppercase()
    );
    let baseline = std::env::var(&baseline_name)
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0);
    if let Some(baseline) = baseline {
        assert!(
            records_per_second > baseline,
            "{} throughput {:.2} records/s did not exceed recorded baseline {:.2} records/s",
            workload.name(),
            records_per_second,
            baseline
        );
    }
    println!(
        "workload={} records={} values={} file_references={} elapsed_seconds={:.6} records_per_second={:.2} baseline_env={} baseline_records_per_second={} speedup_ratio={} history_writes={} file_reference_history_writes={} legacy_estimated_history_and_reinsert_writes={}",
        workload.name(),
        record_count,
        source_value_count,
        source_file_reference_count,
        elapsed.as_secs_f64(),
        records_per_second,
        baseline_name,
        baseline
            .map(|value| format!("{value:.2}"))
            .unwrap_or_else(|| "not_provided".to_owned()),
        baseline
            .map(|value| format!("{:.2}", records_per_second / value))
            .unwrap_or_else(|| "not_provided".to_owned()),
        history_writes,
        file_reference_history_writes,
        (source_value_count + source_file_reference_count) * 2,
    );
}

async fn insert_benchmark_files(
    pool: &PgPool,
    blueprint_id: Uuid,
    record_id: Uuid,
    workload: Workload,
) {
    let workspace_id = Uuid::from_u128(0x00000000000040008000000000000002);
    let attribute_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM attributes WHERE blueprint_id = $1 AND blueprint_version = 1 AND code = 'image'",
    )
    .bind(blueprint_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let context_id: Uuid =
        sqlx::query_scalar("SELECT id FROM attribute_contexts WHERE code = 'default'")
            .fetch_one(pool)
            .await
            .unwrap();
    let value_id = Uuid::new_v4();
    sqlx::query("INSERT INTO attribute_values (id, workspace_id, record_id, attribute_id, context_id, active) VALUES ($1, $2, $3, $4, $5, true)")
        .bind(value_id)
        .bind(workspace_id)
        .bind(record_id)
        .bind(attribute_id)
        .bind(context_id)
        .execute(pool)
        .await
        .unwrap();
    let references = if matches!(workload, Workload::File) {
        8
    } else {
        3
    };
    for position in 0..references {
        let file_id = Uuid::new_v4();
        sqlx::query("INSERT INTO files (id, workspace_id, original_filename, display_filename, mime_type, byte_size, sha256, original_key, status) VALUES ($1, $2, $3, $3, 'image/png', 1, $4, $5, 'ready')")
            .bind(file_id)
            .bind(workspace_id)
            .bind(format!("benchmark-{position}.png"))
            .bind("0".repeat(64))
            .bind(format!("benchmark/{file_id}"))
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO attribute_file_references (attribute_value_id, workspace_id, file_id, position) VALUES ($1, $2, $3, $4)")
            .bind(value_id)
            .bind(workspace_id)
            .bind(file_id)
            .bind(position)
            .execute(pool)
            .await
            .unwrap();
    }
}
