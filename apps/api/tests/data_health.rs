use crate::support;

use std::sync::Arc;

use api::storage::FakeObjectStore;
use support::*;

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn liveness_and_readiness_have_distinct_dependency_semantics(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base_url, server) = start_server_with_object_store(pool, store.clone()).await;
    let client = reqwest::Client::new();

    let live = client
        .get(format!("{base_url}/health/live"))
        .send()
        .await
        .unwrap();
    assert_eq!(live.status(), StatusCode::OK);
    assert_eq!(live.json::<Value>().await.unwrap()["status"], "live");
    assert_eq!(
        client
            .get(format!("{base_url}/health/ready"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );

    store.set_available(false);
    assert_eq!(
        client
            .get(format!("{base_url}/health/live"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let not_ready = client
        .get(format!("{base_url}/health/ready"))
        .send()
        .await
        .unwrap();
    assert_eq!(not_ready.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        not_ready.json::<Value>().await.unwrap()["status"],
        "not_ready"
    );
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn data_health_sections_return_an_empty_attricat(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();

    let summary_response = client
        .get(format!(
            "{base_url}/data-health/summary?stale_after_days=90"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let server_timing = summary_response
        .headers()
        .get("server-timing")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(server_timing.contains("app;dur="));
    assert!(server_timing.contains("cache;desc=BYPASS"));
    let summary: Value = summary_response.json().await.unwrap();
    assert_eq!(summary["active_records"], 0);
    assert_eq!(summary["outdated_records"], 0);

    for path in [
        "/data-health/blueprints?stale_after_days=90",
        "/data-health/freshness",
        "/data-health/completeness",
        "/data-health/contexts",
        "/data-health/relationships",
        "/data-health/storage",
    ] {
        let response = client
            .get(format!("{base_url}{path}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
    }

    let refresh = client
        .post(format!("{base_url}/data-health/refresh"))
        .send()
        .await
        .unwrap();
    assert_eq!(refresh.status(), StatusCode::NO_CONTENT);

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn data_health_cache_reports_miss_then_hit_in_server_timing(pool: PgPool) {
    let (base_url, server) = start_server_with_data_health_cache_ttl(pool, 300).await;
    let client = authenticated_client();

    for expected_cache_status in ["MISS", "HIT"] {
        let response = client
            .get(format!("{base_url}/data-health/freshness"))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
        let server_timing = response
            .headers()
            .get("server-timing")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(server_timing.contains("app;dur="));
        assert!(server_timing.contains(&format!("cache;desc={expected_cache_status}")));
    }

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn metrics_endpoint_exposes_low_cardinality_request_metrics(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();

    client
        .get(format!("{base_url}/health"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    // A session or token with `data_health.read` is not enough.
    assert_eq!(
        client
            .get(format!("{base_url}/metrics"))
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    let metrics = reqwest::Client::new()
        .get(format!("{base_url}/metrics"))
        .bearer_auth(METRICS_TOKEN)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .text()
        .await
        .unwrap();

    assert!(metrics.contains("attricat_http_requests_total"));
    assert!(metrics.contains("route=\"/health\""));
    assert!(!metrics.contains(&base_url));

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn data_health_completeness_counts_default_values(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(
        &client,
        &base_url,
        "format_version: 1\ncode: health_product\nname: Health product\nkind: record\nviews:\n  dropdown_option:\n    type: dropdown_option\n    fields: [title]\nattributes:\n  - code: title\n    value_type: string",
    )
    .await;
    create_record(&client, &base_url, &blueprint).await;

    let completeness: Value = client
        .get(format!("{base_url}/data-health/completeness"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(completeness[0]["active_records"], 1);
    assert_eq!(completeness[0]["current_version"], 1);
    assert_eq!(completeness[0]["outdated_records"], 0);
    assert_eq!(completeness[0]["default_complete_records"], 1);

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn readiness_is_withdrawn_while_a_migration_this_build_needs_is_missing(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let ready = || reqwest::get(format!("{base_url}/health/ready"));
    assert_eq!(ready().await.unwrap().status(), reqwest::StatusCode::OK);
    sqlx::query(
        "DELETE FROM _sqlx_migrations WHERE version = (SELECT max(version) FROM _sqlx_migrations)",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        ready().await.unwrap().status(),
        reqwest::StatusCode::SERVICE_UNAVAILABLE
    );
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn readiness_is_withdrawn_when_the_schema_lacks_what_this_build_reads(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    sqlx::query("ALTER TABLE browser_sessions RENAME COLUMN csrf_digest TO csrf_digest_renamed")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        reqwest::get(format!("{base_url}/health/ready"))
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::SERVICE_UNAVAILABLE
    );
    server.abort();
}
