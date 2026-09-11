mod support;

use support::*;

#[sqlx::test]
async fn search_resolves_three_hop_table_query_filter_and_sort_paths(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();

    let kind = create_blueprint(
        &client,
        &base_url,
        r#"format_version = 1
code = "deep_kind"
name = "Deep kind"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string""#,
    )
    .await;
    let class = create_blueprint(
        &client,
        &base_url,
        r#"format_version = 1
code = "deep_class"
name = "Deep class"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
[[attributes]]
code = "kind"
value_type = "relationship"
target_blueprint = "deep_kind"
cardinality = "one""#,
    )
    .await;
    let family = create_blueprint(
        &client,
        &base_url,
        r#"format_version = 1
code = "deep_family"
name = "Deep family"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
[[attributes]]
code = "class"
value_type = "relationship"
target_blueprint = "deep_class"
cardinality = "one""#,
    )
    .await;
    let sku = create_blueprint(
        &client,
        &base_url,
        r#"format_version = 1
code = "deep_sku"
name = "Deep SKU"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["sku"]
[views.table]
type = "table"
columns = [
  { field = "sku", label = "SKU" },
  { field = "family.class.kind.name", label = "Kind" }
]
[[attributes]]
code = "sku"
value_type = "string"
[[attributes]]
code = "family"
value_type = "relationship"
target_blueprint = "deep_family"
cardinality = "one""#,
    )
    .await;

    let kind_entity = create_entity(&client, &base_url, &kind).await;
    let class_entity = create_entity(&client, &base_url, &class).await;
    let family_entity = create_entity(&client, &base_url, &family).await;
    let sku_entity = create_entity(&client, &base_url, &sku).await;
    for (entity, attribute, value) in [
        (&kind_entity, "name", "Graphics Card"),
        (&class_entity, "name", "Component"),
        (&family_entity, "name", "Arc"),
        (&sku_entity, "sku", "ARC-1"),
    ] {
        client
            .post(format!("{base_url}/entities/{}/values", entity["id"].as_str().unwrap()))
            .json(&json!({"values": [{"kind": "scalar", "attribute_code": attribute, "value": value}]}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }
    for (source, field, target) in [
        (&class_entity, "kind", &kind_entity),
        (&family_entity, "class", &class_entity),
        (&sku_entity, "family", &family_entity),
    ] {
        client
            .post(format!(
                "{base_url}/entities/{}/relationships/replace",
                source["id"].as_str().unwrap()
            ))
            .json(&json!({"relationships": [{
                "attribute_code": field,
                "target_entity_ids": [target["id"]]
            }]}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }

    for body in [
        json!({
            "blueprint": {"code": "deep_sku"},
            "query": "family.class.kind.name:Graphics",
            "page": {"size": 25}
        }),
        json!({
            "blueprint": {"code": "deep_sku"},
            "filters": [{"field": "family.class.kind.name", "operator": "eq", "value": "Graphics Card"}],
            "page": {"size": 25}
        }),
        json!({
            "blueprint": {"code": "deep_sku"},
            "sort": {"field": "family.class.kind.name", "direction": "asc"},
            "page": {"size": 25}
        }),
    ] {
        let raw = client
            .post(format!("{base_url}/v1/entities/search"))
            .json(&body)
            .send()
            .await
            .unwrap();
        let status = raw.status();
        let text = raw.text().await.unwrap();
        assert!(status.is_success(), "search failed with {status}: {text}");
        let response: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(response["items"].as_array().unwrap().len(), 1);
        assert_eq!(
            response["blueprint"]["table_path_attributes"][1],
            json!({
                "code": "family.class.kind.name",
                "value_type": "string",
                "sortable": true
            })
        );
        assert_eq!(
            response["items"][0]["table_values"]["family.class.kind.name"],
            json!(["Graphics Card"])
        );
    }

    let kind_blueprint_id = kind["blueprint"]["id"].as_str().unwrap();
    let kind_draft: Value = client
        .post(format!(
            "{base_url}/blueprints/{kind_blueprint_id}/versions"
        ))
        .json(&json!({"definition": r#"format_version = 1
code = "deep_kind"
name = "Deep kind"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "integer""#}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let before_publish: Value = client
        .get(format!("{base_url}/blueprints/by-code/deep_sku"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        before_publish["table_path_attributes"][1]["value_type"], "string",
        "newer drafts must not affect published path metadata"
    );

    let kind_draft_version = kind_draft["blueprint"]["version"].as_i64().unwrap();
    client
        .post(format!(
            "{base_url}/blueprints/{kind_blueprint_id}/versions/{kind_draft_version}/publish"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let incompatible: Value = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({
            "blueprint": {"code": "deep_sku"},
            "page": {"size": 25}
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        incompatible["blueprint"]["table_path_attributes"][1]["value_type"],
        "integer"
    );
    assert!(
        incompatible["items"][0]["table_values"]
            .get("family.class.kind.name")
            .is_none(),
        "a pinned string leaf must not be hydrated under integer path metadata"
    );

    server.abort();
}

#[sqlx::test]
async fn all_revision_sort_treats_historical_many_hops_as_null(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let leaf = create_blueprint(
        &client,
        &base_url,
        r#"format_version = 1
code = "versioned_sort_leaf"
name = "Versioned sort leaf"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string""#,
    )
    .await;
    let middle = create_blueprint(
        &client,
        &base_url,
        r#"format_version = 1
code = "versioned_sort_middle"
name = "Versioned sort middle"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
[[attributes]]
code = "leaf"
value_type = "relationship"
target_blueprint = "versioned_sort_leaf"
cardinality = "one""#,
    )
    .await;
    let source_v1_definition = r#"format_version = 1
code = "versioned_sort_source"
name = "Versioned sort source"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[views.table]
type = "table"
columns = [{ field = "middle.leaf.name", label = "Leaf" }]
[[attributes]]
code = "name"
value_type = "string"
[[attributes]]
code = "middle"
value_type = "relationship"
target_blueprint = "versioned_sort_middle""#;
    let source_v1 = create_blueprint(&client, &base_url, source_v1_definition).await;
    let leaf_entity = create_entity(&client, &base_url, &leaf).await;
    let middle_entity = create_entity(&client, &base_url, &middle).await;
    let old_source = create_entity(&client, &base_url, &source_v1).await;
    client
        .post(format!(
            "{base_url}/entities/{}/values",
            leaf_entity["id"].as_str().unwrap()
        ))
        .json(&json!({"values": [{"kind": "scalar", "attribute_code": "name", "value": "Alpha"}]}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    for (source, field, target) in [
        (&middle_entity, "leaf", &leaf_entity),
        (&old_source, "middle", &middle_entity),
    ] {
        client
            .post(format!("{base_url}/entities/{}/relationships/replace", source["id"].as_str().unwrap()))
            .json(&json!({"relationships": [{"attribute_code": field, "target_entity_ids": [target["id"]]}]}))
            .send().await.unwrap().error_for_status().unwrap();
    }

    let source_id = source_v1["blueprint"]["id"].as_str().unwrap();
    let revision: Value = client
        .post(format!("{base_url}/blueprints/{source_id}/versions"))
        .json(&json!({"definition": source_v1_definition.replace(
            "target_blueprint = \"versioned_sort_middle\"",
            "target_blueprint = \"versioned_sort_middle\"\ncardinality = \"one\"",
        )}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let version = revision["blueprint"]["version"].as_i64().unwrap();
    let source_v2: Value = client
        .post(format!(
            "{base_url}/blueprints/{source_id}/versions/{version}/publish"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let current_source = create_entity(&client, &base_url, &source_v2).await;
    client
        .post(format!("{base_url}/entities/{}/relationships/replace", current_source["id"].as_str().unwrap()))
        .json(&json!({"relationships": [{"attribute_code": "middle", "target_entity_ids": [middle_entity["id"]]}]}))
        .send().await.unwrap().error_for_status().unwrap();

    let historical_sort = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({
            "blueprint": {"code": "versioned_sort_source", "version": 1},
            "sort": {"field": "middle.leaf.name", "direction": "asc"},
            "page": {"size": 1}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(historical_sort.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let first: Value = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({
            "blueprint": {"code": "versioned_sort_source"},
            "sort": {"field": "middle.leaf.name", "direction": "asc"},
            "page": {"size": 1}
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(first["items"][0]["id"], current_source["id"]);
    let cursor = first["next_cursor"].as_str().expect("first page cursor");
    let second: Value = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({
            "blueprint": {"code": "versioned_sort_source"},
            "sort": {"field": "middle.leaf.name", "direction": "asc"},
            "page": {"size": 1, "cursor": cursor}
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(second["items"][0]["id"], old_source["id"]);
    assert!(second["next_cursor"].is_null());
    server.abort();
}
