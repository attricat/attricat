use crate::support;

use support::{PgPool, StatusCode, authenticated_client, start_server};

#[sqlx::test(migrator = "api::MIGRATOR")]
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
    assert_eq!(sources[0]["source"], "github:attricat/attricat-extensions");
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

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_registry_source_must_exist_and_lists_nothing_twice(pool: PgPool) {
    let repository = api::repository::AttricatRepository::system(pool.clone())
        .for_workspace(uuid::Uuid::from_u128(0x00000000000040008000000000000002));
    let (base, server) = start_server(pool).await;
    let client = authenticated_client();
    let add = async |source: &str| {
        client
            .post(format!("{base}/extension-registries"))
            .json(&serde_json::json!({ "source": source }))
            .send()
            .await
            .unwrap()
            .status()
    };
    assert_eq!(add("acme/missing").await, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(add("acme/invalid").await, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(add("acme/limited").await, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        add("attricat/attricat-extensions").await,
        StatusCode::CONFLICT
    );
    assert_eq!(add("acme/example").await, StatusCode::CREATED);

    // Both the official registry and acme/example list it.
    let discovered: serde_json::Value = client
        .get(format!("{base}/extension-registries/discover"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let discovered = discovered.as_array().unwrap();
    assert_eq!(discovered.len(), 1);
    assert_eq!(
        discovered[0]["registry_source"],
        "github:attricat/attricat-extensions"
    );

    // A source deleted after it was added isn't an outage.
    repository
        .add_extension_registry_source(&"acme/missing".parse().unwrap())
        .await
        .unwrap();
    let details = async |path: &str| {
        client
            .get(format!("{base}/extension-registries/extensions/{path}"))
            .send()
            .await
            .unwrap()
            .status()
    };
    assert_eq!(details("acme/unlisted").await, StatusCode::NOT_FOUND);
    assert_eq!(details("acme/listed").await, StatusCode::OK);
    server.abort();
}
