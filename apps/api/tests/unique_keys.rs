mod support;

use support::*;

const PART: &str = r#"
format_version = 1
code = "uk_part"
name = "Part"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["part_number"]

[[unique_keys]]
code = "part_number"
attributes = ["part_number"]

[[attributes]]
code = "part_number"
value_type = "string"

[[attributes]]
code = "description"
value_type = "string"
"#;

#[sqlx::test]
async fn single_keys_normalize_values_and_report_the_conflicting_entity(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(&client, &base_url, PART).await;

    let first: Value = create_entity_with(
        &client,
        &base_url,
        "uk_part",
        json!([scalar("part_number", json!("ABC-1  Rev"))]),
    )
    .await;

    let duplicate = post_entity(
        &client,
        &base_url,
        "uk_part",
        json!([scalar("part_number", json!("  abc-1 rev "))]),
    )
    .await;
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);
    let body: Value = duplicate.json().await.unwrap();
    assert_eq!(body["error"]["code"], "unique_key_conflict");
    assert_eq!(body["error"]["details"]["key"], "part_number");
    assert_eq!(body["error"]["details"]["context"], "default");
    assert_eq!(body["error"]["details"]["values"], json!(["abc-1 rev"]));
    assert_eq!(
        body["error"]["details"]["conflicting_entity_id"],
        first["id"]
    );

    // Entities without the key attribute do not participate.
    for _ in 0..2 {
        create_entity_with(
            &client,
            &base_url,
            "uk_part",
            json!([scalar("description", json!("no number"))]),
        )
        .await;
    }

    // Changing or deleting the holder releases its old value.
    let second: Value = create_entity_with(
        &client,
        &base_url,
        "uk_part",
        json!([scalar("part_number", json!("XYZ"))]),
    )
    .await;
    client
        .post(format!(
            "{base_url}/entities/{}/values",
            first["id"].as_str().unwrap()
        ))
        .json(&json!({ "values": [scalar("part_number", json!("ABC-2"))] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let reuse = client
        .post(format!(
            "{base_url}/entities/{}/values",
            second["id"].as_str().unwrap()
        ))
        .json(&json!({ "values": [scalar("part_number", json!("abc-1 rev"))] }))
        .send()
        .await
        .unwrap();
    assert_eq!(reuse.status(), StatusCode::CREATED);
    let taken = post_entity(
        &client,
        &base_url,
        "uk_part",
        json!([scalar("part_number", json!("ABC-2"))]),
    )
    .await;
    assert_eq!(taken.status(), StatusCode::CONFLICT);
    client
        .delete(format!(
            "{base_url}/entities/{}",
            first["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    create_entity_with(
        &client,
        &base_url,
        "uk_part",
        json!([scalar("part_number", json!("ABC-2"))]),
    )
    .await;

    server.abort();
}

#[sqlx::test]
async fn concurrent_duplicate_writes_allow_exactly_one(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(&client, &base_url, PART).await;

    let mut attempts = tokio::task::JoinSet::new();
    for index in 0..8 {
        let client = client.clone();
        let base_url = base_url.clone();
        attempts.spawn(async move {
            post_entity(
                &client,
                &base_url,
                "uk_part",
                json!([
                    scalar("part_number", json!("RACE-1")),
                    scalar("description", json!(format!("attempt {index}"))),
                ]),
            )
            .await
            .status()
        });
    }
    let statuses = attempts.join_all().await;
    assert_eq!(
        statuses
            .iter()
            .filter(|status| **status == StatusCode::CREATED)
            .count(),
        1,
        "{statuses:?}"
    );
    assert!(
        statuses
            .iter()
            .all(|status| *status == StatusCode::CREATED || *status == StatusCode::CONFLICT),
        "{statuses:?}"
    );

    server.abort();
}

#[sqlx::test]
async fn composite_keys_combine_relationships_and_scalars(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "uk_document"
name = "Document"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["number"]
[[attributes]]
code = "number"
value_type = "string"
"#,
    )
    .await;
    create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "uk_revision"
name = "Revision"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["label"]
[[unique_keys]]
code = "document_revision"
attributes = ["document", "label"]
case_sensitive = true
[[attributes]]
code = "document"
value_type = "relationship"
target_blueprint = "uk_document"
cardinality = "one"
[[attributes]]
code = "label"
value_type = "string"
"#,
    )
    .await;
    let document = |number: &'static str| {
        let client = client.clone();
        let base_url = base_url.clone();
        async move {
            post_entity(
                &client,
                &base_url,
                "uk_document",
                json!([scalar("number", json!(number))]),
            )
            .await
            .json::<Value>()
            .await
            .unwrap()
        }
    };
    let (first, second) = (document("D-1").await, document("D-2").await);
    let revision = |document: &Value, label: &str| {
        json!([
            {"kind": "relationship", "attribute_code": "document", "context_id": null, "target_entity_id": document["id"]},
            scalar("label", json!(label)),
        ])
    };
    for (document, label) in [(&first, "A"), (&second, "A"), (&first, "a")] {
        create_entity_with(&client, &base_url, "uk_revision", revision(document, label)).await;
    }
    let duplicate = post_entity(&client, &base_url, "uk_revision", revision(&first, "A")).await;
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);
    let body: Value = duplicate.json().await.unwrap();
    assert_eq!(
        body["error"]["details"]["values"],
        json!([first["id"], "A"])
    );

    server.abort();
}

