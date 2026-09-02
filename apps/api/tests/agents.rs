mod support;

use std::{sync::Arc, time::Duration};

use api::{
    agent_tools::{ToolError, execute_read},
    agent_worker,
    agents::AgentProviderConfig,
    file_worker::{FileWorker, WorkerConfig},
    repository::CatalogRepository,
    storage::FakeObjectStore,
};
use axum::{Router, routing::post};
use reqwest::multipart::{Form, Part};
use support::*;
use tokio::time::{sleep, timeout};

#[sqlx::test]
async fn agent_conversation_reads_and_persisted_sse_replay(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let conversation: Value = client
        .post(format!("{base_url}/agent/conversations"))
        .json(&json!({"title": "nightly catalog review"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let conversation_id = conversation["id"].as_str().unwrap();
    let conversations: Value = client
        .get(format!("{base_url}/agent/conversations"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(conversations[0]["id"], conversation["id"]);

    let message_id = Uuid::new_v4();
    let file_id = Uuid::new_v4();
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    sqlx::query("INSERT INTO files (id, workspace_id, original_filename, display_filename, mime_type, byte_size, sha256, original_key, status) VALUES ($1, $2, 'report.pdf', 'report.pdf', 'application/pdf', 7, $3, 'files/report.pdf', 'ready')")
        .bind(file_id).bind(workspace_id).bind("0".repeat(64)).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO conversation_messages (id, conversation_id, sequence, role, content) VALUES ($1, $2, 0, 'user', '\"Review this report\"')")
        .bind(message_id).bind(conversation_id.parse::<Uuid>().unwrap()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO conversation_message_attachments (id, workspace_id, message_id, file_id, position) VALUES ($1, $2, $3, $4, 0)")
        .bind(Uuid::new_v4()).bind(workspace_id).bind(message_id).bind(file_id).execute(&pool).await.unwrap();
    let messages: Value = client
        .get(format!(
            "{base_url}/agent/conversations/{conversation_id}/messages"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(messages[0]["attachments"][0]["id"], file_id.to_string());
    assert_eq!(messages[0]["attachments"][0]["filename"], "report.pdf");

    let run_id = Uuid::new_v4();
    sqlx::query("INSERT INTO agent_runs (id, workspace_id, conversation_id, origin, status, provider_base_url, model, finished_at) VALUES ($1, $2, $3, 'manual', 'skipped', 'https://provider.test/v1', 'test', now())")
        .bind(run_id).bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap()).bind(conversation_id.parse::<Uuid>().unwrap()).execute(&pool).await.unwrap();
    let event_id = Uuid::new_v4();
    sqlx::query("INSERT INTO agent_run_events (id, run_id, sequence, event_type, payload) VALUES ($1, $2, 0, 'terminal', '{\"status\":\"skipped\"}')")
        .bind(event_id).bind(run_id).execute(&pool).await.unwrap();
    let response = client
        .get(format!("{base_url}/agent/runs/{run_id}/events"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let replay = response.text().await.unwrap();
    assert!(replay.contains(&format!("id: {event_id}")));
    assert!(replay.contains("event: terminal"));
    server.abort();
}

#[sqlx::test]
async fn reconciliation_retains_conversation_attachments(pool: PgPool) {
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let conversation_id = Uuid::new_v4();
    let message_id = Uuid::new_v4();
    let attached_file_id = Uuid::new_v4();
    let orphan_file_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO conversations (id, workspace_id, title) VALUES ($1, $2, 'Attachment retention')",
    )
    .bind(conversation_id)
    .bind(workspace_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO conversation_messages (id, conversation_id, sequence, role, content) VALUES ($1, $2, 0, 'user', '\"Keep this file\"')")
        .bind(message_id)
        .bind(conversation_id)
        .execute(&pool)
        .await
        .unwrap();
    for file_id in [attached_file_id, orphan_file_id] {
        sqlx::query("INSERT INTO files (id, workspace_id, original_filename, display_filename, mime_type, byte_size, sha256, original_key, status) VALUES ($1, $2, 'report.pdf', 'report.pdf', 'application/pdf', 7, $3, 'files/report.pdf', 'ready')")
            .bind(file_id)
            .bind(workspace_id)
            .bind("0".repeat(64))
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO conversation_message_attachments (id, workspace_id, message_id, file_id, position) VALUES ($1, $2, $3, $4, 0)")
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(message_id)
        .bind(attached_file_id)
        .execute(&pool)
        .await
        .unwrap();

    let worker = FileWorker::new(
        pool.clone(),
        Arc::new(FakeObjectStore::available()),
        WorkerConfig {
            worker_id: "test-worker".into(),
            max_pixels: 1,
            max_attempts: 1,
            delete_grace: Duration::from_secs(60),
        },
    );
    worker.reconcile().await.unwrap();

    let attached_deleted_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT deleted_at FROM files WHERE id = $1")
            .bind(attached_file_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let orphan_deleted_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT deleted_at FROM files WHERE id = $1")
            .bind(orphan_file_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(attached_deleted_at.is_none());
    assert!(orphan_deleted_at.is_some());
}

async fn slow_provider() -> axum::http::StatusCode {
    sleep(Duration::from_secs(2)).await;
    axum::http::StatusCode::OK
}

#[sqlx::test]
async fn agent_run_timeout_is_durably_failed_without_provider_details(pool: PgPool) {
    let (base_url, api_server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let conversation: Value = client
        .post(format!("{base_url}/agent/conversations"))
        .json(&json!({"title": "timeout test"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let provider_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let provider_address = provider_listener.local_addr().unwrap();
    let provider_server = tokio::spawn(async move {
        axum::serve(
            provider_listener,
            Router::new().route("/v1/chat/completions", post(slow_provider)),
        )
        .await
        .unwrap();
    });
    let config = AgentProviderConfig::from_values(|name| match name {
        "LLM_API_KEY" => Some("test-key".to_owned()),
        "LLM_BASE_URL" => Some(format!("http://{provider_address}/v1")),
        "LLM_RUN_TIMEOUT_SECONDS" => Some("1".to_owned()),
        _ => None,
    })
    .unwrap()
    .unwrap();
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let user_id = BOOTSTRAP_OWNER_ID.parse::<Uuid>().unwrap();
    let repository = CatalogRepository::new(pool.clone())
        .for_workspace(workspace_id)
        .await
        .unwrap();
    let dispatcher = agent_worker::start(
        CatalogRepository::new(pool),
        config.clone(),
        Arc::new(FakeObjectStore::available()),
    )
    .await;
    let run = repository
        .create_agent_run_for_user(
            conversation["id"].as_str().unwrap().parse().unwrap(),
            user_id,
            config.base_url.as_str(),
            &config.model,
        )
        .await
        .unwrap();
    dispatcher.enqueue(workspace_id, run.id).await.unwrap();

    let timed_out_run = timeout(Duration::from_secs(5), async {
        loop {
            let run = repository.get_agent_run(run.id).await.unwrap();
            if run.status == "failed" {
                break run;
            }
            sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("run should fail before the test deadline");
    assert_eq!(timed_out_run.error_code.as_deref(), Some("run_timeout"));
    assert_eq!(
        timed_out_run.error_message.as_deref(),
        Some("agent run exceeded configured timeout")
    );
    assert!(timed_out_run.finished_at.is_some());
    let events = repository.agent_run_events_after(run.id, -1).await.unwrap();
    assert!(events.iter().any(|event| {
        event.event_type == "error"
            && event.payload
                == json!({"code":"run_timeout","message":"agent run exceeded configured timeout"})
    }));
    assert!(events.iter().any(|event| {
        event.event_type == "terminal"
            && event.payload == json!({"status":"failed","code":"run_timeout"})
    }));
    assert!(events.iter().all(|event| {
        !event
            .payload
            .to_string()
            .contains(&provider_address.to_string())
    }));
    api_server.abort();
    provider_server.abort();
}

#[sqlx::test]
async fn agent_read_tools_enforce_initiator_permissions_and_scopes(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let owner = authenticated_client();
    let blueprint = create_blueprint(
        &owner,
        &base_url,
        "format_version = 1\ncode = 'agent_scoped'\nname = 'Agent scoped'\nkind = 'entity'\n\n[views.dropdown_option]\ntype = 'dropdown_option'\nfields = ['title']\n\n[[attributes]]\ncode = 'title'\nvalue_type = 'string'",
    )
    .await;
    let entity = create_entity(&owner, &base_url, &blueprint).await;
    let context: Value = owner
        .post(format!("{base_url}/contexts"))
        .json(&json!({"code": "agent-scoped", "data": {}}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    let workspace = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let actor = Uuid::new_v4();
    let membership = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, $2)")
        .bind(actor)
        .bind(format!("{actor}@example.test"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(membership)
    .bind(workspace)
    .bind(actor)
    .execute(&pool)
    .await
    .unwrap();

    let repository = CatalogRepository::new(pool.clone())
        .for_workspace(workspace)
        .await
        .unwrap();
    let agent_only = repository
        .create_workspace_role(
            BOOTSTRAP_OWNER_ID.parse().unwrap(),
            workspace,
            "agent-only",
            &["agents.run".to_owned()],
        )
        .await
        .unwrap();
    repository
        .grant_workspace_member_role(
            BOOTSTRAP_OWNER_ID.parse().unwrap(),
            workspace,
            membership,
            agent_only,
            "workspace",
            workspace,
        )
        .await
        .unwrap();

    for (name, arguments) in [
        ("list_blueprints", json!({})),
        ("list_contexts", json!({})),
        ("get_entity", json!({"entity_id": entity["id"]})),
    ] {
        assert!(matches!(
            execute_read(&repository, actor, workspace, name, arguments).await,
            Err(ToolError::Forbidden)
        ));
    }

    let scoped_reader = repository
        .create_workspace_role(
            BOOTSTRAP_OWNER_ID.parse().unwrap(),
            workspace,
            "agent-scoped-reader",
            &["entities.read".to_owned(), "contexts.read".to_owned()],
        )
        .await
        .unwrap();
    repository
        .grant_workspace_member_role(
            BOOTSTRAP_OWNER_ID.parse().unwrap(),
            workspace,
            membership,
            scoped_reader,
            "entity",
            entity["id"].as_str().unwrap().parse().unwrap(),
        )
        .await
        .unwrap();
    repository
        .grant_workspace_member_role(
            BOOTSTRAP_OWNER_ID.parse().unwrap(),
            workspace,
            membership,
            scoped_reader,
            "context_subtree",
            context["id"].as_str().unwrap().parse().unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        execute_read(
            &repository,
            actor,
            workspace,
            "get_entity",
            json!({"entity_id": entity["id"]}),
        )
        .await
        .unwrap()["id"],
        entity["id"]
    );
    assert_eq!(
        execute_read(&repository, actor, workspace, "list_contexts", json!({}))
            .await
            .unwrap(),
        json!([context])
    );
    server.abort();
}

#[sqlx::test]
async fn standalone_conversation_upload_survives_reconciliation_until_attached(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base_url, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let client = authenticated_client();
    let conversation: Value = client
        .post(format!("{base_url}/agent/conversations"))
        .json(&json!({"title": "Upload retention"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let conversation_id = conversation["id"]
        .as_str()
        .unwrap()
        .parse::<Uuid>()
        .unwrap();
    let upload: Value = client
        .post(format!(
            "{base_url}/agent/conversations/{conversation_id}/uploads"
        ))
        .multipart(
            Form::new().part(
                "file",
                Part::bytes(b"%PDF-1.4\nattachment".to_vec())
                    .file_name("attachment.pdf")
                    .mime_str("application/pdf")
                    .unwrap(),
            ),
        )
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let file_id = upload["files"][0]["id"]
        .as_str()
        .unwrap()
        .parse::<Uuid>()
        .unwrap();

    let worker = FileWorker::new(
        pool.clone(),
        store,
        WorkerConfig {
            worker_id: "test-worker".into(),
            max_pixels: 1,
            max_attempts: 1,
            delete_grace: Duration::from_secs(60),
        },
    );
    worker.reconcile().await.unwrap();
    let deleted_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT deleted_at FROM files WHERE id = $1")
            .bind(file_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(deleted_at.is_none());

    CatalogRepository::new(pool.clone())
        .for_workspace(BOOTSTRAP_WORKSPACE_ID.parse().unwrap())
        .await
        .unwrap()
        .append_conversation_message_with_attachments(
            conversation_id,
            None,
            "user",
            json!("Read the attachment"),
            &[file_id],
        )
        .await
        .unwrap();
    assert!(
        sqlx::query_scalar::<_, Option<chrono::DateTime<chrono::Utc>>>(
            "SELECT attachment_expires_at FROM files WHERE id = $1",
        )
        .bind(file_id)
        .fetch_one(&pool)
        .await
        .unwrap()
        .is_none()
    );

    assert!(worker.run_once().await.unwrap());
    let messages: Value = client
        .get(format!(
            "{base_url}/agent/conversations/{conversation_id}/messages"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(messages[0]["attachments"][0]["id"], file_id.to_string());
    assert_eq!(messages[0]["attachments"][0]["status"], "ready");

    let never_attached: Value = client
        .post(format!(
            "{base_url}/agent/conversations/{conversation_id}/uploads"
        ))
        .multipart(
            Form::new().part(
                "file",
                Part::bytes(b"%PDF-1.4\nnever attached".to_vec())
                    .file_name("never-attached.pdf")
                    .mime_str("application/pdf")
                    .unwrap(),
            ),
        )
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let never_attached_id = never_attached["files"][0]["id"]
        .as_str()
        .unwrap()
        .parse::<Uuid>()
        .unwrap();
    sqlx::query(
        "UPDATE files SET attachment_expires_at = now() - interval '1 second' WHERE id = $1",
    )
    .bind(never_attached_id)
    .execute(&pool)
    .await
    .unwrap();
    worker.reconcile().await.unwrap();
    assert!(
        sqlx::query_scalar::<_, Option<chrono::DateTime<chrono::Utc>>>(
            "SELECT deleted_at FROM files WHERE id = $1",
        )
        .bind(never_attached_id)
        .fetch_one(&pool)
        .await
        .unwrap()
        .is_some()
    );
    server.abort();
}

#[sqlx::test]
async fn agent_http_limits_accept_the_boundary_and_reject_the_next_value(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let accepted = client
        .post(format!("{base_url}/agent/conversations"))
        .json(&json!({"title": "x".repeat(512)}))
        .send()
        .await
        .unwrap();
    assert_eq!(accepted.status(), StatusCode::CREATED);
    let conversation: Value = accepted.json().await.unwrap();
    let oversized_title = client
        .post(format!("{base_url}/agent/conversations"))
        .json(&json!({"title": "x".repeat(513)}))
        .send()
        .await
        .unwrap();
    assert_eq!(oversized_title.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let message_url = format!(
        "{base_url}/agent/conversations/{}/messages",
        conversation["id"].as_str().unwrap()
    );
    let message_at_limit = client
        .post(&message_url)
        .json(&json!({"content": "x".repeat(32 * 1024)}))
        .send()
        .await
        .unwrap();
    assert_eq!(message_at_limit.status(), StatusCode::SERVICE_UNAVAILABLE);
    let oversized_message = client
        .post(&message_url)
        .json(&json!({"content": "x".repeat(32 * 1024 + 1)}))
        .send()
        .await
        .unwrap();
    assert_eq!(oversized_message.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let attachments_at_limit = client
        .post(&message_url)
        .json(&json!({
            "content": "",
            "attachment_ids": (0..16).map(|_| Uuid::new_v4()).collect::<Vec<_>>(),
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        attachments_at_limit.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let oversized_attachments = client
        .post(&message_url)
        .json(&json!({
            "content": "",
            "attachment_ids": (0..17).map(|_| Uuid::new_v4()).collect::<Vec<_>>(),
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        oversized_attachments.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    server.abort();
}

#[sqlx::test]
async fn agent_message_requires_a_configured_dispatcher(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let conversation: Value = client
        .post(format!("{base_url}/agent/conversations"))
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let response = client
        .post(format!(
            "{base_url}/agent/conversations/{}/messages",
            conversation["id"].as_str().unwrap()
        ))
        .json(&json!({"content":"hello"}))
        .send()
        .await
        .unwrap();
    let status = response.status();
    let response: Value = response.json().await.unwrap();
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{response}");
    assert_eq!(response["error"]["code"], "service_unavailable");
    server.abort();
}
