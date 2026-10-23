//! The export mapping store at catalog scale (#424): 100,000 records each
//! mapped to an external ID stay in host-managed storage, and the lookups an
//! exporter makes per push stay index-backed.

mod support;

use std::time::{Duration, Instant};

use api::repository::{AttricatRepository, ExportBegin, ExportMappingUpdate, MappingKey};
use support::*;

const EXTENSION: &str = "acme.scale";
const RECORDS: i64 = 100_000;
const BLUEPRINT: &str = r#"
format_version = 1
code = "scale_item"
name = "Scale item"
kind = "record"
[views.dropdown_option]
type = "dropdown_option"
fields = ["sku"]
[[attributes]]
code = "sku"
value_type = "string"
"#;

/// The slowest an index-backed lookup may take on a busy CI machine.
const LOOKUP_BUDGET: Duration = Duration::from_millis(250);

#[sqlx::test(migrations = "./migrations")]
async fn a_full_catalog_keeps_every_mapping_in_host_storage(pool: sqlx::PgPool) {
    let repository = AttricatRepository::system(pool.clone())
        .for_workspace(bootstrap_workspace_id())
        .await
        .unwrap();
    let (blueprint, version) = published_blueprint(&repository, BLUEPRINT).await;
    sqlx::query(
        "INSERT INTO records (id, workspace_id, blueprint_id, blueprint_version) \
         SELECT gen_random_uuid(), $1, $2, $3 FROM generate_series(1, $4)",
    )
    .bind(bootstrap_workspace_id())
    .bind(blueprint)
    .bind(version)
    .bind(RECORDS)
    .execute(&pool)
    .await
    .unwrap();
    // Every record was pushed once: one mapping each, as an exporter's
    // commits would leave them.
    sqlx::query(
        "INSERT INTO extension_export_mappings (id, workspace_id, extension_id, record_id, external_id, external_ids, content_hash, record_revision, status, synced_at) \
         SELECT gen_random_uuid(), workspace_id, $2, id, 'SKU-' || row_number() OVER (ORDER BY id), '{}'::jsonb, md5(id::text), 1, 'synced', now() \
         FROM records WHERE workspace_id = $1 AND blueprint_id = $3",
    )
    .bind(bootstrap_workspace_id())
    .bind(EXTENSION)
    .bind(blueprint)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("ANALYZE extension_export_mappings")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        count(&pool, "SELECT count(*) FROM extension_export_mappings").await,
        RECORDS
    );

    // Lookups by external ID (reconciliation, drift) and by record use indexes.
    let started = Instant::now();
    let found = repository
        .export_mapping_get(EXTENSION, None, MappingKey::ExternalId("SKU-77777"))
        .await
        .unwrap()
        .unwrap();
    let by_external = started.elapsed();
    let started = Instant::now();
    let by_record = repository
        .export_mapping_get(EXTENSION, None, MappingKey::Record(found.record_id))
        .await
        .unwrap()
        .unwrap();
    let by_record_elapsed = started.elapsed();
    assert_eq!(by_record.external_id.as_deref(), Some("SKU-77777"));
    for (label, elapsed) in [("external ID", by_external), ("record", by_record_elapsed)] {
        assert!(
            elapsed < LOOKUP_BUDGET,
            "lookup by {label} took {elapsed:?}"
        );
    }
    let plan: Vec<String> = sqlx::query_scalar(
        "EXPLAIN SELECT record_id FROM extension_export_mappings WHERE workspace_id=$1 AND extension_id=$2 AND connection IS NOT DISTINCT FROM NULL AND external_id='SKU-1'",
    )
    .bind(bootstrap_workspace_id())
    .bind(EXTENSION)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(plan.iter().any(|line| line.contains("Index")), "{plan:?}");

    // A push of the next revision of 1,000 records: begin and commit each.
    let started = Instant::now();
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT record_id FROM extension_export_mappings ORDER BY record_id LIMIT 1000",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    for id in &ids {
        let ExportBegin::Proceed { lease_token, .. } = repository
            .export_mapping_begin(EXTENSION, None, *id, 2, None, false, None, None)
            .await
            .unwrap()
        else {
            panic!("a newer revision proceeds");
        };
        repository
            .export_mapping_commit(
                EXTENSION,
                None,
                *id,
                Some(lease_token),
                ExportMappingUpdate {
                    status: "synced".into(),
                    record_revision: Some(2),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
    }
    let per_push = started.elapsed() / ids.len() as u32;
    println!(
        "lookup by external ID {by_external:?}, by record {by_record_elapsed:?}, begin+commit {per_push:?}"
    );
    assert!(
        per_push < LOOKUP_BUDGET,
        "begin and commit took {per_push:?}"
    );

    // Status pages and counts for the export status UI stay bounded.
    let started = Instant::now();
    let counts = repository
        .export_status_counts(Some(EXTENSION))
        .await
        .unwrap();
    assert_eq!(counts.iter().map(|count| count.count).sum::<i64>(), RECORDS);
    let (page, next) = repository
        .export_mapping_list(EXTENSION, None, Some("synced"), None, 100, false)
        .await
        .unwrap();
    assert_eq!(page.len(), 100);
    assert!(next.is_some());
    println!("status counts and first page {:?}", started.elapsed());
}
