//! Leader election for background coordinators.
//!
//! A coordinator's sweeps are idempotent, so running them on every replica is
//! correct but wasteful. The leader holds a session-level advisory lock on a
//! dedicated connection outside the request and task pools; the lock is
//! released when that connection closes, including when the process dies.

use std::time::{Duration, Instant};

use sqlx::{Connection, PgConnection};

use super::{CatalogRepository, RepositoryError, RepositoryScope};

/// How often a non-leader tries to take over.
const ACQUIRE_INTERVAL: Duration = Duration::from_secs(5);
/// How often the leader checks that its lock connection is still open.
const VERIFY_INTERVAL: Duration = Duration::from_secs(30);

/// One coordinator's claim on its advisory lock.
pub struct CoordinatorLeadership {
    name: &'static str,
    connection: Option<PgConnection>,
    last_attempt: Option<Instant>,
}

impl CoordinatorLeadership {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            connection: None,
            last_attempt: None,
        }
    }

    /// Whether this process currently leads. Leadership is taken when the
    /// lock is free and kept while its connection stays open.
    pub async fn is_leader<S: RepositoryScope>(
        &mut self,
        repository: &CatalogRepository<S>,
    ) -> bool {
        if let Some(connection) = self.connection.as_mut() {
            if self
                .last_attempt
                .is_some_and(|last| last.elapsed() < VERIFY_INTERVAL)
            {
                return true;
            }
            self.last_attempt = Some(Instant::now());
            if connection.ping().await.is_ok() {
                return true;
            }
            tracing::warn!(
                coordinator = self.name,
                "coordinator lock connection was lost"
            );
            self.connection = None;
            return false;
        }
        if self
            .last_attempt
            .is_some_and(|last| last.elapsed() < ACQUIRE_INTERVAL)
        {
            return false;
        }
        self.last_attempt = Some(Instant::now());
        match repository.try_coordinator_lock(self.name).await {
            Ok(Some(connection)) => {
                tracing::info!(coordinator = self.name, "coordinator leadership acquired");
                self.connection = Some(connection);
                true
            }
            Ok(None) => false,
            Err(error) => {
                tracing::warn!(coordinator = self.name, %error, "coordinator leadership check failed");
                false
            }
        }
    }
}

impl<S: RepositoryScope> CatalogRepository<S> {
    /// Opens a dedicated connection and takes the coordinator's session lock
    /// on it, or returns `None` when another session holds it.
    pub(crate) async fn try_coordinator_lock(
        &self,
        name: &str,
    ) -> Result<Option<PgConnection>, RepositoryError> {
        let options = self.pool.connect_options();
        let mut connection = PgConnection::connect_with(&options).await?;
        let acquired: bool =
            sqlx::query_scalar("SELECT pg_try_advisory_lock(hashtextextended($1, 0))")
                .bind(format!("attricat.coordinator:{name}"))
                .fetch_one(&mut connection)
                .await?;
        if acquired {
            Ok(Some(connection))
        } else {
            let _ = connection.close().await;
            Ok(None)
        }
    }
}
