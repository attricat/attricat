mod support;

use support::*;

#[sqlx::test]
async fn updating_an_unknown_context_returns_not_found(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = Client::new();

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
