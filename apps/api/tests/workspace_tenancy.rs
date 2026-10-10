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

    let repository = CatalogRepository::system(pool);
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

    repository
        .initialize_workspace(other_workspace)
        .await
        .unwrap();
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
    let repository = CatalogRepository::system(pool);
    let bootstrap = repository.for_workspace(bootstrap_workspace).await.unwrap();
    repository
        .initialize_workspace(other_workspace)
        .await
        .unwrap();
    let other = repository.for_workspace(other_workspace).await.unwrap();
    let definition = "format_version = 1\ncode = 'shared_code'\nname = 'Shared'\nkind = 'record'\n\n[views.dropdown_option]\ntype = 'dropdown_option'\nfields = ['title']\n\n[[attributes]]\ncode = 'title'\nvalue_type = 'string'";
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
async fn workspace_scoped_record_commands_reject_foreign_record_ids(pool: PgPool) {
    let bootstrap_workspace: Uuid = BOOTSTRAP_WORKSPACE_ID.parse().unwrap();
    let other_workspace = Uuid::new_v4();
    let blueprint = Uuid::new_v4();
    let record = Uuid::new_v4();
    sqlx::query("INSERT INTO workspaces (id, slug, name, login_identifier) VALUES ($1, 'tenant-record', 'Tenant record', 'tenant-record.local')")
        .bind(other_workspace).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO blueprints (id, version, name, definition, definition_hash, workspace_id) VALUES ($1, 1, 'Tenant record', 'kind = \"record\"', $2, $3)")
        .bind(blueprint).bind(format!("tenant-record-{blueprint}")).bind(other_workspace).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO records (id, blueprint_id, blueprint_version, workspace_id) VALUES ($1, $2, 1, $3)")
        .bind(record).bind(blueprint).bind(other_workspace).execute(&pool).await.unwrap();

    let repository = CatalogRepository::system(pool);
    let bootstrap = repository.for_workspace(bootstrap_workspace).await.unwrap();
    assert!(bootstrap.get_record(record).await.unwrap().is_none());
    assert!(bootstrap.delete_record(record).await.is_err());

    repository
        .initialize_workspace(other_workspace)
        .await
        .unwrap();
    let other = repository.for_workspace(other_workspace).await.unwrap();
    assert_eq!(other.get_record(record).await.unwrap().unwrap().id, record);
    other.delete_record(record).await.unwrap();
}

