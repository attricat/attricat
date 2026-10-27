use crate::support;

use support::*;

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn search_includes_all_blueprint_versions_and_marks_outdated_records(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let first = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "product"
name = "Product"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"
"#,
    )
    .await;
    let first_record = create_record(&client, &base_url, &first).await;
    let blueprint_id = first["blueprint"]["id"].as_str().unwrap();
    let second: Value = client
        .post(format!("{base_url}/blueprints/{blueprint_id}/versions"))
        .json(&json!({
            "definition": r#"
format_version = 1
code = "product"
name = "Product"
kind = "record"

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
    let second_record = create_record(&client, &base_url, &second).await;

    let search = |version: Option<i64>| {
        let client = client.clone();
        let base_url = base_url.clone();
        async move {
            client
                .post(format!("{base_url}/v1/records/search"))
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
                item["id"] == first_record["id"]
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
                item["id"] == second_record["id"]
                    && item["blueprint_version"] == 2
                    && item["schema_outdated"] == false
            })
    );

    let outdated: Value = client
        .post(format!("{base_url}/v1/records/search"))
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
    assert_eq!(outdated["items"][0]["id"], first_record["id"]);
    assert_eq!(outdated["items"][0]["schema_outdated"], true);

    let first_version = search(Some(1)).await;
    assert_eq!(first_version["items"].as_array().unwrap().len(), 1);
    assert_eq!(first_version["total_count"], 1);
    assert_eq!(first_version["items"][0]["id"], first_record["id"]);
    assert_eq!(first_version["items"][0]["schema_outdated"], true);

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
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
kind = "record"

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
    let first = create_record(&client, &base_url, &blueprint).await;
    let second = create_record(&client, &base_url, &blueprint).await;
    for (record, values) in [
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
                "{base_url}/records/{}/values",
                record["id"].as_str().unwrap()
            ))
            .json(&json!({ "values": values }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }

    let response: Value = client
        .post(format!("{base_url}/v1/records/search"))
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
        .post(format!("{base_url}/v1/records/search"))
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

#[sqlx::test(migrator = "api::MIGRATOR")]
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
kind = "record"

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
kind = "record"

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
    let root = create_record(&client, &base_url, &category).await;
    let alternate_root = create_record(&client, &base_url, &category).await;
    let child = create_record(&client, &base_url, &category).await;
    let first = create_record(&client, &base_url, &product).await;
    let second = create_record(&client, &base_url, &product).await;
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
    for (record, values) in [
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
                "{base_url}/records/{}/values",
                record["id"].as_str().unwrap()
            ))
            .json(&json!({ "values": values }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }
    for (record, field, targets) in [
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
                "{base_url}/v1/records/{}",
                record["id"].as_str().unwrap()
            ))
            .json(&json!({
                "relationships": [{ "attribute_code": field, "target_record_ids": targets }]
            }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }

    let search: Value = client
        .post(format!("{base_url}/v1/records/search"))
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
        .post(format!("{base_url}/v1/records/search"))
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
            "{base_url}/records/{}/hierarchy?context_id={}&field=parent",
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
        .post(format!("{base_url}/v1/records/search"))
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
            "{base_url}/v1/records/facets/relationship-tree/children"
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
    assert_eq!(root_page["selected_paths"], json!([[root["id"]]]));
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
            "{base_url}/v1/records/facets/relationship-tree/children"
        ))
        .json(&json!({
            "blueprint": { "code": "facet_product" },
            "source_relationship_field": "categories",
            "hierarchy_field": "parent",
            "context_id": context["id"],
            "parent_id": root["id"],
            "selected_target_ids": [child["id"]]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    // The child has two parents; its path follows the lower ID.
    let lower_parent = [&root, &alternate_root]
        .into_iter()
        .map(|parent| parent["id"].as_str().unwrap().parse::<Uuid>().unwrap())
        .min()
        .unwrap();
    assert_eq!(
        child_page["selected_paths"],
        json!([[
            lower_parent.to_string(),
            child["id"].as_str().unwrap().to_owned()
        ]])
    );
    assert!(
        child_page["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == child["id"])
    );

    let flat_search: Value = client
        .post(format!("{base_url}/v1/records/search"))
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
            "{base_url}/v1/records/facets/relationship-tree/children"
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

    let relationship_path_search: Value = client
        .post(format!("{base_url}/v1/records/search"))
        .json(&json!({
            "blueprint": { "code": "facet_product" },
            "relationship_filters": [{
                "field": "categories.parent",
                "selected_target_ids": [alternate_root["id"]]
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
    assert_eq!(
        relationship_path_search["items"].as_array().unwrap().len(),
        1
    );
    assert_eq!(relationship_path_search["items"][0]["id"], first["id"]);

    let invalid_relationship_path = client
        .post(format!("{base_url}/v1/records/search"))
        .json(&json!({
            "blueprint": { "code": "facet_product" },
            "relationship_filters": [{
                "field": "categories.name",
                "selected_target_ids": [root["id"]]
            }]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        invalid_relationship_path.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // Without a source relationship, the facet walks the blueprint's own tree:
    // a selected node matches itself and its descendants.
    let own_tree: Value = client
        .post(format!("{base_url}/v1/records/search"))
        .json(&json!({
            "blueprint": { "code": "facet_category" },
            "relationship_tree_facets": [{
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
    let mut own_tree_ids: Vec<_> = own_tree["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].clone())
        .collect();
    own_tree_ids.sort_by_key(|id| id.to_string());
    let mut expected = vec![root["id"].clone(), child["id"].clone()];
    expected.sort_by_key(|id| id.to_string());
    assert_eq!(own_tree_ids, expected);

    let own_roots: Value = client
        .post(format!(
            "{base_url}/v1/records/facets/relationship-tree/children"
        ))
        .json(&json!({
            "blueprint": { "code": "facet_category" },
            "hierarchy_field": "parent",
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
    let own_roots = own_roots["items"].as_array().unwrap();
    assert_eq!(own_roots.len(), 2);
    for item in own_roots {
        // Each root counts itself and the shared child.
        assert_eq!(item["count"], 2);
        assert_eq!(item["has_children"], true);
    }
    let own_children: Value = client
        .post(format!(
            "{base_url}/v1/records/facets/relationship-tree/children"
        ))
        .json(&json!({
            "blueprint": { "code": "facet_category" },
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
    assert_eq!(own_children["items"][0]["id"], child["id"]);
    assert_eq!(own_children["items"][0]["count"], 1);
    assert_eq!(own_children["items"][0]["has_children"], false);

    for (path, body) in [
        (
            "/v1/records/search",
            json!({
                "blueprint": { "code": "facet_category" },
                "relationship_tree_facets": [{
                    "context_id": context["id"],
                    "selected_target_ids": [root["id"]]
                }]
            }),
        ),
        (
            "/v1/records/facets/relationship-tree/children",
            json!({
                "blueprint": { "code": "facet_category" },
                "context_id": context["id"],
            }),
        ),
    ] {
        let missing_hierarchy = client
            .post(format!("{base_url}{path}"))
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(missing_hierarchy.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn relationship_tree_facets_resolve_each_source_in_its_nearest_context(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "nearest_category"
name = "Nearest category"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "parent"
value_type = "relationship"
target_blueprint = "nearest_category"
"#,
    )
    .await;
    create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "nearest_product"
name = "Nearest product"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "category"
value_type = "relationship"
target_blueprint = "nearest_category"

[[attributes]]
code = "local_category"
value_type = "relationship"
target_blueprint = "nearest_category"
context_fallback = "none"
"#,
    )
    .await;
    let category = json!({ "blueprint": { "code": "nearest_category", "version": 1 } });
    let product = json!({ "blueprint": { "code": "nearest_product", "version": 1 } });
    let root = create_record(&client, &base_url, &category).await;
    let child = create_record(&client, &base_url, &category).await;
    let other = create_record(&client, &base_url, &category).await;
    let overridden = create_record(&client, &base_url, &product).await;
    let inherited = create_record(&client, &base_url, &product).await;
    let context: Value = client
        .post(format!("{base_url}/contexts"))
        .json(&json!({ "code": "nearest_child", "data": {} }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    for (record, field, context_id, targets) in [
        (&child, "parent", Value::Null, vec![&root]),
        (&overridden, "category", Value::Null, vec![&child]),
        (&overridden, "category", context["id"].clone(), vec![&other]),
        (&inherited, "category", Value::Null, vec![&child]),
        (&inherited, "local_category", Value::Null, vec![&root]),
    ] {
        client
            .put(format!(
                "{base_url}/v1/records/{}",
                record["id"].as_str().unwrap()
            ))
            .json(&json!({
                "relationships": [{
                    "attribute_code": field,
                    "context_id": context_id,
                    "target_record_ids": targets.iter().map(|t| &t["id"]).collect::<Vec<_>>(),
                }]
            }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }
    let default_id = "00000000-0000-4000-8000-000000000001";
    let search = |field: &'static str, context_id: Value, selected: &Value| {
        let client = client.clone();
        let base_url = base_url.clone();
        let selected = selected["id"].clone();
        async move {
            let response: Value = client
                .post(format!("{base_url}/v1/records/search"))
                .json(&json!({
                    "blueprint": { "code": "nearest_product" },
                    "relationship_tree_facets": [{
                        "source_relationship_field": field,
                        "hierarchy_field": "parent",
                        "context_id": context_id,
                        "selected_target_ids": [selected],
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
            let mut ids: Vec<_> = response["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item["id"].as_str().unwrap().to_owned())
                .collect();
            ids.sort();
            ids
        }
    };
    let ids = |records: &[&Value]| {
        let mut ids: Vec<_> = records
            .iter()
            .map(|record| record["id"].as_str().unwrap().to_owned())
            .collect();
        ids.sort();
        ids
    };
    // The child context's value replaces the inherited one for that record only.
    assert_eq!(
        search("category", json!(default_id), &root).await,
        ids(&[&overridden, &inherited])
    );
    assert_eq!(
        search("category", context["id"].clone(), &root).await,
        ids(&[&inherited])
    );
    assert_eq!(
        search("category", context["id"].clone(), &other).await,
        ids(&[&overridden])
    );
    // Without fallback, a default-context value does not apply in a child context.
    assert_eq!(
        search("local_category", json!(default_id), &root).await,
        ids(&[&inherited])
    );
    assert!(
        search("local_category", context["id"].clone(), &root)
            .await
            .is_empty()
    );

    let roots: Value = client
        .post(format!(
            "{base_url}/v1/records/facets/relationship-tree/children"
        ))
        .json(&json!({
            "blueprint": { "code": "nearest_product" },
            "source_relationship_field": "category",
            "hierarchy_field": "parent",
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
    let count = |id: &Value| {
        roots["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| &item["id"] == id)
            .map(|item| item["count"].clone())
    };
    assert_eq!(count(&root["id"]), Some(json!(1)));
    assert_eq!(count(&other["id"]), Some(json!(1)));
    assert_eq!(count(&child["id"]), None);

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn relationship_tree_facet_counts_without_a_query_may_lag_while_nodes_do_not(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "lagging_category"
name = "Lagging category"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "parent"
value_type = "relationship"
target_blueprint = "lagging_category"
"#,
    )
    .await;
    create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "lagging_product"
name = "Lagging product"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "category"
value_type = "relationship"
target_blueprint = "lagging_category"
"#,
    )
    .await;
    let category = json!({ "blueprint": { "code": "lagging_category", "version": 1 } });
    let product = json!({ "blueprint": { "code": "lagging_product", "version": 1 } });
    let root = create_record(&client, &base_url, &category).await;
    let link = |record: Value, target: Value| {
        let client = client.clone();
        let base_url = base_url.clone();
        async move {
            client
                .put(format!(
                    "{base_url}/v1/records/{}",
                    record["id"].as_str().unwrap()
                ))
                .json(&json!({
                    "relationships": [{
                        "attribute_code": "category",
                        "target_record_ids": [target["id"]],
                    }]
                }))
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap();
        }
    };
    link(
        create_record(&client, &base_url, &product).await,
        root.clone(),
    )
    .await;
    let roots = |query: Option<&'static str>| {
        let client = client.clone();
        let base_url = base_url.clone();
        async move {
            let mut body = json!({
                "blueprint": { "code": "lagging_product" },
                "source_relationship_field": "category",
                "hierarchy_field": "parent",
                "context_id": "00000000-0000-4000-8000-000000000001",
            });
            if let Some(query) = query {
                body["query"] = json!(query);
            }
            let page: Value = client
                .post(format!(
                    "{base_url}/v1/records/facets/relationship-tree/children"
                ))
                .json(&body)
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap()
                .json()
                .await
                .unwrap();
            page["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|item| (item["id"].clone(), item["count"].as_i64().unwrap()))
                .collect::<Vec<_>>()
        }
    };
    assert_eq!(roots(None).await, vec![(root["id"].clone(), 1)]);

    let second_root = create_record(&client, &base_url, &category).await;
    let second = create_record(&client, &base_url, &product).await;
    client
        .post(format!(
            "{base_url}/records/{}/values",
            second["id"].as_str().unwrap()
        ))
        .json(&json!({ "values": [{ "kind": "scalar", "attribute_code": "title", "value": "Second" }] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    link(second, second_root.clone()).await;

    // The new node shows at once; the cached count still predates the link.
    let lagging = roots(None).await;
    assert_eq!(lagging.len(), 2);
    assert!(lagging.contains(&(root["id"].clone(), 1)));
    assert!(lagging.contains(&(second_root["id"].clone(), 0)));
    // A search term counts its matches exactly.
    let searched = roots(Some("Second")).await;
    assert!(searched.contains(&(root["id"].clone(), 0)));
    assert!(searched.contains(&(second_root["id"].clone(), 1)));

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn relationship_tree_facet_children_page_through_roots_by_cursor(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "paged_category"
name = "Paged category"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "parent"
value_type = "relationship"
target_blueprint = "paged_category"
"#,
    )
    .await;
    let category = json!({ "blueprint": { "code": "paged_category", "version": 1 } });
    let mut expected = Vec::new();
    // One more root than the test server's page size.
    for _ in 0..101 {
        let record = create_record(&client, &base_url, &category).await;
        expected.push(record["id"].as_str().unwrap().to_owned());
    }
    expected.sort();
    let mut seen = Vec::new();
    let mut cursor = Value::Null;
    loop {
        let page: Value = client
            .post(format!(
                "{base_url}/v1/records/facets/relationship-tree/children"
            ))
            .json(&json!({
                "blueprint": { "code": "paged_category" },
                "hierarchy_field": "parent",
                "context_id": "00000000-0000-4000-8000-000000000001",
                "cursor": cursor,
            }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        seen.extend(
            page["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item["id"].as_str().unwrap().to_owned()),
        );
        cursor = page["next_cursor"].clone();
        if cursor.is_null() {
            break;
        }
    }
    assert_eq!(seen, expected);

    // A node past the first page can be asked for, without moving the cursor;
    // IDs on the page or outside the level are ignored.
    let children = |include_ids: Value| {
        client
            .post(format!(
                "{base_url}/v1/records/facets/relationship-tree/children"
            ))
            .json(&json!({
                "blueprint": { "code": "paged_category" },
                "hierarchy_field": "parent",
                "context_id": "00000000-0000-4000-8000-000000000001",
                "include_ids": include_ids,
            }))
            .send()
    };
    let first_page: Value = children(json!([expected[100], expected[0], Uuid::new_v4()]))
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let ids: Vec<_> = first_page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ids.len(), 101);
    assert_eq!(ids.last(), Some(&expected[100]));
    assert_eq!(first_page["next_cursor"], expected[99]);
    let too_many: Vec<_> = (0..101).map(|_| Uuid::new_v4()).collect();
    assert_eq!(
        children(json!(too_many)).await.unwrap().status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn relationship_tree_facet_paths_resolve_every_hop_in_its_nearest_context(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    for definition in [
        r#"
format_version = 1
code = "path_style"
name = "Path style"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "parent"
value_type = "relationship"
target_blueprint = "path_style"
cardinality = "one"
tree = true
context_editable = "default"
"#,
        r#"
format_version = 1
code = "path_brand"
name = "Path brand"
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
code = "path_model"
name = "Path model"
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
target_blueprint = "path_brand"
cardinality = "one"

[[attributes]]
code = "style"
value_type = "relationship"
target_blueprint = "path_style"
cardinality = "one"

[[attributes]]
code = "local_style"
value_type = "relationship"
target_blueprint = "path_style"
cardinality = "one"
context_fallback = "none"

[[attributes]]
code = "styles"
value_type = "relationship"
target_blueprint = "path_style"
cardinality = "many"
"#,
        r#"
format_version = 1
code = "path_vehicle"
name = "Path vehicle"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["vin"]

[views.explorer]
type = "explorer"
browse_by = "model.brand"

[[attributes]]
code = "vin"
value_type = "string"

[[attributes]]
code = "model"
value_type = "relationship"
target_blueprint = "path_model"
cardinality = "one"
"#,
    ] {
        create_blueprint(&client, &base_url, definition).await;
    }
    let blueprint = |code: &str| json!({ "blueprint": { "code": code, "version": 1 } });
    let body = create_record(&client, &base_url, &blueprint("path_style")).await;
    let sedan = create_record(&client, &base_url, &blueprint("path_style")).await;
    let coupe = create_record(&client, &base_url, &blueprint("path_style")).await;
    let alpha = create_record(&client, &base_url, &blueprint("path_brand")).await;
    let beta = create_record(&client, &base_url, &blueprint("path_brand")).await;
    let first_model = create_record(&client, &base_url, &blueprint("path_model")).await;
    let second_model = create_record(&client, &base_url, &blueprint("path_model")).await;
    let first = create_record(&client, &base_url, &blueprint("path_vehicle")).await;
    let second = create_record(&client, &base_url, &blueprint("path_vehicle")).await;
    let switched = create_record(&client, &base_url, &blueprint("path_vehicle")).await;
    create_record(&client, &base_url, &blueprint("path_vehicle")).await;
    let context: Value = client
        .post(format!("{base_url}/contexts"))
        .json(&json!({ "code": "path_child", "data": {} }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    // The child context overrides one record on each hop: the second model's
    // style, and the model of the switched vehicle.
    for (record, field, context_id, target) in [
        (&sedan, "parent", Value::Null, &body),
        (&first_model, "brand", Value::Null, &alpha),
        (&first_model, "style", Value::Null, &sedan),
        (&first_model, "local_style", Value::Null, &body),
        (&second_model, "brand", Value::Null, &beta),
        (&second_model, "style", Value::Null, &coupe),
        (&second_model, "style", context["id"].clone(), &sedan),
        (&first, "model", Value::Null, &first_model),
        (&second, "model", Value::Null, &second_model),
        (&switched, "model", Value::Null, &first_model),
        (&switched, "model", context["id"].clone(), &second_model),
    ] {
        client
            .put(format!(
                "{base_url}/v1/records/{}",
                record["id"].as_str().unwrap()
            ))
            .json(&json!({
                "relationships": [{
                    "attribute_code": field,
                    "context_id": context_id,
                    "target_record_ids": [target["id"]],
                }]
            }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }
    client
        .post(format!(
            "{base_url}/records/{}/values",
            second["id"].as_str().unwrap()
        ))
        .json(&json!({ "values": [{ "kind": "scalar", "attribute_code": "vin", "value": "Second" }] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let default_id = json!("00000000-0000-4000-8000-000000000001");
    let child_id = context["id"].clone();
    let search = |path: &'static str, context_id: &Value, selected: &Value| {
        let client = client.clone();
        let base_url = base_url.clone();
        let mut facet = json!({
            "source_relationship_field": path,
            "context_id": context_id,
            "selected_target_ids": [selected["id"]],
        });
        if path.ends_with("style") {
            facet["hierarchy_field"] = json!("parent");
        }
        async move {
            let response: Value = client
                .post(format!("{base_url}/v1/records/search"))
                .json(&json!({
                    "blueprint": { "code": "path_vehicle" },
                    "relationship_tree_facets": [facet],
                }))
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap()
                .json()
                .await
                .unwrap();
            let mut ids: Vec<_> = response["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item["id"].as_str().unwrap().to_owned())
                .collect();
            ids.sort();
            ids
        }
    };
    let ids = |records: &[&Value]| {
        let mut ids: Vec<_> = records
            .iter()
            .map(|record| record["id"].as_str().unwrap().to_owned())
            .collect();
        ids.sort();
        ids
    };

    // A flat path lists the last hop's targets.
    assert_eq!(
        search("model.brand", &default_id, &alpha).await,
        ids(&[&first, &switched])
    );
    assert_eq!(
        search("model.brand", &child_id, &alpha).await,
        ids(&[&first])
    );
    assert_eq!(
        search("model.brand", &child_id, &beta).await,
        ids(&[&second, &switched])
    );
    // A tree path matches records reaching the node or any descendant, with
    // each hop resolved in its nearest context.
    assert_eq!(
        search("model.style", &default_id, &body).await,
        ids(&[&first, &switched])
    );
    assert_eq!(
        search("model.style", &default_id, &coupe).await,
        ids(&[&second])
    );
    assert_eq!(
        search("model.style", &child_id, &body).await,
        ids(&[&first, &second, &switched])
    );
    assert!(search("model.style", &child_id, &coupe).await.is_empty());
    // A hop without fallback reads only the requested context.
    assert_eq!(
        search("model.local_style", &default_id, &body).await,
        ids(&[&first, &switched])
    );
    assert!(
        search("model.local_style", &child_id, &body)
            .await
            .is_empty()
    );

    let children = |path: &'static str, context_id: &Value, query: Option<&'static str>| {
        let client = client.clone();
        let base_url = base_url.clone();
        let mut request = json!({
            "blueprint": { "code": "path_vehicle" },
            "source_relationship_field": path,
            "context_id": context_id,
        });
        if path.ends_with("style") {
            request["hierarchy_field"] = json!("parent");
        }
        if let Some(query) = query {
            request["query"] = json!(query);
        }
        async move {
            let page: Value = client
                .post(format!(
                    "{base_url}/v1/records/facets/relationship-tree/children"
                ))
                .json(&request)
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap()
                .json()
                .await
                .unwrap();
            let mut items: Vec<_> = page["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|item| {
                    (
                        item["id"].as_str().unwrap().to_owned(),
                        item["count"].as_i64().unwrap(),
                        item["has_children"].as_bool().unwrap(),
                    )
                })
                .collect();
            items.sort();
            items
        }
    };
    let node = |record: &Value, count: i64, has_children: bool| {
        (
            record["id"].as_str().unwrap().to_owned(),
            count,
            has_children,
        )
    };
    let sorted = |mut items: Vec<(String, i64, bool)>| {
        items.sort();
        items
    };
    assert_eq!(
        children("model.brand", &default_id, None).await,
        sorted(vec![node(&alpha, 2, false), node(&beta, 1, false)])
    );
    assert_eq!(
        children("model.brand", &child_id, None).await,
        sorted(vec![node(&alpha, 1, false), node(&beta, 2, false)])
    );
    // Tree counts roll up from descendants; the path is part of the cache key.
    assert_eq!(
        children("model.style", &default_id, None).await,
        sorted(vec![node(&body, 2, true), node(&coupe, 1, false)])
    );
    assert_eq!(
        children("model.local_style", &default_id, None).await,
        sorted(vec![node(&body, 2, true), node(&coupe, 0, false)])
    );
    assert_eq!(
        children("model.style", &child_id, None).await,
        sorted(vec![node(&body, 3, true), node(&coupe, 0, false)])
    );
    // A search term counts its matches exactly.
    assert_eq!(
        children("model.style", &child_id, Some("Second")).await,
        sorted(vec![node(&body, 1, true), node(&coupe, 0, false)])
    );

    for (path, message) in [
        ("model.styles", "cardinality"),
        ("model.name", "must be a relationship attribute"),
        ("model.brand.name.other", "at most three relationship hops"),
        ("model.missing", "must be a relationship attribute"),
    ] {
        let response = client
            .post(format!(
                "{base_url}/v1/records/facets/relationship-tree/children"
            ))
            .json(&json!({
                "blueprint": { "code": "path_vehicle" },
                "source_relationship_field": path,
                "context_id": default_id,
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "{path}"
        );
        let error = response.text().await.unwrap();
        assert!(error.contains(message), "{path}: {error}");
    }

    server.abort();
}
