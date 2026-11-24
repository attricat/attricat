//! Request bodies and response checks shared by the HTTP integration tests.

use reqwest::{Client, IntoUrl, Response, StatusCode};
use serde_json::{Value, json};
use uuid::Uuid;

/// A scalar value write in the default context.
pub fn scalar(code: &str, value: impl Into<Value>) -> Value {
    json!({"kind": "scalar", "attribute_code": code, "value": value.into()})
}

/// A relationship value write pointing at `target`, a record response body.
pub fn relationship(code: &str, target: &Value) -> Value {
    json!({"kind": "relationship", "attribute_code": code, "target_record_id": target["id"]})
}

/// The response status and JSON body, or `Value::Null` when the body is not JSON.
pub async fn status_json(response: Response) -> (StatusCode, Value) {
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

/// Asserts the response status and returns its JSON body (`Value::Null` when
/// there is none); the body is shown when the status differs.
pub async fn expect_status(response: Response, expected: StatusCode) -> Value {
    let (status, body) = status_json(response).await;
    assert_eq!(status, expected, "{body}");
    body
}

/// Asserts the response status and stable error code and returns the body.
pub async fn expect_error(response: Response, expected: StatusCode, code: &str) -> Value {
    let body = expect_status(response, expected).await;
    assert_eq!(body["error"]["code"], code, "{body}");
    body
}

/// Asserts a success status and returns the JSON body; the body is shown
/// when the status is not a success.
pub async fn expect_success(response: Response) -> Value {
    let (status, body) = status_json(response).await;
    assert!(status.is_success(), "{status}: {body}");
    body
}

/// The ID of a record response body.
pub fn record_uuid(record: &Value) -> Uuid {
    Uuid::parse_str(record["id"].as_str().unwrap()).unwrap()
}

/// Saves `definition` as a draft, which must succeed, and asks to publish
/// it, returning the publish response.
pub async fn publish_response(client: &Client, base_url: &str, definition: &str) -> Response {
    let draft = expect_status(
        client
            .post(format!("{base_url}/blueprints"))
            .json(&json!({ "definition": definition }))
            .send()
            .await
            .unwrap(),
        StatusCode::CREATED,
    )
    .await;
    client
        .post(format!(
            "{base_url}/blueprints/{}/versions/{}/publish",
            draft["blueprint"]["id"].as_str().unwrap(),
            draft["blueprint"]["version"].as_i64().unwrap(),
        ))
        .send()
        .await
        .unwrap()
}

/// GETs `url`, requiring a success status, and returns the JSON body.
pub async fn get_json(client: &Client, url: impl IntoUrl) -> Value {
    client
        .get(url)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}

/// POSTs a new record of the latest published `blueprint` with `values`.
pub async fn post_record(
    client: &Client,
    base_url: &str,
    blueprint: &str,
    values: Value,
) -> Response {
    client
        .post(format!("{base_url}/v1/records"))
        .json(&json!({"blueprint": {"code": blueprint}, "values": values}))
        .send()
        .await
        .unwrap()
}

/// Creates a record of the latest published `blueprint`, requiring success.
pub async fn create_record_with(
    client: &Client,
    base_url: &str,
    blueprint: &str,
    values: Value,
) -> Value {
    let (status, body) = status_json(post_record(client, base_url, blueprint, values).await).await;
    assert!(status.is_success(), "{status}: {body}");
    body
}
