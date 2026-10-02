use std::time::Duration;

use tokio::sync::watch;

use catalog_repository::repository::SystemRepository;

/// The documented purge bound is one hour after plan expiry or the application
/// resumability deadline. The first tick runs immediately at process startup.
const HOUSEKEEPING_INTERVAL: Duration = Duration::from_secs(60 * 60);

pub fn start(
    repository: SystemRepository,
    mut shutdown: watch::Receiver<()>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(HOUSEKEEPING_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = shutdown.changed() => return,
                _ = interval.tick() => run_once(&repository).await,
            }
        }
    })
}

pub(crate) async fn run_once(repository: &SystemRepository) {
    let workspaces = match repository.solution_pack_housekeeping_workspaces().await {
        Ok(workspaces) => workspaces,
        Err(error) => {
            tracing::error!(%error, "could not discover workspaces for solution-pack housekeeping");
            return;
        }
    };
    for workspace_id in workspaces {
        let workspace_repository = match repository.for_workspace(workspace_id).await {
            Ok(repository) => repository,
            Err(error) => {
                tracing::error!(%workspace_id, %error, "could not scope solution-pack housekeeping");
                continue;
            }
        };
        if let Err(error) = workspace_repository
            .cleanup_solution_pack_sample_staging()
            .await
        {
            tracing::error!(%workspace_id, %error, "solution-pack housekeeping failed");
        }
    }
}
