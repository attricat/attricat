mod support;

use support::*;

#[sqlx::test]
async fn one_to_one_relationships_reject_source_and_target_conflicts_per_context(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let target_blueprint = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "cardinality_target"
name = "Cardinality target"
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
    let source_blueprint = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "cardinality_source"
name = "Cardinality source"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "target"
value_type = "relationship"
target_blueprint = "cardinality_target"
cardinality = "one_to_one"
"#,
    )
    .await;
    let target_a = create_entity(&client, &base_url, &target_blueprint).await;
    let target_b = create_entity(&client, &base_url, &target_blueprint).await;
    let source_a = create_entity(&client, &base_url, &source_blueprint).await;
    let source_b = create_entity(&client, &base_url, &source_blueprint).await;

    let replace = |source: &Value, targets: Value| {
        client.post(format!(
        "{base_url}/entities/{}/relationships/replace", source["id"].as_str().unwrap()
    )).json(&json!({"relationships": [{"attribute_code": "target", "target_entity_ids": targets}]}))
    };

    assert_eq!(
        replace(&source_a, json!([target_a["id"]]))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CREATED
    );
    let source_conflict = replace(&source_a, json!([target_a["id"], target_b["id"]]))
        .send()
        .await
        .unwrap();
    assert_eq!(source_conflict.status(), StatusCode::CONFLICT);
    assert_eq!(
        source_conflict.json::<Value>().await.unwrap()["error"]["code"],
        "relationship_cardinality_conflict"
    );

    let target_conflict = replace(&source_b, json!([target_a["id"]]))
        .send()
        .await
        .unwrap();
    assert_eq!(target_conflict.status(), StatusCode::CONFLICT);
    assert_eq!(
        target_conflict.json::<Value>().await.unwrap()["error"]["code"],
        "relationship_cardinality_conflict"
    );

    server.abort();
}

#[sqlx::test]
async fn cardinality_is_enforced_when_an_entity_migrates_to_declaring_revision(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let target = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "migration_cardinality_target"
name = "Target"
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
    let source_definition = r#"
format_version = 1
code = "migration_cardinality_source"
name = "Source"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
[[attributes]]
code = "target"
value_type = "relationship"
target_blueprint = "migration_cardinality_target"
"#;
    let source = create_blueprint(&client, &base_url, source_definition).await;
    let entity = create_entity(&client, &base_url, &source).await;
    let first_target = create_entity(&client, &base_url, &target).await;
    let second_target = create_entity(&client, &base_url, &target).await;
    client.post(format!(
        "{base_url}/entities/{}/relationships/replace",
        entity["id"].as_str().unwrap()
    ))
        .json(&json!({"relationships": [{"attribute_code": "target", "target_entity_ids": [first_target["id"], second_target["id"]]}]}))
        .send().await.unwrap().error_for_status().unwrap();

    let blueprint_id = source["blueprint"]["id"].as_str().unwrap();
    let revision: Value = client.post(format!("{base_url}/blueprints/{blueprint_id}/versions"))
        .json(&json!({"definition": source_definition.replace("target_blueprint = \"migration_cardinality_target\"", "target_blueprint = \"migration_cardinality_target\"\ncardinality = \"one_to_one\"")}))
        .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
    let version = revision["blueprint"]["version"].as_i64().unwrap();
    client
        .post(format!(
            "{base_url}/blueprints/{blueprint_id}/versions/{version}/publish"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let preview: Value = client
        .post(format!(
            "{base_url}/v1/entities/{}/blueprint-migration/preview",
            entity["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(preview["status"], "needs_input");
    assert!(
        preview["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue["kind"] == "relationship_cardinality_conflict")
    );
    server.abort();
}
