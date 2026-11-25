//! Qualified search terms on number, integer and boolean leaves compare
//! values by value rather than by their text.

use crate::support;

use reqwest::StatusCode;
use serde_json::{Value, json};
use support::*;

const PART_A: &str = r#"
format_version: 1
code: ns_part_a
name: Part A
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [label]
attributes:
  - code: label
    value_type: string
  - code: code
    value_type: string
  - code: width_mm
    value_type: number
  - code: qty
    value_type: integer
  - code: in_stock
    value_type: boolean
  - code: size
    value_type: number
"#;

const PART_B: &str = r#"
format_version: 1
code: ns_part_b
name: Part B
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [label]
attributes:
  - code: label
    value_type: string
  - code: width_mm
    value_type: number
"#;

/// Declares no `width_mm`, and `size` as text where Part A has a number.
const PART_C: &str = r#"
format_version: 1
code: ns_part_c
name: Part C
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [label]
attributes:
  - code: label
    value_type: string
  - code: size
    value_type: string
"#;

const LINE: &str = r#"
format_version: 1
code: ns_line
name: Line
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [label]
attributes:
  - code: label
    value_type: string
  - code: spec
    value_type: relationship
    target_blueprints: [ns_part_a, ns_part_b, ns_part_c]
    cardinality: one
"#;

/// The status and body of a search for `query` on `blueprint`.
async fn search_response(
    client: &reqwest::Client,
    base_url: &str,
    blueprint: &str,
    query: &str,
) -> (StatusCode, Value) {
    status_json(
        client
            .post(format!("{base_url}/v1/records/search"))
            .json(&json!({ "blueprint": { "code": blueprint }, "query": query, "page": { "size": 25 } }))
            .send()
            .await
            .unwrap(),
    )
    .await
}

async fn search(client: &reqwest::Client, base_url: &str, blueprint: &str, query: &str) -> Value {
    let (status, body) = search_response(client, base_url, blueprint, query).await;
    assert_eq!(status, StatusCode::OK, "{query}: {body}");
    body
}

