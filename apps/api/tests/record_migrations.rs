mod support;

use api::{
    model::{CreateBlueprint, MigrateRecordRequest, NewAttributeValue},
    repository::CatalogRepository,
};
use serde_json::json;
use sqlx::PgPool;
use support::{authenticated_client, start_server};

const SCALAR_DEFINITION: &str = r#"
format_version = 1
code = "migration_identity_product"
name = "Migration identity product"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"
"#;

#[sqlx::test]
async fn compatible_scalar_migration_preserves_row_identity_without_history(pool: PgPool) {
    let repository = CatalogRepository::new(
        pool.clone(),
        support::BOOTSTRAP_WORKSPACE_ID.parse().unwrap(),
    );
    let source = repository
        .create_blueprint(CreateBlueprint {
            definition: SCALAR_DEFINITION.to_owned(),
        })
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(source.blueprint.id, 1)
        .await
        .unwrap();
    let record = repository
        .create_record_with_values(
            source.blueprint.id,
            1,
            vec![NewAttributeValue::Scalar {
                attribute_id: None,
                attribute_code: Some("title".to_owned()),
                context_id: None,
                value: json!("unchanged"),
            }],
            Vec::new(),
            json!({}),
        )
        .await
        .unwrap();
    let before = sqlx::query_as::<_, (uuid::Uuid, uuid::Uuid, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, attribute_id, created_at FROM attribute_values WHERE record_id = $1",
    )
    .bind(record.id)
    .fetch_one(&pool)
    .await
    .unwrap();

    repository
        .create_blueprint_revision(
            source.blueprint.id,
            CreateBlueprint {
                definition: format!("{SCALAR_DEFINITION}\n# revision two\n"),
            },
        )
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(source.blueprint.id, 2)
        .await
        .unwrap();
    let preview = repository
        .preview_record_migration(record.id)
        .await
        .unwrap();
    assert_eq!(preview.status, "ready");
    repository
        .migrate_record_to_latest(
            record.id,
            MigrateRecordRequest {
                migration_id: preview.migration_id,
                expected_target_version: 2,
                values: Vec::new(),
                relationships: Vec::new(),
                discard_attributes: Vec::new(),
                removal_policy: None,
            },
        )
        .await
        .unwrap();

    let after = sqlx::query_as::<_, (uuid::Uuid, uuid::Uuid, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, attribute_id, created_at FROM attribute_values WHERE record_id = $1",
    )
    .bind(record.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after.0, before.0, "the current value ID is stable");
    assert_eq!(after.2, before.2, "the original creation time is stable");
    assert_ne!(after.1, before.1, "only the revision attribute ID changes");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM attribute_value_history WHERE record_id = $1",
        )
        .bind(record.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM domain_events WHERE aggregate_id = $1 AND event_type = 'record.migrated.v1'",
        )
        .bind(record.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT status FROM record_blueprint_migrations WHERE id = $1",
        )
        .bind(preview.migration_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "migrated"
    );
}

