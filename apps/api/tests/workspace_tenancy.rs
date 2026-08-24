mod support;

use sqlx::Row;
use support::*;

const BOOTSTRAP_WORKSPACE_ID: &str = "00000000-0000-4000-8000-000000000002";

#[sqlx::test]
async fn workspace_row_policy_hides_other_workspace_catalog_rows(pool: PgPool) {
    let other_workspace = Uuid::new_v4();
    let other_context = Uuid::new_v4();

    // Workspace administration is intentionally not exposed yet; create the
    // second tenant directly so this test exercises the catalog boundary that
    // later authenticated workspace selection will use.
    sqlx::query("INSERT INTO workspaces (id, slug, name) VALUES ($1, $2, $3)")
        .bind(other_workspace)
        .bind("other")
        .bind("Other workspace")
        .execute(&pool)
        .await
        .unwrap();

    let mut transaction = pool.begin().await.unwrap();
    sqlx::query("SELECT set_config('catalog.workspace_id', $1, true)")
        .bind(other_workspace.to_string())
        .execute(&mut *transaction)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO attribute_contexts (id, code, data, parent_id) VALUES ($1, 'private', '{}'::jsonb, NULL)",
    )
    .bind(other_context)
    .execute(&mut *transaction)
    .await
    .unwrap();
    transaction.commit().await.unwrap();

    // A bootstrap-scoped request cannot discover the alternate context by its
    // exact UUID or through a workspace-unqualified listing query.
    sqlx::query("DO $$ BEGIN CREATE ROLE catalog_workspace_test NOLOGIN; EXCEPTION WHEN duplicate_object THEN NULL; END $$")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("GRANT USAGE ON SCHEMA public TO catalog_workspace_test")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("GRANT SELECT, INSERT ON attribute_contexts TO catalog_workspace_test")
        .execute(&pool)
        .await
        .unwrap();

    let mut bootstrap = pool.begin().await.unwrap();
    sqlx::query("SET LOCAL ROLE catalog_workspace_test")
        .execute(&mut *bootstrap)
        .await
        .unwrap();
    sqlx::query("SELECT set_config('catalog.workspace_id', $1, true)")
        .bind(BOOTSTRAP_WORKSPACE_ID)
        .execute(&mut *bootstrap)
        .await
        .unwrap();
    let visible = sqlx::query("SELECT id FROM attribute_contexts WHERE id = $1")
        .bind(other_context)
        .fetch_optional(&mut *bootstrap)
        .await
        .unwrap();
    assert!(visible.is_none());
    let count: i64 = sqlx::query("SELECT count(*) AS count FROM attribute_contexts")
        .fetch_one(&mut *bootstrap)
        .await
        .unwrap()
        .get("count");
    assert_eq!(count, 1, "only the bootstrap default context is visible");

    // Explicitly smuggling a different workspace ID into an insert is rejected
    // by the storage policy; API JSON models do not contain this field.
    sqlx::query("SAVEPOINT rejected_workspace_insert")
        .execute(&mut *bootstrap)
        .await
        .unwrap();
    let result = sqlx::query(
        "INSERT INTO attribute_contexts (id, workspace_id, code, data, parent_id) VALUES ($1, $2, 'leak', '{}'::jsonb, NULL)",
    )
    .bind(Uuid::new_v4())
    .bind(other_workspace)
    .execute(&mut *bootstrap)
    .await;
    assert!(result.is_err());
    sqlx::query("ROLLBACK TO SAVEPOINT rejected_workspace_insert")
        .execute(&mut *bootstrap)
        .await
        .unwrap();

    let active: String = sqlx::query_scalar("SELECT catalog_workspace_id()::text")
        .fetch_one(&mut *bootstrap)
        .await
        .unwrap();
    assert_eq!(active, BOOTSTRAP_WORKSPACE_ID);
    bootstrap.rollback().await.unwrap();
}
