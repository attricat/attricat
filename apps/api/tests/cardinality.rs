mod support;

use support::*;

#[sqlx::test]
async fn directional_cardinality_rejects_source_and_target_conflicts_per_context(pool: PgPool) {
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
cardinality = "one"
target_cardinality = "one"
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
async fn source_one_allows_many_sources_to_share_a_target(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let target = create_blueprint(
        &client,
        &base_url,
        r#"format_version = 1
code = "shared_cardinality_target"
name = "Shared target"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string""#,
    )
    .await;
    let source = create_blueprint(
        &client,
        &base_url,
        r#"format_version = 1
code = "shared_cardinality_source"
name = "Shared source"
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
target_blueprint = "shared_cardinality_target"
cardinality = "one""#,
    )
    .await;
    let target = create_entity(&client, &base_url, &target).await;
    for source in [
        create_entity(&client, &base_url, &source).await,
        create_entity(&client, &base_url, &source).await,
    ] {
        let response = client
            .post(format!(
                "{base_url}/entities/{}/relationships/replace",
                source["id"].as_str().unwrap()
            ))
            .json(&json!({"relationships": [{
                "attribute_code": "target",
                "target_entity_ids": [target["id"]]
            }]}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }
    server.abort();
}

#[sqlx::test]
async fn concurrent_opposite_order_appends_do_not_deadlock(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let target = create_blueprint(
        &client,
        &base_url,
        r#"format_version = 1
code = "concurrent_cardinality_target"
name = "Concurrent target"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string""#,
    )
    .await;
    let source = create_blueprint(
        &client,
        &base_url,
        r#"format_version = 1
code = "concurrent_cardinality_source"
name = "Concurrent source"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
[[attributes]]
code = "targets"
value_type = "relationship"
target_blueprint = "concurrent_cardinality_target"
target_cardinality = "one""#,
    )
    .await;
    let target_a = create_entity(&client, &base_url, &target).await;
    let target_b = create_entity(&client, &base_url, &target).await;
    let source_a = create_entity(&client, &base_url, &source).await;
    let source_b = create_entity(&client, &base_url, &source).await;
    let target_a_id = target_a["id"].as_str().unwrap().to_owned();
    let target_b_id = target_b["id"].as_str().unwrap().to_owned();
    let append = |source: Value, targets: Vec<String>| {
        let client = client.clone();
        let base_url = base_url.clone();
        async move {
            client
                .post(format!(
                    "{base_url}/entities/{}/values",
                    source["id"].as_str().unwrap()
                ))
                .json(
                    &json!({"values": targets.into_iter().map(|target_entity_id| json!({
                    "kind": "relationship",
                    "attribute_code": "targets",
                    "target_entity_id": target_entity_id
                })).collect::<Vec<_>>() }),
                )
                .send()
                .await
                .unwrap()
                .status()
        }
    };
    let (first, second) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        tokio::join!(
            append(source_a, vec![target_a_id.clone(), target_b_id.clone()]),
            append(source_b, vec![target_b_id, target_a_id]),
        )
    })
    .await
    .expect("opposite-order cardinality writers must not deadlock");
    assert!(
        matches!(first, StatusCode::CREATED | StatusCode::CONFLICT)
            && matches!(second, StatusCode::CREATED | StatusCode::CONFLICT)
    );
    assert_ne!(first, second);
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
        .json(&json!({"definition": source_definition.replace("target_blueprint = \"migration_cardinality_target\"", "target_blueprint = \"migration_cardinality_target\"\ncardinality = \"one\"")}))
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
