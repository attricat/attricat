use crate::support;

use support::*;

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn blueprint_catalogue_lists_all_kinds_and_revision_history(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let first: Value = client
        .post(format!("{base_url}/blueprints"))
        .json(&json!({
            "definition": "format_version = 1\ncode = \"catalogued_record\"\nname = \"Catalogued record revision two\"\nkind = \"record\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"title\"]\n\n[[attributes]]\ncode = \"title\"\nvalue_type = \"string\""
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
            "definition": "format_version = 1\ncode = \"catalogued_record\"\nname = \"Catalogued record\"\nkind = \"record\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"title\"]\n\n[[attributes]]\ncode = \"title\"\nvalue_type = \"string\""
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
        blueprint["code"] == "catalogued_record"
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

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn persists_readonly_attribute_metadata_and_describes_attributes(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "system_managed_product"
name = "System managed product"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "external_id"
description = "The ID in the supplier's system"
value_type = "string"
readonly = true
"#,
    )
    .await;

    assert_eq!(blueprint["attributes"][0]["readonly"], false);
    assert_eq!(blueprint["attributes"][1]["readonly"], true);
    // Descriptions come from the definition, on create and on every read.
    assert_eq!(blueprint["attributes"][0]["description"], Value::Null);
    assert_eq!(
        blueprint["attributes"][1]["description"],
        "The ID in the supplier's system"
    );
    let read: Value = client
        .get(format!(
            "{base_url}/blueprints/by-code/system_managed_product"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        read["attributes"][1]["description"],
        "The ID in the supplier's system"
    );

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
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
kind = "record"

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

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn rejects_invalid_toml_and_mixin_record_creation(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();

    let invalid = client
        .post(format!("{base_url}/blueprints"))
        .json(&json!({
            "definition": "format_version = 1\ncode = 'bad'\nname = 'Bad'\nkind = 'record'"
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
    let record = client
        .post(format!("{base_url}/v1/records"))
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
    assert_eq!(record.status(), StatusCode::NOT_FOUND);

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
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
kind = "record"

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
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[views.table]
type = "table"
columns = [{ field = "category.name", renderer = { id = "attricat.table_display", version = 1 } }]

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

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn explorer_browse_by_paths_resolve_through_published_targets(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    for definition in [
        r#"
format_version = 1
code = "browse_brand"
name = "Browse brand"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"
"#,
        r#"
format_version = 1
code = "browse_model"
name = "Browse model"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "brand"
value_type = "relationship"
target_blueprint = "browse_brand"
cardinality = "one"

[[attributes]]
code = "brands"
value_type = "relationship"
target_blueprint = "browse_brand"
cardinality = "many"
"#,
    ] {
        create_blueprint(&client, &base_url, definition).await;
    }
    let vehicle = |browse_by: &str| {
        format!(
            r#"
format_version = 1
code = "browse_vehicle"
name = "Browse vehicle"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["vin"]

[views.explorer]
type = "explorer"
browse_by = "{browse_by}"

[[attributes]]
code = "vin"
value_type = "string"

[[attributes]]
code = "model"
value_type = "relationship"
target_blueprint = "browse_model"
cardinality = "one"
"#
        )
    };
    for (path, error) in [
        ("model.brands", "segment 'brands' must be a relationship"),
        ("model.name", "segment 'name' must be a relationship"),
        ("model.missing", "segment 'missing' must be a relationship"),
        (
            "model.brand.name.other",
            "browse_by 'model.brand.name.other'",
        ),
        ("vin.brand", "browse_by 'vin.brand'"),
    ] {
        let response = client
            .post(format!("{base_url}/blueprints"))
            .json(&json!({ "definition": vehicle(path) }))
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "{path}"
        );
        let body = response.text().await.unwrap();
        assert!(body.contains(error), "{path}: {body}");
    }
    let saved = create_blueprint(&client, &base_url, &vehicle("model.brand")).await;
    assert_eq!(
        saved["blueprint"]["views"]["explorer"]["browse_by"],
        "model.brand"
    );
    server.abort();
}

fn blueprint_with_rule(code: &str) -> String {
    format!(
        r#"format_version = 1
code = "{code}"
name = "{code}"
kind = "record"

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

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn blueprint_rules_cannot_take_a_rule_code_owned_by_another_blueprint(pool: PgPool) {
    let repository = api::repository::AttricatRepository::new(
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
        Err(api::repository::RepositoryError::CodeTaken(
            api::repository::CodedResource::Rule
        ))
    ));
    let families: i64 =
        sqlx::query_scalar("SELECT count(DISTINCT id) FROM rules WHERE code = 'title-required'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(families, 1);
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn validates_definitions_without_storing_them(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let definition = |code: &str, field: &str| {
        format!(
            "format_version = 1\ncode = \"{code}\"\nname = \"Checked\"\nkind = \"record\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"{field}\"]\n\n[[attributes]]\ncode = \"title\"\nvalue_type = \"string\""
        )
    };

    let checked: Value = client
        .post(format!("{base_url}/blueprints/validate"))
        .json(&json!({ "definition": definition("checked", "title") }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(checked["blueprint"]["version"], 1);
    assert_eq!(checked["attributes"][0]["code"], "title");
    let missing = client
        .get(format!("{base_url}/blueprints/by-code/checked"))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);

    let unknown_field = client
        .post(format!("{base_url}/blueprints/validate"))
        .json(&json!({ "definition": definition("checked", "missing") }))
        .send()
        .await
        .unwrap();
    assert_eq!(unknown_field.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let error = unknown_field.json::<Value>().await.unwrap();
    assert_eq!(error["error"]["code"], "invalid_blueprint_definition");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("unknown attribute 'missing'")
    );

    let created = create_blueprint(&client, &base_url, &definition("checked", "title")).await;
    let blueprint_id = created["blueprint"]["id"].as_str().unwrap();
    let taken = client
        .post(format!("{base_url}/blueprints/validate"))
        .json(&json!({ "definition": definition("checked", "title") }))
        .send()
        .await
        .unwrap();
    assert!(taken.status().is_client_error());
    let revision: Value = client
        .post(format!(
            "{base_url}/blueprints/{blueprint_id}/versions/validate"
        ))
        .json(&json!({
            "definition": definition("checked", "title").replace("Checked", "Checked again")
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(revision["blueprint"]["version"], 2);
    let renamed = client
        .post(format!(
            "{base_url}/blueprints/{blueprint_id}/versions/validate"
        ))
        .json(&json!({ "definition": definition("renamed", "title") }))
        .send()
        .await
        .unwrap();
    assert_eq!(renamed.status(), StatusCode::UNPROCESSABLE_ENTITY);
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
    assert_eq!(revisions.as_array().unwrap().len(), 1);

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn checks_enum_value_labels(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let blueprint = |labels: &str| {
        format!(
            "format_version = 1\ncode = \"sized\"\nname = \"Sized\"\nkind = \"record\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"size\"]\n\n[[attributes]]\ncode = \"size\"\nvalue_type = \"string\"\nvalue_schema = '{{\"type\":\"string\",\"enum\":[\"s\",\"l\"],\"x-attricat-enum-labels\":{labels}}}'\n"
        )
    };
    let reusable = |labels: &str| {
        format!(
            "code = \"size\"\nname = \"Size\"\nvalue_type = \"string\"\nvalue_schema = {{ type = \"string\", enum = [\"s\", \"l\"], \"x-attricat-enum-labels\" = {labels} }}\n"
        )
    };
    for (route, definition) in [
        (
            "blueprints/validate",
            blueprint(r#"{"s":"Small","l":"{{Large}}"}"#),
        ),
        (
            "reusable-attributes/validate",
            reusable(r#"{ s = "Small" }"#),
        ),
    ] {
        let response = client
            .post(format!("{base_url}/{route}"))
            .json(&json!({ "definition": definition }))
            .send()
            .await
            .unwrap();
        assert!(
            response.status().is_success(),
            "{route}: {}",
            response.text().await.unwrap()
        );
    }
    for (route, definition, message) in [
        (
            "blueprints/validate",
            blueprint(r#"{"m":"Medium"}"#),
            "labels 'm', which is not in the enum",
        ),
        (
            "blueprints/validate",
            blueprint(r#"{"s":"{{Small"}"#),
            "enum value 's' label has an invalid lexicon reference",
        ),
        (
            "reusable-attributes/validate",
            reusable(r#"{ s = " " }"#),
            "label for 's' must be 1 to 200 characters",
        ),
    ] {
        let response = client
            .post(format!("{base_url}/{route}"))
            .json(&json!({ "definition": definition }))
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "{route}"
        );
        let error = response.json::<Value>().await.unwrap();
        assert!(
            error["error"]["message"]
                .as_str()
                .unwrap()
                .contains(message),
            "{error}"
        );
    }

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn checks_an_unchanged_definition_as_the_revision_it_matches(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let definition = "format_version = 1\ncode = \"unchanged\"\nname = \"Unchanged\"\nkind = \"record\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"title\"]\n\n[[attributes]]\ncode = \"title\"\nvalue_type = \"string\"";
    let created = create_blueprint(&client, &base_url, definition).await;
    let blueprint_id = created["blueprint"]["id"].as_str().unwrap();

    let checked: Value = client
        .post(format!(
            "{base_url}/blueprints/{blueprint_id}/versions/validate"
        ))
        .json(&json!({ "definition": definition }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(checked["blueprint"]["version"], 1);

    let saved = client
        .post(format!("{base_url}/blueprints/{blueprint_id}/versions"))
        .json(&json!({ "definition": definition }))
        .send()
        .await
        .unwrap();
    assert_eq!(saved.status(), StatusCode::CONFLICT);
    let error = saved.json::<Value>().await.unwrap();
    assert_eq!(
        error["error"]["message"],
        "the definition is identical to version 1 of this blueprint"
    );

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_revision_can_return_to_an_earlier_definition(pool: PgPool) {
    let repository = api::repository::AttricatRepository::new(
        pool.clone(),
        BOOTSTRAP_WORKSPACE_ID.parse().unwrap(),
    );
    let original = blueprint_with_rule("reverted");
    let first = repository
        .create_blueprint(api::model::CreateBlueprint {
            definition: original.clone(),
        })
        .await
        .unwrap();
    let blueprint_id = first.blueprint.id;
    // Adding a field keeps the embedded rule's definition as it was.
    repository
        .create_blueprint_revision(
            blueprint_id,
            api::model::CreateBlueprint {
                definition: format!(
                    "{original}\n[[attributes]]\ncode = \"subtitle\"\nvalue_type = \"string\"\n"
                ),
            },
        )
        .await
        .unwrap();
    // Removing it again matches version 1, which is not the latest.
    let reverted = repository
        .create_blueprint_revision(
            blueprint_id,
            api::model::CreateBlueprint {
                definition: original.clone(),
            },
        )
        .await
        .unwrap();
    assert_eq!(reverted.blueprint.version, 3);
    assert!(matches!(
        repository
            .create_blueprint_revision(
                blueprint_id,
                api::model::CreateBlueprint {
                    definition: original,
                },
            )
            .await,
        Err(api::repository::RepositoryError::BlueprintRevisionUnchanged { version: 3 })
    ));
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn checks_running_during_a_save_number_the_revision_after_it(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let definition = |name: &str| {
        format!(
            "format_version = 1\ncode = \"raced\"\nname = \"{name}\"\nkind = \"record\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"title\"]\n\n[[attributes]]\ncode = \"title\"\nvalue_type = \"string\""
        )
    };
    let created = create_blueprint(&client, &base_url, &definition("Raced")).await;
    let blueprint_id = created["blueprint"]["id"].as_str().unwrap();
    let post = |path: &str, definition: String| {
        client
            .post(format!(
                "{base_url}/blueprints/{blueprint_id}/versions{path}"
            ))
            .json(&json!({ "definition": definition }))
            .send()
    };

    // A check that waits for a save must see the saved revision, whether it
    // checks that definition or another one.
    for round in 0..10 {
        let (same, saved, other) = tokio::join!(
            post("/validate", definition(&format!("Saved {round}"))),
            post("", definition(&format!("Saved {round}"))),
            post("/validate", definition(&format!("Other {round}"))),
        );
        for response in [same, saved, other] {
            let response = response.unwrap();
            assert!(
                response.status().is_success(),
                "round {round}: {}",
                response.text().await.unwrap()
            );
        }
    }

    server.abort();
}
