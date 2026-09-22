mod support;
use std::time::Duration;

use api::{
    event_dispatcher::{self, DispatcherConfig, EventHandlerRegistry},
    repository::CatalogRepository,
    workflow_runtime,
};
use support::*;
use tokio::sync::watch;

const DEFINITION: &str = "format_version = 1\ncode = \"tag_new_products\"\nname = \"Tag new products\"\n[[triggers]]\nevent_type = \"entity.created.v1\"\n[[actions]]\ntype = \"system_tags_add\"\ntags = [\"new\"]";

#[sqlx::test]
async fn workflow_lifecycle_keeps_immutable_revisions(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let invalid = client.post(format!("{base_url}/workflows/validate")).json(&json!({"definition":"format_version=1\ncode='x'\nname='x'\nscript='no'\ntriggers=[]\nactions=[]"})).send().await.unwrap();
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let created: Value = client
        .post(format!("{base_url}/workflows"))
        .json(&json!({"definition":DEFINITION}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = created["id"].as_str().unwrap();
    assert_eq!(created["status"], "draft");
    let enabled = client
        .post(format!("{base_url}/workflows/{id}/versions/1/enable"))
        .send()
        .await
        .unwrap();
    assert_eq!(enabled.status(), StatusCode::UNPROCESSABLE_ENTITY);
    client
        .post(format!("{base_url}/workflows/{id}/versions/1/publish"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let enabled: Value = client
        .post(format!("{base_url}/workflows/{id}/versions/1/enable"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(enabled["enabled_version"], 1);
    let revised: Value = client
        .post(format!("{base_url}/workflows/{id}/versions"))
        .json(&json!({"definition":DEFINITION.replace("Tag new products", "Tag products") }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(revised["version"], 2);
    assert_eq!(revised["status"], "draft");
    let disabled: Value = client
        .post(format!("{base_url}/workflows/{id}/disable"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(disabled["enabled_version"].is_null());
    server.abort();
}

async fn wait_for_completed_retry(pool: &PgPool, run_id: Uuid) {
    for _ in 0..100 {
        let (status, attempts): (String, i32) =
            sqlx::query_as("SELECT status, attempts FROM workflow_runs WHERE id=$1")
                .bind(run_id)
                .fetch_one(pool)
                .await
                .unwrap();
        if status == "completed" && attempts >= 2 {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("workflow run {run_id} was not reclaimed after its action committed");
}

async fn wait_for_tag(pool: &PgPool, entity_id: Uuid, tag: &str) {
    for _ in 0..100 {
        let tags: Vec<String> = sqlx::query_scalar("SELECT system_tags FROM entities WHERE id=$1")
            .bind(entity_id)
            .fetch_one(pool)
            .await
            .unwrap();
        if tags.iter().any(|current| current == tag) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let runs: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT status, last_error FROM workflow_runs ORDER BY created_at")
            .fetch_all(pool)
            .await
            .unwrap();
    panic!("workflow action did not add {tag}; runs: {runs:?}");
}

#[sqlx::test]
async fn sample_marker_is_preserved_by_ordinary_tag_add_automation(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "sample_workflow_product"
name = "Sample workflow product"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
[[attributes]]
code = "title"
value_type = "string"
default_value = "untitled"
"#,
    )
    .await;
    let workflow: Value = client
        .post(format!("{base_url}/workflows"))
        .json(&json!({"definition":DEFINITION}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let workflow_id = workflow["id"].as_str().unwrap();
    client
        .post(format!(
            "{base_url}/workflows/{workflow_id}/versions/1/publish"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    client
        .post(format!(
            "{base_url}/workflows/{workflow_id}/versions/1/enable"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let repository = CatalogRepository::new(pool.clone());
    let (shutdown_tx, shutdown_rx) = watch::channel(());
    let dispatcher = event_dispatcher::start(
        repository.clone(),
        workflow_runtime::add_to_registry(EventHandlerRegistry::default_handlers()),
        DispatcherConfig::new(
            Duration::from_millis(50),
            Duration::from_millis(10),
            Duration::from_millis(50),
            5,
            Duration::from_millis(10),
        )
        .unwrap(),
        shutdown_rx.clone(),
    );
    tokio::time::sleep(Duration::from_millis(50)).await;

    let ordinary = create_entity(&client, &base_url, &blueprint).await;
    let ordinary_id = Uuid::parse_str(ordinary["id"].as_str().unwrap()).unwrap();
    let rejected = client
        .put(format!("{base_url}/v1/entities/{ordinary_id}"))
        .json(&json!({"system_tags":["attricat.sample"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let sample = create_entity(&client, &base_url, &blueprint).await;
    let sample_id = Uuid::parse_str(sample["id"].as_str().unwrap()).unwrap();
    sqlx::query("UPDATE entities SET system_tags=ARRAY['attricat.sample']::text[] WHERE id=$1")
        .bind(sample_id)
        .execute(&pool)
        .await
        .unwrap();

    let worker = workflow_runtime::start(repository, shutdown_rx);
    wait_for_tag(&pool, sample_id, "new").await;
    let tags: Vec<String> = sqlx::query_scalar("SELECT system_tags FROM entities WHERE id=$1")
        .bind(sample_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(tags.contains(&"attricat.sample".to_owned()));
    assert!(tags.contains(&"new".to_owned()));

    let preserved = client
        .put(format!("{base_url}/v1/entities/{sample_id}"))
        .json(&json!({"system_tags":["attricat.sample","new","manual"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(preserved.status(), StatusCode::OK);
    let removed = client
        .put(format!("{base_url}/v1/entities/{sample_id}"))
        .json(&json!({"system_tags":["new","manual"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(removed.status(), StatusCode::OK);
    assert_eq!(removed.json::<Value>().await.unwrap()["is_sample"], false);

    shutdown_tx.send(()).unwrap();
    for handle in dispatcher {
        handle.await.unwrap();
    }
    worker.await.unwrap();
    server.abort();
}

#[sqlx::test]
async fn workflow_outbox_dispatcher_and_worker_are_idempotent_and_disable_safe(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let repository = CatalogRepository::new(pool.clone());
    let (shutdown_tx, shutdown_rx) = watch::channel(());
    let dispatcher = event_dispatcher::start(
        repository.clone(),
        workflow_runtime::add_to_registry(EventHandlerRegistry::default_handlers()),
        DispatcherConfig::new(
            Duration::from_millis(50),
            Duration::from_millis(10),
            Duration::from_millis(50),
            5,
            Duration::from_millis(10),
        )
        .unwrap(),
        shutdown_rx.clone(),
    );
    let worker = workflow_runtime::start(repository, shutdown_rx);

    let blueprint = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "workflow_test_product"
name = "Workflow test product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"
default_value = "untitled"
"#,
    )
    .await;
    // This entity predates activation and must never be selected by the
    // trigger entity's action.
    let untouched = create_entity(&client, &base_url, &blueprint).await;
    let untouched_id = Uuid::parse_str(untouched["id"].as_str().unwrap()).unwrap();

    let created: Value = client
        .post(format!("{base_url}/workflows"))
        .json(&json!({"definition": DEFINITION}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let workflow_id = Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();
    client
        .post(format!(
            "{base_url}/workflows/{workflow_id}/versions/1/publish"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    client
        .post(format!(
            "{base_url}/workflows/{workflow_id}/versions/1/enable"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let entity = create_entity(&client, &base_url, &blueprint).await;
    let entity_id = Uuid::parse_str(entity["id"].as_str().unwrap()).unwrap();
    wait_for_tag(&pool, entity_id, "new").await;
    let untouched_tags: Vec<String> =
        sqlx::query_scalar("SELECT system_tags FROM entities WHERE id=$1")
            .bind(untouched_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!untouched_tags.iter().any(|tag| tag == "new"));

    let (run_id, trigger_event_id): (Uuid, Uuid) = sqlx::query_as(
        "SELECT id, trigger_event_id FROM workflow_runs WHERE workflow_id=$1 AND workflow_version=1",
    )
    .bind(workflow_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM workflow_run_actions WHERE run_id=$1 AND action_index=0",
        )
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    let action_events: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM domain_events WHERE causation_id=$1 AND source_name=$2 AND aggregate_id=$3",
    )
    .bind(trigger_event_id)
    .bind(format!("workflow:{workflow_id}"))
    .bind(entity_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(action_events, 1);
    let workflow_audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events WHERE metadata->>'workflow_run_id'=$1",
    )
    .bind(run_id.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(workflow_audits, 1);

    // Re-delivering the durable trigger after completion exercises intake's
    // unique run key; it must not produce another mutation, audit, or outbox row.
    sqlx::query(
        "UPDATE event_deliveries SET status='pending', completed_at=NULL, next_attempt_at=clock_timestamp(), lease_owner=NULL, lease_until=NULL WHERE event_id=$1",
    )
    .bind(trigger_event_id)
    .execute(&pool)
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM workflow_runs WHERE workflow_id=$1 AND trigger_event_id=$2",
        )
        .bind(workflow_id)
        .bind(trigger_event_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM audit_events WHERE metadata->>'workflow_run_id'=$1",
        )
        .bind(run_id.to_string())
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM domain_events WHERE causation_id=$1 AND source_name=$2 AND aggregate_id=$3",
        )
        .bind(trigger_event_id)
        .bind(format!("workflow:{workflow_id}"))
        .bind(entity_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );

    // Model a worker crash after the action transaction commits but before run
    // acknowledgement. Reclaiming this expired lease reaches the action marker
    // and must only acknowledge, never write a second effect or audit record.
    assert_eq!(
        sqlx::query(
            "UPDATE workflow_runs SET status='leased', lease_owner='crashed-worker', lease_until=clock_timestamp()-interval '1 millisecond', completed_at=NULL WHERE id=$1 AND status='completed'",
        )
        .bind(run_id)
        .execute(&pool)
        .await
        .unwrap()
        .rows_affected(),
        1
    );
    wait_for_completed_retry(&pool, run_id).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM workflow_run_actions WHERE run_id=$1 AND action_index=0",
        )
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM audit_events WHERE metadata->>'workflow_run_id'=$1",
        )
        .bind(run_id.to_string())
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM domain_events WHERE causation_id=$1 AND source_name=$2 AND aggregate_id=$3",
        )
        .bind(trigger_event_id)
        .bind(format!("workflow:{workflow_id}"))
        .bind(entity_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );

    // Stop the worker so the next durable run remains pending, then disable it.
    // Disable atomically cancels queued/leased runs and no action marker is written.
    let _ = shutdown_tx.send(());
    worker.await.unwrap();
    for handle in dispatcher {
        handle.await.unwrap();
    }
    let blocked = create_entity(&client, &base_url, &blueprint).await;
    let blocked_id = Uuid::parse_str(blocked["id"].as_str().unwrap()).unwrap();
    let blocked_event_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM domain_events WHERE aggregate_id=$1 AND event_type='entity.created.v1' ORDER BY sequence DESC LIMIT 1",
    )
    .bind(blocked_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    CatalogRepository::new(pool.clone())
        .for_workspace(BOOTSTRAP_WORKSPACE_ID.parse().unwrap())
        .await
        .unwrap()
        .fan_out_workflow_runs(
            &sqlx::query_as::<_, api::domain_events::DomainEvent>(
                "SELECT id,sequence,workspace_id,occurred_at,event_type,aggregate_kind,aggregate_id,correlation_id,causation_id,source_kind,source_name,metadata,payload FROM domain_events WHERE id=$1",
            )
            .bind(blocked_event_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        )
        .await
        .unwrap();
    // Hold the pending row in the same durable state produced by a worker
    // claim. The worker is stopped above so disable, rather than execution,
    // owns the transition from this claimed state.
    assert_eq!(
        sqlx::query(
            "UPDATE workflow_runs SET status='leased', lease_owner='test-claim', lease_until=clock_timestamp()+interval '1 minute' WHERE workflow_id=$1 AND trigger_event_id=$2 AND status='pending'",
        )
        .bind(workflow_id)
        .bind(blocked_event_id)
        .execute(&pool)
        .await
        .unwrap()
        .rows_affected(),
        1
    );
    client
        .post(format!("{base_url}/workflows/{workflow_id}/disable"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let (status, actions): (String, i64) = sqlx::query_as(
        "SELECT r.status, (SELECT count(*) FROM workflow_run_actions a WHERE a.run_id=r.id) FROM workflow_runs r WHERE r.workflow_id=$1 AND r.trigger_event_id=$2",
    )
    .bind(workflow_id)
    .bind(blocked_event_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "cancelled");
    assert_eq!(actions, 0);
    let tags: Vec<String> = sqlx::query_scalar("SELECT system_tags FROM entities WHERE id=$1")
        .bind(blocked_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!tags.iter().any(|tag| tag == "new"));
    server.abort();
}
