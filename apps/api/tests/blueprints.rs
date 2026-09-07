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
