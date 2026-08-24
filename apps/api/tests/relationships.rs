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
