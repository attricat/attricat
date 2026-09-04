mod support;

use support::{PgPool, StatusCode, authenticated_client, start_server};

#[sqlx::test(migrations = "./migrations")]
async fn registry_sources_include_official_source_and_manage_custom_sources(pool: PgPool) {
    let (base, server) = start_server(pool).await;
    let client = authenticated_client();

    let sources: serde_json::Value = client
        .get(format!("{base}/extension-registries"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(sources.as_array().unwrap().len(), 1);
    assert_eq!(sources[0]["source"], "github:attricat/catalog-extensions");
    assert_eq!(sources[0]["official"], true);

    let response = client
        .post(format!("{base}/extension-registries"))
        .json(&serde_json::json!({"source": "https://github.com/Acme/Example"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let custom: serde_json::Value = response.json().await.unwrap();
    assert_eq!(custom["source"], "github:acme/example");
    let id = custom["id"].as_str().unwrap();

    let response = client
        .post(format!("{base}/extension-registries"))
        .json(&serde_json::json!({"source": "https://evil.test/Acme/Example"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let response = client
        .delete(format!("{base}/extension-registries/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    server.abort();
}
