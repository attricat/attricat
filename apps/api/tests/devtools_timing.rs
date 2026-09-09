mod support;

use support::*;

#[sqlx::test]
async fn devtools_search_timings_are_aggregate_and_sanitized(pool: PgPool) {
    let (base_url, server) = start_server_with_devtools(pool).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "timing_product"
name = "Timing product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[views.table]
type = "table"
columns = [{ field = "title", label = "Title" }]

[[attributes]]
code = "title"
value_type = "string"
"#,
    )
    .await;
    let entity = create_entity(&client, &base_url, &blueprint).await;
    client
        .post(format!(
            "{base_url}/entities/{}/values",
            entity["id"].as_str().unwrap()
        ))
        .json(&json!({ "values": [{ "kind": "scalar", "attribute_code": "title", "value": "private query value" }] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let response = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({
            "blueprint": { "code": "timing_product" },
            "sort": { "field": "title", "direction": "asc" },
            "page": { "size": 10 }
        }))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    let timing = response
        .headers()
        .get("server-timing")
        .unwrap()
        .to_str()
        .unwrap();
    for phase in ["candidate", "page", "related", "serialize"] {
        assert!(timing.contains(&format!("{phase};dur=")), "{timing}");
    }
    assert!(!timing.contains("private query value"));
    assert!(!timing.contains("SELECT"));
    assert!(!timing.contains(entity["id"].as_str().unwrap()));

    server.abort();
}
