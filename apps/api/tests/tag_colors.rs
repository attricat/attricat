use crate::support;

use reqwest::StatusCode;
use serde_json::{Value, json};
use sqlx::PgPool;
use support::*;

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn admins_color_tags_and_every_reader_sees_the_colors(pool: PgPool) {
    let (base, server) = start_server(pool.clone()).await;
    let owner = authenticated_client();
    let path = format!("{base}/workspace/tag-colors");
    let get = |client: &reqwest::Client| client.get(&path).send();

    let colors: Value = get(&owner).await.unwrap().json().await.unwrap();
    assert_eq!(
        colors,
        json!({"defaults": {"attricat.needs_attention": "yellow"}, "overrides": {}})
    );

    let updated: Value = owner
        .patch(&path)
        .json(&json!({"set": {"reviewed": "green", "attricat.needs_attention": "red"}}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        updated["overrides"],
        json!({"attricat.needs_attention": "red", "reviewed": "green"})
    );
    // A second change keeps the first one's other tags.
    let updated: Value = owner
        .patch(&path)
        .json(&json!({"set": {"draft": "gray"}, "reset": ["attricat.needs_attention"]}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        updated["overrides"],
        json!({"draft": "gray", "reviewed": "green"})
    );

    for invalid in [
        json!({"set": {"reviewed": "plaid"}}),
        json!({"set": {"": "blue"}}),
        json!({"set": {"a": "blue"}, "reset": ["a"]}),
    ] {
        let status = owner
            .patch(&path)
            .json(&invalid)
            .send()
            .await
            .unwrap()
            .status();
        assert!(status.is_client_error(), "{invalid}: {status}");
    }

    // Viewers render the colors but cannot change them.
    let viewer = client_for(member_with_role(&pool, VIEWER_ROLE_ID).await);
    let seen: Value = get(&viewer).await.unwrap().json().await.unwrap();
    assert_eq!(seen, updated);
    let status = viewer
        .patch(&path)
        .json(&json!({"set": {"reviewed": "red"}}))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, StatusCode::FORBIDDEN);
    server.abort();
}
