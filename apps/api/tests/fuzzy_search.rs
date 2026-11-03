use crate::support;

use serde_json::{Value, json};
use support::*;

const PART: &str = r#"
format_version: 1
code: fuzzy_part
name: Part
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [name]
attributes:
  - code: name
    value_type: string
    tags: ['search:fuzzy']
  - code: note
    value_type: string
"#;

async fn search(client: &reqwest::Client, base_url: &str, query: &str) -> Value {
    client
        .post(format!("{base_url}/v1/records/search"))
        .json(&json!({ "blueprint": { "code": "fuzzy_part" }, "query": query, "page": { "size": 25 } }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}

/// The display labels of the records a search found, sorted.
fn found(response: &Value) -> Vec<String> {
    let mut names: Vec<String> = response["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["display"]["default"].as_str().unwrap().to_owned())
        .collect();
    names.sort();
    names
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn tagged_text_attributes_also_match_similar_values(pool: sqlx::PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(&client, &base_url, PART).await;
    for (name, note) in [
        ("Tarcza tnąca 125 mm", "do stali"),
        ("Tarcze listkowe", "zestaw"),
        ("Tarka", "kuchnia"),
        ("Wiertło", "tarcze w zestawie"),
    ] {
        create_record_with(
            &client,
            &base_url,
            "fuzzy_part",
            json!([scalar("name", name), scalar("note", note)]),
        )
        .await;
    }

    // Another inflection of the word finds the tagged name; an untagged
    // attribute matches only what contains the term.
    let response = search(&client, &base_url, "tarcze").await;
    assert_eq!(
        found(&response),
        ["Tarcza tnąca 125 mm", "Tarcze listkowe", "Wiertło"],
        "{response}"
    );

    // A letter left out finds it too; an unrelated word does not.
    assert!(
        found(&search(&client, &base_url, "tarca").await)
            .contains(&"Tarcza tnąca 125 mm".to_owned())
    );
    assert!(
        found(&search(&client, &base_url, "tarcza").await)
            .contains(&"Tarcza tnąca 125 mm".to_owned())
    );
    assert!(!found(&search(&client, &base_url, "tarcze").await).contains(&"Tarka".to_owned()));

    // Prefix terms and short terms stay exact: "Tarcza" does not start with it.
    assert_eq!(
        found(&search(&client, &base_url, "tarcze*").await),
        ["Tarcze listkowe", "Wiertło"]
    );
    assert!(found(&search(&client, &base_url, "trk").await).is_empty());
    // An untagged attribute never matches by similarity.
    assert!(found(&search(&client, &base_url, "kuhnia").await).is_empty());

    server.abort();
}
