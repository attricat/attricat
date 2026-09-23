mod support;

use std::{
    io::Cursor,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use sha2::{Digest, Sha256};

use api::storage::{
    FakeObjectStore, ObjectStore, ObjectStoreError, StoredObject, StoredObjectStream,
};
use async_trait::async_trait;
use reqwest::multipart;
use support::*;
use tokio::sync::Mutex;

const SAFE_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1"><rect width="2" height="1" fill="#fff"/></svg>"##;
const NORMALIZED_SVG: &[u8] = br##"<svg height="1" width="2" xmlns="http://www.w3.org/2000/svg"><rect fill="#fff" height="1" width="2"/></svg>"##;

struct FlakyObjectStore {
    inner: FakeObjectStore,
    fail_next_range: AtomicBool,
    corrupt_next_put: AtomicBool,
    ranges: Mutex<Vec<Option<String>>>,
    ignore_ranges: AtomicBool,
    stream_chunks_yielded: Arc<AtomicUsize>,
}

impl FlakyObjectStore {
    fn new() -> Self {
        Self {
            inner: FakeObjectStore::available(),
            fail_next_range: AtomicBool::new(false),
            corrupt_next_put: AtomicBool::new(false),
            ranges: Mutex::new(Vec::new()),
            ignore_ranges: AtomicBool::new(false),
            stream_chunks_yielded: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[async_trait]
impl ObjectStore for FlakyObjectStore {
    async fn put(&self, key: &str, mut object: StoredObject) -> Result<(), ObjectStoreError> {
        if self.corrupt_next_put.swap(false, Ordering::SeqCst) {
            object.bytes = bytes::Bytes::from_static(b"corrupt");
        }
        self.inner.put(key, object).await
    }

    async fn put_file(
        &self,
        key: &str,
        path: &Path,
        content_type: Option<&str>,
    ) -> Result<(), ObjectStoreError> {
        self.inner.put_file(key, path, content_type).await
    }

    async fn get(&self, key: &str) -> Result<StoredObject, ObjectStoreError> {
        self.inner.get(key).await
    }

    async fn get_stream(&self, key: &str) -> Result<StoredObjectStream, ObjectStoreError> {
        self.inner.get_stream(key).await
    }

    async fn get_range(
        &self,
        key: &str,
        range: Option<&str>,
    ) -> Result<StoredObject, ObjectStoreError> {
        self.inner.get_range(key, range).await
    }

    async fn get_range_stream(
        &self,
        key: &str,
        range: Option<&str>,
    ) -> Result<StoredObjectStream, ObjectStoreError> {
        self.ranges.lock().await.push(range.map(str::to_owned));
        if self.fail_next_range.swap(false, Ordering::SeqCst) {
            return Err(ObjectStoreError::Operation("download"));
        }
        if self.ignore_ranges.load(Ordering::SeqCst) {
            let yielded = self.stream_chunks_yielded.clone();
            let stream = async_stream::stream! {
                for _ in 0..(4 * 1024 * 1024) {
                    yielded.fetch_add(1, Ordering::SeqCst);
                    yield Ok(bytes::Bytes::from_static(b"x"));
                }
            };
            return Ok(StoredObjectStream {
                stream: Box::pin(stream),
                content_type: Some("image/svg+xml".to_owned()),
            });
        }
        self.inner.get_range_stream(key, range).await
    }

    async fn delete(&self, key: &str) -> Result<(), ObjectStoreError> {
        self.inner.delete(key).await
    }

    async fn readiness(&self) -> Result<(), ObjectStoreError> {
        self.inner.readiness().await
    }
}

fn asset_archive(version: &str, svg: &[u8]) -> Vec<u8> {
    asset_archive_with(
        version,
        &[("assets/brand-logo", "assets/brand-logo.svg", "logo", svg)],
    )
}

fn asset_archive_with(version: &str, assets: &[(&str, &str, &str, &[u8])]) -> Vec<u8> {
    asset_archive_with_requirement(version, true, assets)
}

fn asset_archive_with_requirement(
    version: &str,
    required: bool,
    assets: &[(&str, &str, &str, &[u8])],
) -> Vec<u8> {
    let declarations = assets
        .iter()
        .map(|(key, path, purpose, bytes)| {
            json!({
                "key": key,
                "path": path,
                "required": required,
                "purpose": purpose,
                "media_type": "image/svg+xml",
                "sha256": format!("{:x}", Sha256::digest(bytes)),
            })
        })
        .collect::<Vec<_>>();
    let manifest = serde_json::to_vec(&json!({
        "manifest_version":1,
        "id":"attricat.brand",
        "name":"Brand",
        "version":version,
        "description":"Brand assets",
        "catalog":{"host_api":"^1.0"},
        "resources":{"presentation_assets":declarations}
    }))
    .unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        let mut files = vec![("solution-pack.json", manifest.as_slice())];
        files.extend(assets.iter().map(|(_, path, _, bytes)| (*path, *bytes)));
        for (path, bytes) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, path, bytes).unwrap();
        }
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn bearer_client(secret: &str) -> Client {
    Client::builder()
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                "authorization",
                reqwest::header::HeaderValue::from_str(&format!("Bearer {secret}")).unwrap(),
            );
            headers
        })
        .build()
        .unwrap()
}

