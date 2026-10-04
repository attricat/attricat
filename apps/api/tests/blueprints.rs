mod support;

use support::*;

#[sqlx::test]
async fn blueprint_catalogue_lists_all_kinds_and_revision_history(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let first: Value = client
        .post(format!("{base_url}/blueprints"))
        .json(&json!({
            "definition": "format_version = 1\ncode = \"catalogued_entity\"\nname = \"Catalogued entity revision two\"\nkind = \"entity\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"title\"]\n\n[[attributes]]\ncode = \"title\"\nvalue_type = \"string\""
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let blueprint_id = first["blueprint"]["id"].as_str().unwrap();
    client
        .post(format!("{base_url}/blueprints/{blueprint_id}/versions"))
        .json(&json!({
            "definition": "format_version = 1\ncode = \"catalogued_entity\"\nname = \"Catalogued entity\"\nkind = \"entity\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"title\"]\n\n[[attributes]]\ncode = \"title\"\nvalue_type = \"string\""
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    create_blueprint(
        &client,
        &base_url,
        "format_version = 1\ncode = \"catalogued_mixin\"\nname = \"Catalogued mixin\"\nkind = \"mixin\"\n\n[[attributes]]\ncode = \"label\"\nvalue_type = \"string\"",
    )
    .await;

    let catalogue: Value = client
        .get(format!("{base_url}/blueprints/catalogue"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(catalogue.as_array().unwrap().iter().any(|blueprint| {
        blueprint["code"] == "catalogued_entity"
            && blueprint["version"] == 2
            && blueprint["status"] == "draft"
    }));
    assert!(
        catalogue
            .as_array()
            .unwrap()
            .iter()
            .any(|blueprint| blueprint["kind"] == "mixin")
    );

    let revisions: Value = client
        .get(format!("{base_url}/blueprints/{blueprint_id}/versions"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        revisions
            .as_array()
            .unwrap()
            .iter()
            .map(|blueprint| blueprint["version"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        vec![2, 1]
    );

    server.abort();
}

#[sqlx::test]
async fn persists_readonly_attribute_metadata(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "system_managed_product"
name = "System managed product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "external_id"
value_type = "string"
readonly = true
"#,
    )
    .await;

    assert_eq!(blueprint["attributes"][0]["readonly"], false);
    assert_eq!(blueprint["attributes"][1]["readonly"], true);

    server.abort();
}

#[sqlx::test]
async fn persists_attribute_names_through_includes(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "named_seo"
name = "Named SEO"
kind = "mixin"

[[attributes]]
code = "meta_title"
name = "SEO title"
value_type = "string"
"#,
    )
    .await;
    let blueprint = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "named_product"
name = "Named product"
kind = "entity"

[[includes]]
alias = "seo"
code = "named_seo"
version = 1

[views.dropdown_option]
type = "dropdown_option"
fields = ["product_family"]

[[attributes]]
code = "product_family"
name = "Family"
value_type = "string"

[[attributes]]
code = "sku"
value_type = "string"

[[attributes]]
code = "meta_title"
from = "seo.meta_title"
"#,
    )
    .await;
    let blueprint_id = blueprint["blueprint"]["id"].as_str().unwrap();
    let current: Value = client
        .get(format!("{base_url}/blueprints/{blueprint_id}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    for response in [&blueprint, &current] {
        let names: Vec<_> = response["attributes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|attribute| attribute["name"].clone())
            .collect();
        assert_eq!(names, [json!("Family"), Value::Null, json!("SEO title")]);
    }

    server.abort();
}

#[sqlx::test]
async fn rejects_invalid_toml_and_mixin_entity_creation(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();

    let invalid = client
        .post(format!("{base_url}/blueprints"))
        .json(&json!({
            "definition": "format_version = 1\ncode = 'bad'\nname = 'Bad'\nkind = 'entity'"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        invalid.json::<Value>().await.unwrap()["error"]["code"],
        "invalid_blueprint_definition"
    );

    let mixin = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "visibility"
name = "Visibility"
kind = "mixin"

[[attributes]]
code = "visible"
value_type = "boolean"
"#,
    )
    .await;
    let entity = client
        .post(format!("{base_url}/v1/entities"))
        .json(&json!({
            "blueprint": {
                "code": mixin["blueprint"]["code"],
                "version": mixin["blueprint"]["version"],
            },
            "values": [],
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(entity.status(), StatusCode::NOT_FOUND);

    server.abort();
}

#[sqlx::test]
async fn saves_rich_table_columns_with_resolved_relationship_targets(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "table_category"
name = "Table category"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"
"#,
    )
    .await;
    let response = client
        .post(format!("{base_url}/blueprints"))
        .json(&json!({
            "definition": r#"
format_version = 1
code = "table_product"
name = "Table product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[views.table]
type = "table"
columns = [{ field = "category.name", renderer = { id = "catalog.table_display", version = 1 } }]

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "category"
value_type = "relationship"
target_blueprint = "table_category"
cardinality = "one"
"#
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let saved = response.json::<Value>().await.unwrap();
    assert_eq!(saved["attributes"][1]["cardinality"], "one");
    assert_eq!(saved["attributes"][1]["target_cardinality"], "many");
    assert_eq!(
        saved["blueprint"]["views"]["table"]["columns"][0]["field"],
        "category.name"
    );
    server.abort();
}

fn blueprint_with_rule(code: &str) -> String {
    format!(
        r#"format_version = 1
code = "{code}"
name = "{code}"
kind = "entity"

[[rules]]
code = "title-required"
name = "Title required"
severity = "error"
[[rules.triggers]]
type = "manual"
[rules.predicate]
type = "required"
attribute_code = "title"

[[attributes]]
code = "title"
value_type = "string"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
"#
    )
}

#[sqlx::test]
async fn blueprint_rules_cannot_take_a_rule_code_owned_by_another_blueprint(pool: PgPool) {
    let repository = api::repository::CatalogRepository::new(
        pool.clone(),
        BOOTSTRAP_WORKSPACE_ID.parse().unwrap(),
    );
    let first = repository
        .create_blueprint(api::model::CreateBlueprint {
            definition: blueprint_with_rule("first_ruled"),
        })
        .await
        .unwrap();
    // The owning blueprint's next revision extends its own rule family.
    repository
        .create_blueprint_revision(
            first.blueprint.id,
            api::model::CreateBlueprint {
                definition: blueprint_with_rule("first_ruled")
                    .replace("Title required", "Title is required"),
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        repository
            .create_blueprint(api::model::CreateBlueprint {
                definition: blueprint_with_rule("second_ruled"),
            })
            .await,
        Err(api::repository::RepositoryError::RuleCodeTaken)
    ));
    let families: i64 =
        sqlx::query_scalar("SELECT count(DISTINCT id) FROM rules WHERE code = 'title-required'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(families, 1);
}
