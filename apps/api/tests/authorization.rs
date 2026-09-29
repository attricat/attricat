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

/// Session capability flags come from one batched query; it must agree with
/// the per-permission `is_authorized` check for every permission and for
/// every principal shape, including scoped-only grants and inactive members.
#[sqlx::test]
async fn batched_workspace_permissions_match_per_permission_checks(pool: PgPool) {
    // Starting the server bootstraps the workspace owner.
    let (_, server) = start_server(pool.clone()).await;
    let repository = api::repository::CatalogRepository::system(pool.clone());
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let permissions: Vec<String> =
        sqlx::query_scalar("SELECT DISTINCT permission_code FROM role_permissions")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(!permissions.is_empty());
    let permission_refs = permissions.iter().map(String::as_str).collect::<Vec<_>>();

    let mut principals = vec![BOOTSTRAP_OWNER_ID.parse::<Uuid>().unwrap()];
    for (email, scope, state) in [
        ("editor@example.test", "workspace", "active"),
        ("context-only@example.test", "context_subtree", "active"),
        ("inactive@example.test", "workspace", "inactive"),
    ] {
        let user_id = Uuid::new_v4();
        let membership_id = Uuid::new_v4();
        sqlx::query("INSERT INTO users (id, email) VALUES ($1, $2)")
            .bind(user_id)
            .bind(email)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO workspace_memberships (id, workspace_id, user_id, state) VALUES ($1, $2, $3, $4)")
            .bind(membership_id)
            .bind(workspace_id)
            .bind(user_id)
            .bind(state)
            .execute(&pool)
            .await
            .unwrap();
        let target = if scope == "workspace" {
            workspace_id
        } else {
            "00000000-0000-4000-8000-000000000001".parse().unwrap()
        };
        sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, '00000000-0000-4000-8000-000000000103', $4, $5)")
            .bind(Uuid::new_v4())
            .bind(workspace_id)
            .bind(membership_id)
            .bind(scope)
            .bind(target)
            .execute(&pool)
            .await
            .unwrap();
        principals.push(user_id);
    }

    let mut granted_counts = Vec::new();
    for user_id in principals {
        let batched = repository
            .workspace_permissions(user_id, workspace_id, &permission_refs)
            .await
            .unwrap();
        for permission in &permissions {
            let expected = repository
                .is_authorized(user_id, workspace_id, permission, None, None)
                .await
                .unwrap();
            assert_eq!(
                batched.contains(permission),
                expected,
                "{user_id} {permission}"
            );
        }
        granted_counts.push(batched.len());
    }
    // Owner and workspace editor hold permissions; scoped-only and inactive
    // members hold none workspace-wide.
    assert!(
        granted_counts[0] > 0 && granted_counts[1] > 0,
        "{granted_counts:?}"
    );
    assert_eq!(&granted_counts[2..], &[0, 0]);
    server.abort();
}

/// Agent lists authorize entity-bound items in one batched query; it must
/// accept exactly the IDs the per-target `is_authorized` check accepts.
#[sqlx::test]
async fn batched_entity_authorization_matches_per_target_checks(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let owner = authenticated_client();
    let definition = |code: &str| {
        format!(
            "format_version = 1\ncode = \"{code}\"\nname = \"{code}\"\nkind = \"entity\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"name\"]\n\n[[attributes]]\ncode = \"name\"\nvalue_type = \"string\"\n"
        )
    };
    let blueprint_a = create_blueprint(&owner, &base_url, &definition("batch_a")).await;
    let blueprint_b = create_blueprint(&owner, &base_url, &definition("batch_b")).await;
    let id = |value: &Value| value.as_str().unwrap().parse::<Uuid>().unwrap();
    let entity_a = id(&create_entity(&owner, &base_url, &blueprint_a).await["id"]);
    let entity_b = id(&create_entity(&owner, &base_url, &blueprint_b).await["id"]);
    let missing = Uuid::new_v4();
    let requested = [entity_a, entity_b, missing];

    let repository = api::repository::CatalogRepository::system(pool.clone());
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let mut principals = vec![BOOTSTRAP_OWNER_ID.parse::<Uuid>().unwrap()];
    for (email, scope, target, state) in [
        ("entity-grant@example.test", "entity", entity_a, "active"),
        (
            "family-grant@example.test",
            "blueprint_family",
            id(&blueprint_b["blueprint"]["id"]),
            "active",
        ),
        (
            "context-grant@example.test",
            "context_subtree",
            "00000000-0000-4000-8000-000000000001".parse().unwrap(),
            "active",
        ),
        (
            "inactive-grant@example.test",
            "workspace",
            workspace_id,
            "inactive",
        ),
    ] {
        let user_id = Uuid::new_v4();
        let membership_id = Uuid::new_v4();
        sqlx::query("INSERT INTO users (id, email) VALUES ($1, $2)")
            .bind(user_id)
            .bind(email)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO workspace_memberships (id, workspace_id, user_id, state) VALUES ($1, $2, $3, $4)")
            .bind(membership_id)
            .bind(workspace_id)
            .bind(user_id)
            .bind(state)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, '00000000-0000-4000-8000-000000000103', $4, $5)")
            .bind(Uuid::new_v4())
            .bind(workspace_id)
            .bind(membership_id)
            .bind(scope)
            .bind(target)
            .execute(&pool)
            .await
            .unwrap();
        principals.push(user_id);
    }

    let mut accepted = Vec::new();
    for user_id in principals {
        let batched = repository
            .authorized_entity_ids(user_id, workspace_id, "entities.read", &requested)
            .await
            .unwrap();
        for entity_id in requested {
            let expected = repository
                .is_authorized(
                    user_id,
                    workspace_id,
                    "entities.read",
                    Some(entity_id),
                    None,
                )
                .await
                .unwrap();
            assert_eq!(
                batched.contains(&entity_id),
                expected,
                "{user_id} {entity_id}"
            );
        }
        accepted.push(batched);
    }
    // Workspace grants accept every ID; scoped grants only their own entity.
    assert_eq!(accepted[0].len(), 3);
    assert_eq!(accepted[1], [entity_a].into());
    assert_eq!(accepted[2], [entity_b].into());
    assert!(accepted[3].is_empty() && accepted[4].is_empty());
    server.abort();
}
