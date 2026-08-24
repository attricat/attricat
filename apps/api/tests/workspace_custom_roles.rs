mod support;

use support::*;

const WORKSPACE_ID: &str = "00000000-0000-4000-8000-000000000002";
const OWNER_ROLE_ID: &str = "00000000-0000-4000-8000-000000000101";

async fn workspace_owner(pool: &PgPool) -> (Uuid, Uuid, Uuid) {
    let workspace = WORKSPACE_ID.parse::<Uuid>().unwrap();
    let user = Uuid::new_v4();
    let membership = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, $2)")
        .bind(user)
        .bind(format!("{user}@example.test"))
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(membership)
    .bind(workspace)
    .bind(user)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, $4, 'workspace', $2)")
        .bind(Uuid::new_v4())
        .bind(workspace)
        .bind(membership)
        .bind(OWNER_ROLE_ID.parse::<Uuid>().unwrap())
        .execute(pool)
        .await
        .unwrap();
    (workspace, user, membership)
}

#[sqlx::test]
async fn custom_roles_have_a_local_lifecycle_and_require_explicit_grant_migration(pool: PgPool) {
    let (workspace, owner, target_membership) = workspace_owner(&pool).await;
    let reader = Uuid::new_v4();
    let writer = Uuid::new_v4();

    let created: Uuid =
        sqlx::query_scalar("SELECT catalog_create_workspace_role($1, $2, $3, $4, $5)")
            .bind(owner)
            .bind(workspace)
            .bind(reader)
            .bind("catalog-reader")
            .bind(vec!["entities.read"])
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(created, reader);

    let duplicate: Uuid =
        sqlx::query_scalar("SELECT catalog_duplicate_workspace_role($1, $2, $3, $4, $5)")
            .bind(owner)
            .bind(workspace)
            .bind(reader)
            .bind(writer)
            .bind("catalog-writer")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(duplicate, writer);
    sqlx::query("SELECT catalog_update_workspace_role($1, $2, $3, $4, $5)")
        .bind(owner)
        .bind(workspace)
        .bind(writer)
        .bind("catalog-writer")
        .bind(vec!["entities.read", "entities.write"])
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("SELECT catalog_grant_workspace_role($1, $2, $3, $4, $5, 'workspace', $2)")
        .bind(owner)
        .bind(workspace)
        .bind(Uuid::new_v4())
        .bind(target_membership)
        .bind(reader)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        sqlx::query("SELECT catalog_retire_workspace_role($1, $2, $3, NULL)")
            .bind(owner)
            .bind(workspace)
            .bind(reader)
            .execute(&pool)
            .await
            .is_err()
    );

    sqlx::query("SELECT catalog_retire_workspace_role($1, $2, $3, $4)")
        .bind(owner)
        .bind(workspace)
        .bind(reader)
        .bind(writer)
        .execute(&pool)
        .await
        .unwrap();
    let migrated: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM role_grants WHERE membership_id = $1 AND role_id = $2",
    )
    .bind(target_membership)
    .bind(writer)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(migrated, 1);
    let retired: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM roles WHERE id = $1)")
        .bind(reader)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!retired);
}

#[sqlx::test]
async fn role_delegation_cannot_exceed_the_actor_permission_or_grant_scope(pool: PgPool) {
    let (workspace, owner, _) = workspace_owner(&pool).await;
    let entity = Uuid::new_v4();
    let blueprint = Uuid::new_v4();
    sqlx::query("INSERT INTO blueprints (id, version, name, definition, definition_hash, workspace_id) VALUES ($1, 1, 'Delegation test', 'kind = \"entity\"', $2, $3)")
        .bind(blueprint)
        .bind(format!("delegation-{blueprint}"))
        .bind(workspace)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO entities (id, blueprint_id, blueprint_version, workspace_id) VALUES ($1, $2, 1, $3)")
        .bind(entity)
        .bind(blueprint)
        .bind(workspace)
        .execute(&pool)
        .await
        .unwrap();

    let actor = Uuid::new_v4();
    let actor_membership = Uuid::new_v4();
    let recipient_membership = Uuid::new_v4();
    for (user, membership) in [
        (actor, actor_membership),
        (Uuid::new_v4(), recipient_membership),
    ] {
        sqlx::query("INSERT INTO users (id, email) VALUES ($1, $2)")
            .bind(user)
            .bind(format!("{user}@example.test"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
        )
        .bind(membership)
        .bind(workspace)
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    }

    let delegator = Uuid::new_v4();
    let writer = Uuid::new_v4();
    for (role, code, permissions) in [
        (
            delegator,
            "entity-delegator",
            vec!["roles.grant", "entities.read"],
        ),
        (writer, "entity-writer", vec!["entities.write"]),
    ] {
        sqlx::query("SELECT catalog_create_workspace_role($1, $2, $3, $4, $5)")
            .bind(owner)
            .bind(workspace)
            .bind(role)
            .bind(code)
            .bind(permissions)
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, $4, 'entity', $5)")
        .bind(Uuid::new_v4())
        .bind(workspace)
        .bind(actor_membership)
        .bind(delegator)
        .bind(entity)
        .execute(&pool)
        .await
        .unwrap();

    assert!(
        sqlx::query("SELECT catalog_grant_workspace_role($1, $2, $3, $4, $5, 'entity', $6)")
            .bind(actor)
            .bind(workspace)
            .bind(Uuid::new_v4())
            .bind(recipient_membership)
            .bind(writer)
            .bind(entity)
            .execute(&pool)
            .await
            .is_err()
    );
}
