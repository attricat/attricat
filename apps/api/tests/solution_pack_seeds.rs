mod support;

use std::{io::Cursor, sync::Arc};

use api::storage::{FakeObjectStore, ObjectStore};
use sha2::{Digest, Sha256};
use support::*;

const PRODUCT_BLUEPRINT: &[u8] = br#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "category"
value_type = "relationship"
cardinality = "one"
target_blueprint = "blueprints/category"
"#;
const CATEGORY_BLUEPRINT: &[u8] = br#"
format_version = 1
code = "category"
name = "Category"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"
"#;
const DOCUMENT_BLUEPRINT: &[u8] = br#"
format_version = 1
code = "document"
name = "Document"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "summary"
value_type = "string"

[[attributes]]
code = "evidence"
value_type = "file"
cardinality = "many"
"#;
const EU_CONTEXT: &[u8] =
    br#"{"format_version":1,"kind":"solution_pack_context","data":{"region":"eu"}}"#;
const PL_CONTEXT: &[u8] = br#"{"format_version":1,"kind":"solution_pack_context","data":{"language":"pl"},"parent":"contexts/eu","publication_channel":{"enabled":true}}"#;
const NAME_RULE: &[u8] = br#"
format_version = 1
code = "name-required"
name = "Products have a name"
severity = "error"
blueprint = "blueprints/product"
context = "contexts/pl"
enabled = true

[[triggers]]
type = "manual"

[predicate]
type = "required"
attribute_code = "name"
"#;
const REVIEW_WORKFLOW: &[u8] = br#"
format_version = 2
code = "mark-reviewed"
name = "Mark reviewed"
enabled = false

[[triggers]]
type = "manual"

[[actions]]
type = "system_tags_add"
tags = ["reviewed"]
"#;
const UNNAMED_SEARCH: &[u8] = br#"{"format_version":1,"kind":"solution_pack_saved_search","name":"Unnamed products","description":"Products awaiting a name","state":{"blueprint":"blueprints/product","context":"contexts/pl","attributeFilters":[{"field":"name","operator":"eq","value":""}],"sort":{"field":"category.name","direction":"asc"}}}"#;
const PDF_BYTES: &[u8] = b"%PDF-1.7\nsynthetic evidence";

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn resource(key: &str, path: &str, bytes: &[u8]) -> Value {
    json!({"key": key, "path": path, "required": true, "sha256": digest(bytes)})
}

