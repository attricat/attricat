mod support;

use support::*;

#[sqlx::test]
async fn schema_version_sort_paginates_all_revisions_in_both_directions(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let definition = r#"format_version = 1
code = "schema_sort_product"
name = "Schema sort product"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
[views.table]
type = "table"
fields = ["title"]
[[attributes]]
code = "title"
value_type = "string""#;
    let first = create_blueprint(&client, &base_url, definition).await;
    let old_a = create_entity(&client, &base_url, &first).await;
    let old_b = create_entity(&client, &base_url, &first).await;
    let id = first["blueprint"]["id"].as_str().unwrap();
    let revision: Value = client
        .post(format!("{base_url}/blueprints/{id}/versions"))
        .json(&json!({"definition": definition.replace("name = \"Schema sort product\"", "name = \"Schema sort products\"")}))
        .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
    let version = revision["blueprint"]["version"].as_i64().unwrap();
    let second: Value = client
        .post(format!(
            "{base_url}/blueprints/{id}/versions/{version}/publish"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let new_a = create_entity(&client, &base_url, &second).await;
    let new_b = create_entity(&client, &base_url, &second).await;

    for direction in ["asc", "desc"] {
        let mut cursor: Option<String> = None;
        let mut found = Vec::new();
        loop {
            let response: Value = client
                .post(format!("{base_url}/v1/entities/search"))
                .json(&json!({
                    "blueprint": {"code": "schema_sort_product"},
                    "sort": {"field": "blueprint_version", "direction": direction},
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
            found.push(response["items"][0]["id"].clone());
            cursor = response["next_cursor"].as_str().map(str::to_owned);
            if cursor.is_none() {
                break;
            }
        }
        // Entity ID is the deterministic tie breaker within each revision.
        let mut older = vec![old_a["id"].clone(), old_b["id"].clone()];
        older.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
        let mut newer = vec![new_a["id"].clone(), new_b["id"].clone()];
        newer.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
        let expected_order = if direction == "asc" {
            [older, newer].concat()
        } else {
            [newer, older].concat()
        };
        assert_eq!(found, expected_order);
        assert_eq!(found.len(), 4);
    }

    let single_version: Value = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({
            "blueprint": {"code": "schema_sort_product", "version": 1},
            "sort": {"field": "blueprint_version", "direction": "asc"},
            "page": {"size": 10}
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(single_version["items"].as_array().unwrap().len(), 2);
    assert!(
        single_version["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["blueprint_version"] == 1)
    );
    server.abort();
}
