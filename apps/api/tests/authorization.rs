mod support;

use support::*;

#[sqlx::test]
async fn catalog_routes_distinguish_missing_identity_from_missing_permission(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;

    let unauthenticated = Client::new()
        .get(format!("{base_url}/blueprints"))
        .send()
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        unauthenticated.json::<Value>().await.unwrap()["error"]["code"],
        "unauthenticated"
    );

    let unknown_principal = Client::new()
        .get(format!("{base_url}/blueprints"))
        .header("x-catalog-user-id", Uuid::new_v4().to_string())
        .header("x-catalog-workspace-id", BOOTSTRAP_WORKSPACE_ID)
        .send()
        .await
        .unwrap();
    assert_eq!(unknown_principal.status(), StatusCode::UNAUTHORIZED);

    let wrong_workspace = Client::new()
        .get(format!("{base_url}/blueprints"))
        .header("x-catalog-user-id", BOOTSTRAP_OWNER_ID)
        .header("x-catalog-workspace-id", Uuid::new_v4().to_string())
        .send()
        .await
        .unwrap();
    assert_eq!(wrong_workspace.status(), StatusCode::FORBIDDEN);

    let viewer_id = Uuid::new_v4();
    let membership_id = Uuid::new_v4();
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, 'viewer-auth@example.test')")
        .bind(viewer_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(membership_id)
    .bind(workspace_id)
    .bind(viewer_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, '00000000-0000-4000-8000-000000000104', 'workspace', $2)")
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(membership_id)
        .execute(&pool)
        .await
        .unwrap();

    let forbidden = Client::new()
        .post(format!("{base_url}/blueprints"))
        .header("x-catalog-user-id", viewer_id.to_string())
        .header("x-catalog-workspace-id", workspace_id.to_string())
        .json(&json!({ "definition": "kind = \"entity\"\ncode = \"forbidden\"" }))
        .send()
        .await
        .unwrap();
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        forbidden.json::<Value>().await.unwrap()["error"]["code"],
        "forbidden"
    );

    let health = Client::new()
        .get(format!("{base_url}/health"))
        .send()
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);
    server.abort();
}

#[sqlx::test]
async fn context_subtree_grants_allow_descendants_but_not_siblings(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let owner = authenticated_client();
    let root = owner
        .post(format!("{base_url}/contexts"))
        .json(&json!({ "code": "scoped-root", "data": {}, "parent_id": "00000000-0000-4000-8000-000000000001" }))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let child = owner
        .post(format!("{base_url}/contexts"))
        .json(&json!({ "code": "scoped-child", "data": {}, "parent_id": root["id"] }))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let grandchild = owner
        .post(format!("{base_url}/contexts"))
        .json(&json!({ "code": "scoped-grandchild", "data": {}, "parent_id": child["id"] }))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let sibling = owner
        .post(format!("{base_url}/contexts"))
        .json(&json!({ "code": "scoped-sibling", "data": {}, "parent_id": "00000000-0000-4000-8000-000000000001" }))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();

    let editor_id = Uuid::new_v4();
    let membership_id = Uuid::new_v4();
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, 'scoped-editor@example.test')")
        .bind(editor_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(membership_id)
    .bind(workspace_id)
    .bind(editor_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, '00000000-0000-4000-8000-000000000103', 'context_subtree', $4)")
        .bind(Uuid::new_v4()).bind(workspace_id).bind(membership_id).bind(child["id"].as_str().unwrap().parse::<Uuid>().unwrap()).execute(&pool).await.unwrap();

    let editor = Client::builder()
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                "x-catalog-user-id",
                reqwest::header::HeaderValue::from_str(&editor_id.to_string()).unwrap(),
            );
            headers.insert(
                "x-catalog-workspace-id",
                reqwest::header::HeaderValue::from_static(BOOTSTRAP_WORKSPACE_ID),
            );
            headers
        })
        .build()
        .unwrap();
    assert_eq!(
        editor
            .get(format!("{base_url}/contexts/scoped-child"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        editor
            .get(format!("{base_url}/contexts/scoped-grandchild"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        editor
            .get(format!("{base_url}/contexts/scoped-root"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        editor
            .get(format!("{base_url}/contexts/scoped-sibling"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let contexts = editor
        .get(format!("{base_url}/contexts"))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(contexts.as_array().unwrap().len(), 2);
    assert!(contexts.to_string().contains("scoped-child"));
    assert!(contexts.to_string().contains("scoped-grandchild"));
    assert!(sibling["id"].is_string() && grandchild["id"].is_string());
    server.abort();
}
