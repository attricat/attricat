//! Request bodies and response checks shared by the HTTP integration tests.

use reqwest::{Client, IntoUrl, Response, StatusCode};
use serde_json::{Value, json};

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
