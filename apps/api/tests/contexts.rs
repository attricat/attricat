use crate::support;

use support::*;

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn omitted_parent_uses_the_workspace_default_context(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let contexts = client
        .get(format!("{base_url}/contexts"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Vec<Value>>()
        .await
        .unwrap();
    let default_id = contexts
        .iter()
        .find(|context| context["code"] == "default")
        .unwrap()["id"]
        .clone();

    let context = client
        .post(format!("{base_url}/contexts"))
        .json(&json!({ "code": "web", "data": {} }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();

    assert_eq!(context["parent_id"], default_id);
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn updating_an_unknown_context_returns_not_found(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();

    let response = client
        .put(format!("{base_url}/contexts/id/{}", Uuid::new_v4()))
        .json(&json!({
            "parent_id": "00000000-0000-4000-8000-000000000001",
            "data": {}
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "not_found"
    );

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn deleting_a_context_a_conversation_references_reports_it_in_use(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(
        &client,
        &base_url,
        "format_version: 1\ncode: context_reference\nname: Reference\nkind: record\nviews:\n  dropdown_option:\n    type: dropdown_option\n    fields: [title]\nattributes:\n  - code: title\n    value_type: string\n",
    )
    .await;
    let record = create_record(&client, &base_url, &blueprint).await;
    let context = client
        .post(format!("{base_url}/contexts"))
        .json(&json!({ "code": "referenced", "data": {} }))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let context_id: Uuid = context["id"].as_str().unwrap().parse().unwrap();
    let record_id: Uuid = record["id"].as_str().unwrap().parse().unwrap();
    sqlx::query("INSERT INTO conversations (id, workspace_id, title, record_id, context_id) VALUES ($1, $2, 'Scoped', $3, $4)")
        .bind(Uuid::new_v4())
        .bind(bootstrap_workspace_id())
        .bind(record_id)
        .bind(context_id)
        .execute(&pool)
        .await
        .unwrap();

    let response = client
        .delete(format!("{base_url}/contexts/id/{context_id}"))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "context_in_use"
    );
    server.abort();
}