/// The display labels of the records a search found, sorted.
fn found(response: &Value) -> Vec<String> {
    let mut labels: Vec<String> = response["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["display"]["default"].as_str().unwrap().to_owned())
        .collect();
    labels.sort();
    labels
}

fn sorted(labels: &[&str]) -> Vec<String> {
    let mut labels: Vec<String> = labels.iter().map(|label| (*label).to_owned()).collect();
    labels.sort();
    labels
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn number_terms_compare_values_not_text(pool: sqlx::PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    for definition in [PART_A, PART_B, PART_C, LINE] {
        create_blueprint(&client, &base_url, definition).await;
    }
    let mut parts = Vec::new();
    for (label, values) in [
        (
            "four",
            vec![
                scalar("width_mm", 4),
                scalar("qty", 4),
                scalar("in_stock", true),
            ],
        ),
        (
            "forty",
            vec![
                scalar("width_mm", 40),
                scalar("qty", 40),
                scalar("in_stock", false),
                scalar("code", "W-40"),
            ],
        ),
        ("fourteen", vec![scalar("width_mm", 14)]),
        ("four and a half", vec![scalar("width_mm", 4.5)]),
        ("thirty", vec![scalar("width_mm", 30)]),
        ("sixty", vec![scalar("width_mm", 60)]),
    ] {
        let mut values = values;
        values.push(scalar("label", label));
        parts.push((
            label,
            create_record_with(&client, &base_url, "ns_part_a", json!(values)).await,
        ));
    }
    for (label, values) in [
        (
            "bee four",
            json!([scalar("label", "bee four"), scalar("width_mm", 4)]),
        ),
        (
            "bee forty",
            json!([scalar("label", "bee forty"), scalar("width_mm", 40)]),
        ),
    ] {
        parts.push((
            label,
            create_record_with(&client, &base_url, "ns_part_b", values).await,
        ));
    }
    parts.push((
        "sea",
        create_record_with(
            &client,
            &base_url,
            "ns_part_c",
            json!([scalar("label", "sea"), scalar("size", "4")]),
        )
        .await,
    ));
    for (label, part) in &parts {
        create_record_with(
            &client,
            &base_url,
            "ns_line",
            json!([
                scalar("label", format!("line {label}")),
                relationship("spec", part)
            ]),
        )
        .await;
    }

    // Selected-blueprint number, integer and boolean leaves, at the bounds.
    for (query, expected) in [
        ("width_mm:4", &["four"][..]),
        ("width_mm:=4", &["four"]),
        ("width_mm:4.00", &["four"]),
        ("width_mm:0", &[]),
        ("width_mm:4,5", &["four and a half"]),
        ("width_mm:4.5", &["four and a half"]),
        ("width_mm:>40", &["sixty"]),
        ("width_mm:>=40", &["forty", "sixty"]),
        ("width_mm:<14", &["four", "four and a half"]),
        ("width_mm:<=14", &["four", "four and a half", "fourteen"]),
        ("width_mm:14..40", &["fourteen", "thirty", "forty"]),
        ("width_mm:4..4", &["four"]),
        ("ns_part_a.width_mm:>59,9", &["sixty"]),
        ("qty:4", &["four"]),
        ("qty:>=40", &["forty"]),
        ("qty:5..39", &[]),
        ("in_stock:true", &["four"]),
        ("in_stock:FALSE", &["forty"]),
        // Text leaves keep substring and prefix matching.
        ("code:4", &["forty"]),
        ("code:W-4*", &["forty"]),
        // Bare terms keep matching any value's text, numbers included.
        ("4", &["four", "forty", "fourteen", "four and a half"]),
    ] {
        let response = search(&client, &base_url, "ns_part_a", query).await;
        assert_eq!(found(&response), sorted(expected), "{query}: {response}");
    }
    let response = search(&client, &base_url, "ns_part_a", "width_mm:4").await;
    let witness = &response["items"][0]["match_explanations"][0];
    assert_eq!(witness["matching_attribute_code"], "width_mm");
    assert_eq!(witness["traversal_depth"], 0);

    // Through a hop to several blueprints: the term compares numbers on
    // those that declare the leaf, and skips the one that does not.
    for (query, expected) in [
        ("spec.width_mm:4", &["line four", "line bee four"][..]),
        (
            "spec.width_mm:40..60",
            &["line forty", "line sixty", "line bee forty"],
        ),
        ("spec.width_mm:<4,5", &["line four", "line bee four"]),
        ("spec.qty:40", &["line forty"]),
        // `*:` names no leaf, so it matches any value's text as bare terms do.
        (
            "*:4",
            &[
                "line four",
                "line forty",
                "line fourteen",
                "line four and a half",
                "line bee four",
                "line bee forty",
                "line sea",
            ],
        ),
    ] {
        let response = search(&client, &base_url, "ns_line", query).await;
        assert_eq!(found(&response), sorted(expected), "{query}: {response}");
    }
    let response = search(&client, &base_url, "ns_line", "spec.width_mm:4").await;
    for item in response["items"].as_array().unwrap() {
        let witness = &item["match_explanations"][0];
        assert_eq!(witness["matching_attribute_code"], "width_mm", "{witness}");
        assert_eq!(witness["traversal_depth"], 1, "{witness}");
        assert_eq!(witness["relationship_path"][0]["attribute_code"], "spec");
    }

    for (blueprint, query, message) in [
        ("ns_part_a", "width_mm:abc", "'abc' is not a number"),
        ("ns_part_a", "width_mm:4*", "wildcards are not allowed"),
        ("ns_part_a", "width_mm:60..40", "starts after it ends"),
        ("ns_part_a", "qty:4,5", "is not a whole number"),
        ("ns_part_a", "in_stock:yes", "must be true or false"),
        ("ns_line", "spec.width_mm:4mm", "'4mm' is not a number"),
    ] {
        let (status, body) = search_response(&client, &base_url, blueprint, query).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{query}: {body}");
        assert_eq!(body["error"]["code"], "invalid_input", "{query}");
        assert!(
            body["error"]["message"].as_str().unwrap().contains(message),
            "{query}: {body}"
        );
    }
    // Targets that declare the leaf with different value types cannot share
    // one comparison.
    let (status, body) = search_response(&client, &base_url, "ns_line", "spec.size:4").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["error"]["code"], "relationship_path_type_conflict");
    server.abort();
}
