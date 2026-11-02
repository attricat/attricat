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

const LISTED_RELEASE_ID: u64 = 2;
const UNLISTED_RELEASE_ID: u64 = 1;
const REGISTRY_INDEX: &str = r#"{
  "registry_version": 2,
  "extensions": [{
    "id": "acme.widget",
    "name": "Widget",
    "description": "A widget",
    "repository": "acme/widget",
    "versions": [
      {"version": "1.1.0", "host_api": "^1"},
      {"version": "1.2.0", "host_api": ">=99.0.0"}
    ]
  }]
}"#;

/// Serves the official index and the releases of `acme/widget`, of which the
/// index lists only `v1.1.0`.
async fn start_fake_github() -> (String, tokio::task::JoinHandle<()>) {
    let asset = |tag: &str| {
        serde_json::json!({
            "id": 1,
            "name": "widget.tar.zst",
            "browser_download_url":
                format!("https://github.com/acme/widget/releases/download/{tag}/widget.tar.zst"),
            "digest": null
        })
    };
    let release = |id: u64, tag: &str| {
        serde_json::json!({
            "id": id, "tag_name": tag, "name": tag, "draft": false, "prerelease": false,
            "published_at": null, "assets": [asset(tag)]
        })
    };
    let releases = serde_json::json!([
        release(UNLISTED_RELEASE_ID, "v1.0.0"),
        release(LISTED_RELEASE_ID, "v1.1.0"),
        release(3, "v9.0.0"),
    ]);
    let app = axum::Router::new()
        .route(
            "/attricat/attricat-extensions/HEAD/registry.json",
            axum::routing::get(|| async { REGISTRY_INDEX }),
        )
        .route(
            "/repos/acme/widget/readme",
            axum::routing::get(|| async { "# Widget" }),
        )
        .route(
            "/repos/acme/widget/releases",
            axum::routing::get(move || {
                let releases = releases.clone();
                async move { axum::Json(releases) }
            }),
        );
    let listener = support::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}/", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (origin, server)
}

async fn start_server_with_fake_github(pool: PgPool) -> (String, Vec<tokio::task::JoinHandle<()>>) {
    let (origin, github) = start_fake_github().await;
    let origin = reqwest::Url::parse(&origin).unwrap();
    let (base, server) = support::start_server_with_config(pool, move |state| {
        state.registry = std::sync::Arc::new(
            api::extension_registry::GitHubRegistry::with_origins(origin.clone(), origin).unwrap(),
        );
    })
    .await;
    (base, vec![github, server])
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn only_releases_the_registry_lists_are_offered_or_installable(pool: PgPool) {
    let (base, servers) = start_server_with_fake_github(pool).await;
    let client = authenticated_client();

    let details: serde_json::Value = client
        .get(format!(
            "{base}/extension-registries/extensions/acme/widget"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let tags = details["releases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|release| release["tag_name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(tags, ["v1.1.0"]);

    let response = client
        .post(format!("{base}/extensions"))
        .json(&serde_json::json!({
            "owner": "acme",
            "repository": "widget",
            "release_id": UNLISTED_RELEASE_ID,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    for server in servers {
        server.abort();
    }
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn updates_report_the_newest_compatible_listed_version(pool: PgPool) {
    let (base, servers) = start_server_with_fake_github(pool.clone()).await;
    let client = authenticated_client();
    let updates = || async {
        client
            .get(format!("{base}/extension-registries/updates"))
            .send()
            .await
            .unwrap()
            .json::<serde_json::Value>()
            .await
            .unwrap()
    };
    assert_eq!(updates().await, serde_json::json!([]));

    let release_id = uuid::Uuid::new_v4();
    let workspace_id: uuid::Uuid = support::BOOTSTRAP_WORKSPACE_ID.parse().unwrap();
    for (version, source, extension_id) in [
        ("1.0.0", "github:acme/widget@v1.0.0", "acme.widget"),
        ("1.0.0", "sideload", "acme.sideloaded"),
    ] {
        let release_id = if extension_id == "acme.widget" {
            release_id
        } else {
            uuid::Uuid::new_v4()
        };
        sqlx::query("INSERT INTO installed_extension_releases (id, workspace_id, extension_id, version, manifest, manifest_sha256, source) VALUES ($1, $2, $3, $4, '{}'::jsonb, repeat('0', 64), $5)")
            .bind(release_id)
            .bind(workspace_id)
            .bind(extension_id)
            .bind(version)
            .bind(source)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO extension_installations (id, workspace_id, extension_id, installed_release_id, state, configuration) VALUES ($1, $2, $3, $4, 'disabled', '{}'::jsonb)")
            .bind(uuid::Uuid::new_v4())
            .bind(workspace_id)
            .bind(extension_id)
            .bind(release_id)
            .execute(&pool)
            .await
            .unwrap();
    }

    assert_eq!(
        updates().await,
        serde_json::json!([{
            "extension_id": "acme.widget",
            "installed_version": "1.0.0",
            "latest_version": "1.1.0",
            "repository": "github:acme/widget",
            "registry_source": "github:attricat/attricat-extensions",
        }])
    );
    for server in servers {
        server.abort();
    }
}
