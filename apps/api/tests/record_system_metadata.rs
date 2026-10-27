use crate::support;

use api::repository::MAX_SYSTEM_TAG_FILTERS;
use support::*;

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn system_annotations_are_persisted_updated_and_searchable(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(
        &client,
        &base_url,
        "format_version = 1\ncode = 'annotated_product'\nname = 'Annotated product'\nkind = 'record'\n\n[views.dropdown_option]\ntype = 'dropdown_option'\nfields = ['title']\n\n[[attributes]]\ncode = 'title'\nvalue_type = 'string'",
    )
    .await;

    let record: Value = client
        .post(format!("{base_url}/v1/records"))
        .json(&json!({
            "blueprint": { "code": "annotated_product" },
            "system_tags": ["agent:pending", "debug"],
            "system_metadata": { "run_id": "run-42", "attempt": 1 }
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let record_id = record["id"].as_str().unwrap();
    assert_eq!(record["system_tags"], json!(["agent:pending", "debug"]));
    assert_eq!(record["system_metadata"]["run_id"], "run-42");

    let search: Value = client
        .post(format!("{base_url}/v1/records/search"))
        .json(&json!({
            "blueprint": { "code": "annotated_product" },
            "filters": [],
            "system_tags": ["agent:pending", "debug"],
            "page": { "size": 10 }
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(search["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        search["items"][0]["system_tags"],
        json!(["agent:pending", "debug"])
    );

    let updated: Value = client
        .put(format!("{base_url}/v1/records/{record_id}"))
        .json(&json!({ "system_metadata": { "run_id": "run-42", "result": "ok" } }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(updated["system_tags"], json!(["agent:pending", "debug"]));
    assert_eq!(updated["system_metadata"]["result"], "ok");

    let updated: Value = client
        .put(format!("{base_url}/v1/records/{record_id}"))
        .json(&json!({ "system_tags": ["agent:complete"] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(updated["system_tags"], json!(["agent:complete"]));
    assert_eq!(updated["system_metadata"]["result"], "ok");

    let search: Value = client
        .post(format!("{base_url}/v1/records/search"))
        .json(&json!({
            "blueprint": { "code": "annotated_product" },
            "filters": [],
            "system_tags": ["agent:complete"],
            "page": { "size": 10 }
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(search["items"].as_array().unwrap().len(), 1);
    assert_eq!(search["items"][0]["id"], record["id"]);

    let cleared: Value = client
        .put(format!("{base_url}/v1/records/{record_id}"))
        .json(&json!({ "system_tags": [], "system_metadata": {} }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(cleared["system_tags"], json!([]));
    assert_eq!(cleared["system_metadata"], json!({}));

    // A search filters by as many tags as the Explorer and saved searches
    // hold, and no more.
    let tags = |count: usize| (0..count).map(|i| format!("tag-{i}")).collect::<Vec<_>>();
    let search_tags = |tags: Vec<String>| {
        client
            .post(format!("{base_url}/v1/records/search"))
            .json(&json!({ "blueprint": { "code": "annotated_product" }, "system_tags": tags }))
            .send()
    };
    let allowed = search_tags(tags(MAX_SYSTEM_TAG_FILTERS)).await.unwrap();
    assert_eq!(allowed.status(), StatusCode::OK);
    let refused = search_tags(tags(MAX_SYSTEM_TAG_FILTERS + 1)).await.unwrap();
    assert_eq!(refused.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let error: Value = refused.json().await.unwrap();
    assert_eq!(error["error"]["code"], "invalid_input");

    assert_eq!(blueprint["blueprint"]["code"], "annotated_product");
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn system_tag_facet_counts_tags_most_used_first(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    for code in ["tagged_product", "other_product"] {
        create_blueprint(
            &client,
            &base_url,
            &format!("format_version = 1\ncode = '{code}'\nname = '{code}'\nkind = 'record'\n\n[views.dropdown_option]\ntype = 'dropdown_option'\nfields = ['title']\n\n[[attributes]]\ncode = 'title'\nvalue_type = 'string'"),
        )
        .await;
    }
    let create = |blueprint: &'static str, tags: Value| {
        let client = client.clone();
        let base_url = base_url.clone();
        async move {
            let record: Value = client
                .post(format!("{base_url}/v1/records"))
                .json(&json!({ "blueprint": { "code": blueprint }, "system_tags": tags }))
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap()
                .json()
                .await
                .unwrap();
            record["id"].as_str().unwrap().parse::<Uuid>().unwrap()
        }
    };
    create("tagged_product", json!(["Needs-Review", "imported"])).await;
    create("tagged_product", json!(["Needs-Review"])).await;
    let deleted = create("tagged_product", json!(["Needs-Review", "stale"])).await;
    let sample = create("tagged_product", json!(["imported"])).await;
    create("other_product", json!(["elsewhere"])).await;
    client
        .delete(format!("{base_url}/records/{deleted}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    // Only the host marks sample data, so set the marker directly.
    sqlx::query("UPDATE records SET system_tags = array_append(system_tags, 'attricat.sample') WHERE id = $1")
        .bind(sample)
        .execute(&pool)
        .await
        .unwrap();
    let facet = |body: Value| {
        let client = client.clone();
        let base_url = base_url.clone();
        async move {
            client
                .post(format!("{base_url}/v1/records/facets/system-tags"))
                .json(&body)
                .send()
                .await
                .unwrap()
        }
    };

    let all: Value = facet(json!({ "blueprint": { "code": "tagged_product" } }))
        .await
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        all,
        json!({
            "items": [
                { "tag": "Needs-Review", "count": 2 },
                { "tag": "imported", "count": 2 }
            ],
            "truncated": false
        })
    );

    let limited: Value = facet(json!({ "blueprint": { "code": "tagged_product" }, "limit": 1 }))
        .await
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        limited["items"],
        json!([{ "tag": "Needs-Review", "count": 2 }])
    );
    assert_eq!(limited["truncated"], true);

    let prefixed: Value =
        facet(json!({ "blueprint": { "code": "tagged_product" }, "prefix": " needs" }))
            .await
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
    assert_eq!(
        prefixed["items"],
        json!([{ "tag": "Needs-Review", "count": 2 }])
    );

    for body in [
        json!({ "blueprint": { "code": "tagged_product" }, "limit": 0 }),
        json!({ "blueprint": { "code": "tagged_product" }, "limit": 101 }),
        json!({ "blueprint": { "code": "tagged_product" }, "prefix": "x".repeat(129) }),
    ] {
        let response = facet(body).await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let error: Value = response.json().await.unwrap();
        assert_eq!(error["error"]["code"], "invalid_input");
    }
    assert_eq!(
        facet(json!({ "blueprint": { "code": "missing" } }))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn system_annotations_require_an_object_and_unique_tags(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(
        &client,
        &base_url,
        "format_version = 1\ncode = 'system_annotation_validation'\nname = 'System annotation validation'\nkind = 'record'\n\n[views.dropdown_option]\ntype = 'dropdown_option'\nfields = ['title']\n\n[[attributes]]\ncode = 'title'\nvalue_type = 'string'", 
    )
    .await;

    let response = client
        .post(format!("{base_url}/v1/records"))
        .json(&json!({
            "blueprint": { "code": "system_annotation_validation" },
            "system_tags": ["duplicate", "duplicate"]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], "invalid_system_tags");

    let response = client
        .post(format!("{base_url}/v1/records"))
        .json(&json!({
            "blueprint": { "code": "system_annotation_validation" },
            "system_metadata": []
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], "invalid_system_metadata");

    server.abort();
}
