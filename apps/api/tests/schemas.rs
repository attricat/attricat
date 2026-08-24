mod support;

use support::*;

#[sqlx::test]
async fn validates_attribute_and_entity_json_schemas(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(
        &client,
        &base_url,
        r#"format_version = 1
code = "schema_product"
name = "Schema product"
kind = "entity"
entity_schema = '{"type":"object","required":["title","price"]}'

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"
value_schema = '{"type":"string","minLength":3}'

[[attributes]]
code = "price"
value_type = "number"
value_schema = '{"type":"number","minimum":0}'"#,
    )
    .await;
    let contexts: Vec<Value> = client
        .get(format!("{base_url}/contexts"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let context_id = contexts
        .iter()
        .find(|context| context["code"] == "default")
        .unwrap()["id"]
        .clone();
    let create = |values: Value| {
        client.post(format!("{base_url}/v1/entities")).json(&json!({
            "blueprint": {
                "code": blueprint["blueprint"]["code"],
                "version": blueprint["blueprint"]["version"],
            },
            "values": values,
        }))
    };
    let response = create(json!([
        { "kind": "scalar", "attribute_code": "title", "context_id": context_id, "value": "ok" },
        { "kind": "scalar", "attribute_code": "price", "context_id": context_id, "value": 10 },
    ]))
    .send()
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "attribute_value_schema_mismatch"
    );

    let response = create(json!([
        { "kind": "scalar", "attribute_code": "title", "context_id": context_id, "value": "Valid" },
    ]))
    .send()
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "entity_schema_mismatch"
    );

    let response = create(json!([
        { "kind": "scalar", "attribute_code": "title", "context_id": context_id, "value": "Valid" },
        { "kind": "scalar", "attribute_code": "price", "context_id": context_id, "value": 10 },
    ]))
    .send()
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let entity: Value = response.json().await.unwrap();
    let response = client
        .put(format!("{base_url}/v1/entities/{}", entity["id"].as_str().unwrap()))
        .json(&json!({
            "values": [{ "kind": "scalar", "attribute_code": "price", "context_id": context_id, "value": -1 }]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "attribute_value_schema_mismatch"
    );
    server.abort();
}
