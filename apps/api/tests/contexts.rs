mod support;

use support::*;

#[sqlx::test]
async fn omitted_parent_uses_the_workspace_default_context(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let contexts = client
        .get(format!("{base_url}/contexts"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Vec<Value>>()
        .await
        .unwrap();
    let default_id = contexts
        .iter()
        .find(|context| context["code"] == "default")
        .unwrap()["id"]
        .clone();

    let context = client
        .post(format!("{base_url}/contexts"))
        .json(&json!({ "code": "web", "data": {} }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();

    assert_eq!(context["parent_id"], default_id);
    server.abort();
}

#[sqlx::test]
async fn updating_an_unknown_context_returns_not_found(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();

    let response = client
        .put(format!("{base_url}/contexts/id/{}", Uuid::new_v4()))
        .json(&json!({
            "parent_id": "00000000-0000-4000-8000-000000000001",
            "data": {}
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "not_found"
    );

    server.abort();
}
