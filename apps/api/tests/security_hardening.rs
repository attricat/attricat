mod support;

use api::{
    agent_tools::{ToolError, execute_read},
    agents::AgentProviderConfig,
    repository::{AttricatRepository, RepositoryError},
};
use attricat_agent_runtime::agent_runner::mutation_authorized;
use chrono::Utc;
use support::*;

async fn token(owner: &Client, base: &str, permissions: &[&str]) -> Value {
    owner
        .post(format!("{base}/personal-access-tokens"))
        .json(&json!({"label":"hardening", "permissions":permissions}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}

fn bearer(token: &Value) -> Client {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "authorization",
        format!("Bearer {}", token["secret"].as_str().unwrap())
            .parse()
            .unwrap(),
    );
    Client::builder().default_headers(headers).build().unwrap()
}

#[sqlx::test]
async fn token_cannot_launder_owner_authority_through_roles_or_invitations(pool: PgPool) {
    let (base, server) = start_server(pool.clone()).await;
    let owner = authenticated_client();
    let token = token(
        &owner,
        &base,
        &[
            "roles.manage",
            "roles.grant",
            "members.manage",
            "records.read",
        ],
    )
    .await;
    let limited = bearer(&token);
    let (_, member) = add_workspace_user(&pool).await;
    let reader = create_role(&pool, "hardening-reader", &["records.read"]).await;
    let role_payload = json!({"role_id":OWNER_ROLE_ID, "scope_type":"workspace", "scope_target_id":BOOTSTRAP_WORKSPACE_ID});
    let expiry = Utc::now() + chrono::Duration::hours(1);
    let requests = [
        limited.post(format!("{base}/workspace/roles")).json(&json!({"code":"amplified", "permissions":["records.write"]})),
        limited.put(format!("{base}/workspace/roles/{reader}")).json(&json!({"code":"hardening-reader", "permissions":["records.write"]})),
        limited.post(format!("{base}/workspace/roles/{OWNER_ROLE_ID}/duplicate")).json(&json!({"code":"owner-copy"})),
        limited.post(format!("{base}/workspace/members/{member}/grants")).json(&role_payload),
        limited.post(format!("{base}/workspace/members/{member}/transfer-ownership")),
        limited.post(format!("{base}/workspace/invitations")).json(&json!({"email":"invite@example.test", "role_id":OWNER_ROLE_ID, "scope_type":"workspace", "scope_target_id":BOOTSTRAP_WORKSPACE_ID, "expires_at":expiry})),
        limited.post(format!("{base}/workspace/users")).json(&json!({"email":"new@example.test", "role_id":OWNER_ROLE_ID, "scope_type":"workspace", "scope_target_id":BOOTSTRAP_WORKSPACE_ID, "expires_at":expiry})),
        limited.post(format!("{base}/workspace/roles/{reader}/retire")).json(&json!({"replacement_role_id":OWNER_ROLE_ID})),
    ];
    for request in requests {
        let response = request.send().await.unwrap();
        let status = response.status();
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "{}",
            response.text().await.unwrap()
        );
    }
    // Narrow delegation remains supported; requiring every owner permission
    // would close the hole by unnecessarily disabling legitimate PAT use.
    let response = limited.post(format!("{base}/workspace/members/{member}/grants"))
        .json(&json!({"role_id":reader, "scope_type":"workspace", "scope_target_id":BOOTSTRAP_WORKSPACE_ID}))
        .send().await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "{}",
        response.text().await.unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM users WHERE email='new@example.test'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    server.abort();
}

#[sqlx::test]
async fn role_cannot_retire_into_itself_and_delete_its_grants(pool: PgPool) {
    let (_, server) = start_server(pool.clone()).await;
    let repository = AttricatRepository::system(pool.clone());
    let role = create_role(&pool, "self-replacement", &["records.read"]).await;
    let (_, member) = add_workspace_user(&pool).await;
    let grant = grant_role(&pool, member, role, GrantScope::Workspace).await;
    assert!(
        repository
            .retire_workspace_role(
                BOOTSTRAP_OWNER_ID.parse().unwrap(),
                bootstrap_workspace_id(),
                role,
                Some(role)
            )
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, Uuid>("SELECT role_id FROM role_grants WHERE id=$1")
            .bind(grant)
            .fetch_one(&pool)
            .await
            .unwrap(),
        role
    );
    server.abort();
}

