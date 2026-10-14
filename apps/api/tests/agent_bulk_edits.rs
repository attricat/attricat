mod support;

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use api::{
    agent_tools::{ToolError, execute_mutation, tool_error_payload},
    agents::AgentProviderConfig,
    bulk_edits,
    repository::{ApprovalDecision, AttricatRepository},
    storage::FakeObjectStore,
};
use axum::{Router, routing::post};
use support::*;

const PRODUCT: &str = r#"format_version = 1
code = 'bulk_product'
name = 'Product'
kind = 'record'
[views.dropdown_option]
type = 'dropdown_option'
fields = ['title']
[[attributes]]
code = 'title'
value_type = 'string'
[[attributes]]
code = 'brand'
name = 'Brand'
value_type = 'string'
[[attributes]]
code = 'season'
name = 'Season'
value_type = 'string'
[[attributes]]
code = 'state'
value_type = 'string'
value_schema = '''{"type":"string","enum":["draft","live"],"x-attricat-status":{"version":1,
  "options":[{"code":"draft","label":"Draft"},{"code":"live","label":"Live"}],
  "transitions":[{"from":null,"to":"draft"},{"from":"draft","to":"live"}]}}'''
"#;

const EDITOR_ROLE_ID: &str = "00000000-0000-4000-8000-000000000103";
const VIEWER_ROLE_ID: &str = "00000000-0000-4000-8000-000000000104";

fn owner() -> Uuid {
    BOOTSTRAP_OWNER_ID.parse().unwrap()
}

async fn scoped(pool: &PgPool) -> AttricatRepository {
    AttricatRepository::system(pool.clone())
        .for_workspace(bootstrap_workspace_id())
        .await
        .unwrap()
}

/// Creates a product titled `title` with the given brand.
async fn product(repository: &AttricatRepository, title: &str, brand: &str) -> Uuid {
    let record = execute_mutation(
        repository,
        owner(),
        "create_record",
        json!({"blueprint":{"code":"bulk_product"},"values":[
            {"kind":"scalar","attribute_code":"title","context_id":null,"value":title},
            {"kind":"scalar","attribute_code":"brand","context_id":null,"value":brand},
        ]}),
    )
    .await
    .unwrap();
    record["id"].as_str().unwrap().parse().unwrap()
}

async fn updated_at(pool: &PgPool, record: Uuid) -> String {
    let updated: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT updated_at FROM records WHERE id = $1")
            .bind(record)
            .fetch_one(pool)
            .await
            .unwrap();
    json!(updated).as_str().unwrap().to_owned()
}

async fn brand(repository: &AttricatRepository, record: Uuid) -> Value {
    let record = api::agent_tools::execute_read(
        repository,
        owner(),
        bootstrap_workspace_id(),
        "get_record",
        json!({"record_id": record}),
    )
    .await
    .unwrap();
    record["values"]
        .as_array()
        .unwrap()
        .iter()
        .find(|value| value["attribute_code"] == "brand")
        .map(|value| value["value"].clone())
        .unwrap_or(Value::Null)
}

