mod support;

use api::repository::CatalogRepository;
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
    sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1,$2,$3,$4,'workspace',$2)").bind(Uuid::new_v4()).bind(workspace).bind(membership).bind(OWNER_ROLE_ID.parse::<Uuid>().unwrap()).execute(pool).await.unwrap();
    (workspace, user, membership)
}

#[sqlx::test]
async fn custom_roles_are_managed_by_the_repository(pool: PgPool) {
    let (workspace, owner, membership) = workspace_owner(&pool).await;
    let repository = CatalogRepository::system(pool.clone());
    let reader = repository
        .create_workspace_role(
            owner,
            workspace,
            "catalog-reader",
            &["entities.read".to_owned()],
        )
        .await
        .unwrap();
    let writer = repository
        .duplicate_workspace_role(owner, workspace, reader, "catalog-writer")
        .await
        .unwrap();
    repository
        .update_workspace_role(
            owner,
            workspace,
            writer,
            "catalog-writer",
            &["entities.read".to_owned(), "entities.write".to_owned()],
        )
        .await
        .unwrap();
    repository
        .grant_workspace_member_role(owner, workspace, membership, reader, "workspace", workspace)
        .await
        .unwrap();
    assert!(
        repository
            .retire_workspace_role(owner, workspace, reader, None)
            .await
            .is_err()
    );
    repository
        .retire_workspace_role(owner, workspace, reader, Some(writer))
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM role_grants WHERE role_id = $1")
            .bind(writer)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[sqlx::test]
async fn retiring_a_role_into_owner_keeps_owner_grants_workspace_scoped(pool: PgPool) {
    let (workspace, owner, membership) = workspace_owner(&pool).await;
    let repository = CatalogRepository::system(pool.clone());
    let reader = repository
        .create_workspace_role(
            owner,
            workspace,
            "scoped-reader",
            &["entities.read".to_owned()],
        )
        .await
        .unwrap();
    let context: Uuid = sqlx::query_scalar(
        "SELECT id FROM attribute_contexts WHERE workspace_id = $1 AND code = 'default'",
    )
    .bind(workspace)
    .fetch_one(&pool)
    .await
    .unwrap();
    repository
        .grant_workspace_member_role(
            owner,
            workspace,
            membership,
            reader,
            "context_subtree",
            context,
        )
        .await
        .unwrap();

    assert!(
        repository
            .retire_workspace_role(
                owner,
                workspace,
                reader,
                Some(OWNER_ROLE_ID.parse().unwrap()),
            )
            .await
            .is_err()
    );
    let scoped_owner_grants: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM role_grants WHERE workspace_id = $1 AND role_id = $2 AND scope_type <> 'workspace'",
    )
    .bind(workspace)
    .bind(OWNER_ROLE_ID.parse::<Uuid>().unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(scoped_owner_grants, 0);
}