async fn create_pat(client: &Client, base_url: &str, permissions: &[&str]) -> String {
    client
        .post(format!("{base_url}/personal-access-tokens"))
        .json(&json!({"label": Uuid::new_v4().to_string(), "permissions": permissions}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Value>()
        .await
        .unwrap()["secret"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn apply_asset_pack(client: &Client, base_url: &str, version: &str, svg: &[u8]) -> Value {
    let plan: Value = client
        .post(format!(
            "{base_url}/solution-packs/plans?prefix=brand&blueprint_publication=draft"
        ))
        .header("content-type", "application/zstd")
        .body(asset_archive(version, svg))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    client
        .post(format!(
            "{base_url}/solution-packs/plans/{}/apply",
            plan["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let listed: Value = client
        .get(format!("{base_url}/presentation-assets"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    listed[0].clone()
}

#[sqlx::test(migrations = "./migrations")]
async fn private_asset_list_metadata_and_content_are_bounded(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base_url, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    assert_eq!(
        reqwest::Client::new()
            .get(format!("{base_url}/presentation-assets"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let client = authenticated_client();
    assert_eq!(
        client
            .post(format!("{base_url}/presentation-assets"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let asset = apply_asset_pack(&client, &base_url, "1.0.0", SAFE_SVG).await;
    let denied_pat = create_pat(&client, &base_url, &["contexts.read"]).await;
    let allowed_pat = create_pat(&client, &base_url, &["solution_packs.manage"]).await;
    assert_eq!(
        bearer_client(&denied_pat)
            .get(format!("{base_url}/presentation-assets"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        bearer_client(&allowed_pat)
            .get(format!("{base_url}/presentation-assets"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(asset["purpose"], "logo");
    assert_eq!(asset["media_type"], "image/svg+xml");
    assert!(asset.get("object_key").is_none());
    assert_eq!(store.object_count().await, 1);

    let id = asset["id"].as_str().unwrap();
    for suffix in ["", "/content"] {
        assert_eq!(
            reqwest::Client::new()
                .get(format!("{base_url}/presentation-assets/{id}{suffix}"))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    let listed_response = client
        .get(format!("{base_url}/presentation-assets?limit=1&offset=0"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert_eq!(
        listed_response.headers()["cache-control"],
        "private, no-store"
    );
    let listed: Value = listed_response.json().await.unwrap();
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert!(listed[0].get("object_key").is_none());
    for query in ["limit=0", "limit=101", "offset=-1", "offset=10001"] {
        assert_eq!(
            client
                .get(format!("{base_url}/presentation-assets?{query}"))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    let metadata = client
        .get(format!("{base_url}/presentation-assets/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(metadata.status(), StatusCode::OK);
    assert_eq!(metadata.headers()["cache-control"], "private, no-store");

    let content = client
        .get(format!("{base_url}/presentation-assets/{id}/content"))
        .send()
        .await
        .unwrap();
    assert_eq!(content.status(), StatusCode::OK);
    assert_eq!(content.headers()["content-type"], "image/svg+xml");
    assert_eq!(content.headers()["cache-control"], "private, no-store");
    assert_eq!(content.headers()["x-content-type-options"], "nosniff");
    assert_eq!(
        content.headers()["content-security-policy"],
        "default-src 'none'"
    );
    assert_eq!(content.bytes().await.unwrap().as_ref(), NORMALIZED_SVG);

    let permissionless_user = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id,email) VALUES ($1,'permissionless-assets@example.test')")
        .bind(permissionless_user)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO workspace_memberships (id,workspace_id,user_id) VALUES ($1,'00000000-0000-4000-8000-000000000002',$2)")
        .bind(Uuid::new_v4()).bind(permissionless_user).execute(&pool).await.unwrap();
    for path in [
        "/presentation-assets".to_owned(),
        format!("/presentation-assets/{id}"),
        format!("/presentation-assets/{id}/content"),
    ] {
        assert_eq!(
            reqwest::Client::new()
                .get(format!("{base_url}{path}"))
                .header("x-catalog-user-id", permissionless_user.to_string())
                .header(
                    "x-catalog-workspace-id",
                    "00000000-0000-4000-8000-000000000002"
                )
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
    }

    let other_workspace = Uuid::new_v4();
    let other_user = Uuid::new_v4();
    let membership = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO workspaces (id,slug,name,login_identifier) VALUES ($1,'other-assets','Other workspace','other-assets.local')",
    )
    .bind(other_workspace)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO users (id,email) VALUES ($1,'other-assets@example.test')")
        .bind(other_user)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO workspace_memberships (id,workspace_id,user_id) VALUES ($1,$2,$3)")
        .bind(membership)
        .bind(other_workspace)
        .bind(other_user)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO role_grants (id,workspace_id,membership_id,role_id,scope_type,scope_target_id) VALUES ($1,$2,$3,'00000000-0000-4000-8000-000000000101','workspace',$2)")
        .bind(Uuid::new_v4()).bind(other_workspace).bind(membership).execute(&pool).await.unwrap();
    let other_client = reqwest::Client::new();
    for suffix in ["", "/content"] {
        assert_eq!(
            other_client
                .get(format!("{base_url}/presentation-assets/{id}{suffix}"))
                .header("x-catalog-user-id", other_user.to_string())
                .header("x-catalog-workspace-id", other_workspace.to_string())
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::NOT_FOUND
        );
    }
    let foreign_map = json!({"key":"assets/brand-logo","id":id}).to_string();
    assert_eq!(
        other_client
            .post(format!(
                "{base_url}/solution-packs/plans?prefix=brand&blueprint_publication=draft"
            ))
            .header("x-catalog-user-id", other_user.to_string())
            .header("x-catalog-workspace-id", other_workspace.to_string())
            .multipart(
                multipart::Form::new()
                    .part(
                        "archive",
                        multipart::Part::bytes(asset_archive("2.0.0", SAFE_SVG))
                            .file_name("brand.tar.zst")
                            .mime_str("application/zstd")
                            .unwrap()
                    )
                    .text("asset_map", foreign_map)
            )
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn solution_pack_explicit_exact_asset_map_writes_no_object(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base_url, server) = start_server_with_object_store(pool, store.clone()).await;
    let client = authenticated_client();
    let created = apply_asset_pack(&client, &base_url, "1.0.0", SAFE_SVG).await;
    let archive = asset_archive("1.0.0", SAFE_SVG);
    let mapping = json!({"key":"assets/brand-logo","id":created["id"]}).to_string();
    let plan: Value = client
        .post(format!(
            "{base_url}/solution-packs/plans?prefix=brand&blueprint_publication=draft"
        ))
        .multipart(
            multipart::Form::new()
                .part(
                    "archive",
                    multipart::Part::bytes(archive)
                        .file_name("brand.tar.zst")
                        .mime_str("application/zstd")
                        .unwrap(),
                )
                .text("asset_map", mapping),
        )
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(plan["actions"][0]["action"], "map");
    assert_eq!(store.object_count().await, 1);
    let plan_id = plan["id"].as_str().unwrap();
    let application: Value = client
        .post(format!("{base_url}/solution-packs/plans/{plan_id}/apply"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        application["steps"][0]["result_snapshot"]["outcome"],
        "reused"
    );
    assert_eq!(store.object_count().await, 1);
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn planner_never_adopts_by_digest_and_incompatible_explicit_map_conflicts(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base_url, server) = start_server_with_object_store(pool, store.clone()).await;
    let client = authenticated_client();
    let created = apply_asset_pack(&client, &base_url, "0.9.0", SAFE_SVG).await;
    let automatic: Value = client
        .post(format!(
            "{base_url}/solution-packs/plans?prefix=brand&blueprint_publication=draft"
        ))
        .header("content-type", "application/zstd")
        .body(asset_archive("1.0.0", SAFE_SVG))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(automatic["actions"][0]["action"], "create");
    assert_eq!(store.object_count().await, 2);

    let changed_svg =
        br##"<svg xmlns="http://www.w3.org/2000/svg"><circle cx="1" cy="1" r="1"/></svg>"##;
    let mapping = json!({"key":"assets/brand-logo","id":created["id"]}).to_string();
    let incompatible: Value = client
        .post(format!(
            "{base_url}/solution-packs/plans?prefix=other&blueprint_publication=draft"
        ))
        .multipart(
            multipart::Form::new()
                .part(
                    "archive",
                    multipart::Part::bytes(asset_archive("2.0.0", changed_svg))
                        .file_name("brand.tar.zst")
                        .mime_str("application/zstd")
                        .unwrap(),
                )
                .text("asset_map", mapping),
        )
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(incompatible["ready"], false);
    assert_eq!(incompatible["actions"][0]["action"], "conflict");
    assert_eq!(
        incompatible["actions"][0]["reason_code"],
        "existing_asset_incompatible"
    );
    assert_eq!(store.object_count().await, 2);
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn later_release_reuses_unchanged_asset_and_blocks_changed_asset(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base_url, server) = start_server_with_object_store(pool, store.clone()).await;
    let client = authenticated_client();
    let plan: Value = client
        .post(format!(
            "{base_url}/solution-packs/plans?prefix=brand&blueprint_publication=draft"
        ))
        .header("content-type", "application/zstd")
        .body(asset_archive("1.0.0", SAFE_SVG))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let application: Value = client
        .post(format!(
            "{base_url}/solution-packs/plans/{}/apply",
            plan["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let application_id = application["id"].as_str().unwrap();

    let unchanged_response = client
        .post(format!("{base_url}/solution-packs/plans?prefix=brand&blueprint_publication=draft&from_application={application_id}"))
        .header("content-type", "application/zstd")
        .body(asset_archive("1.1.0", SAFE_SVG))
        .send().await.unwrap();
    let unchanged_status = unchanged_response.status();
    let unchanged_body = unchanged_response.text().await.unwrap();
    assert!(
        unchanged_status.is_success(),
        "{unchanged_status}: {unchanged_body}"
    );
    let unchanged: Value = serde_json::from_str(&unchanged_body).unwrap();
    assert_eq!(unchanged["ready"], true);
    assert_eq!(unchanged["actions"][0]["action"], "map");
    assert_eq!(
        unchanged["actions"][0]["reason_code"],
        "unchanged_from_prior_application"
    );
    assert_eq!(unchanged["release_changes"][0]["change_kind"], "unchanged");
    assert_eq!(store.object_count().await, 1);

    let changed_svg =
        br##"<svg xmlns="http://www.w3.org/2000/svg"><circle cx="1" cy="1" r="1"/></svg>"##;
    let changed: Value = client
        .post(format!("{base_url}/solution-packs/plans?prefix=brand&blueprint_publication=draft&from_application={application_id}"))
        .header("content-type", "application/zstd")
        .body(asset_archive("1.2.0", changed_svg))
        .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
    assert_eq!(changed["ready"], false);
    assert_eq!(changed["actions"][0]["action"], "conflict");
    assert_eq!(changed["actions"][0]["reason_code"], "update_not_supported");
    assert_eq!(changed["release_changes"][0]["change_kind"], "changed");
    assert_eq!(store.object_count().await, 1);
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn later_release_records_added_and_removed_without_deleting_prior_asset(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base_url, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let client = authenticated_client();
    let first_plan: Value = client
        .post(format!(
            "{base_url}/solution-packs/plans?prefix=brand&blueprint_publication=draft"
        ))
        .header("content-type", "application/zstd")
        .body(asset_archive("1.0.0", SAFE_SVG))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let first_application: Value = client
        .post(format!(
            "{base_url}/solution-packs/plans/{}/apply",
            first_plan["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let application_id = first_application["id"].as_str().unwrap();
    let replacement = br##"<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0L1 1"/></svg>"##;
    let next_archive = asset_archive_with(
        "1.1.0",
        &[(
            "assets/new-icon",
            "assets/new-icon.svg",
            "icon",
            replacement,
        )],
    );
    let next: Value = client
        .post(format!("{base_url}/solution-packs/plans?prefix=brand&blueprint_publication=draft&from_application={application_id}"))
        .header("content-type", "application/zstd")
        .body(next_archive)
        .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
    assert_eq!(next["ready"], true);
    assert_eq!(next["actions"][0]["logical_key"], "assets/new-icon");
    assert_eq!(next["actions"][0]["action"], "create");
    let changes = next["release_changes"].as_array().unwrap();
    assert!(changes.iter().any(
        |change| change["logical_key"] == "assets/new-icon" && change["change_kind"] == "added"
    ));
    assert!(
        changes
            .iter()
            .any(|change| change["logical_key"] == "assets/brand-logo"
                && change["change_kind"] == "removed")
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM presentation_assets")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(store.object_count().await, 2);
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn plan_rejects_a_successfully_acknowledged_corrupt_asset_write(pool: PgPool) {
    let store = Arc::new(FlakyObjectStore::new());
    let (base_url, server) =
        start_server_with_custom_object_store(pool.clone(), store.clone()).await;
    store.corrupt_next_put.store(true, Ordering::SeqCst);
    let response = authenticated_client()
        .post(format!(
            "{base_url}/solution-packs/plans?prefix=brand&blueprint_publication=draft"
        ))
        .header("content-type", "application/zstd")
        .body(asset_archive("1.0.0", SAFE_SVG))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let (ready, state): (bool, String) = sqlx::query_as(
        "SELECT p.ready,a.state FROM solution_pack_plans p JOIN solution_pack_plan_asset_objects a ON a.plan_id=p.id"
    ).fetch_one(&pool).await.unwrap();
    assert!(!ready);
    assert_eq!(state, "cleanup_pending");
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn apply_storage_outage_is_retryable_and_missing_object_fails_closed(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base_url, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let client = authenticated_client();
    let plan: Value = client
        .post(format!(
            "{base_url}/solution-packs/plans?prefix=brand&blueprint_publication=draft"
        ))
        .header("content-type", "application/zstd")
        .body(asset_archive("1.0.0", SAFE_SVG))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let plan_id = plan["id"].as_str().unwrap();
    store.set_available(false);
    let unavailable = client
        .post(format!("{base_url}/solution-packs/plans/{plan_id}/apply"))
        .send()
        .await
        .unwrap();
    assert_eq!(unavailable.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM solution_pack_applications WHERE plan_id=$1"
        )
        .bind(Uuid::parse_str(plan_id).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap(),
        "running"
    );
    store.set_available(true);
    let object_key: String = sqlx::query_scalar(
        "SELECT object_key FROM solution_pack_plan_asset_objects WHERE plan_id=$1",
    )
    .bind(Uuid::parse_str(plan_id).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    store.delete(&object_key).await.unwrap();
    let missing = client
        .post(format!("{base_url}/solution-packs/plans/{plan_id}/apply"))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let error: Value = missing.json().await.unwrap();
    assert_eq!(error["error"]["code"], "asset_object_integrity_failed");
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM solution_pack_applications WHERE plan_id=$1"
        )
        .bind(Uuid::parse_str(plan_id).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap(),
        "invalid"
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn provider_ignored_ranges_are_locally_bounded_for_content_mapping_and_apply(pool: PgPool) {
    let store = Arc::new(FlakyObjectStore::new());
    let (base_url, server) =
        start_server_with_custom_object_store(pool.clone(), store.clone()).await;
    let client = authenticated_client();
    let created = apply_asset_pack(&client, &base_url, "1.0.0", SAFE_SVG).await;
    let created_id = Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();
    let (object_key, expected_size): (String, i64) =
        sqlx::query_as("SELECT object_key,byte_size FROM presentation_assets WHERE id=$1")
            .bind(created_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let oversized = StoredObject {
        bytes: bytes::Bytes::from(vec![b'x'; 4 * 1024 * 1024]),
        content_type: Some("image/svg+xml".into()),
    };
    store
        .inner
        .put(&object_key, oversized.clone())
        .await
        .unwrap();
    store.ignore_ranges.store(true, Ordering::SeqCst);
    store.stream_chunks_yielded.store(0, Ordering::SeqCst);
    assert_eq!(
        client
            .get(format!(
                "{base_url}/presentation-assets/{created_id}/content"
            ))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        store.stream_chunks_yielded.load(Ordering::SeqCst),
        usize::try_from(expected_size).unwrap() + 1
    );
    store.ignore_ranges.store(false, Ordering::SeqCst);
    let mapping = json!({"key":"assets/brand-logo","id":created_id}).to_string();
    assert_eq!(
        client
            .post(format!(
                "{base_url}/solution-packs/plans?prefix=brand&blueprint_publication=draft"
            ))
            .multipart(
                multipart::Form::new()
                    .part(
                        "archive",
                        multipart::Part::bytes(asset_archive("2.0.0", SAFE_SVG))
                            .file_name("brand.tar.zst")
                            .mime_str("application/zstd")
                            .unwrap()
                    )
                    .text("asset_map", mapping)
            )
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    store.inner.delete(&object_key).await.unwrap();
    assert_eq!(
        client
            .get(format!(
                "{base_url}/presentation-assets/{created_id}/content"
            ))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    let plan: Value = client
        .post(format!(
            "{base_url}/solution-packs/plans?prefix=brand&blueprint_publication=draft"
        ))
        .header("content-type", "application/zstd")
        .body(asset_archive("2.0.0", SAFE_SVG))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let plan_id = Uuid::parse_str(plan["id"].as_str().unwrap()).unwrap();
    let staged_key: String = sqlx::query_scalar(
        "SELECT object_key FROM solution_pack_plan_asset_objects WHERE plan_id=$1",
    )
    .bind(plan_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    store.inner.put(&staged_key, oversized).await.unwrap();
    assert_eq!(
        client
            .post(format!("{base_url}/solution-packs/plans/{plan_id}/apply"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let expected_range = format!("bytes=0-{expected_size}");
    assert!(
        store
            .ranges
            .lock()
            .await
            .iter()
            .filter_map(Option::as_ref)
            .filter(|range| *range == &expected_range)
            .count()
            >= 3
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn transient_provider_operation_during_apply_is_retryable(pool: PgPool) {
    let store = Arc::new(FlakyObjectStore::new());
    let (base_url, server) =
        start_server_with_custom_object_store(pool.clone(), store.clone()).await;
    let client = authenticated_client();
    let plan: Value = client
        .post(format!(
            "{base_url}/solution-packs/plans?prefix=brand&blueprint_publication=draft"
        ))
        .header("content-type", "application/zstd")
        .body(asset_archive("1.0.0", SAFE_SVG))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let plan_id = Uuid::parse_str(plan["id"].as_str().unwrap()).unwrap();
    store.fail_next_range.store(true, Ordering::SeqCst);
    assert_eq!(
        client
            .post(format!("{base_url}/solution-packs/plans/{plan_id}/apply"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM solution_pack_applications WHERE plan_id=$1"
        )
        .bind(plan_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "running"
    );
    let retried: Value = client
        .post(format!("{base_url}/solution-packs/plans/{plan_id}/apply"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(retried["state"], "completed");
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn expired_plan_cleanup_never_deletes_running_application_staging(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base_url, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let client = authenticated_client();
    let plan: Value = client
        .post(format!(
            "{base_url}/solution-packs/plans?prefix=brand&blueprint_publication=draft"
        ))
        .header("content-type", "application/zstd")
        .body(asset_archive("1.0.0", SAFE_SVG))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let plan_id = Uuid::parse_str(plan["id"].as_str().unwrap()).unwrap();
    store.set_available(false);
    assert_eq!(
        client
            .post(format!("{base_url}/solution-packs/plans/{plan_id}/apply"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    sqlx::query("UPDATE solution_pack_plans SET created_at=now()-interval '2 seconds', expires_at=now()-interval '1 second' WHERE id=$1")
        .bind(plan_id)
        .execute(&pool)
        .await
        .unwrap();
    store.set_available(true);
    let optional_archive = asset_archive_with_requirement(
        "2.0.0",
        false,
        &[(
            "assets/brand-logo",
            "assets/brand-logo.svg",
            "logo",
            SAFE_SVG,
        )],
    );
    client
        .post(format!(
            "{base_url}/solution-packs/plans?prefix=brand&blueprint_publication=draft"
        ))
        .header("content-type", "application/zstd")
        .body(optional_archive)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert_eq!(store.object_count().await, 1);
    let retried: Value = client
        .post(format!("{base_url}/solution-packs/plans/{plan_id}/apply"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(retried["state"], "completed");
    assert_eq!(store.object_count().await, 1);
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn staging_metadata_mutation_invalidates_plan_evidence(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base_url, server) = start_server_with_object_store(pool.clone(), store).await;
    let client = authenticated_client();
    let plan: Value = client
        .post(format!(
            "{base_url}/solution-packs/plans?prefix=brand&blueprint_publication=draft"
        ))
        .header("content-type", "application/zstd")
        .body(asset_archive("1.0.0", SAFE_SVG))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let plan_id = Uuid::parse_str(plan["id"].as_str().unwrap()).unwrap();
    sqlx::query("UPDATE solution_pack_plan_asset_objects SET sha256=$2 WHERE plan_id=$1")
        .bind(plan_id)
        .bind("f".repeat(64))
        .execute(&pool)
        .await
        .unwrap();
    let response = client
        .post(format!("{base_url}/solution-packs/plans/{plan_id}/apply"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], "invalid_input");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM presentation_assets")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn planning_storage_failure_records_cleanup_without_ready_plan(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    store.set_available(false);
    let (base_url, server) = start_server_with_object_store(pool.clone(), store).await;
    let response = authenticated_client()
        .post(format!(
            "{base_url}/solution-packs/plans?prefix=brand&blueprint_publication=draft"
        ))
        .header("content-type", "application/zstd")
        .body(asset_archive("1.0.0", SAFE_SVG))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        !sqlx::query_scalar::<_, bool>("SELECT ready FROM solution_pack_plans")
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT state FROM solution_pack_plan_asset_objects")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "cleanup_pending"
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn optional_asset_is_skipped_without_staging_or_application_step(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base_url, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let client = authenticated_client();
    let archive = asset_archive_with_requirement(
        "1.0.0",
        false,
        &[(
            "assets/brand-logo",
            "assets/brand-logo.svg",
            "logo",
            SAFE_SVG,
        )],
    );
    let plan: Value = client
        .post(format!(
            "{base_url}/solution-packs/plans?prefix=brand&blueprint_publication=draft"
        ))
        .header("content-type", "application/zstd")
        .body(archive)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(plan["ready"], true);
    assert_eq!(plan["actions"][0]["action"], "skip");
    assert_eq!(plan["actions"][0]["reason_code"], "optional_not_selected");
    assert_eq!(store.object_count().await, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM solution_pack_plan_asset_objects")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let application: Value = client
        .post(format!(
            "{base_url}/solution-packs/plans/{}/apply",
            plan["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(application["state"], "completed");
    assert_eq!(application["steps"].as_array().unwrap().len(), 0);
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn solution_pack_stages_and_applies_normalized_asset_once(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base_url, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let client = authenticated_client();
    let archive = asset_archive("1.0.0", SAFE_SVG);
    let plan: Value = client
        .post(format!(
            "{base_url}/solution-packs/plans?prefix=brand&blueprint_publication=draft"
        ))
        .header("content-type", "application/zstd")
        .body(archive)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(plan["ready"], true);
    assert_eq!(plan["actions"][0]["resource_kind"], "presentation_asset");
    assert_eq!(plan["actions"][0]["action"], "create");
    assert!(!plan.to_string().contains("presentation-assets/"));
    assert!(!plan.to_string().contains("<svg"));
    assert_eq!(store.object_count().await, 1);
    let plan_id = plan["id"].as_str().unwrap();
    let application: Value = client
        .post(format!("{base_url}/solution-packs/plans/{plan_id}/apply"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(application["state"], "completed");
    assert_eq!(
        application["steps"][0]["resource_kind"],
        "presentation_asset"
    );
    assert_eq!(application["steps"][0]["state"], "completed");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM presentation_assets")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    let repeated = client
        .post(format!("{base_url}/solution-packs/plans/{plan_id}/apply"))
        .send()
        .await
        .unwrap();
    assert_eq!(repeated.status(), StatusCode::OK);
    assert_eq!(store.object_count().await, 1);
    let audit_text = sqlx::query_scalar::<_, String>(
        "SELECT coalesce(string_agg(target::text || metadata::text, ''), '') FROM audit_events",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!audit_text.contains("presentation-assets/"));
    assert!(!audit_text.contains("<svg"));
    server.abort();
}
