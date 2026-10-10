//! Workspace generations: counters that generation-keyed caches read.
//!
//! A write transaction that changes cached state advances the matching
//! counter before it commits. A reader reads the counters in a query it
//! already runs (authentication, the record lock), and only then loads or
//! looks up cached state under that generation, so a cached entry is never
//! older than the generation it is filed under. Correctness never depends on
//! cache invalidation messages.

use std::{collections::HashMap, sync::Arc};

use catalog_cache::{CacheKey, Policy};
use sqlx::PgConnection;
use uuid::Uuid;

use super::{
    CatalogRepository, RepositoryError, SystemScope,
    checks::{EnabledRule, enabled_rule_sets},
    record_values::ContextTree,
};

/// A workspace's generations, read together.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, sqlx::FromRow)]
pub struct WorkspaceGenerations {
    /// Published blueprint revisions, enabled rules, Explore navigation.
    pub catalog_generation: i64,
    /// The context tree.
    pub contexts_generation: i64,
    /// Extension installations, grants, configuration and layout.
    pub extensions_generation: i64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Generation {
    Catalog,
    Contexts,
    Extensions,
}

impl Generation {
    fn column(self) -> &'static str {
        match self {
            Self::Catalog => "catalog_generation",
            Self::Contexts => "contexts_generation",
            Self::Extensions => "extensions_generation",
        }
    }
}

/// Advances one generation in the caller's write transaction.
pub(crate) async fn advance_generation(
    connection: &mut PgConnection,
    workspace_id: Uuid,
    generation: Generation,
) -> Result<(), RepositoryError> {
    let column = generation.column();
    sqlx::query(&format!(
        "UPDATE workspaces SET {column} = {column} + 1 WHERE id = $1"
    ))
    .bind(workspace_id)
    .execute(connection)
    .await?;
    Ok(())
}

/// Workspace state for a record write, read from committed data before the
/// write's transaction opens and filed under the generations it was read
/// at. A transaction uses it only if the generations its own record lock
/// reads are the same, which also rules out uncommitted changes of its own.
#[derive(Clone)]
pub(crate) struct WritePrefetch {
    pub generations: WorkspaceGenerations,
    pub tree: Arc<ContextTree>,
    pub rules: Arc<EnabledRuleSets>,
}

/// Enabled rules per blueprint revision.
pub(crate) type EnabledRuleSets = HashMap<(Uuid, i64), Vec<EnabledRule>>;

impl CatalogRepository<SystemScope> {
    /// This database's random identity, created on first use. Shared cache
    /// keys include it, so two databases behind one Redis never share
    /// entries. A restored copy of a database keeps the identity, so its
    /// shared cache must be flushed after the restore.
    pub async fn ensure_database_identity(&self) -> Result<Uuid, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO database_identity (id) VALUES ($1) ON CONFLICT DO NOTHING")
            .bind(Uuid::new_v4())
            .execute(&mut *tx)
            .await?;
        let id = sqlx::query_scalar("SELECT id FROM database_identity")
            .fetch_one(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(id)
    }
}

impl CatalogRepository {
    /// The context tree at this request's generation.
    pub(crate) async fn cached_context_tree(
        &self,
    ) -> Result<Option<Arc<ContextTree>>, RepositoryError> {
        let Some(generations) = self.generations() else {
            return Ok(None);
        };
        let workspace_id = self.workspace_id.0;
        self.cache
            .fetch(
                CacheKey::new(
                    "context_tree",
                    &[&workspace_id, &generations.contexts_generation],
                ),
                &[],
                Policy::Generation,
                || async {
                    let mut connection = self.pool.acquire().await?;
                    ContextTree::load(&mut connection, workspace_id).await
                },
            )
            .await
            .map(Some)
    }

    /// Every revision's enabled rules at this request's generation.
    pub(crate) async fn cached_enabled_rule_sets(
        &self,
    ) -> Result<Option<Arc<EnabledRuleSets>>, RepositoryError> {
        let Some(generations) = self.generations() else {
            return Ok(None);
        };
        let workspace_id = self.workspace_id.0;
        self.cache
            .fetch(
                CacheKey::new(
                    "enabled_rules",
                    &[&workspace_id, &generations.catalog_generation],
                ),
                &[],
                Policy::Generation,
                || async {
                    let mut connection = self.pool.acquire().await?;
                    enabled_rule_sets(&mut connection, workspace_id).await
                },
            )
            .await
            .map(Some)
    }

    /// State a record write in this request can take from the cache.
    pub(crate) async fn write_prefetch(&self) -> Result<Option<WritePrefetch>, RepositoryError> {
        let Some(generations) = self.generations() else {
            return Ok(None);
        };
        // A failed prefetch only means the write reads its own state.
        let (Ok(Some(tree)), Ok(Some(rules))) = (
            self.cached_context_tree().await,
            self.cached_enabled_rule_sets().await,
        ) else {
            return Ok(None);
        };
        Ok(Some(WritePrefetch {
            generations,
            tree,
            rules,
        }))
    }
}
