mod support;

use support::*;

#[sqlx::test]
async fn json_values_round_trip_through_current_and_history_reads(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "json_product"
name = "JSON product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["metadata"]

[[attributes]]
code = "metadata"
value_type = "json"
value_schema = '{"type":"object","required":["currency"]}'
"#,
    )
    .await;
    let entity: Value = client
        .post(format!("{base_url}/v1/entities"))
        .json(&json!({
            "blueprint": {
                "code": blueprint["blueprint"]["code"],
                "version": blueprint["blueprint"]["version"],
            },
            "values": [{
                "kind": "scalar",
                "attribute_code": "metadata",
                "value": {"currency": "USD", "amount": 12},
            }],
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let entity_id = entity["id"].as_str().unwrap();
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
    assert_eq!(values[0]["value"], json!({"currency": "USD", "amount": 12}));

    let replacement = client
        .post(format!("{base_url}/entities/{entity_id}/values"))
        .json(&json!({"values": [{
            "kind": "scalar", "attribute_code": "metadata",
            "value": {"currency": "EUR", "amount": 14}
        }]}))
        .send()
        .await
        .unwrap();
    assert!(replacement.status().is_success());
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
    assert_eq!(
        history[0]["value"],
        json!({"currency": "USD", "amount": 12})
    );
    server.abort();
}
