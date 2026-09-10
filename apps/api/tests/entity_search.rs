mod support;

use support::*;

#[sqlx::test]
async fn search_includes_all_blueprint_versions_and_marks_outdated_entities(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let first = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"
"#,
    )
    .await;
    let first_entity = create_entity(&client, &base_url, &first).await;
    let blueprint_id = first["blueprint"]["id"].as_str().unwrap();
    let second: Value = client
        .post(format!("{base_url}/blueprints/{blueprint_id}/versions"))
        .json(&json!({
            "definition": r#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["sku"]

[[attributes]]
code = "sku"
value_type = "string"
"#
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let second: Value = client
        .post(format!(
            "{base_url}/blueprints/{blueprint_id}/versions/{}/publish",
            second["blueprint"]["version"].as_i64().unwrap(),
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let second_entity = create_entity(&client, &base_url, &second).await;

    let search = |version: Option<i64>| {
        let client = client.clone();
        let base_url = base_url.clone();
        async move {
            client
                .post(format!("{base_url}/v1/entities/search"))
                .json(&json!({
                    "blueprint": { "code": "product", "version": version },
                    "include_total": true,
                    "filters": [],
                    "page": { "size": 25, "cursor": null }
                }))
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()
        }
    };

    let all_versions = search(None).await;
    assert_eq!(all_versions["blueprint"]["blueprint"]["version"], 2);
    assert_eq!(all_versions["items"].as_array().unwrap().len(), 2);
    assert_eq!(all_versions["total_count"], 2);
    assert_eq!(all_versions["total_count_capped"], false);
    assert!(
        all_versions["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| {
                item["id"] == first_entity["id"]
                    && item["blueprint_version"] == 1
                    && item["schema_outdated"] == true
            })
    );
    assert!(
        all_versions["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| {
                item["id"] == second_entity["id"]
                    && item["blueprint_version"] == 2
                    && item["schema_outdated"] == false
            })
    );

    let outdated: Value = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({
            "blueprint": { "code": "product" },
            "outdated": true,
            "page": { "size": 25 }
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(outdated["items"].as_array().unwrap().len(), 1);
    assert_eq!(outdated["items"][0]["id"], first_entity["id"]);
    assert_eq!(outdated["items"][0]["schema_outdated"], true);

    let first_version = search(Some(1)).await;
    assert_eq!(first_version["items"].as_array().unwrap().len(), 1);
    assert_eq!(first_version["total_count"], 1);
    assert_eq!(first_version["items"][0]["id"], first_entity["id"]);
    assert_eq!(first_version["items"][0]["schema_outdated"], true);

    server.abort();
}

#[sqlx::test]
async fn search_filters_scalar_attributes_with_type_appropriate_operators(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "filter_product"
name = "Filter product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "price"
value_type = "number"

[[attributes]]
code = "stock"
value_type = "integer"

[[attributes]]
code = "available"
value_type = "boolean"

[[attributes]]
code = "released_on"
value_type = "date"
"#,
    )
    .await;
    let first = create_entity(&client, &base_url, &blueprint).await;
    let second = create_entity(&client, &base_url, &blueprint).await;
    for (entity, values) in [
        (
            &first,
            json!([
                { "kind": "scalar", "attribute_code": "name", "value": "Red shoe" },
                { "kind": "scalar", "attribute_code": "price", "value": 120.5 },
                { "kind": "scalar", "attribute_code": "stock", "value": 8 },
                { "kind": "scalar", "attribute_code": "available", "value": true },
                { "kind": "scalar", "attribute_code": "released_on", "value": "2026-04-01" }
            ]),
        ),
        (
            &second,
            json!([
                { "kind": "scalar", "attribute_code": "name", "value": "Blue bag" },
                { "kind": "scalar", "attribute_code": "price", "value": 75 },
                { "kind": "scalar", "attribute_code": "stock", "value": 2 },
                { "kind": "scalar", "attribute_code": "available", "value": false },
                { "kind": "scalar", "attribute_code": "released_on", "value": "2025-01-01" }
            ]),
        ),
    ] {
        client
            .post(format!(
                "{base_url}/entities/{}/values",
                entity["id"].as_str().unwrap()
            ))
            .json(&json!({ "values": values }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }

    let response: Value = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({
            "blueprint": { "code": "filter_product" },
            "filters": [
                { "field": "name", "operator": "contains", "value": "shoe" },
                { "field": "price", "operator": "gte", "value": 100 },
                { "field": "stock", "operator": "gt", "value": 5 },
                { "field": "available", "operator": "eq", "value": true },
                { "field": "released_on", "operator": "gt", "value": "2026-01-01" }
            ],
            "include_total": true
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(response["items"].as_array().unwrap().len(), 1);
    assert_eq!(response["items"][0]["id"], first["id"]);
    assert_eq!(response["total_count"], 1);

    let invalid = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({
            "blueprint": { "code": "filter_product" },
            "filters": [{ "field": "available", "operator": "gt", "value": true }]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);

    server.abort();
}

#[sqlx::test]
async fn search_returns_contextual_relationship_tree_facet_and_filters_selected_subtrees(
    pool: PgPool,
) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let category = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "facet_category"
name = "Facet category"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "parent"
value_type = "relationship"
target_blueprint = "facet_category"
"#,
    )
    .await;
    let product = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "facet_product"
name = "Facet product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "categories"
value_type = "relationship"
target_blueprint = "facet_category"
"#,
    )
    .await;
    let root = create_entity(&client, &base_url, &category).await;
    let alternate_root = create_entity(&client, &base_url, &category).await;
    let child = create_entity(&client, &base_url, &category).await;
    let first = create_entity(&client, &base_url, &product).await;
    let second = create_entity(&client, &base_url, &product).await;
    let context: Value = client
        .post(format!("{base_url}/contexts"))
        .json(&json!({ "code": "facet_context", "data": {} }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    for (entity, values) in [
        (
            &root,
            json!([{ "kind": "scalar", "attribute_code": "name", "value": "Root" }]),
        ),
        (
            &child,
            json!([{ "kind": "scalar", "attribute_code": "name", "value": "Child" }]),
        ),
        (
            &alternate_root,
            json!([{ "kind": "scalar", "attribute_code": "name", "value": "Alternate root" }]),
        ),
        (
            &first,
            json!([{ "kind": "scalar", "attribute_code": "title", "value": "First" }]),
        ),
        (
            &second,
            json!([{ "kind": "scalar", "attribute_code": "title", "value": "Second" }]),
        ),
    ] {
        client
            .post(format!(
                "{base_url}/entities/{}/values",
                entity["id"].as_str().unwrap()
            ))
            .json(&json!({ "values": values }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }
    for (entity, field, targets) in [
        (
            &child,
            "parent",
            vec![root["id"].clone(), alternate_root["id"].clone()],
        ),
        (&first, "categories", vec![child["id"].clone()]),
        (&second, "categories", vec![root["id"].clone()]),
    ] {
        client
            .put(format!(
                "{base_url}/v1/entities/{}",
                entity["id"].as_str().unwrap()
            ))
            .json(&json!({
                "relationships": [{ "attribute_code": field, "target_entity_ids": targets }]
            }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }

    let search: Value = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({
            "blueprint": { "code": "facet_product" },
            "relationship_tree_facets": [{
                "source_relationship_field": "categories",
                "hierarchy_field": "parent",
                "context_id": context["id"],
                "selected_target_ids": [root["id"]]
            }]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(search["items"].as_array().unwrap().len(), 2);

    let intersected: Value = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({
            "blueprint": { "code": "facet_product" },
            "relationship_tree_facets": [{
                "source_relationship_field": "categories",
                "hierarchy_field": "parent",
                "context_id": context["id"],
                "selected_target_ids": [root["id"]]
            }, {
                "source_relationship_field": "categories",
                "hierarchy_field": "parent",
                "context_id": context["id"],
                "selected_target_ids": [child["id"]]
            }]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(intersected["items"].as_array().unwrap().len(), 1);
    assert_eq!(intersected["items"][0]["id"], first["id"]);

    let hierarchy: Value = client
        .get(format!(
            "{base_url}/entities/{}/hierarchy?context_id={}&field=parent",
            child["id"].as_str().unwrap(),
            context["id"].as_str().unwrap(),
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(hierarchy["paths"].as_array().unwrap().len(), 2);
    assert!(hierarchy["multiple_parents"].as_bool().unwrap());

    let text_filtered: Value = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({
            "blueprint": { "code": "facet_product" },
            "query": "First",
            "relationship_tree_facets": [{
                "source_relationship_field": "categories",
                "hierarchy_field": "parent",
                "context_id": context["id"],
                "selected_target_ids": []
            }]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(text_filtered["items"].as_array().unwrap().len(), 1);

    let root_page: Value = client
        .post(format!(
            "{base_url}/v1/entities/facets/relationship-tree/children"
        ))
        .json(&json!({
            "blueprint": { "code": "facet_product" },
            "source_relationship_field": "categories",
            "hierarchy_field": "parent",
            "context_id": context["id"],
            "selected_target_ids": [root["id"]]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(root_page["next_cursor"], Value::Null);
    assert_eq!(root_page["selected_items"][0]["id"], root["id"]);
    assert_eq!(root_page["selected_items"][0]["display"], "Root");
    let root_item = root_page["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == root["id"])
        .unwrap();
    assert_eq!(root_item["count"], 2);
    assert_eq!(root_item["has_children"], true);

    let child_page: Value = client
        .post(format!(
            "{base_url}/v1/entities/facets/relationship-tree/children"
        ))
        .json(&json!({
            "blueprint": { "code": "facet_product" },
            "source_relationship_field": "categories",
            "hierarchy_field": "parent",
            "context_id": context["id"],
            "parent_id": root["id"],
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        child_page["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == child["id"])
    );

    let flat_search: Value = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({
            "blueprint": { "code": "facet_product" },
            "relationship_tree_facets": [{
                "source_relationship_field": "categories",
                "context_id": context["id"],
                "selected_target_ids": [root["id"]]
            }]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(flat_search["items"].as_array().unwrap().len(), 1);
    assert_eq!(flat_search["items"][0]["id"], second["id"]);

    let flat_page: Value = client
        .post(format!(
            "{base_url}/v1/entities/facets/relationship-tree/children"
        ))
        .json(&json!({
            "blueprint": { "code": "facet_product" },
            "source_relationship_field": "categories",
            "context_id": context["id"],
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let flat_root = flat_page["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == root["id"])
        .unwrap();
    assert_eq!(flat_root["count"], 1);
    assert_eq!(flat_root["has_children"], false);

    server.abort();
}