#[sqlx::test]
async fn supplied_scalar_replacement_archives_the_old_row_once(pool: PgPool) {
    let repository = CatalogRepository::new(
        pool.clone(),
        support::BOOTSTRAP_WORKSPACE_ID.parse().unwrap(),
    );
    let source = repository
        .create_blueprint(CreateBlueprint {
            definition: SCALAR_DEFINITION.replace(
                "migration_identity_product",
                "migration_replacement_product",
            ),
        })
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(source.blueprint.id, 1)
        .await
        .unwrap();
    let record = repository
        .create_record_with_values(
            source.blueprint.id,
            1,
            vec![NewAttributeValue::Scalar {
                attribute_id: None,
                attribute_code: Some("title".to_owned()),
                context_id: None,
                value: json!("old"),
            }],
            Vec::new(),
            json!({}),
        )
        .await
        .unwrap();
    let old_id: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM attribute_values WHERE record_id = $1")
            .bind(record.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    repository
        .create_blueprint_revision(
            source.blueprint.id,
            CreateBlueprint {
                definition: format!(
                    "{}\n# revision two\n",
                    SCALAR_DEFINITION.replace(
                        "migration_identity_product",
                        "migration_replacement_product"
                    )
                ),
            },
        )
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(source.blueprint.id, 2)
        .await
        .unwrap();
    let preview = repository
        .preview_record_migration(record.id)
        .await
        .unwrap();
    repository
        .migrate_record_to_latest(
            record.id,
            MigrateRecordRequest {
                migration_id: preview.migration_id,
                expected_target_version: 2,
                values: vec![NewAttributeValue::Scalar {
                    attribute_id: None,
                    attribute_code: Some("title".to_owned()),
                    context_id: None,
                    value: json!("new"),
                }],
                relationships: Vec::new(),
                discard_attributes: Vec::new(),
                removal_policy: None,
            },
        )
        .await
        .unwrap();

    let new_id: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM attribute_values WHERE record_id = $1")
            .bind(record.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_ne!(new_id, old_id);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM attribute_value_history WHERE record_id = $1 AND id = $2",
        )
        .bind(record.id)
        .bind(old_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}

#[sqlx::test]
async fn compatible_relationship_migration_preserves_row_identity(pool: PgPool) {
    let repository = CatalogRepository::new(
        pool.clone(),
        support::BOOTSTRAP_WORKSPACE_ID.parse().unwrap(),
    );
    let target_definition = r#"
format_version = 1
code = "migration_identity_target"
name = "Migration identity target"
kind = "record"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
"#;
    let target_blueprint = repository
        .create_blueprint(CreateBlueprint {
            definition: target_definition.to_owned(),
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
    let source_definition = r#"
format_version = 1
code = "migration_identity_source"
name = "Migration identity source"
kind = "record"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
[[attributes]]
code = "target"
value_type = "relationship"
target_blueprint = "migration_identity_target"
cardinality = "one"
"#;
    let source_blueprint = repository
        .create_blueprint(CreateBlueprint {
            definition: source_definition.to_owned(),
        })
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(source_blueprint.blueprint.id, 1)
        .await
        .unwrap();
    let source = repository
        .create_record_with_values(
            source_blueprint.blueprint.id,
            1,
            vec![
                NewAttributeValue::Scalar {
                    attribute_id: None,
                    attribute_code: Some("name".to_owned()),
                    context_id: None,
                    value: json!("source"),
                },
                NewAttributeValue::Relationship {
                    attribute_id: None,
                    attribute_code: Some("target".to_owned()),
                    context_id: None,
                    target_record_id: target.id,
                },
            ],
            Vec::new(),
            json!({}),
        )
        .await
        .unwrap();
    let before = sqlx::query_as::<_, (uuid::Uuid, uuid::Uuid, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, attribute_id, created_at FROM attribute_values WHERE record_id = $1 AND relationship_target_record_id = $2",
    )
    .bind(source.id)
    .bind(target.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    repository
        .create_blueprint_revision(
            source_blueprint.blueprint.id,
            CreateBlueprint {
                definition: format!("{source_definition}\n# revision two\n"),
            },
        )
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(source_blueprint.blueprint.id, 2)
        .await
        .unwrap();
    let preview = repository
        .preview_record_migration(source.id)
        .await
        .unwrap();
    assert_eq!(preview.status, "ready");
    repository
        .migrate_record_to_latest(
            source.id,
            MigrateRecordRequest {
                migration_id: preview.migration_id,
                expected_target_version: 2,
                values: Vec::new(),
                relationships: Vec::new(),
                discard_attributes: Vec::new(),
                removal_policy: None,
            },
        )
        .await
        .unwrap();
    let after = sqlx::query_as::<_, (uuid::Uuid, uuid::Uuid, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, attribute_id, created_at FROM attribute_values WHERE record_id = $1 AND relationship_target_record_id = $2",
    )
    .bind(source.id)
    .bind(target.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after.0, before.0);
    assert_eq!(after.2, before.2);
    assert_ne!(after.1, before.1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attribute_value_history WHERE id = $1")
            .bind(before.0)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[sqlx::test]
async fn explicitly_discarded_removed_value_is_the_only_row_archived(pool: PgPool) {
    let repository = CatalogRepository::new(
        pool.clone(),
        support::BOOTSTRAP_WORKSPACE_ID.parse().unwrap(),
    );
    let source_definition = format!(
        r#"{SCALAR_DEFINITION}
[[attributes]]
code = "obsolete"
value_type = "string"
"#,
    );
    let source = repository
        .create_blueprint(CreateBlueprint {
            definition: source_definition,
        })
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(source.blueprint.id, 1)
        .await
        .unwrap();
    let record = repository
        .create_record_with_values(
            source.blueprint.id,
            1,
            vec![
                NewAttributeValue::Scalar {
                    attribute_id: None,
                    attribute_code: Some("title".to_owned()),
                    context_id: None,
                    value: json!("preserved"),
                },
                NewAttributeValue::Scalar {
                    attribute_id: None,
                    attribute_code: Some("obsolete".to_owned()),
                    context_id: None,
                    value: json!("discarded"),
                },
            ],
            Vec::new(),
            json!({}),
        )
        .await
        .unwrap();
    let rows = sqlx::query_as::<_, (uuid::Uuid, String)>(
        "SELECT av.id, a.code FROM attribute_values av JOIN attributes a ON a.id = av.attribute_id WHERE av.record_id = $1",
    )
    .bind(record.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    let title_id = rows.iter().find(|row| row.1 == "title").unwrap().0;
    let obsolete_id = rows.iter().find(|row| row.1 == "obsolete").unwrap().0;
    repository
        .create_blueprint_revision(
            source.blueprint.id,
            CreateBlueprint {
                definition: format!("{SCALAR_DEFINITION}\n# removed obsolete\n"),
            },
        )
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(source.blueprint.id, 2)
        .await
        .unwrap();
    let preview = repository
        .preview_record_migration(record.id)
        .await
        .unwrap();
    assert_eq!(preview.status, "needs_input");
    repository
        .migrate_record_to_latest(
            record.id,
            MigrateRecordRequest {
                migration_id: preview.migration_id,
                expected_target_version: 2,
                values: Vec::new(),
                relationships: Vec::new(),
                discard_attributes: vec!["obsolete".to_owned()],
                removal_policy: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, uuid::Uuid>(
            "SELECT id FROM attribute_values WHERE record_id = $1",
        )
        .bind(record.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        title_id
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM attribute_value_history WHERE record_id = $1 AND id = $2",
        )
        .bind(record.id)
        .bind(obsolete_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM attribute_value_history WHERE record_id = $1 AND id = $2",
        )
        .bind(record.id)
        .bind(title_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}

#[sqlx::test]
async fn ui_shaped_unchanged_payload_preserves_scalar_and_relationship_rows(pool: PgPool) {
    let repository = CatalogRepository::new(
        pool.clone(),
        support::BOOTSTRAP_WORKSPACE_ID.parse().unwrap(),
    );
    let target_blueprint = repository
        .create_blueprint(CreateBlueprint {
            definition: r#"format_version = 1
code = "migration_http_target"
name = "Migration HTTP target"
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
    let definition = r#"format_version = 1
code = "migration_http_source"
name = "Migration HTTP source"
kind = "record"
[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
[[attributes]]
code = "title"
value_type = "string"
[[attributes]]
code = "target"
value_type = "relationship"
target_blueprint = "migration_http_target"
cardinality = "one"
"#;
    let source_blueprint = repository
        .create_blueprint(CreateBlueprint {
            definition: definition.to_owned(),
        })
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(source_blueprint.blueprint.id, 1)
        .await
        .unwrap();
    let record = repository
        .create_record_with_values(
            source_blueprint.blueprint.id,
            1,
            vec![
                NewAttributeValue::Scalar {
                    attribute_id: None,
                    attribute_code: Some("title".to_owned()),
                    context_id: None,
                    value: json!("unchanged"),
                },
                NewAttributeValue::Relationship {
                    attribute_id: None,
                    attribute_code: Some("target".to_owned()),
                    context_id: None,
                    target_record_id: target.id,
                },
            ],
            Vec::new(),
            json!({}),
        )
        .await
        .unwrap();
    let before = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT id FROM attribute_values WHERE record_id = $1 ORDER BY id",
    )
    .bind(record.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    repository
        .create_blueprint_revision(
            source_blueprint.blueprint.id,
            CreateBlueprint {
                definition: format!("{definition}\n# revision two\n"),
            },
        )
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(source_blueprint.blueprint.id, 2)
        .await
        .unwrap();
    let preview = repository
        .preview_record_migration(record.id)
        .await
        .unwrap();

    let (base_url, _server) = start_server(pool.clone()).await;
    authenticated_client()
        .post(format!(
            "{base_url}/v1/records/{}/blueprint-migration",
            record.id
        ))
        .json(&json!({
            "migration_id": preview.migration_id,
            "expected_target_version": 2,
            "values": [{
                "kind": "scalar",
                "attribute_id": null,
                "attribute_code": "title",
                "context_id": null,
                "value": "unchanged"
            }],
            "relationships": [{
                "attribute_id": null,
                "attribute_code": "target",
                "context_id": null,
                "target_record_ids": [target.id]
            }],
            "discard_attributes": []
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let after = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT id FROM attribute_values WHERE record_id = $1 ORDER BY id",
    )
    .bind(record.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(after, before);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM attribute_value_history WHERE record_id = $1",
        )
        .bind(record.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}

#[sqlx::test]
async fn migration_requires_replacements_in_every_affected_context(pool: PgPool) {
    let repository = CatalogRepository::new(
        pool.clone(),
        support::BOOTSTRAP_WORKSPACE_ID.parse().unwrap(),
    );
    let other_context = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO attribute_contexts (id, code, data) VALUES ($1, 'migration-other', '{}'::jsonb)")
        .bind(other_context).execute(&pool).await.unwrap();
    let definition = format!(
        "{SCALAR_DEFINITION}\n[[attributes]]\ncode = \"rating\"\nvalue_type = \"string\"\ncontext_editable = \"all\"\n"
    );
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
    let record = repository
        .create_record_with_values(
            blueprint.blueprint.id,
            1,
            vec![
                NewAttributeValue::Scalar {
                    attribute_id: None,
                    attribute_code: Some("title".into()),
                    context_id: None,
                    value: json!("name"),
                },
                NewAttributeValue::Scalar {
                    attribute_id: None,
                    attribute_code: Some("rating".into()),
                    context_id: None,
                    value: json!("one"),
                },
                NewAttributeValue::Scalar {
                    attribute_id: None,
                    attribute_code: Some("rating".into()),
                    context_id: Some(other_context),
                    value: json!("two"),
                },
            ],
            Vec::new(),
            json!({}),
        )
        .await
        .unwrap();
    repository
        .create_blueprint_revision(
            blueprint.blueprint.id,
            CreateBlueprint {
                definition: definition.replace(
                    "value_type = \"string\"\ncontext_editable = \"all\"",
                    "value_type = \"integer\"\ncontext_editable = \"all\"",
                ),
            },
        )
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(blueprint.blueprint.id, 2)
        .await
        .unwrap();
    let preview = repository
        .preview_record_migration(record.id)
        .await
        .unwrap();
    let error = repository
        .migrate_record_to_latest(
            record.id,
            MigrateRecordRequest {
                migration_id: preview.migration_id,
                expected_target_version: 2,
                values: vec![NewAttributeValue::Scalar {
                    attribute_id: None,
                    attribute_code: Some("rating".into()),
                    context_id: None,
                    value: json!(1),
                }],
                relationships: Vec::new(),
                discard_attributes: Vec::new(),
                removal_policy: None,
            },
        )
        .await
        .unwrap_err();
    assert!(
        matches!(error, api::repository::RepositoryError::MigrationNeedsResolution(ref codes) if codes.contains(&"rating".to_owned()))
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attribute_values WHERE record_id = $1")
            .bind(record.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        3
    );
}

#[sqlx::test]
async fn migration_rejects_non_default_value_when_target_becomes_default_only(pool: PgPool) {
    let repository = CatalogRepository::new(
        pool.clone(),
        support::BOOTSTRAP_WORKSPACE_ID.parse().unwrap(),
    );
    let context_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO attribute_contexts (id, code, data, parent_id) VALUES ($1, 'migration-custom', '{}'::jsonb, NULL)",
    )
    .bind(context_id)
    .execute(&pool)
    .await
    .unwrap();
    let source_definition = r#"format_version = 1
code = "migration_context_product"
name = "Migration context product"
kind = "record"
[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
[[attributes]]
code = "title"
value_type = "string"
context_editable = "all"
"#;
    let source = repository
        .create_blueprint(CreateBlueprint {
            definition: source_definition.to_owned(),
        })
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(source.blueprint.id, 1)
        .await
        .unwrap();
    let record = repository
        .create_record_with_values(
            source.blueprint.id,
            1,
            vec![NewAttributeValue::Scalar {
                attribute_id: None,
                attribute_code: Some("title".to_owned()),
                context_id: Some(context_id),
                value: json!("custom context"),
            }],
            Vec::new(),
            json!({}),
        )
        .await
        .unwrap();
    let value_id: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM attribute_values WHERE record_id = $1")
            .bind(record.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    repository
        .create_blueprint_revision(
            source.blueprint.id,
            CreateBlueprint {
                definition: source_definition.replace(
                    "context_editable = \"all\"",
                    "context_editable = \"default\"",
                ),
            },
        )
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(source.blueprint.id, 2)
        .await
        .unwrap();
    let preview = repository
        .preview_record_migration(record.id)
        .await
        .unwrap();
    assert_eq!(preview.status, "needs_input");
    assert!(preview.issues.iter().any(|issue| {
        issue.attribute_code.as_deref() == Some("title") && issue.kind == "context_not_editable"
    }));

    let error = repository
        .migrate_record_to_latest(
            record.id,
            MigrateRecordRequest {
                migration_id: preview.migration_id,
                expected_target_version: 2,
                values: Vec::new(),
                relationships: Vec::new(),
                discard_attributes: Vec::new(),
                removal_policy: None,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        api::repository::RepositoryError::MigrationNeedsResolution(_)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, uuid::Uuid>("SELECT id FROM attribute_values WHERE record_id = $1")
            .bind(record.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        value_id
    );
}

#[sqlx::test]
async fn migration_audits_field_changes_and_reconciles_publication(pool: PgPool) {
    let actor = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, 'migration-publisher@example.test')")
        .bind(actor)
        .execute(&pool)
        .await
        .unwrap();
    let repository = CatalogRepository::new(
        pool.clone(),
        support::BOOTSTRAP_WORKSPACE_ID.parse().unwrap(),
    )
    .with_audit_context(api::repository::AuditContext {
        actor_user_id: Some(actor),
        actor_token_id: None,
        request_id: uuid::Uuid::new_v4(),
        correlation_id: uuid::Uuid::new_v4(),
        action: "record.migrate".into(),
        authorization_scope: json!({}),
        target: json!({"type": "record"}),
        metadata: json!({}),
        agent: None,
    });
    let source = repository
        .create_blueprint(CreateBlueprint {
            definition: format!(
                "{SCALAR_DEFINITION}\n[[attributes]]\ncode = \"obsolete\"\nvalue_type = \"string\"\n"
            ),
        })
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(source.blueprint.id, 1)
        .await
        .unwrap();
    let record = repository
        .create_record_with_values(
            source.blueprint.id,
            1,
            ["title", "obsolete"]
                .map(|code| NewAttributeValue::Scalar {
                    attribute_id: None,
                    attribute_code: Some(code.to_owned()),
                    context_id: None,
                    value: json!(code),
                })
                .into(),
            Vec::new(),
            json!({}),
        )
        .await
        .unwrap();
    let context = repository
        .create_context(api::model::CreateAttributeContext {
            code: "migration-web".into(),
            data: json!({}),
            parent_id: None,
        })
        .await
        .unwrap();
    repository
        .set_publication_channel(context.id, true)
        .await
        .unwrap();
    repository
        .publish_record(record.id, context.id)
        .await
        .unwrap();
    repository
        .create_blueprint_revision(
            source.blueprint.id,
            CreateBlueprint {
                definition: SCALAR_DEFINITION.to_owned(),
            },
        )
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(source.blueprint.id, 2)
        .await
        .unwrap();
    let preview = repository
        .preview_record_migration(record.id)
        .await
        .unwrap();
    repository
        .migrate_record_to_latest(
            record.id,
            MigrateRecordRequest {
                migration_id: preview.migration_id,
                expected_target_version: 2,
                values: Vec::new(),
                relationships: Vec::new(),
                discard_attributes: vec!["obsolete".to_owned()],
                removal_policy: None,
            },
        )
        .await
        .unwrap();

    // Only the discarded value is a field change; the preserved title moved
    // to the new revision's attribute without changing.
    let changes: Vec<(String, String)> = sqlx::query_as(
        "SELECT c.attribute_code, c.change_kind FROM audit_event_changes c WHERE c.record_id = $1 AND c.audit_event_id = (SELECT latest.audit_event_id FROM audit_event_changes latest WHERE latest.record_id = $1 ORDER BY latest.created_at DESC LIMIT 1)",
    )
    .bind(record.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(changes, vec![("obsolete".to_owned(), "remove".to_owned())]);
    let unpublished: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM domain_events WHERE aggregate_id = $1 AND event_type = 'record.unpublished.v1'",
    )
    .bind(record.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(unpublished, 1);
}