#[sqlx::test]
async fn publishing_a_key_reports_existing_duplicates_and_then_covers_older_revisions(
    pool: PgPool,
) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let without_key = PART.replace(
        "[[unique_keys]]\ncode = \"part_number\"\nattributes = [\"part_number\"]\n",
        "",
    );
    let blueprint = create_blueprint(&client, &base_url, &without_key).await;
    let blueprint_id = blueprint["blueprint"]["id"].as_str().unwrap().to_owned();
    let mut ids = Vec::new();
    for number in ["P-1", "p-1", "P-2"] {
        let entity: Value = create_entity_with(
            &client,
            &base_url,
            "uk_part",
            json!([scalar("part_number", json!(number))]),
        )
        .await;
        ids.push(entity["id"].as_str().unwrap().to_owned());
    }

    let revision: Value = client
        .post(format!("{base_url}/blueprints/{blueprint_id}/versions"))
        .json(&json!({ "definition": PART }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let version = revision["blueprint"]["version"].as_i64().unwrap();
    let publish = || {
        client
            .post(format!(
                "{base_url}/blueprints/{blueprint_id}/versions/{version}/publish"
            ))
            .send()
    };
    let rejected = publish().await.unwrap();
    assert_eq!(rejected.status(), StatusCode::CONFLICT);
    let body: Value = rejected.json().await.unwrap();
    assert_eq!(body["error"]["code"], "unique_key_duplicates");
    assert_eq!(body["error"]["details"]["total"], 1);
    let duplicate = &body["error"]["details"]["duplicates"][0];
    assert_eq!(duplicate["key"], "part_number");
    assert_eq!(duplicate["values"], json!(["p-1"]));
    let mut expected = vec![ids[0].clone(), ids[1].clone()];
    expected.sort();
    assert_eq!(duplicate["entity_ids"], json!(expected));

    client
        .post(format!("{base_url}/entities/{}/values", ids[1]))
        .json(&json!({ "values": [scalar("part_number", json!("P-3"))] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    publish().await.unwrap().error_for_status().unwrap();

    // Entities pinned to revision 1 are covered by the family's key.
    let pinned_write = client
        .post(format!("{base_url}/entities/{}/values", ids[2]))
        .json(&json!({ "values": [scalar("part_number", json!(" P-1 "))] }))
        .send()
        .await
        .unwrap();
    assert_eq!(pinned_write.status(), StatusCode::CONFLICT);
    assert_eq!(
        pinned_write.json::<Value>().await.unwrap()["error"]["details"]["conflicting_entity_id"],
        ids[0]
    );

    server.abort();
}

#[sqlx::test]
async fn context_scoped_keys_compare_resolved_values_in_every_context(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "uk_page"
name = "Page"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["slug"]
[[unique_keys]]
code = "slug"
attributes = ["slug"]
scope = "context"
[[attributes]]
code = "slug"
value_type = "string"
"#,
    )
    .await;
    let polish: Value = client
        .post(format!("{base_url}/contexts"))
        .json(&json!({ "code": "uk-pl", "data": {} }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let shirt: Value = create_entity_with(
        &client,
        &base_url,
        "uk_page",
        json!([scalar("slug", json!("shirt"))]),
    )
    .await;
    let dress: Value = create_entity_with(
        &client,
        &base_url,
        "uk_page",
        json!([scalar("slug", json!("dress"))]),
    )
    .await;
    let set_slug = |entity: &Value, context: &Value, slug: &str| {
        client
            .post(format!(
                "{base_url}/entities/{}/values",
                entity["id"].as_str().unwrap()
            ))
            .json(&json!({ "values": [{
                "kind": "scalar", "attribute_code": "slug",
                "context_id": context["id"], "value": slug,
            }] }))
            .send()
    };

    // The dress's Polish override collides with the shirt's inherited slug.
    let collision = set_slug(&dress, &polish, "SHIRT").await.unwrap();
    assert_eq!(collision.status(), StatusCode::CONFLICT);
    let body: Value = collision.json().await.unwrap();
    assert_eq!(body["error"]["details"]["context"], "uk-pl");
    assert_eq!(
        body["error"]["details"]["conflicting_entity_id"],
        shirt["id"]
    );
    // Overriding the shirt in Polish frees "shirt" there.
    set_slug(&shirt, &polish, "koszula")
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    set_slug(&dress, &polish, "shirt")
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    // A new child context inherits the Polish values and their keys.
    let web: Value = client
        .post(format!("{base_url}/contexts"))
        .json(&json!({ "code": "uk-pl-web", "data": {}, "parent_id": polish["id"] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let inherited = set_slug(&shirt, &web, "shirt").await.unwrap();
    assert_eq!(inherited.status(), StatusCode::CONFLICT);
    assert_eq!(
        inherited.json::<Value>().await.unwrap()["error"]["details"]["conflicting_entity_id"],
        dress["id"]
    );

    server.abort();
}

async fn current_values(client: &Client, base_url: &str, entity: &Value) -> Vec<Value> {
    client
        .get(format!(
            "{base_url}/entities/{}/values/current",
            entity["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Vec<Value>>()
        .await
        .unwrap()
        .into_iter()
        .map(|value| value["value"].clone())
        .collect()
}

#[sqlx::test]
async fn duplicating_leaves_out_enforced_key_values(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(&client, &base_url, PART).await;
    create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "uk_listing"
name = "Listing"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["slug"]
[[unique_keys]]
code = "slug"
attributes = ["slug"]
scope = "context"
[[attributes]]
code = "slug"
value_type = "string"
[[attributes]]
code = "title"
value_type = "string"
"#,
    )
    .await;
    let polish: Value = client
        .post(format!("{base_url}/contexts"))
        .json(&json!({ "code": "uk-dup-pl", "data": {} }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let duplicate = |entity: &Value| {
        client
            .post(format!(
                "{base_url}/v1/entities/{}/duplicate",
                entity["id"].as_str().unwrap()
            ))
            .send()
    };

    // Workspace keys: the default-context key value is left out, other
    // values are copied, and the copy can take a value of its own.
    let part: Value = create_entity_with(
        &client,
        &base_url,
        "uk_part",
        json!([
            scalar("part_number", json!("P-1")),
            scalar("description", json!("Bolt")),
        ]),
    )
    .await;
    let response = duplicate(&part).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let copy: Value = response.json().await.unwrap();
    assert_eq!(
        current_values(&client, &base_url, &copy).await,
        vec![json!("Bolt")]
    );
    client
        .post(format!(
            "{base_url}/entities/{}/values",
            copy["id"].as_str().unwrap()
        ))
        .json(&json!({ "values": [scalar("part_number", json!("P-2"))] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    // Context keys: the key's values are left out in every context.
    let listing: Value = create_entity_with(
        &client,
        &base_url,
        "uk_listing",
        json!([
            scalar("slug", json!("shirt")),
            scalar("title", json!("Shirt")),
        ]),
    )
    .await;
    client
        .post(format!(
            "{base_url}/entities/{}/values",
            listing["id"].as_str().unwrap()
        ))
        .json(&json!({ "values": [{
            "kind": "scalar", "attribute_code": "slug",
            "context_id": polish["id"], "value": "koszula",
        }] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let response = duplicate(&listing).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let copy: Value = response.json().await.unwrap();
    assert_eq!(
        current_values(&client, &base_url, &copy).await,
        vec![json!("Shirt")]
    );

    server.abort();
}

const DEFAULT_CONTEXT: &str = "00000000-0000-4000-8000-000000000001";

async fn create_context(client: &Client, base_url: &str, code: &str, parent: &str) -> Value {
    client
        .post(format!("{base_url}/contexts"))
        .json(&json!({ "code": code, "data": {}, "parent_id": parent }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}

async fn set_scalar(
    client: &Client,
    base_url: &str,
    entity: &Value,
    context: &Value,
    code: &str,
    value: &str,
) -> reqwest::Response {
    client
        .post(format!(
            "{base_url}/entities/{}/values",
            entity["id"].as_str().unwrap()
        ))
        .json(&json!({ "values": [{
            "kind": "scalar", "attribute_code": code,
            "context_id": context["id"], "value": value,
        }] }))
        .send()
        .await
        .unwrap()
}

#[sqlx::test]
async fn new_contexts_do_not_inherit_keys_of_attributes_without_fallback(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "uk_handle"
name = "Handle"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["handle"]
[[unique_keys]]
code = "handle"
attributes = ["handle"]
scope = "context"
[[attributes]]
code = "handle"
value_type = "string"
context_fallback = "none"
"#,
    )
    .await;
    let first = create_entity_with(
        &client,
        &base_url,
        "uk_handle",
        json!([scalar("handle", json!("x"))]),
    )
    .await;
    let second = create_entity_with(&client, &base_url, "uk_handle", json!([])).await;
    let polish = create_context(&client, &base_url, "uk-none-pl", DEFAULT_CONTEXT).await;

    // The first entity has no Polish handle, so "x" is free there.
    set_scalar(&client, &base_url, &second, &polish, "handle", "x")
        .await
        .error_for_status()
        .unwrap();
    let taken = set_scalar(&client, &base_url, &first, &polish, "handle", "X").await;
    assert_eq!(taken.status(), StatusCode::CONFLICT);
    assert_eq!(
        taken.json::<Value>().await.unwrap()["error"]["details"]["conflicting_entity_id"],
        second["id"]
    );

    server.abort();
}

#[sqlx::test]
async fn reparenting_a_context_can_swap_two_entities_key_values(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "uk_swap"
name = "Swap"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["slug"]
[[unique_keys]]
code = "slug"
attributes = ["slug"]
scope = "context"
[[attributes]]
code = "slug"
value_type = "string"
"#,
    )
    .await;
    let polish = create_context(&client, &base_url, "uk-swap-pl", DEFAULT_CONTEXT).await;
    let moved = create_context(&client, &base_url, "uk-swap-x", DEFAULT_CONTEXT).await;
    let slugs = |default: &str, local: &str| {
        let mut local_slug = scalar("slug", json!(local));
        local_slug["context_id"] = polish["id"].clone();
        json!([scalar("slug", json!(default)), local_slug])
    };
    let first = create_entity_with(&client, &base_url, "uk_swap", slugs("a", "b")).await;
    create_entity_with(&client, &base_url, "uk_swap", slugs("b", "a")).await;

    // X resolves a/b under default and b/a under Polish.
    client
        .put(format!(
            "{base_url}/contexts/id/{}",
            moved["id"].as_str().unwrap()
        ))
        .json(&json!({ "parent_id": polish["id"], "data": {} }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let third = create_entity_with(
        &client,
        &base_url,
        "uk_swap",
        json!([scalar("slug", json!("c"))]),
    )
    .await;
    let taken = set_scalar(&client, &base_url, &third, &moved, "slug", "b").await;
    assert_eq!(taken.status(), StatusCode::CONFLICT);
    assert_eq!(
        taken.json::<Value>().await.unwrap()["error"]["details"]["conflicting_entity_id"],
        first["id"]
    );

    server.abort();
}
