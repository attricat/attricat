mod support;

use api::repository::CatalogRepository;
use support::*;

const BOOTSTRAP_WORKSPACE_ID: &str = "00000000-0000-4000-8000-000000000002";

#[sqlx::test]
async fn workspace_scoped_repository_hides_other_workspace_catalog_rows(pool: PgPool) {
    let other_workspace = Uuid::new_v4();
    let other_context = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO workspaces (id, slug, name, login_identifier) VALUES ($1, 'other', 'Other workspace', 'other.local')",
    )
    .bind(other_workspace)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO attribute_contexts (id, workspace_id, code, data, parent_id) VALUES ($1, $2, 'private', '{}'::jsonb, NULL)",
    )
    .bind(other_context)
    .bind(other_workspace)
    .execute(&pool)
    .await
    .unwrap();

    let repository = CatalogRepository::new(pool);
    let bootstrap = repository
        .for_workspace(BOOTSTRAP_WORKSPACE_ID.parse().unwrap())
        .await
        .unwrap();
    assert!(
        bootstrap
            .get_context_by_code("private")
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(bootstrap.list_contexts().await.unwrap().len(), 1);

    let other = repository.for_workspace(other_workspace).await.unwrap();
    assert_eq!(
        other
            .get_context_by_code("private")
            .await
            .unwrap()
            .unwrap()
            .id,
        other_context
    );

    let bootstrap_default = bootstrap
        .get_context_by_code("default")
        .await
        .unwrap()
        .unwrap();
    let other_default = other.get_context_by_code("default").await.unwrap().unwrap();
    assert_ne!(bootstrap_default.id, other_default.id);
    assert!(bootstrap_default.parent_id.is_none());
    assert!(other_default.parent_id.is_none());
    assert_eq!(
        other
            .list_contexts()
            .await
            .unwrap()
            .iter()
            .filter(|context| context.code == "default")
            .count(),
        1
    );
}

#[sqlx::test]
async fn workspace_scoped_blueprints_reject_foreign_ids_codes_versions_and_attributes(
    pool: PgPool,
) {
    use api::{model::CreateBlueprint, repository::CatalogRepository};

    let bootstrap_workspace: Uuid = BOOTSTRAP_WORKSPACE_ID.parse().unwrap();
    let other_workspace = Uuid::new_v4();
    sqlx::query("INSERT INTO workspaces (id, slug, name, login_identifier) VALUES ($1, 'tenant-blueprint', 'Tenant blueprint', 'tenant-blueprint.local')")
        .bind(other_workspace).execute(&pool).await.unwrap();
    let repository = CatalogRepository::new(pool);
    let bootstrap = repository.for_workspace(bootstrap_workspace).await.unwrap();
    let other = repository.for_workspace(other_workspace).await.unwrap();
    let definition = "format_version = 1\ncode = 'shared_code'\nname = 'Shared'\nkind = 'entity'\n\n[views.dropdown_option]\ntype = 'dropdown_option'\nfields = ['title']\n\n[[attributes]]\ncode = 'title'\nvalue_type = 'string'";
    let other_blueprint = other
        .create_blueprint(CreateBlueprint {
            definition: definition.to_owned(),
        })
        .await
        .unwrap();
    let bootstrap_blueprint = bootstrap
        .create_blueprint(CreateBlueprint {
            definition: definition.to_owned(),
        })
        .await
        .unwrap();

    assert!(
        bootstrap
            .get_blueprint_revision(other_blueprint.blueprint.id, 1)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        bootstrap
            .get_current_blueprint(other_blueprint.blueprint.id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        bootstrap
            .get_blueprint_by_code_and_version("shared_code", 1)
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        bootstrap
            .get_blueprint_by_code_and_version("shared_code", 1)
            .await
            .unwrap()
            .unwrap()
            .blueprint
            .id,
        bootstrap_blueprint.blueprint.id
    );
    assert!(
        bootstrap
            .list_attributes(other_blueprint.blueprint.id, 1)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        bootstrap
            .publish_blueprint_revision(other_blueprint.blueprint.id, 1)
            .await
            .is_err()
    );
    assert_eq!(
        other
            .list_attributes(other_blueprint.blueprint.id, 1)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        other
            .publish_blueprint_revision(other_blueprint.blueprint.id, 1)
            .await
            .is_ok()
    );
}

#[sqlx::test]
async fn workspace_scoped_entity_commands_reject_foreign_entity_ids(pool: PgPool) {
    let bootstrap_workspace: Uuid = BOOTSTRAP_WORKSPACE_ID.parse().unwrap();
    let other_workspace = Uuid::new_v4();
    let blueprint = Uuid::new_v4();
    let entity = Uuid::new_v4();
    sqlx::query("INSERT INTO workspaces (id, slug, name, login_identifier) VALUES ($1, 'tenant-entity', 'Tenant entity', 'tenant-entity.local')")
        .bind(other_workspace).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO blueprints (id, version, name, definition, definition_hash, workspace_id) VALUES ($1, 1, 'Tenant entity', 'kind = \"entity\"', $2, $3)")
        .bind(blueprint).bind(format!("tenant-entity-{blueprint}")).bind(other_workspace).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO entities (id, blueprint_id, blueprint_version, workspace_id) VALUES ($1, $2, 1, $3)")
        .bind(entity).bind(blueprint).bind(other_workspace).execute(&pool).await.unwrap();

    let repository = CatalogRepository::new(pool);
    let bootstrap = repository.for_workspace(bootstrap_workspace).await.unwrap();
    assert!(bootstrap.get_entity(entity).await.unwrap().is_none());
    assert!(bootstrap.delete_entity(entity).await.is_err());

    let other = repository.for_workspace(other_workspace).await.unwrap();
    assert_eq!(other.get_entity(entity).await.unwrap().unwrap().id, entity);
    other.delete_entity(entity).await.unwrap();
}
