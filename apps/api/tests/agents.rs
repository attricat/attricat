mod support;

use support::*;

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
    let response: Value = client
        .post(format!(
            "{base_url}/agent/conversations/{}/messages",
            conversation["id"]
        ))
        .json(&json!({"content":"hello"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(response["error"]["code"], "service_unavailable");
    server.abort();
}