fn coded(error: ToolError) -> String {
    tool_error_payload(&error)["code"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[sqlx::test]
async fn filters_resolve_to_pinned_records_within_the_limit_and_never_statuses(pool: PgPool) {
    let (base, server) = start_server(pool.clone()).await;
    create_blueprint(&authenticated_client(), &base, PRODUCT).await;
    let repository = scoped(&pool).await;
    let mut acme = Vec::new();
    for title in ["Boot", "Coat", "Scarf"] {
        acme.push(product(&repository, title, "Acme").await);
    }
    let other = product(&repository, "Hat", "Other").await;
    acme.sort();

    let proposal = json!({
        "filter": {"blueprint": {"code": "bulk_product"}, "attribute_filters": [{"field":"brand","operator":"eq","value":"Acme"}]},
        "values": [{"attribute_code":"brand","value":"Acme Co"},{"attribute_code":"season","value":"Spring"}],
    });
    let resolved = bulk_edits::resolve(
        &repository,
        owner(),
        bootstrap_workspace_id(),
        proposal.clone(),
        500,
    )
    .await
    .unwrap();
    let records = resolved["records"].as_array().unwrap();
    let ids: Vec<Uuid> = records
        .iter()
        .map(|record| record["record_id"].as_str().unwrap().parse().unwrap())
        .collect();
    assert_eq!(ids, acme, "only the matching records, in a stable order");
    assert!(!ids.contains(&other));
    for record in records {
        let id = record["record_id"].as_str().unwrap().parse().unwrap();
        assert_eq!(record["expected_updated_at"], updated_at(&pool, id).await);
        assert_eq!(record["values"][0]["previous"], "Acme");
        assert_eq!(record["values"][0]["value"], "Acme Co");
        assert_eq!(record["values"][1]["previous"], Value::Null);
        assert!(record["label"].is_string());
    }
    assert_eq!(
        resolved["selection"]["filter"]["blueprint"]["code"],
        "bulk_product"
    );
    assert_eq!(
        bulk_edits::change_summary(&resolved).unwrap(),
        "Set Brand, Season on 3 Product records."
    );

    // Over the workspace limit, the agent is told to narrow the selection.
    let error = bulk_edits::resolve(
        &repository,
        owner(),
        bootstrap_workspace_id(),
        proposal.clone(),
        2,
    )
    .await
    .unwrap_err();
    assert!(
        error.to_string().contains("Narrow the selection"),
        "{error}"
    );
    assert_eq!(coded(error), "bulk_edit_too_large");

    // Status attributes are never bulk edited.
    let error = bulk_edits::resolve(
        &repository,
        owner(),
        bootstrap_workspace_id(),
        json!({"items": [{"record_id": acme[0], "values": [{"attribute_code":"state","value":"draft"}]}]}),
        500,
    )
    .await
    .unwrap_err();
    assert_eq!(coded(error), "bulk_edit_status_attribute");

    // A filter that matches nothing proposes nothing.
    let error = bulk_edits::resolve(
        &repository,
        owner(),
        bootstrap_workspace_id(),
        json!({"filter": {"blueprint": {"code": "bulk_product"}, "attribute_filters": [{"field":"brand","operator":"eq","value":"Nobody"}]}, "values": [{"attribute_code":"season","value":"Spring"}]}),
        500,
    )
    .await
    .unwrap_err();
    assert_eq!(coded(error), "bulk_edit_empty");

    // A resolved list cannot be smuggled in as a proposal.
    assert!(
        bulk_edits::resolve(
            &repository,
            owner(),
            bootstrap_workspace_id(),
            resolved,
            500
        )
        .await
        .is_err()
    );
    server.abort();
}

/// An OpenAI-compatible provider that answers with `first` once, then with a
/// plain reply.
async fn scripted_provider(first: Value) -> (String, JoinHandle<()>, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route(
                "/v1/chat/completions",
                post(move || {
                    let first = first.clone();
                    let call = counted.fetch_add(1, Ordering::SeqCst);
                    async move {
                        let delta = if call == 0 {
                            json!({"tool_calls":[{"index":0,"id":"call_bulk","type":"function","function":{"name":"bulk_set_record_values","arguments":first.to_string()}}]})
                        } else {
                            json!({"content":"Done."})
                        };
                        axum::response::Response::builder()
                            .header("content-type", "text/event-stream")
                            .body(axum::body::Body::from(format!(
                                "data: {}\n\ndata: [DONE]\n\n",
                                json!({"choices":[{"delta":delta}]})
                            )))
                            .unwrap()
                    }
                }),
            ),
        )
        .await
        .unwrap();
    });
    (format!("http://{address}/v1"), server, calls)
}

struct Run {
    repository: AttricatRepository,
    client: api::agent_provider::OpenAiCompatibleClient,
    store: Arc<dyn api::storage::ObjectStore>,
    run: Uuid,
    conversation: Uuid,
}

async fn start_run(pool: &PgPool, initiator: Uuid, provider: &str) -> Run {
    let config = AgentProviderConfig::from_values(|name| match name {
        "LLM_API_KEY" => Some("test-key".to_owned()),
        "LLM_BASE_URL" => Some(provider.to_owned()),
        _ => None,
    })
    .unwrap()
    .unwrap();
    let repository = scoped(pool).await;
    let conversation = repository
        .create_conversation(Some(initiator), "bulk")
        .await
        .unwrap();
    let run = repository
        .create_agent_run_for_user(
            conversation.id,
            initiator,
            config.base_url.as_str(),
            &config.model,
        )
        .await
        .unwrap();
    let client = api::agent_provider::OpenAiCompatibleClient::new(&config).unwrap();
    let store: Arc<dyn api::storage::ObjectStore> = Arc::new(FakeObjectStore::available());
    api::agent_runner::run(&repository, &client, &store, run.id, conversation.id)
        .await
        .unwrap();
    Run {
        repository,
        client,
        store,
        run: run.id,
        conversation: conversation.id,
    }
}

