mod support;

use std::time::Duration;

use api::repository::AttricatRepository;
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

#[sqlx::test]
async fn workspace_scopes_share_one_bounded_pool_under_concurrency_and_close_cleanly(pool: PgPool) {
    let workspace_ids: Vec<_> = (0..48).map(|_| Uuid::new_v4()).collect();
    for (index, workspace_id) in workspace_ids.iter().enumerate() {
        sqlx::query(
            "INSERT INTO workspaces (id, slug, name, login_identifier) VALUES ($1, $2, $3, $4)",
        )
        .bind(workspace_id)
        .bind(format!("pool-{index}"))
        .bind(format!("Pool {index}"))
        .bind(format!("pool-{index}.local"))
        .execute(&pool)
        .await
        .unwrap();
        AttricatRepository::system(pool.clone())
            .initialize_workspace(*workspace_id)
            .await
            .unwrap();
    }

    let shared_pool = PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(2))
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap();
    let session = AttricatRepository::new(shared_pool.clone(), workspace_ids[0]);

    let mut requests = tokio::task::JoinSet::new();
    for workspace_id in workspace_ids.iter().copied() {
        let session = session.clone();
        requests.spawn(async move {
            let repository = session.for_workspace(workspace_id).await.unwrap();
            repository.list_contexts().await.unwrap()
        });
    }
    while let Some(result) = requests.join_next().await {
        assert_eq!(result.unwrap().len(), 1);
    }
    assert!(
        shared_pool.size() <= 2,
        "workspace count must not grow pool size"
    );

    // Exhausting the only two shared connections rejects a third request; it
    // never creates a connection for the workspace making that request.
    let exhausted = PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_millis(50))
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap();
    let exhausted_session = AttricatRepository::new(exhausted.clone(), workspace_ids[0]);
    let held_first = exhausted.acquire().await.unwrap();
    let held_second = exhausted.acquire().await.unwrap();
    // Scope derivation is pure, even with no available database connections.
    let scoped = exhausted_session
        .for_workspace(workspace_ids[0])
        .await
        .unwrap();
    assert!(scoped.list_contexts().await.is_err());
    drop(held_first);
    drop(held_second);
    exhausted.close().await;

    // Closing the global pool makes every extant workspace scope fail rather
    // than leaving a per-workspace pool alive during shutdown.
    shared_pool.close().await;
    let scoped = session.for_workspace(Uuid::new_v4()).await.unwrap();
    assert!(scoped.list_contexts().await.is_err());
}
