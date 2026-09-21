use std::{fs, path::PathBuf};

use sqlx::PgPool;
use uuid::Uuid;

const REMOVE_CONTEXTS_MIGRATION: &str = "20261013000000_remove_solution_pack_contexts.sql";

async fn apply_migration(pool: &PgPool, path: &PathBuf) {
    let sql = fs::read_to_string(path).unwrap();
    sqlx::raw_sql(&sql).execute(pool).await.unwrap();
}

#[sqlx::test(migrations = false)]
async fn context_constraint_upgrade_preserves_history_and_rejects_new_rows(pool: PgPool) {
    let migrations = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let mut paths = fs::read_dir(&migrations)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "sql"))
        .collect::<Vec<_>>();
    paths.sort();
    for path in paths
        .iter()
        .filter(|path| path.file_name().unwrap().to_str().unwrap() < REMOVE_CONTEXTS_MIGRATION)
    {
        apply_migration(&pool, path).await;
    }

    let workspace_id = Uuid::parse_str("00000000-0000-4000-8000-000000000002").unwrap();
    let plan_id = Uuid::new_v4();
    let application_id = Uuid::new_v4();
    let context_target_id = Uuid::new_v4();
    sqlx::query("INSERT INTO solution_pack_plans (id,workspace_id,source_kind,source_metadata,archive_sha256,manifest_version,pack_id,pack_name,pack_version,pack_description,host_api,prefix,blueprint_publication,ready,expires_at) VALUES ($1,$2,'local_archive','{\"side_loaded\":true}'::jsonb,$3,1,'attricat.legacy-context','Legacy context','1.0.0','Legacy context evidence','^1.0','legacy','draft',true,clock_timestamp() + interval '24 hours')")
        .bind(plan_id)
        .bind(workspace_id)
        .bind("0".repeat(64))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO solution_pack_plan_mappings (plan_id,workspace_id,position,resource_kind,logical_key,target_id,target_code,mapping_kind,snapshot) VALUES ($1,$2,0,'context','system/default',$3,'default','system','{\"system\":true}'::jsonb),($1,$2,1,'context','contexts/legacy',$4,'legacy_context','create','{}'::jsonb)")
        .bind(plan_id)
        .bind(workspace_id)
        .bind(Uuid::new_v4())
        .bind(context_target_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO solution_pack_plan_actions (plan_id,workspace_id,position,resource_kind,logical_key,action,reason_code,summary,normalized_payload,preconditions) VALUES ($1,$2,0,'context','contexts/legacy','create','target_absent','{}'::jsonb,'{}'::jsonb,'[{\"kind\":\"target_absent\",\"resource_kind\":\"context\",\"code\":\"legacy_context\"}]'::jsonb)")
        .bind(plan_id)
        .bind(workspace_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO solution_pack_applications (id,workspace_id,plan_id,request_id,correlation_id,source_kind,source_metadata,archive_sha256,pack_id,pack_version,blueprint_publication,state,mapping_snapshot) VALUES ($1,$2,$3,$4,$4,'local_archive','{\"side_loaded\":true}'::jsonb,$5,'attricat.legacy-context','1.0.0','draft','running','[]'::jsonb)")
        .bind(application_id)
        .bind(workspace_id)
        .bind(plan_id)
        .bind(Uuid::new_v4())
        .bind("0".repeat(64))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO solution_pack_application_steps (application_id,workspace_id,position,plan_id,resource_kind,logical_key,target_id,target_code,state) VALUES ($1,$2,0,$3,'context','contexts/legacy',$4,'legacy_context','pending')")
        .bind(application_id)
        .bind(workspace_id)
        .bind(plan_id)
        .bind(context_target_id)
        .execute(&pool)
        .await
        .unwrap();

    apply_migration(&pool, &migrations.join(REMOVE_CONTEXTS_MIGRATION)).await;

    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM solution_pack_plan_mappings WHERE plan_id=$1 AND resource_kind='context'"
        )
        .bind(plan_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        2
    );
    assert!(
        sqlx::query("INSERT INTO solution_pack_plan_mappings (plan_id,workspace_id,position,resource_kind,logical_key,target_id,target_code,mapping_kind,snapshot) VALUES ($1,$2,2,'context','contexts/new',$3,'new_context','create','{}'::jsonb)")
            .bind(plan_id)
            .bind(workspace_id)
            .bind(Uuid::new_v4())
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("INSERT INTO solution_pack_plan_mappings (plan_id,workspace_id,position,resource_kind,logical_key,target_id,target_code,mapping_kind,snapshot) VALUES ($1,$2,2,'blueprint','blueprints/system',$3,'system_blueprint','system','{}'::jsonb)")
            .bind(plan_id)
            .bind(workspace_id)
            .bind(Uuid::new_v4())
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("INSERT INTO solution_pack_plan_actions (plan_id,workspace_id,position,resource_kind,logical_key,action,reason_code,summary,normalized_payload,preconditions) VALUES ($1,$2,1,'context','contexts/new','skip','optional_not_selected','{}'::jsonb,NULL,'[]'::jsonb)")
            .bind(plan_id)
            .bind(workspace_id)
            .execute(&pool)
            .await
            .is_err()
    );

    let blueprint_target_id = Uuid::new_v4();
    sqlx::query("INSERT INTO solution_pack_plan_mappings (plan_id,workspace_id,position,resource_kind,logical_key,target_id,target_code,target_version,mapping_kind,snapshot) VALUES ($1,$2,2,'blueprint','blueprints/new',$3,'new_blueprint',1,'create','{}'::jsonb)")
        .bind(plan_id)
        .bind(workspace_id)
        .bind(blueprint_target_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO solution_pack_plan_actions (plan_id,workspace_id,position,resource_kind,logical_key,action,reason_code,summary,normalized_payload,preconditions) VALUES ($1,$2,1,'blueprint','blueprints/new','create','target_absent','{}'::jsonb,'{}'::jsonb,'[{\"kind\":\"target_absent\",\"resource_kind\":\"blueprint\",\"code\":\"new_blueprint\"}]'::jsonb)")
        .bind(plan_id)
        .bind(workspace_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        sqlx::query("INSERT INTO solution_pack_application_steps (application_id,workspace_id,position,plan_id,resource_kind,logical_key,target_id,target_code,target_version,state) VALUES ($1,$2,1,$3,'context','blueprints/new',$4,'new_blueprint',1,'pending')")
            .bind(application_id)
            .bind(workspace_id)
            .bind(plan_id)
            .bind(blueprint_target_id)
            .execute(&pool)
            .await
            .is_err()
    );
}
