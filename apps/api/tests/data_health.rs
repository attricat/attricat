mod support;

use support::*;

#[sqlx::test]
async fn data_health_sections_return_an_empty_catalog(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = Client::new();

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
    assert_eq!(summary["active_entities"], 0);
    assert_eq!(summary["outdated_entities"], 0);

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

#[sqlx::test]
async fn data_health_cache_reports_miss_then_hit_in_server_timing(pool: PgPool) {
    let (base_url, server) = start_server_with_data_health_cache_ttl(pool, 300).await;
    let client = Client::new();

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

#[sqlx::test]
async fn metrics_endpoint_exposes_low_cardinality_request_metrics(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = Client::new();

    client
        .get(format!("{base_url}/health"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let metrics = client
        .get(format!("{base_url}/metrics"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .text()
        .await
        .unwrap();

    assert!(metrics.contains("catalog_http_requests_total"));
    assert!(metrics.contains("route=\"/health\""));
    assert!(!metrics.contains(&base_url));

    server.abort();
}

#[sqlx::test]
async fn data_health_completeness_counts_default_values(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = Client::new();
    let blueprint = create_blueprint(
        &client,
        &base_url,
        "format_version = 1\ncode = \"health_product\"\nname = \"Health product\"\nkind = \"entity\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"title\"]\n\n[[attributes]]\ncode = \"title\"\nvalue_type = \"string\"",
    )
    .await;
    create_entity(&client, &base_url, &blueprint).await;

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
    assert_eq!(completeness[0]["active_entities"], 1);
    assert_eq!(completeness[0]["current_version"], 1);
    assert_eq!(completeness[0]["outdated_entities"], 0);
    assert_eq!(completeness[0]["default_complete_entities"], 1);

    server.abort();
}