impl Run {
    async fn approve(&self) -> Uuid {
        let pending = self
            .repository
            .pending_agent_tool_calls(Some(self.conversation))
            .await
            .unwrap();
        assert_eq!(pending.len(), 1, "one approval covers the whole edit");
        self.repository
            .decide_tool_call(pending[0].id, owner(), ApprovalDecision::Approve)
            .await
            .unwrap();
        api::agent_runner::resume(&self.repository, &self.client, &self.store, self.run)
            .await
            .unwrap();
        pending[0].id
    }
}

async fn system_comments(pool: &PgPool, record: Uuid) -> Vec<String> {
    sqlx::query_scalar("SELECT c.body FROM record_comments c JOIN users u ON u.id = c.author_user_id WHERE c.record_id = $1 AND u.kind = 'system'")
        .bind(record)
        .fetch_all(pool)
        .await
        .unwrap()
}

#[sqlx::test]
async fn approved_bulk_edits_apply_record_by_record_and_stop_at_a_stale_record(pool: PgPool) {
    let (base, server) = start_server(pool.clone()).await;
    create_blueprint(&authenticated_client(), &base, PRODUCT).await;
    let repository = scoped(&pool).await;
    let first = product(&repository, "Boot", "Acme").await;
    let stale = product(&repository, "Coat", "Acme").await;
    let last = product(&repository, "Scarf", "Acme").await;
    let item = |record: Uuid| json!({"record_id": record, "values": [{"attribute_code":"brand","value":"Acme Co"}]});
    let (provider, provider_server, _) =
        scripted_provider(json!({"items": [item(first), item(stale), item(last)]})).await;
    let run = start_run(&pool, owner(), &provider).await;

    let pending = repository
        .pending_agent_tool_calls(Some(run.conversation))
        .await
        .unwrap();
    assert_eq!(
        pending[0].change_summary.as_deref(),
        Some("Set Brand on 3 Product records.")
    );
    assert_eq!(pending[0].arguments["records"].as_array().unwrap().len(), 3);

    // Someone saves the second record while the edit awaits approval.
    execute_mutation(
        &repository,
        owner(),
        "set_record_values",
        json!({"record_id": stale, "values": [{"kind":"scalar","attribute_code":"season","context_id":null,"value":"Winter"}]}),
    )
    .await
    .unwrap();
    let call = run.approve().await;

    assert_eq!(brand(&repository, first).await, "Acme Co");
    assert_eq!(brand(&repository, stale).await, "Acme");
    assert_eq!(
        brand(&repository, last).await,
        "Acme",
        "not run after the stop"
    );
    let call = repository.get_agent_tool_call(call).await.unwrap();
    let result = call.result.unwrap();
    assert_eq!(result["applied"], json!([first]));
    assert_eq!(result["failed"][0]["record_id"], json!(stale));
    assert_eq!(result["failed"][0]["code"], "stale_record");
    assert_eq!(result["not_run"], json!([last]));

    let comments = system_comments(&pool, stale).await;
    assert_eq!(comments.len(), 1);
    assert!(comments[0].contains("`stale_record`"), "{}", comments[0]);
    assert!(comments[0].contains(&format!("(/agents/{})", run.conversation)));
    assert!(system_comments(&pool, first).await.is_empty());
    assert!(system_comments(&pool, last).await.is_empty());

    // One audit entry per changed record, linked to the approved call.
    let audited: Vec<(Value, Option<Uuid>, String)> = sqlx::query_as(
        "SELECT target, agent_tool_call_id, action FROM audit_events WHERE agent_tool_call_id = $1",
    )
    .bind(call.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(audited.len(), 1, "{audited:?}");
    assert_eq!(audited[0].0["id"], json!(first));
    assert_eq!(audited[0].2, "attricat.agent.bulk_set_record_values");

    let events = repository
        .agent_run_events_after(run.run, -1)
        .await
        .unwrap();
    let progress: Vec<&Value> = events
        .iter()
        .filter(|event| event.event_type == "bulk_progress")
        .map(|event| &event.payload)
        .collect();
    assert_eq!(progress.len(), 2);
    assert_eq!(progress[0]["state"], "applied");
    assert_eq!(progress[1]["state"], "failed");
    assert_eq!(progress[1]["total"], 3);
    let run_status = repository.get_agent_run(run.run).await.unwrap().status;
    assert_eq!(run_status, "completed");
    server.abort();
    provider_server.abort();
}

#[sqlx::test]
async fn proposals_over_the_workspace_limit_return_to_the_agent(pool: PgPool) {
    let (base, server) = start_server(pool.clone()).await;
    create_blueprint(&authenticated_client(), &base, PRODUCT).await;
    let repository = scoped(&pool).await;
    for title in ["Boot", "Coat", "Scarf"] {
        product(&repository, title, "Acme").await;
    }
    sqlx::query("UPDATE workspaces SET settings = jsonb_set(settings, '{agent}', '{\"bulk_edit_max_records\": 2}') WHERE id = $1")
        .bind(bootstrap_workspace_id())
        .execute(&pool)
        .await
        .unwrap();
    let (provider, provider_server, calls) = scripted_provider(json!({
        "filter": {"blueprint": {"code": "bulk_product"}},
        "values": [{"attribute_code":"season","value":"Spring"}],
    }))
    .await;
    let run = start_run(&pool, owner(), &provider).await;
    assert!(
        repository
            .pending_agent_tool_calls(Some(run.conversation))
            .await
            .unwrap()
            .is_empty(),
        "nothing awaits approval"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2, "the agent saw the error");
    let messages = repository
        .conversation_messages(run.conversation)
        .await
        .unwrap();
    let result = messages
        .iter()
        .find(|message| message.role == "tool")
        .unwrap();
    assert_eq!(result.content["result"]["code"], "bulk_edit_too_large");
    assert_eq!(
        repository.get_agent_run(run.run).await.unwrap().status,
        "completed"
    );
    server.abort();
    provider_server.abort();
}

#[sqlx::test]
async fn every_record_needs_the_initiators_write_access(pool: PgPool) {
    let (base, server) = start_server(pool.clone()).await;
    create_blueprint(&authenticated_client(), &base, PRODUCT).await;
    let repository = scoped(&pool).await;
    let writable = product(&repository, "Boot", "Acme").await;
    let readonly = product(&repository, "Coat", "Acme").await;
    // A workspace-wide viewer who may edit only the first record.
    let user = Uuid::new_v4();
    let membership = Uuid::new_v4();
    let workspace = bootstrap_workspace_id();
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, 'bulk-editor@example.test')")
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(membership)
    .bind(workspace)
    .bind(user)
    .execute(&pool)
    .await
    .unwrap();
    for (role, scope, target) in [
        (VIEWER_ROLE_ID, "workspace", workspace),
        (EDITOR_ROLE_ID, "record", writable),
    ] {
        sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, $4::uuid, $5, $6)")
            .bind(Uuid::new_v4())
            .bind(workspace)
            .bind(membership)
            .bind(role)
            .bind(scope)
            .bind(target)
            .execute(&pool)
            .await
            .unwrap();
    }
    let item = |record: Uuid| json!({"record_id": record, "values": [{"attribute_code":"brand","value":"Acme Co"}]});
    let (provider, provider_server, _) =
        scripted_provider(json!({"items": [item(writable), item(readonly)]})).await;
    let run = start_run(&pool, user, &provider).await;
    let call = run.approve().await;

    let result = repository
        .get_agent_tool_call(call)
        .await
        .unwrap()
        .result
        .unwrap();
    assert_eq!(result["applied"], json!([writable]));
    assert_eq!(result["failed"][0]["record_id"], json!(readonly));
    assert_eq!(result["failed"][0]["code"], "forbidden");
    assert_eq!(result["not_run"], json!([]));
    assert_eq!(brand(&repository, writable).await, "Acme Co");
    assert_eq!(brand(&repository, readonly).await, "Acme");
    assert_eq!(system_comments(&pool, readonly).await.len(), 1);
    server.abort();
    provider_server.abort();
}