fn archive(manifest: &Value, files: &[(&str, &[u8])]) -> Vec<u8> {
    let manifest = serde_json::to_vec(manifest).unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        for (path, bytes) in std::iter::once(("solution-pack.json", manifest.as_slice()))
            .chain(files.iter().copied())
        {
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

fn manifest(id: &str, version: &str) -> Value {
    json!({
        "manifest_version": 1,
        "id": id,
        "name": "Seed fixture",
        "version": version,
        "description": "Seed resources fixture",
        "catalog": {"host_api": "^1.0"},
        "resources": {}
    })
}

fn seed_archive() -> Vec<u8> {
    seed_archive_version("1.0.0")
}

fn seed_archive_version(version: &str) -> Vec<u8> {
    let mut manifest = manifest("attricat.seeds", version);
    manifest["resources"] = json!({
        "blueprints": [
            resource("blueprints/product", "blueprints/product.toml", PRODUCT_BLUEPRINT),
            resource("blueprints/category", "blueprints/category.toml", CATEGORY_BLUEPRINT),
        ],
        "contexts": [
            resource("contexts/eu", "contexts/eu.json", EU_CONTEXT),
            resource("contexts/pl", "contexts/pl.json", PL_CONTEXT),
        ],
        "rules": [resource("rules/name-required", "rules/name-required.toml", NAME_RULE)],
        "workflows": [resource("workflows/mark-reviewed", "workflows/mark-reviewed.toml", REVIEW_WORKFLOW)],
        "saved_searches": [resource("saved-searches/unnamed", "saved-searches/unnamed.json", UNNAMED_SEARCH)],
    });
    archive(
        &manifest,
        &[
            ("blueprints/product.toml", PRODUCT_BLUEPRINT),
            ("blueprints/category.toml", CATEGORY_BLUEPRINT),
            ("contexts/eu.json", EU_CONTEXT),
            ("contexts/pl.json", PL_CONTEXT),
            ("rules/name-required.toml", NAME_RULE),
            ("workflows/mark-reviewed.toml", REVIEW_WORKFLOW),
            ("saved-searches/unnamed.json", UNNAMED_SEARCH),
        ],
    )
}

async fn create_plan(
    client: &Client,
    base_url: &str,
    archive: Vec<u8>,
    query: &str,
    context_maps: &[(&str, &str)],
) -> reqwest::Response {
    let url = format!("{base_url}/solution-packs/plans?{query}");
    if context_maps.is_empty() {
        return client
            .post(url)
            .header("content-type", "application/zstd")
            .body(archive)
            .send()
            .await
            .unwrap();
    }
    let archive = reqwest::multipart::Part::bytes(archive)
        .file_name("pack.tar.zst")
        .mime_str("application/zstd")
        .unwrap();
    let mut form = reqwest::multipart::Form::new().part("archive", archive);
    for (key, code) in context_maps {
        form = form.text("context_map", json!({"key": key, "code": code}).to_string());
    }
    client.post(url).multipart(form).send().await.unwrap()
}

async fn apply(client: &Client, base_url: &str, plan: &Value) -> Value {
    let response = client
        .post(format!(
            "{base_url}/solution-packs/plans/{}/apply",
            plan["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap();
    let status = response.status();
    let body = response.json::<Value>().await.unwrap();
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

fn action<'a>(plan: &'a Value, key: &str) -> &'a Value {
    plan["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["logical_key"] == key)
        .unwrap_or_else(|| panic!("missing action {key}"))
}

async fn count(pool: &PgPool, query: &str) -> i64 {
    sqlx::query_scalar(query).fetch_one(pool).await.unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn seeds_install_contexts_channels_rules_workflows_and_saved_searches(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();

    let inspection = client
        .post(format!("{base_url}/solution-packs/inspect"))
        .header("content-type", "application/zstd")
        .body(seed_archive())
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(inspection["seeds"]["rules"][0]["enabled"], true);
    assert_eq!(inspection["seeds"]["workflows"][0]["enabled"], false);
    assert_eq!(
        inspection["seeds"]["contexts"][1]["publication_channel"],
        json!({"enabled": true})
    );
    assert_eq!(
        inspection["seeds"]["saved_searches"][0]["name"],
        "Unnamed products"
    );

    let plan = create_plan(
        &client,
        &base_url,
        seed_archive(),
        "prefix=seed&blueprint_publication=publish",
        &[],
    )
    .await;
    assert_eq!(plan.status(), StatusCode::CREATED);
    let plan = plan.json::<Value>().await.unwrap();
    assert_eq!(plan["ready"], true, "{plan}");
    for (key, kind) in [
        ("contexts/eu", "context"),
        ("contexts/pl", "context"),
        ("channels/pl", "publication_channel"),
        ("rules/name-required", "rule"),
        ("workflows/mark-reviewed", "workflow"),
        ("saved-searches/unnamed", "saved_search"),
    ] {
        let action = action(&plan, key);
        assert_eq!(action["resource_kind"], kind);
        assert_eq!(action["action"], "create", "{action}");
        assert!(action.get("normalized_payload").is_none());
    }
    assert_eq!(
        action(&plan, "rules/name-required")["summary"]["target_code"],
        "seed_name-required"
    );
    assert_eq!(
        action(&plan, "rules/name-required")["summary"]["context_code"],
        "seed_pl"
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM attribute_contexts WHERE code LIKE 'seed_%'"
        )
        .await,
        0,
        "planning must not mutate the workspace"
    );

    let application = apply(&client, &base_url, &plan).await;
    assert_eq!(application["state"], "completed", "{application}");

    let (eu_id, pl_id, pl_parent): (Uuid, Uuid, Option<Uuid>) = sqlx::query_as(
        "SELECT eu.id, pl.id, pl.parent_id FROM attribute_contexts eu JOIN attribute_contexts pl ON pl.workspace_id = eu.workspace_id WHERE eu.code = 'seed_eu' AND pl.code = 'seed_pl'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pl_parent, Some(eu_id));
    assert!(
        sqlx::query_scalar::<_, bool>(
            "SELECT enabled FROM publication_channels WHERE context_id = $1"
        )
        .bind(pl_id)
        .fetch_one(&pool)
        .await
        .unwrap()
    );
    let (rule_status, rule_context, enabled_version): (String, Option<Uuid>, Option<i64>) =
        sqlx::query_as(
            "SELECT r.status, r.context_id, l.enabled_version FROM rules r JOIN rule_lifecycles l ON l.rule_id = r.id WHERE r.code = 'seed_name-required'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        (rule_status.as_str(), rule_context, enabled_version),
        ("published", Some(pl_id), Some(1))
    );
    let (workflow_status, workflow_enabled): (String, Option<i64>) = sqlx::query_as(
        "SELECT w.status, l.enabled_version FROM workflows w JOIN workflow_lifecycles l ON l.workflow_id = w.id WHERE w.code = 'seed_mark-reviewed'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (workflow_status.as_str(), workflow_enabled),
        ("published", None)
    );

    let searches = client
        .get(format!("{base_url}/saved-views"))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let search = searches
        .as_array()
        .unwrap()
        .iter()
        .find(|view| view["name"] == "Unnamed products")
        .expect("seeded search is listed for Explore");
    assert_eq!(search["visibility"], "workspace");
    assert_eq!(search["state"]["blueprint"], "seed_product");
    assert_eq!(search["state"]["context"], "seed_pl");
    assert_eq!(search["state"]["sort"]["field"], "category.name");

    // Retrying a completed plan is a no-op.
    let retried = apply(&client, &base_url, &plan).await;
    assert_eq!(retried["id"], application["id"]);
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM rules WHERE code = 'seed_name-required'"
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM saved_views WHERE name = 'Unnamed products'"
        )
        .await,
        1
    );

    // An application interrupted before its last seed step resumes it once.
    let application_id = Uuid::parse_str(application["id"].as_str().unwrap()).unwrap();
    let saved_search_id = Uuid::parse_str(search["id"].as_str().unwrap()).unwrap();
    sqlx::query("DELETE FROM saved_views WHERE id = $1")
        .bind(saved_search_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE solution_pack_application_steps SET state = 'failed', diagnostic_code = 'step_failed', diagnostic_message = 'interrupted', result_snapshot = NULL, completed_at = NULL WHERE application_id = $1 AND resource_kind = 'saved_search'")
        .bind(application_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE solution_pack_applications SET state = 'failed', completed_at = NULL, diagnostic_code = 'step_failed', diagnostic_message = 'interrupted' WHERE id = $1")
        .bind(application_id)
        .execute(&pool)
        .await
        .unwrap();
    let resumed = apply(&client, &base_url, &plan).await;
    assert_eq!(resumed["state"], "completed", "{resumed}");
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM saved_views WHERE name = 'Unnamed products'"
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM rules WHERE code = 'seed_name-required'"
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM attribute_contexts WHERE code LIKE 'seed_%'"
        )
        .await,
        2
    );

    // A later plan with the same prefix reports conflicts instead of duplicates.
    let again = create_plan(
        &client,
        &base_url,
        seed_archive(),
        "prefix=seed&blueprint_publication=publish",
        &[],
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    assert_eq!(again["ready"], false);
    assert_eq!(
        action(&again, "contexts/eu")["reason_code"],
        "target_code_exists"
    );
    assert_eq!(
        action(&again, "blueprints/product")["reason_code"],
        "target_code_exists"
    );
    assert_eq!(action(&again, "rules/name-required")["action"], "blocked");
    assert_eq!(
        action(&again, "saved-searches/unnamed")["action"],
        "blocked"
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn installers_map_pack_contexts_to_existing_contexts_and_channels(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let context = client
        .post(format!("{base_url}/contexts"))
        .json(&json!({"code": "PL", "data": {"language": "pl"}}))
        .send()
        .await
        .unwrap();
    assert_eq!(context.status(), StatusCode::CREATED);
    let context = context.json::<Value>().await.unwrap();
    let context_id = Uuid::parse_str(context["id"].as_str().unwrap()).unwrap();
    sqlx::query("INSERT INTO publication_channels (workspace_id, context_id, enabled) SELECT workspace_id, id, false FROM attribute_contexts WHERE id = $1")
        .bind(context_id)
        .execute(&pool)
        .await
        .unwrap();

    let unknown = create_plan(
        &client,
        &base_url,
        seed_archive(),
        "prefix=mapped&blueprint_publication=publish",
        &[("contexts/pl", "missing")],
    )
    .await;
    assert_eq!(unknown.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let mismatch = create_plan(
        &client,
        &base_url,
        seed_archive(),
        "prefix=mapped&blueprint_publication=publish",
        &[("contexts/pl", "PL")],
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    assert_eq!(mismatch["ready"], false);
    assert_eq!(action(&mismatch, "contexts/pl")["action"], "map");
    assert_eq!(
        action(&mismatch, "channels/pl")["reason_code"],
        "publication_channel_mismatch"
    );

    sqlx::query("UPDATE publication_channels SET enabled = true WHERE context_id = $1")
        .bind(context_id)
        .execute(&pool)
        .await
        .unwrap();
    let plan = create_plan(
        &client,
        &base_url,
        seed_archive(),
        "prefix=mapped&blueprint_publication=publish",
        &[("contexts/pl", "PL")],
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    assert_eq!(plan["ready"], true, "{plan}");
    assert_eq!(
        action(&plan, "contexts/pl")["reason_code"],
        "existing_context_selected"
    );
    assert_eq!(action(&plan, "channels/pl")["action"], "satisfied");
    assert_eq!(
        action(&plan, "rules/name-required")["summary"]["context_code"],
        "PL"
    );

    // The installer's channel choice is revalidated when applying.
    sqlx::query("UPDATE publication_channels SET enabled = false WHERE context_id = $1")
        .bind(context_id)
        .execute(&pool)
        .await
        .unwrap();
    let stale = client
        .post(format!(
            "{base_url}/solution-packs/plans/{}/apply",
            plan["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM attribute_contexts WHERE code LIKE 'mapped_%'"
        )
        .await,
        0
    );

    sqlx::query("UPDATE publication_channels SET enabled = true WHERE context_id = $1")
        .bind(context_id)
        .execute(&pool)
        .await
        .unwrap();
    let plan = create_plan(
        &client,
        &base_url,
        seed_archive(),
        "prefix=mapped&blueprint_publication=publish",
        &[("contexts/pl", "PL")],
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    let application = apply(&client, &base_url, &plan).await;
    assert_eq!(application["state"], "completed", "{application}");
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM attribute_contexts WHERE code = 'mapped_pl'"
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM attribute_contexts WHERE code = 'mapped_eu'"
        )
        .await,
        1
    );
    let rule_context: Option<Uuid> =
        sqlx::query_scalar("SELECT context_id FROM rules WHERE code = 'mapped_name-required'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rule_context, Some(context_id));
    server.abort();
}

fn base_archive(version: &str) -> Vec<u8> {
    let mut manifest = manifest("attricat.base", version);
    manifest["resources"]["blueprints"] = json!([resource(
        "blueprints/category",
        "blueprints/category.toml",
        CATEGORY_BLUEPRINT
    )]);
    archive(
        &manifest,
        &[("blueprints/category.toml", CATEGORY_BLUEPRINT)],
    )
}

fn dependent_archive(range: &str) -> Vec<u8> {
    let mut manifest = manifest("attricat.shop", "1.0.0");
    manifest["prerequisites"] =
        json!([{"key": "prerequisites/base", "id": "attricat.base", "version": range}]);
    let mut category = resource(
        "blueprints/category",
        "blueprints/category.toml",
        CATEGORY_BLUEPRINT,
    );
    category["reuse"] =
        json!({"prerequisite": "prerequisites/base", "blueprint": "blueprints/category"});
    manifest["resources"]["blueprints"] = json!([
        resource(
            "blueprints/product",
            "blueprints/product.toml",
            PRODUCT_BLUEPRINT
        ),
        category,
    ]);
    archive(
        &manifest,
        &[
            ("blueprints/product.toml", PRODUCT_BLUEPRINT),
            ("blueprints/category.toml", CATEGORY_BLUEPRINT),
        ],
    )
}

#[sqlx::test(migrations = "./migrations")]
async fn seeds_reuse_exact_blueprints_from_applied_prerequisites(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let query = "prefix=shop&blueprint_publication=publish";

    let missing = create_plan(&client, &base_url, dependent_archive("^1.2"), query, &[])
        .await
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(missing["ready"], false);
    assert_eq!(
        action(&missing, "prerequisites/base")["reason_code"],
        "prerequisite_missing"
    );
    assert_eq!(
        action(&missing, "blueprints/category")["reason_code"],
        "prerequisite_unavailable"
    );
    assert_eq!(
        action(&missing, "blueprints/product")["reason_code"],
        "dependency_not_creatable"
    );

    let base = create_plan(
        &client,
        &base_url,
        base_archive("1.3.0"),
        "prefix=base&blueprint_publication=publish",
        &[],
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    let base_application = apply(&client, &base_url, &base).await;

    let incompatible = create_plan(&client, &base_url, dependent_archive("^2.0"), query, &[])
        .await
        .json::<Value>()
        .await
        .unwrap();
    let prerequisite = action(&incompatible, "prerequisites/base");
    assert_eq!(prerequisite["reason_code"], "prerequisite_incompatible");
    assert_eq!(
        prerequisite["summary"]["resolution"]["available_versions"],
        json!(["1.3.0"])
    );

    let plan = create_plan(&client, &base_url, dependent_archive("^1.2"), query, &[])
        .await
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(plan["ready"], true, "{plan}");
    let prerequisite = action(&plan, "prerequisites/base");
    assert_eq!(prerequisite["action"], "map");
    assert_eq!(
        prerequisite["summary"]["resolution"]["application_id"],
        base_application["id"]
    );
    let category = action(&plan, "blueprints/category");
    assert_eq!(category["action"], "map");
    assert_eq!(category["reason_code"], "prerequisite_blueprint_match");
    assert_eq!(category["summary"]["target_code"], "base_category");

    let application = apply(&client, &base_url, &plan).await;
    assert_eq!(application["state"], "completed", "{application}");
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM blueprints WHERE code = 'shop_category'"
        )
        .await,
        0
    );
    let target: String = sqlx::query_scalar(
        "SELECT a.target_blueprint_code FROM attributes a JOIN blueprints b ON b.id = a.blueprint_id AND b.version = a.blueprint_version WHERE b.code = 'shop_product' AND a.code = 'category'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(target, "base_category");

    // A changed prerequisite blueprint is no longer an exact match.
    let category_id: Uuid =
        sqlx::query_scalar("SELECT id FROM blueprints WHERE code = 'base_category' LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let changed = client
        .post(format!("{base_url}/blueprints/{category_id}/versions"))
        .json(&json!({"definition": String::from_utf8(CATEGORY_BLUEPRINT.to_vec()).unwrap().replace("code = \"category\"", "code = \"base_category\"").replace("value_type = \"string\"", "value_type = \"string\"\n\n[[attributes]]\ncode = \"rank\"\nvalue_type = \"integer\"")}))
        .send()
        .await
        .unwrap();
    assert!(
        changed.status().is_success(),
        "{}",
        changed.text().await.unwrap()
    );
    let published = client
        .post(format!(
            "{base_url}/blueprints/{category_id}/versions/2/publish"
        ))
        .send()
        .await
        .unwrap();
    assert!(
        published.status().is_success(),
        "{}",
        published.text().await.unwrap()
    );
    let drifted = create_plan(
        &client,
        &base_url,
        dependent_archive("^1.2"),
        "prefix=shop2&blueprint_publication=publish",
        &[],
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    assert_eq!(
        action(&drifted, "blueprints/category")["reason_code"],
        "prerequisite_blueprint_incompatible"
    );
    server.abort();
}

fn sample_archive() -> (Vec<u8>, Vec<u8>) {
    let sample = serde_json::to_vec(&json!({
        "format_version": 1,
        "kind": "solution_pack_sample_data",
        "classification": "synthetic",
        "entities": [{
            "key": "sample-entities/spec",
            "blueprint": "blueprints/document",
            "facts": [
                {"attribute": "blueprints/document/attributes/title", "value": "Sample specification"},
                {"attribute": "blueprints/document/attributes/summary", "value": "Default summary"},
                {"attribute": "blueprints/document/attributes/summary", "value": "EU summary", "context": "contexts/eu"}
            ],
            "relationships": [],
            "files": [{
                "attribute": "blueprints/document/attributes/evidence",
                "context": "contexts/eu",
                "files": [{"path": "sample-data/files/spec.pdf", "filename": "Specification.pdf", "media_type": "application/pdf"}]
            }]
        }]
    }))
    .unwrap();
    let mut manifest = manifest("attricat.documents", "1.0.0");
    manifest["resources"] = json!({
        "blueprints": [resource("blueprints/document", "blueprints/document.toml", DOCUMENT_BLUEPRINT)],
        "contexts": [resource("contexts/eu", "contexts/eu.json", EU_CONTEXT)],
        "sample_data": {
            "key": "sample-data/default",
            "path": "sample-data/sample-data.json",
            "sha256": digest(&sample),
            "files": [{"path": "sample-data/files/spec.pdf", "sha256": digest(PDF_BYTES)}]
        }
    });
    let archive = archive(
        &manifest,
        &[
            ("blueprints/document.toml", DOCUMENT_BLUEPRINT),
            ("contexts/eu.json", EU_CONTEXT),
            ("sample-data/sample-data.json", &sample),
            ("sample-data/files/spec.pdf", PDF_BYTES),
        ],
    );
    (archive, sample)
}

#[sqlx::test(migrations = "./migrations")]
async fn samples_store_bundled_files_and_contextual_values(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base_url, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let client = authenticated_client();
    let (archive, _) = sample_archive();

    let inspection = client
        .post(format!("{base_url}/solution-packs/inspect"))
        .header("content-type", "application/zstd")
        .body(archive.clone())
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(inspection["sample_data"]["file_count"], 1);

    let plan = create_plan(
        &client,
        &base_url,
        archive,
        "prefix=docs&blueprint_publication=publish&include_sample_data=true",
        &[],
    )
    .await;
    assert_eq!(plan.status(), StatusCode::CREATED);
    let plan = plan.json::<Value>().await.unwrap();
    assert_eq!(plan["ready"], true, "{plan}");
    let plan_id = Uuid::parse_str(plan["id"].as_str().unwrap()).unwrap();
    let (object_key, file_id): (String, Uuid) = sqlx::query_as(
        "SELECT object_key, file_id FROM solution_pack_plan_sample_files WHERE plan_id = $1",
    )
    .bind(plan_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        store.get(&object_key).await.unwrap().bytes.as_ref(),
        PDF_BYTES,
        "planning stages the bundled file"
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM file_upload_intents WHERE cleanup_after > now() + interval '29 days'").await,
        1
    );

    let application = apply(&client, &base_url, &plan).await;
    assert_eq!(application["state"], "completed", "{application}");
    let entity_id: Uuid = sqlx::query_scalar(
        "SELECT target_id FROM solution_pack_application_steps WHERE resource_kind = 'sample_entity' AND plan_id = $1",
    )
    .bind(plan_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let eu_id: Uuid =
        sqlx::query_scalar("SELECT id FROM attribute_contexts WHERE code = 'docs_eu'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let summaries: Vec<(String, String)> = sqlx::query_as(
        "SELECT c.code, v.value_text FROM attribute_values v JOIN attributes a ON a.id = v.attribute_id JOIN attribute_contexts c ON c.id = v.context_id WHERE v.entity_id = $1 AND a.code = 'summary' AND v.active ORDER BY c.code",
    )
    .bind(entity_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        summaries,
        vec![
            ("default".to_owned(), "Default summary".to_owned()),
            ("docs_eu".to_owned(), "EU summary".to_owned()),
        ]
    );
    let (filename, sha256, stored_key, context_id): (String, String, String, Uuid) = sqlx::query_as(
        "SELECT f.display_filename, f.sha256, f.original_key, v.context_id FROM files f JOIN attribute_file_references r ON r.file_id = f.id JOIN attribute_values v ON v.id = r.attribute_value_id WHERE v.entity_id = $1 AND f.id = $2",
    )
    .bind(entity_id)
    .bind(file_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(filename, "Specification.pdf");
    assert_eq!(sha256, digest(PDF_BYTES));
    assert_eq!(stored_key, object_key);
    assert_eq!(context_id, eu_id);
    assert_eq!(
        count(&pool, "SELECT count(*) FROM file_upload_intents").await,
        0,
        "an attached file is owned by ordinary storage"
    );
    let metadata = client
        .get(format!("{base_url}/files/{file_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(metadata.status(), StatusCode::OK);
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn unapplied_sample_file_staging_is_released_for_cleanup(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base_url, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let client = authenticated_client();
    let (archive, _) = sample_archive();
    let plan = create_plan(
        &client,
        &base_url,
        archive,
        "prefix=docs&blueprint_publication=publish&include_sample_data=true",
        &[],
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    let plan_id = Uuid::parse_str(plan["id"].as_str().unwrap()).unwrap();
    sqlx::query("UPDATE solution_pack_plans SET created_at = now() - interval '2 days', expires_at = now() - interval '1 day' WHERE id = $1")
        .bind(plan_id)
        .execute(&pool)
        .await
        .unwrap();
    // Planning runs the same housekeeping as the hourly worker.
    let other = create_plan(
        &client,
        &base_url,
        seed_archive(),
        "prefix=other&blueprint_publication=publish",
        &[],
    )
    .await;
    assert_eq!(other.status(), StatusCode::CREATED);
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM file_upload_intents WHERE cleanup_after <= now()"
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM solution_pack_plan_sample_entities"
        )
        .await,
        0
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn later_releases_reuse_seed_resources_without_recreating_them(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let plan = create_plan(
        &client,
        &base_url,
        seed_archive(),
        "prefix=seed&blueprint_publication=publish",
        &[],
    )
    .await
    .json::<Value>()
    .await
    .unwrap();
    let application = apply(&client, &base_url, &plan).await;

    let later = create_plan(
        &client,
        &base_url,
        seed_archive_version("1.1.0"),
        &format!(
            "prefix=seed&blueprint_publication=publish&from_application={}",
            application["id"].as_str().unwrap()
        ),
        &[],
    )
    .await;
    assert_eq!(later.status(), StatusCode::CREATED);
    let later = later.json::<Value>().await.unwrap();
    assert_eq!(later["ready"], true, "{later}");
    for key in ["contexts/eu", "contexts/pl"] {
        let action = action(&later, key);
        assert_eq!(action["action"], "map");
        assert_eq!(action["reason_code"], "unchanged_from_prior_application");
    }
    assert_eq!(action(&later, "channels/pl")["action"], "satisfied");
    for key in [
        "rules/name-required",
        "workflows/mark-reviewed",
        "saved-searches/unnamed",
    ] {
        let action = action(&later, key);
        assert_eq!(action["action"], "skip", "{action}");
        assert_eq!(action["reason_code"], "provided_by_prior_application");
    }
    let applied = apply(&client, &base_url, &later).await;
    assert_eq!(applied["state"], "completed", "{applied}");
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM attribute_contexts WHERE code LIKE 'seed_%'"
        )
        .await,
        2
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM rules WHERE code = 'seed_name-required'"
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM saved_views WHERE name = 'Unnamed products'"
        )
        .await,
        1
    );
    server.abort();
}