#[sqlx::test]
async fn queued_agent_runs_retain_and_enforce_the_initiating_token(pool: PgPool) {
    let (base, server) = start_server_with_config(pool.clone(), |state| {
        state.agent_provider = AgentProviderConfig::from_values(|key| match key {
            "LLM_API_KEY" => Some("test-only".into()),
            "LLM_BASE_URL" => Some("http://127.0.0.1:1/v1".into()),
            _ => None,
        })
        .unwrap();
    })
    .await;
    let owner = authenticated_client();
    let token = token(&owner, &base, &["agents.run", "blueprints.read"]).await;
    let limited = bearer(&token);
    let blueprint = create_blueprint(&owner, &base,
        "format_version = 1\ncode = 'token_private'\nname = 'Private'\nkind = 'record'\n\n[views.dropdown_option]\ntype = 'dropdown_option'\nfields = ['title']\n\n[[attributes]]\ncode = 'title'\nvalue_type = 'string'").await;
    let record = create_record(&owner, &base, &blueprint).await;
    let anchored = json!({"title":"Private record", "record_id":record["id"]});
    let private: Value = owner
        .post(format!("{base}/agent/conversations"))
        .json(&anchored)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        limited
            .post(format!("{base}/agent/conversations"))
            .json(&anchored)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        limited
            .get(format!(
                "{base}/agent/conversations/{}",
                private["id"].as_str().unwrap()
            ))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(limited.post(format!("{base}/agent/smart-fill"))
        .json(&json!({"record_id":record["id"], "is_default_context":true, "content":"Read private values"}))
        .send().await.unwrap().status(), StatusCode::FORBIDDEN);
    let conversation: Value = limited
        .post(format!("{base}/agent/conversations"))
        .json(&json!({"title":"Token authority"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let run: Value = limited
        .post(format!(
            "{base}/agent/conversations/{}/messages",
            conversation["id"].as_str().unwrap()
        ))
        .json(&json!({"content":"Read the catalog"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let repository = AttricatRepository::new(pool.clone(), bootstrap_workspace_id());
    let actor = repository
        .agent_run_actor(run["id"].as_str().unwrap().parse().unwrap())
        .await
        .unwrap();
    assert_eq!(
        actor.token_id,
        Some(token["id"].as_str().unwrap().parse().unwrap())
    );
    let repository = repository.with_authorization_actor(actor);
    assert!(
        execute_read(
            &repository,
            actor.user_id,
            bootstrap_workspace_id(),
            "list_blueprints",
            json!({})
        )
        .await
        .is_ok()
    );
    for (name, arguments) in [
        ("list_contexts", json!({})),
        ("get_record_labels", json!({"record_ids":[]})),
        ("read_file", json!({"file_id":Uuid::new_v4()})),
    ] {
        assert!(
            matches!(
                execute_read(
                    &repository,
                    actor.user_id,
                    bootstrap_workspace_id(),
                    name,
                    arguments
                )
                .await,
                Err(ToolError::Forbidden)
            ),
            "{name}"
        );
    }
    assert!(
        !mutation_authorized(
            &repository,
            actor.user_id,
            bootstrap_workspace_id(),
            "create_blueprint",
            &json!({})
        )
        .await
        .unwrap()
    );
    owner
        .delete(format!(
            "{base}/personal-access-tokens/{}",
            token["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert!(matches!(
        execute_read(
            &repository,
            actor.user_id,
            bootstrap_workspace_id(),
            "list_blueprints",
            json!({})
        )
        .await,
        Err(ToolError::Forbidden)
    ));
    // A fresh worker repository has no in-memory request actor. Exercise the
    // actual provider/tool loop to prove it reloads the persisted credential.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let round = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let provider_server = tokio::spawn(async move {
        axum::serve(listener, axum::Router::new().route("/v1/chat/completions", axum::routing::post(move || {
            let first = round.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0;
            async move {
                let delta = if first {
                    json!({"tool_calls":[{"index":0,"id":"call-hidden","type":"function","function":{"name":"list_blueprints","arguments":"{}"}}]})
                } else { json!({"content":"Done"}) };
                ([("content-type", "text/event-stream")], format!("data: {}\n\ndata: [DONE]\n\n", json!({"choices":[{"delta":delta}]})))
            }
        }))).await.unwrap();
    });
    let config = AgentProviderConfig::from_values(|key| match key {
        "LLM_API_KEY" => Some("test-only".into()),
        "LLM_BASE_URL" => Some(format!("http://{address}/v1")),
        _ => None,
    })
    .unwrap()
    .unwrap();
    let provider = api::agent_provider::OpenAiCompatibleClient::new(&config).unwrap();
    let store: std::sync::Arc<dyn api::storage::ObjectStore> =
        std::sync::Arc::new(api::storage::FakeObjectStore::available());
    let run_id = run["id"].as_str().unwrap().parse().unwrap();
    api::agent_runner::run(
        &AttricatRepository::new(pool.clone(), bootstrap_workspace_id()),
        &provider,
        &store,
        run_id,
        conversation["id"].as_str().unwrap().parse().unwrap(),
    )
    .await
    .unwrap();
    let error: Value = sqlx::query_scalar("SELECT error FROM agent_tool_calls WHERE run_id=$1")
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(error["code"], "forbidden");
    // Legacy rows cannot safely interpret a missing token as session authority.
    sqlx::query(
        "UPDATE agent_runs SET authority_recorded=false, initiated_by_token_id=NULL WHERE id=$1",
    )
    .bind(run_id)
    .execute(&pool)
    .await
    .unwrap();
    assert!(matches!(
        repository.agent_run_actor(run_id).await,
        Err(RepositoryError::InvalidAgentState(_))
    ));
    provider_server.abort();
    server.abort();
}

#[sqlx::test]
async fn approving_another_users_agent_change_requires_the_mutation_permission(pool: PgPool) {
    let (base, server) = start_server_with_config(pool.clone(), |state| {
        state.agent_provider = AgentProviderConfig::from_values(|key| {
            (key == "LLM_API_KEY").then(|| "test-only".into())
        })
        .unwrap();
    })
    .await;
    let owner = authenticated_client();
    let token = token(&owner, &base, &["agents.run"]).await;
    let agent_only = create_role(&pool, "agent-only", &["agents.run"]).await;
    let user = member_with_role(&pool, agent_only).await;
    let repository = AttricatRepository::new(pool.clone(), bootstrap_workspace_id());
    let conversation = repository
        .create_conversation(
            Some(BOOTSTRAP_OWNER_ID.parse().unwrap()),
            "Shared conversation",
        )
        .await
        .unwrap();
    let run = repository
        .create_agent_run_for_user(
            conversation.id,
            BOOTSTRAP_OWNER_ID.parse().unwrap(),
            "http://127.0.0.1:1/v1",
            "test",
        )
        .await
        .unwrap();
    repository
        .transition_agent_run(run.id, "running", None, None)
        .await
        .unwrap();
    let call = repository
        .create_agent_tool_call(
            run.id,
            Some("call-1"),
            "create_blueprint",
            json!({"definition":"unused"}),
            Some("Create blueprint"),
            "pending_approval",
        )
        .await
        .unwrap();
    repository
        .transition_agent_run(run.id, "awaiting_approval", None, None)
        .await
        .unwrap();
    let url = format!("{base}/agent/tool-calls/{}/approve", call.id);
    for client in [bearer(&token), client_for(user)] {
        assert_eq!(
            client.post(&url).send().await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        repository.get_agent_tool_call(call.id).await.unwrap().state,
        "pending_approval"
    );
    assert_eq!(
        owner.post(&url).send().await.unwrap().status(),
        StatusCode::OK
    );
    server.abort();
}

#[sqlx::test]
async fn relationship_hydration_does_not_expose_unreadable_records(pool: PgPool) {
    let (base, server) = start_server(pool.clone()).await;
    let owner = authenticated_client();
    let blueprint = create_blueprint(&owner, &base,
        "format_version = 1\ncode = 'scoped_links'\nname = 'Links'\nkind = 'record'\n\n[views.dropdown_option]\ntype = 'dropdown_option'\nfields = ['title']\n\n[[attributes]]\ncode = 'title'\nvalue_type = 'string'\n\n[[attributes]]\ncode = 'related'\nvalue_type = 'relationship'\ntarget_blueprint = 'scoped_links'").await;
    let root = create_record(&owner, &base, &blueprint).await;
    let hidden = create_record(&owner, &base, &blueprint).await;
    let visible = create_record(&owner, &base, &blueprint).await;
    for (record, title, targets) in [
        (&root, "Root", json!([hidden["id"], visible["id"]])),
        (&hidden, "CONFIDENTIAL", json!([root["id"]])),
        (&visible, "Visible", json!([root["id"]])),
    ] {
        owner.put(format!("{base}/v1/records/{}", record["id"].as_str().unwrap()))
            .json(&json!({"values":[scalar("title", title)], "relationships":[{"attribute_code":"related", "target_record_ids":targets}], "remove_values":[]}))
            .send().await.unwrap().error_for_status().unwrap();
    }
    let (user, membership) = add_workspace_user(&pool).await;
    for record in [&root, &visible] {
        grant_role(
            &pool,
            membership,
            VIEWER_ROLE_ID,
            GrantScope::Record(record["id"].as_str().unwrap().parse().unwrap()),
        )
        .await;
    }
    let scoped = client_for(user);
    let root_id = root["id"].as_str().unwrap();
    let context: Uuid = sqlx::query_scalar(
        "SELECT id FROM attribute_contexts WHERE workspace_id=$1 AND code='default'",
    )
    .bind(bootstrap_workspace_id())
    .fetch_one(&pool)
    .await
    .unwrap();
    for suffix in [
        "preview?relationship_depth=2".to_owned(),
        format!("resolved-preview?context_id={context}"),
        format!("hierarchy?context_id={context}&field=related"),
    ] {
        let path = format!("{base}/records/{root_id}/{suffix}");
        let full = owner
            .get(&path)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(
            full.contains("CONFIDENTIAL"),
            "{suffix}: fixture must expose hidden data to owner"
        );
        let filtered = scoped
            .get(&path)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(!filtered.contains("CONFIDENTIAL"), "{suffix}");
        assert!(filtered.contains("Visible"), "{suffix}");
    }
    let path = format!("{base}/v1/records/{root_id}/incoming-relationships");
    let mut cursor = Value::Null;
    let mut ids = Vec::new();
    loop {
        let page: Value = scoped.post(&path).json(&json!({"relationships":[{"source_blueprint":"scoped_links", "field":"related"}], "page":{"size":1, "cursor":cursor}}))
            .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
        assert!(!page.to_string().contains("CONFIDENTIAL"));
        ids.extend(
            page["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item["id"].clone()),
        );
        cursor = page["next_cursor"].clone();
        if cursor.is_null() {
            break;
        }
    }
    assert_eq!(ids, vec![visible["id"].clone()]);
    let repository = AttricatRepository::new(pool, bootstrap_workspace_id());
    let preview = execute_read(
        &repository,
        user,
        bootstrap_workspace_id(),
        "get_record_context_preview",
        json!({"record_id":root["id"], "context_id":context}),
    )
    .await
    .unwrap();
    assert!(!preview.to_string().contains("CONFIDENTIAL"));
    server.abort();
}

#[sqlx::test]
async fn relationship_previews_have_a_global_expansion_budget(pool: PgPool) {
    let (base, server) = start_server(pool.clone()).await;
    let owner = authenticated_client();
    let blueprint = create_blueprint(&owner, &base,
        "format_version = 1\ncode = 'bounded_links'\nname = 'Links'\nkind = 'record'\n\n[views.dropdown_option]\ntype = 'dropdown_option'\nfields = ['title']\n\n[[attributes]]\ncode = 'title'\nvalue_type = 'string'\n\n[[attributes]]\ncode = 'related'\nvalue_type = 'relationship'\ntarget_blueprint = 'bounded_links'").await;
    let root = create_record(&owner, &base, &blueprint).await;
    let root_id: Uuid = root["id"].as_str().unwrap().parse().unwrap();
    let blueprint_id: Uuid = blueprint["blueprint"]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let workspace = bootstrap_workspace_id();
    let targets: Vec<Uuid> = (0..4097).map(|_| Uuid::new_v4()).collect();
    sqlx::query("INSERT INTO records (id, workspace_id, blueprint_id, blueprint_version, projections) SELECT id, $2, $3, 1, $4 FROM unnest($1::uuid[]) id")
        .bind(&targets).bind(workspace).bind(blueprint_id).bind(json!({"preview":{"default":{}}})).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO attribute_values (id, workspace_id, record_id, attribute_id, context_id, active, relationship_target_record_id) SELECT gen_random_uuid(), $2, $3, a.id, c.id, true, target FROM unnest($1::uuid[]) target CROSS JOIN attributes a CROSS JOIN attribute_contexts c WHERE a.blueprint_id=$4 AND a.blueprint_version=1 AND a.code='related' AND c.workspace_id=$2 AND c.code='default'")
        .bind(&targets).bind(workspace).bind(root_id).bind(blueprint_id).execute(&pool).await.unwrap();
    let repository = AttricatRepository::new(pool, workspace);
    assert!(repository.preview(root_id, 1, 10).await.is_ok());
    assert!(matches!(
        repository.preview(root_id, 1, 5000).await,
        Err(RepositoryError::PreviewExpansionLimit)
    ));
    server.abort();
}

#[sqlx::test]
async fn login_attempt_budget_is_shared_across_workspaces(pool: PgPool) {
    let (base, server) = start_server(pool.clone()).await;
    sqlx::query("INSERT INTO workspaces (id, slug, name, login_identifier) VALUES ($1, 'other', 'Other', 'other.local')")
        .bind(Uuid::new_v4()).execute(&pool).await.unwrap();
    let client = Client::new();
    for _ in 0..5 {
        assert_eq!(client.post(format!("{base}/auth/login"))
            .json(&json!({"login_identifier":"default.local", "email":"victim@example.test", "password":"wrong"}))
            .send().await.unwrap().status(), StatusCode::UNAUTHORIZED);
    }
    assert_eq!(client.post(format!("{base}/auth/login"))
        .json(&json!({"login_identifier":"other.local", "email":" VICTIM@example.test ", "password":"wrong"}))
        .send().await.unwrap().status(), StatusCode::TOO_MANY_REQUESTS);
    server.abort();
}

#[sqlx::test]
async fn concurrent_ownership_transfers_recheck_the_owner_after_locking(pool: PgPool) {
    let (_, server) = start_server(pool.clone()).await;
    let (_, first) = add_workspace_user(&pool).await;
    let (_, second) = add_workspace_user(&pool).await;
    let repository = AttricatRepository::system(pool.clone());
    let owner = BOOTSTRAP_OWNER_ID.parse().unwrap();
    let workspace = bootstrap_workspace_id();
    let (left, right) = tokio::join!(
        repository.transfer_workspace_ownership(owner, workspace, first),
        repository.transfer_workspace_ownership(owner, workspace, second),
    );
    assert_ne!(
        left.is_ok(),
        right.is_ok(),
        "exactly one transfer can use the old owner's authority"
    );
    let failed = if left.is_err() { left } else { right };
    assert!(matches!(
        failed,
        Err(RepositoryError::NotFound("owner authority"))
    ));
    server.abort();
}
