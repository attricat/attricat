mod support;

use support::*;

#[sqlx::test]
async fn restores_a_scalar_value_from_synchronous_history(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(
        &client,
        &base_url,
        "format_version = 1\ncode = \"restorable_product\"\nname = \"Restorable product\"\nkind = \"entity\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"title\"]\n\n[[attributes]]\ncode = \"title\"\nvalue_type = \"string\"",
    )
    .await;
    let entity = create_entity(&client, &base_url, &blueprint).await;
    let entity_id = entity["id"].as_str().unwrap();

    for title in ["Original title", "Replacement title"] {
        client
            .post(format!("{base_url}/entities/{entity_id}/values"))
            .json(&json!({ "values": [{
                "kind": "scalar", "attribute_code": "title", "value": title
            }] }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }

    let history: Vec<Value> = client
        .get(format!("{base_url}/entities/{entity_id}/values/history"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0]["value"], "Original title");
    let history_id = history[0]["id"].as_str().unwrap();

    client
        .post(format!(
            "{base_url}/entities/{entity_id}/values/history/{history_id}/restore"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let current: Vec<Value> = client
        .get(format!("{base_url}/entities/{entity_id}/values/current"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(current.len(), 1);
    assert_eq!(current[0]["value"], "Original title");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM attribute_value_history WHERE entity_id = $1"
        )
        .bind(entity_id.parse::<Uuid>().unwrap())
        .fetch_one(&pool)
        .await
        .unwrap(),
        2
    );

    server.abort();
}

#[sqlx::test]
async fn stores_typed_scalar_values_in_native_columns(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "measurement"
name = "Measurement"
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
code = "quantity"
value_type = "integer"

[[attributes]]
code = "available"
value_type = "boolean"

[[attributes]]
code = "available_on"
value_type = "date"

[[attributes]]
code = "released_at"
value_type = "datetime"

[[attributes]]
code = "cutoff"
value_type = "time"
"#,
    )
    .await;
    let entity = create_entity(&client, &base_url, &blueprint).await;
    let entity_id: Uuid = entity["id"].as_str().unwrap().parse().unwrap();
    client
        .post(format!("{base_url}/entities/{entity_id}/values"))
        .json(&json!({ "values": [
            { "kind": "scalar", "attribute_code": "name", "value": "Widget" },
            { "kind": "scalar", "attribute_code": "price", "value": 12.50 },
            { "kind": "scalar", "attribute_code": "quantity", "value": 3 },
            { "kind": "scalar", "attribute_code": "available", "value": true },
            { "kind": "scalar", "attribute_code": "available_on", "value": "2026-08-12" },
            { "kind": "scalar", "attribute_code": "released_at", "value": "2026-08-12T14:30:00Z" },
            { "kind": "scalar", "attribute_code": "cutoff", "value": { "time": "09:30:00", "time_zone": "America/New_York" } }
        ] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let values: Vec<Value> = client
        .get(format!("{base_url}/entities/{entity_id}/values/current"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(values.iter().any(|value| value["value"] == json!(true)));
    assert!(values.iter().any(|value| value["value"] == json!(12.50)));
    assert!(
        values
            .iter()
            .any(|value| value["value"] == json!("2026-08-12T14:30:00+00:00"))
    );
    assert!(
        values.iter().any(|value| value["value"]
            == json!({ "time": "09:30:00", "time_zone": "America/New_York" }))
    );
    assert_eq!(
        sqlx::query_scalar::<_, rust_decimal::Decimal>(
            "SELECT SUM(value_number) FROM attribute_values WHERE entity_id = $1"
        )
        .bind(entity_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "12.50".parse().unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, bool>(
            "SELECT value_boolean FROM attribute_values WHERE entity_id = $1 AND value_boolean IS NOT NULL"
        )
        .bind(entity_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        true
    );

    server.abort();
}

#[sqlx::test]
async fn rejects_mismatched_native_value_storage_on_read(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "typed_read"
name = "Typed read"
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
    let entity = create_entity(&client, &base_url, &blueprint).await;
    let entity_id: Uuid = entity["id"].as_str().unwrap().parse().unwrap();
    let attribute_id: Uuid = blueprint["attributes"][0]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    sqlx::query(
        "INSERT INTO attribute_values (id, entity_id, attribute_id, context_id, value_number) VALUES ($1, $2, $3, $4, 12.50)",
    )
    .bind(Uuid::new_v4())
    .bind(entity_id)
    .bind(attribute_id)
    .bind(Uuid::from_u128(0x00000000000040008000000000000001))
    .execute(&pool)
    .await
    .unwrap();

    let response = client
        .get(format!("{base_url}/entities/{entity_id}/values/current"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    server.abort();
}

#[sqlx::test]
async fn rejects_values_from_another_blueprint_version(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();

    let first = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "first"
name = "First"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"
tags = ["searchable"]
"#,
    )
    .await;
    let second = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "second"
name = "Second"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"
tags = ["searchable"]
"#,
    )
    .await;
    let entity = create_entity(&client, &base_url, &first).await;

    let response = client
        .post(format!(
            "{base_url}/entities/{}/values",
            entity["id"].as_str().unwrap()
        ))
        .json(&json!({
            "values": [{
                "kind": "scalar",
                "attribute_id": second["attributes"][0]["id"],
                "context_id": null,
                "value": "invalid"
            }]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response.json::<Value>().await.unwrap(),
        json!({
            "error": {
                "code": "attribute_not_applicable",
                "message": "attribute does not belong to the entity blueprint version"
            }
        })
    );

    let response = client
        .post(format!(
            "{base_url}/entities/{}/values",
            entity["id"].as_str().unwrap()
        ))
        .json(&json!({
            "values": [{
                "kind": "scalar",
                "attribute_id": first["attributes"][0]["id"],
                "attribute_code": "title",
                "value": "invalid"
            }]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response.json::<Value>().await.unwrap(),
        json!({
            "error": {
                "code": "invalid_input",
                "message": "provide exactly one of attribute_id or attribute_code"
            }
        })
    );

    server.abort();
}
