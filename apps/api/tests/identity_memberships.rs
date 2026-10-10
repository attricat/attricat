mod support;

use std::collections::BTreeSet;

use api::repository::AttricatRepository;
use support::*;

const BOOTSTRAP_WORKSPACE_ID: &str = "00000000-0000-4000-8000-000000000002";
const OWNER_ROLE_ID: &str = "00000000-0000-4000-8000-000000000101";

async fn role_id(pool: &PgPool, code: &str) -> Uuid {
    sqlx::query_scalar("SELECT id FROM roles WHERE code = $1 AND is_system")
        .bind(code)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn permission_set(pool: &PgPool, role: &str) -> BTreeSet<String> {
    sqlx::query_scalar(
        "SELECT permission_code FROM role_permissions JOIN roles ON roles.id = role_permissions.role_id WHERE roles.code = $1 ORDER BY permission_code",
    )
    .bind(role)
    .fetch_all(pool)
    .await
    .unwrap()
    .into_iter()
    .collect()
}

#[sqlx::test]
async fn identity_memberships_seeded_roles_and_scoped_grants(pool: PgPool) {
    let all_permissions = BTreeSet::from([
        "blueprints.publish",
        "blueprints.read",
        "blueprints.write",
        "contexts.read",
        "contexts.write",
        "data_health.read",
        "records.delete",
        "records.read",
        "records.write",
        "members.manage",
        "roles.grant",
        "roles.manage",
        "tokens.manage",
        "workspace.manage",
        "workspace_navigation.manage",
    ])
    .into_iter()
    .map(str::to_owned)
    .collect::<BTreeSet<_>>();
    assert_eq!(permission_set(&pool, "owner").await, all_permissions);
    assert_eq!(
        permission_set(&pool, "admin").await,
        permission_set(&pool, "owner")
            .await
            .into_iter()
            .filter(|code| code != "workspace.manage")
            .collect()
    );
    assert_eq!(
        permission_set(&pool, "editor").await,
        BTreeSet::from([
            "blueprints.read",
            "blueprints.write",
            "contexts.read",
            "contexts.write",
            "data_health.read",
            "records.delete",
            "records.read",
            "records.write"
        ])
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    assert_eq!(
        permission_set(&pool, "viewer").await,
        BTreeSet::from([
            "blueprints.read",
            "contexts.read",
            "data_health.read",
            "records.read"
        ])
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    let owner_is_system: bool = sqlx::query_scalar("SELECT is_system FROM roles WHERE id = $1")
        .bind(OWNER_ROLE_ID.parse::<Uuid>().unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(owner_is_system);

    let user = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, 'person@example.test')")
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        sqlx::query("INSERT INTO users (id, email) VALUES ($1, 'Person@Example.test')")
            .bind(Uuid::new_v4())
            .execute(&pool)
            .await
            .is_err()
    );

    let other_workspace = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO workspaces (id, slug, name, login_identifier) VALUES ($1, 'identity-other', 'Identity other', 'identity-other.local')",
    )
    .bind(other_workspace)
    .execute(&pool)
    .await
    .unwrap();
    let bootstrap_membership = Uuid::new_v4();
    let other_membership = Uuid::new_v4();
    for (membership, workspace) in [
        (
            bootstrap_membership,
            BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap(),
        ),
        (other_membership, other_workspace),
    ] {
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
    assert!(
        sqlx::query(
            "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)"
        )
        .bind(Uuid::new_v4())
        .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
        .bind(user)
        .execute(&pool)
        .await
        .is_err()
    );

    let blueprint = Uuid::new_v4();
    let record = Uuid::new_v4();
    let context = Uuid::new_v4();
    sqlx::query("INSERT INTO blueprints (id, version, name, definition, definition_hash, workspace_id) VALUES ($1, 1, 'Identity test', 'kind = \"record\"', $2, $3)")
        .bind(blueprint).bind(format!("identity-{blueprint}")).bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO records (id, blueprint_id, blueprint_version, workspace_id) VALUES ($1, $2, 1, $3)")
        .bind(record).bind(blueprint).bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO attribute_contexts (id, code, data, parent_id, workspace_id) VALUES ($1, 'identity-context', '{}'::jsonb, NULL, $2)")
        .bind(context).bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap()).execute(&pool).await.unwrap();

    let viewer = role_id(&pool, "viewer").await;
    let workspace = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    for (scope_type, target) in [
        ("workspace", workspace),
        ("blueprint_family", blueprint),
        ("record", record),
        ("context_subtree", context),
    ] {
        sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, $4, $5, $6)")
            .bind(Uuid::new_v4()).bind(workspace).bind(bootstrap_membership).bind(viewer).bind(scope_type).bind(target)
            .execute(&pool).await.unwrap();
    }
    // The same membership's viewer and editor grants coexist: permissions are additive.
    let editor = role_id(&pool, "editor").await;
    sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, $4, 'workspace', $2)")
        .bind(Uuid::new_v4()).bind(workspace).bind(bootstrap_membership).bind(editor).execute(&pool).await.unwrap();
    let effective: BTreeSet<String> = sqlx::query_scalar("SELECT DISTINCT permission_code FROM role_grants JOIN role_permissions USING (role_id) WHERE membership_id = $1")
        .bind(bootstrap_membership).fetch_all(&pool).await.unwrap().into_iter().collect();
    assert!(effective.contains("records.write"));
    assert!(effective.contains("blueprints.read"));

    sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, $4, 'workspace', $2)")
        .bind(Uuid::new_v4()).bind(workspace).bind(bootstrap_membership).bind(OWNER_ROLE_ID.parse::<Uuid>().unwrap())
        .execute(&pool).await.unwrap();
    let owner_user = Uuid::new_v4();
    let owner_membership = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, 'owner@example.test')")
        .bind(owner_user)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(owner_membership)
    .bind(workspace)
    .bind(owner_user)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, $4, 'workspace', $2)")
        .bind(Uuid::new_v4()).bind(workspace).bind(owner_membership).bind(OWNER_ROLE_ID.parse::<Uuid>().unwrap())
        .execute(&pool).await.unwrap();
    sqlx::query("UPDATE workspace_memberships SET state = 'inactive' WHERE id = $1")
        .bind(owner_membership)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        AttricatRepository::system(pool.clone())
            .set_workspace_membership_state(bootstrap_membership, user, workspace, "inactive")
            .await
            .is_err()
    );
}

#[sqlx::test]
async fn bootstrap_owner_record_is_normalized_and_unique(pool: PgPool) {
    let workspace = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let user = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, $2)")
        .bind(user)
        .bind("owner@example.test")
        .execute(&pool)
        .await
        .unwrap();
    let membership = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(membership)
    .bind(workspace)
    .bind(user)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, $4, 'workspace', $2)")
        .bind(Uuid::new_v4()).bind(workspace).bind(membership).bind(OWNER_ROLE_ID.parse::<Uuid>().unwrap()).execute(&pool).await.unwrap();
    let owners: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM role_grants WHERE role_id = $1 AND workspace_id = $2",
    )
    .bind(OWNER_ROLE_ID.parse::<Uuid>().unwrap())
    .bind(workspace)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(owners, 1);
}
