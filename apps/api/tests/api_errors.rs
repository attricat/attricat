mod support;

use support::*;

#[sqlx::test]
async fn rejects_unsafe_reference_codes_in_api_requests(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "product-type"
name = "Product type"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["display-name"]

[[attributes]]
code = "display-name"
value_type = "string"
"#,
    )
    .await;
    let entity = create_entity(&client, &base_url, &blueprint).await;
    let default_context: Value = client
        .get(format!("{base_url}/contexts/default"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    for url in [
        format!("{base_url}/blueprints/by-code/product.type"),
        format!("{base_url}/contexts/en.GB"),
    ] {
        assert_eq!(
            client.get(url).send().await.unwrap().status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }

    let invalid_search = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({ "blueprint": { "code": " product-type" } }))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid_search.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let invalid_preview_list = client
        .get(format!(
            "{base_url}/entities?blueprint=product-type&related_from={}&relationship=related.products",
            entity["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(
        invalid_preview_list.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let invalid_value = client
        .post(format!(
            "{base_url}/entities/{}/values",
            entity["id"].as_str().unwrap()
        ))
        .json(&json!({ "values": [{
            "kind": "scalar",
            "attribute_code": "display.name",
            "context_id": default_context["id"],
            "value": "Product"
        }] }))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid_value.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        invalid_value.json::<Value>().await.unwrap()["error"]["code"],
        "invalid_input"
    );

    let invalid_relationship = client
        .post(format!(
            "{base_url}/entities/{}/relationships/replace",
            entity["id"].as_str().unwrap()
        ))
        .json(&json!({ "relationships": [{
            "attribute_code": "related.products",
            "context_id": default_context["id"],
            "target_entity_ids": []
        }] }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        invalid_relationship.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let invalid_remove = client
        .put(format!(
            "{base_url}/v1/entities/{}",
            entity["id"].as_str().unwrap()
        ))
        .json(&json!({ "remove_values": [{
            "attribute_code": "display.name",
            "context_id": default_context["id"]
        }] }))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid_remove.status(), StatusCode::UNPROCESSABLE_ENTITY);

    server.abort();
}
