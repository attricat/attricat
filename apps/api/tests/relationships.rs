mod support;

use support::*;

#[sqlx::test]
async fn typed_category_and_color_relationships_can_be_replaced_and_removed(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let category = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "category"
name = "Category"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"
tags = ["searchable"]
"#,
    )
    .await;
    let color = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "color"
name = "Color"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name", "hex"]

[[attributes]]
code = "name"
value_type = "string"
tags = ["searchable"]

[[attributes]]
code = "hex"
value_type = "string"
"#,
    )
    .await;
    let product = create_blueprint(
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
tags = ["searchable"]

[[attributes]]
code = "categories"
value_type = "relationship"
target_blueprint = "category"

[[attributes]]
code = "colors"
value_type = "relationship"
target_blueprint = "color"
"#,
    )
    .await;
    assert_eq!(
        product["attributes"][1]["target_blueprint_code"],
        "category"
    );

    let shirts = create_entity(&client, &base_url, &category).await;
    let sale = create_entity(&client, &base_url, &category).await;
    let navy = create_entity(&client, &base_url, &color).await;
    let shirt = create_entity(&client, &base_url, &product).await;
    let shirt_id = shirt["id"].as_str().unwrap();

    for (entity, values) in [
        (
            &shirts,
            json!({ "values": [{ "kind": "scalar", "attribute_code": "name", "value": "Shirts" }] }),
        ),
        (
            &sale,
            json!({ "values": [{ "kind": "scalar", "attribute_code": "name", "value": "Sale" }] }),
        ),
        (
            &navy,
            json!({ "values": [
            { "kind": "scalar", "attribute_code": "name", "value": "Navy" },
            { "kind": "scalar", "attribute_code": "hex", "value": "#1c2d4a" }
        ] }),
        ),
    ] {
        client
            .post(format!(
                "{base_url}/entities/{}/values",
                entity["id"].as_str().unwrap()
            ))
            .json(&values)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }

    client
        .post(format!("{base_url}/entities/{shirt_id}/values"))
        .json(&json!({ "values": [{
            "kind": "scalar", "attribute_code": "title", "value": "Navy shirt"
        }] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let invalid = client
        .post(format!("{base_url}/entities/{shirt_id}/values"))
        .json(&json!({ "values": [{
            "kind": "relationship", "attribute_code": "categories", "target_entity_id": navy["id"]
        }] }))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        invalid.json::<Value>().await.unwrap()["error"]["code"],
        "relationship_target_type_mismatch"
    );

    let replace = client
        .post(format!(
            "{base_url}/entities/{shirt_id}/relationships/replace"
        ))
        .json(&json!({ "relationships": [
            { "attribute_code": "categories", "target_entity_ids": [shirts["id"], sale["id"]] },
            { "attribute_code": "colors", "target_entity_ids": [navy["id"]] }
        ] }))
        .send()
        .await
        .unwrap();
    assert_eq!(replace.status(), StatusCode::CREATED);

    let preview: Value = client
        .get(format!("{base_url}/entities/{shirt_id}/preview"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        preview["context"]["default"]["categories"]["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        preview["context"]["default"]["categories"]["truncated"],
        false
    );
    assert_eq!(
        preview["context"]["default"]["colors"]["items"][0]["display"],
        "Navy · #1c2d4a"
    );

    let bounded_preview: Value = client
        .get(format!(
            "{base_url}/entities/{shirt_id}/preview?relationship_limit=1"
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
        bounded_preview["context"]["default"]["categories"]["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        bounded_preview["context"]["default"]["categories"]["truncated"],
        true
    );

    let scalar_preview: Value = client
        .get(format!(
            "{base_url}/entities/{shirt_id}/preview?relationship_depth=0"
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
        scalar_preview["context"],
        json!({
            "default": {
                "title": "Navy shirt",
                "categories": { "items": [], "truncated": true },
                "colors": { "items": [], "truncated": true }
            }
        })
    );

    let excessive_depth = client
        .get(format!(
            "{base_url}/entities/{shirt_id}/preview?relationship_depth=4"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(excessive_depth.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let first_page: Value = client
        .get(format!(
            "{base_url}/entities?blueprint=category&related_from={shirt_id}&relationship=categories&limit=1"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(first_page["items"].as_array().unwrap().len(), 1);
    let cursor = first_page["next_cursor"].as_str().unwrap();
    let second_page: Value = client
        .get(format!(
            "{base_url}/entities?blueprint=category&related_from={shirt_id}&relationship=categories&limit=1&cursor={cursor}"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(second_page["items"].as_array().unwrap().len(), 1);
    assert!(second_page["next_cursor"].is_null());

    client
        .post(format!(
            "{base_url}/entities/{shirt_id}/relationships/remove"
        ))
        .json(&json!({ "relationships": [{
            "attribute_code": "categories", "target_entity_ids": [sale["id"]]
        }] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let preview: Value = client
        .get(format!("{base_url}/entities/{shirt_id}/preview"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        preview["context"]["default"]["categories"]["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        preview["context"]["default"]["categories"]["items"][0]["display"],
        "Shirts"
    );

    let history: Vec<Value> = client
        .get(format!("{base_url}/entities/{shirt_id}/values/history"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let removed_sale = history
        .iter()
        .find(|value| value["relationship_target_entity_id"] == sale["id"])
        .unwrap();
    client
        .post(format!(
            "{base_url}/entities/{shirt_id}/values/history/{}/restore",
            removed_sale["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let restored_preview: Value = client
        .get(format!("{base_url}/entities/{shirt_id}/preview"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        restored_preview["context"]["default"]["categories"]["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let delete = client
        .delete(format!(
            "{base_url}/entities/{}",
            shirts["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(delete.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        client
            .get(format!(
                "{base_url}/entities/{}",
                shirts["id"].as_str().unwrap()
            ))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    let preview: Value = client
        .get(format!("{base_url}/entities/{shirt_id}/preview"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        preview["context"]["default"]["categories"]["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        preview["context"]["default"]["categories"]["items"][0]["display"],
        "Sale"
    );
    assert_eq!(
        client
            .delete(format!(
                "{base_url}/entities/{}",
                shirts["id"].as_str().unwrap()
            ))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );

    server.abort();
}

#[sqlx::test]
async fn previews_expand_cycles_and_shared_targets_per_path(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "linked_node"
name = "Linked node"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "links"
value_type = "relationship"
target_blueprint = "linked_node"
"#,
    )
    .await;
    let mut ids = Vec::new();
    for name in ["a", "b", "c", "d"] {
        let entity = create_entity(&client, &base_url, &blueprint).await;
        let id = entity["id"].as_str().unwrap().to_owned();
        client
            .post(format!("{base_url}/entities/{id}/values"))
            .json(&json!({ "values": [
                { "kind": "scalar", "attribute_code": "name", "value": name }
            ] }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
        ids.push(id);
    }
    // a -> b, c; b -> a, c, d; c -> d; d -> a
    for (source, targets) in [
        (0, vec![1, 2]),
        (1, vec![0, 2, 3]),
        (2, vec![3]),
        (3, vec![0]),
    ] {
        client
            .post(format!(
                "{base_url}/entities/{}/relationships/replace",
                ids[source]
            ))
            .json(&json!({ "relationships": [{
                "attribute_code": "links",
                "target_entity_ids": targets.iter().map(|target: &usize| ids[*target].clone()).collect::<Vec<_>>()
            }] }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }
    let names = ["a", "b", "c", "d"];
    let mut rendered = Vec::new();
    for (depth, limit) in [(0, 2), (1, 5), (2, 5), (3, 5)] {
        let preview = client
            .get(format!(
                "{base_url}/entities/{}/preview?relationship_depth={depth}&relationship_limit={limit}",
                ids[0]
            ))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .text()
            .await
            .unwrap();
        let mut preview = preview;
        for (id, name) in ids.iter().zip(names) {
            preview = preview.replace(id.as_str(), &format!("<{name}>"));
        }
        let mut value: Value = serde_json::from_str(&preview).unwrap();
        value["entity"]["blueprint_id"] = json!("<blueprint>");
        sort_items(&mut value["context"]);
        rendered.push(value["context"].clone());
    }
    // An entity on the expansion path is listed but not expanded again; the
    // same target reached through different paths is expanded on each.
    let expected: Value = serde_json::from_str(
        r#"[{"default":{"links":{"items":[],"truncated":true},"name":"a"}},{"default":{"links":{"items":[{"display":"b","id":"<b>","links":{"items":[],"truncated":true}},{"display":"c","id":"<c>","links":{"items":[],"truncated":true}}],"truncated":false},"name":"a"}},{"default":{"links":{"items":[{"display":"b","id":"<b>","links":{"items":[{"display":"a","id":"<a>"},{"display":"c","id":"<c>","links":{"items":[],"truncated":true}},{"display":"d","id":"<d>","links":{"items":[],"truncated":true}}],"truncated":false}},{"display":"c","id":"<c>","links":{"items":[{"display":"d","id":"<d>","links":{"items":[],"truncated":true}}],"truncated":false}}],"truncated":false},"name":"a"}},{"default":{"links":{"items":[{"display":"b","id":"<b>","links":{"items":[{"display":"a","id":"<a>"},{"display":"c","id":"<c>","links":{"items":[{"display":"d","id":"<d>","links":{"items":[],"truncated":true}}],"truncated":false}},{"display":"d","id":"<d>","links":{"items":[{"display":"a","id":"<a>"}],"truncated":false}}],"truncated":false}},{"display":"c","id":"<c>","links":{"items":[{"display":"d","id":"<d>","links":{"items":[{"display":"a","id":"<a>"}],"truncated":false}}],"truncated":false}}],"truncated":false},"name":"a"}}]"#,
    )
    .unwrap();
    assert_eq!(Value::Array(rendered), expected);
    server.abort();
}

/// Orders every relationship item list by its target, so snapshots do not
/// depend on the random entity IDs that decide edge order.
fn sort_items(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                sort_items(child);
                if key == "items"
                    && let Value::Array(items) = child
                {
                    items.sort_by_key(|item| item["id"].as_str().unwrap_or_default().to_owned());
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(sort_items),
        _ => {}
    }
}