#[sqlx::test]
async fn current_values_hides_foreign_record_values(pool: PgPool) {
    use api::model::{CreateBlueprint, NewAttributeValue};

    let bootstrap_workspace: Uuid = BOOTSTRAP_WORKSPACE_ID.parse().unwrap();
    let other_workspace = Uuid::new_v4();
    sqlx::query("INSERT INTO workspaces (id, slug, name, login_identifier) VALUES ($1, 'tenant-values', 'Tenant values', 'tenant-values.local')")
        .bind(other_workspace)
        .execute(&pool)
        .await
        .unwrap();
    let repository = CatalogRepository::system(pool);
    let bootstrap = repository.for_workspace(bootstrap_workspace).await.unwrap();
    repository
        .initialize_workspace(other_workspace)
        .await
        .unwrap();
    let other = repository.for_workspace(other_workspace).await.unwrap();
    let blueprint = other
        .create_blueprint(CreateBlueprint {
            definition: "format_version = 1\ncode = 'private_values'\nname = 'Private values'\nkind = 'record'\n\n[views.dropdown_option]\ntype = 'dropdown_option'\nfields = ['title']\n\n[[attributes]]\ncode = 'title'\nvalue_type = 'string'".to_owned(),
        })
        .await
        .unwrap();
    other
        .publish_blueprint_revision(blueprint.blueprint.id, 1)
        .await
        .unwrap();
    let record = other
        .create_record_with_values(
            blueprint.blueprint.id,
            1,
            vec![NewAttributeValue::Scalar {
                attribute_id: None,
                attribute_code: Some("title".to_owned()),
                context_id: None,
                value: serde_json::json!("private"),
            }],
            Vec::new(),
            serde_json::json!({}),
        )
        .await
        .unwrap();

    assert_eq!(other.current_values(record.id).await.unwrap().len(), 1);
    assert!(
        bootstrap
            .current_values(record.id)
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test]
async fn reachable_search_never_traverses_another_workspace(pool: PgPool) {
    use api::model::{CreateBlueprint, NewAttributeValue};

    let bootstrap_workspace: Uuid = BOOTSTRAP_WORKSPACE_ID.parse().unwrap();
    let other_workspace = Uuid::new_v4();
    sqlx::query("INSERT INTO workspaces (id, slug, name, login_identifier) VALUES ($1, 'tenant-search', 'Tenant search', 'tenant-search.local')")
        .bind(other_workspace)
        .execute(&pool)
        .await
        .unwrap();
    let repository = CatalogRepository::system(pool);
    let bootstrap = repository.for_workspace(bootstrap_workspace).await.unwrap();
    repository
        .initialize_workspace(other_workspace)
        .await
        .unwrap();
    let other = repository.for_workspace(other_workspace).await.unwrap();
    let target_definition = "format_version = 1\ncode = 'shared_search_target'\nname = 'Shared target'\nkind = 'record'\n\n[views.dropdown_option]\ntype = 'dropdown_option'\nfields = ['name']\n\n[[attributes]]\ncode = 'name'\nvalue_type = 'string'";
    let source_definition = "format_version = 1\ncode = 'shared_search_source'\nname = 'Shared source'\nkind = 'record'\n\n[views.dropdown_option]\ntype = 'dropdown_option'\nfields = ['name']\n\n[[attributes]]\ncode = 'name'\nvalue_type = 'string'\n\n[[attributes]]\ncode = 'target'\nvalue_type = 'relationship'\ntarget_blueprint = 'shared_search_target'";

    for scoped in [&bootstrap, &other] {
        let target = scoped
            .create_blueprint(CreateBlueprint {
                definition: target_definition.to_owned(),
            })
            .await
            .unwrap();
        scoped
            .publish_blueprint_revision(target.blueprint.id, 1)
            .await
            .unwrap();
        let source = scoped
            .create_blueprint(CreateBlueprint {
                definition: source_definition.to_owned(),
            })
            .await
            .unwrap();
        scoped
            .publish_blueprint_revision(source.blueprint.id, 1)
            .await
            .unwrap();
    }

    let other_target_blueprint = other
        .get_blueprint_by_code("shared_search_target")
        .await
        .unwrap()
        .unwrap();
    let other_source_blueprint = other
        .get_blueprint_by_code("shared_search_source")
        .await
        .unwrap()
        .unwrap();
    let target = other
        .create_record_with_values(
            other_target_blueprint.blueprint.id,
            1,
            vec![NewAttributeValue::Scalar {
                attribute_id: None,
                attribute_code: Some("name".to_owned()),
                context_id: None,
                value: json!("cross-workspace-secret"),
            }],
            Vec::new(),
            json!({}),
        )
        .await
        .unwrap();
    other
        .create_record_with_values(
            other_source_blueprint.blueprint.id,
            1,
            vec![NewAttributeValue::Relationship {
                attribute_id: None,
                attribute_code: Some("target".to_owned()),
                context_id: None,
                target_record_id: target.id,
            }],
            Vec::new(),
            json!({}),
        )
        .await
        .unwrap();

    let bootstrap_source = bootstrap
        .get_blueprint_by_code("shared_search_source")
        .await
        .unwrap()
        .unwrap();
    let resolved = bootstrap
        .resolve_search(&bootstrap_source, None, Some("*:cross-workspace-secret"))
        .await
        .unwrap();
    assert!(resolved.ids.is_empty());
    assert!(resolved.explanations.is_empty());
}
