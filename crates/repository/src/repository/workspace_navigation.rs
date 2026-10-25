use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use attricat_cache::{CacheKey, Policy};
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::generations::{Generation, advance_generation};
use super::{AttricatRepository, RepositoryError};
use attricat_domain::permissions::Permission;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ExploreNavigationEntry {
    pub blueprint_code: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub visible_to_role_codes: Vec<String>,
}

/// The workspace-wide part of Explore navigation.
#[derive(Debug, Deserialize, Serialize)]
struct ExploreNavigationConfiguration {
    entries: Vec<ExploreNavigationEntry>,
    /// Latest published name by blueprint code.
    names: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct ExploreNavigationItem {
    pub blueprint_code: String,
    pub blueprint_name: String,
}

impl AttricatRepository {
    async fn require_workspace_navigation_permission(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<(), RepositoryError> {
        if self
            .is_authorized(
                actor_id,
                workspace_id,
                Permission::WorkspaceNavigationManage,
                None,
                None,
            )
            .await?
        {
            Ok(())
        } else {
            Err(RepositoryError::NotFound("permission"))
        }
    }

    pub async fn list_explore_navigation(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<ExploreNavigationItem>, RepositoryError> {
        let configuration = self.explore_navigation_configuration(workspace_id).await?;
        if configuration.entries.is_empty() {
            return Ok(Vec::new());
        }
        let held_roles: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT r.code FROM workspace_memberships m JOIN role_grants g ON g.membership_id = m.id AND g.workspace_id = m.workspace_id JOIN roles r ON r.id = g.role_id WHERE m.user_id = $1 AND m.workspace_id = $2 AND m.state = 'active'",
        )
        .bind(actor_id)
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;
        let held_roles: HashSet<_> = held_roles.into_iter().collect();
        let entries: Vec<&ExploreNavigationEntry> = configuration
            .entries
            .iter()
            .filter(|entry| {
                entry.visible_to_role_codes.is_empty()
                    || entry
                        .visible_to_role_codes
                        .iter()
                        .any(|role| held_roles.contains(role))
            })
            .collect();
        if entries.is_empty() {
            return Ok(Vec::new());
        }
        let codes: Vec<String> = entries
            .iter()
            .map(|entry| entry.blueprint_code.clone())
            .collect();
        let readable = self
            .authorized_target_codes(actor_id, workspace_id, Permission::RecordsRead, &codes)
            .await?;
        let names = &configuration.names;
        let mut items = Vec::new();
        for entry in entries {
            if !readable.contains(&entry.blueprint_code) {
                continue;
            }
            if let Some(blueprint_name) = names.get(&entry.blueprint_code) {
                items.push(ExploreNavigationItem {
                    blueprint_name: blueprint_name.clone(),
                    blueprint_code: entry.blueprint_code.clone(),
                });
            }
        }
        Ok(items)
    }

    /// The configured entries and their blueprints' latest published names,
    /// the same for every member. Navigation writes and publications advance
    /// the attricat generation the cache key carries.
    async fn explore_navigation_configuration(
        &self,
        workspace_id: Uuid,
    ) -> Result<Arc<ExploreNavigationConfiguration>, RepositoryError> {
        let load = || async {
            // Stored configuration is written only by the validated method
            // below. Treat malformed legacy data as no navigation rather than
            // failing sidebar rendering.
            let settings: serde_json::Value = sqlx::query_scalar(
                "SELECT COALESCE(settings->'explore_navigation', '[]'::jsonb) FROM workspaces WHERE id = $1 AND deleted_at IS NULL",
            )
            .bind(workspace_id)
            .fetch_optional(&self.pool)
            .await?
            .unwrap_or(serde_json::json!([]));
            let entries: Vec<ExploreNavigationEntry> =
                serde_json::from_value(settings).unwrap_or_default();
            let codes: Vec<&str> = entries
                .iter()
                .map(|entry| entry.blueprint_code.as_str())
                .collect();
            let names = if codes.is_empty() {
                HashMap::new()
            } else {
                sqlx::query_as::<_, (String, String)>(
                    "SELECT DISTINCT ON (code) code, name FROM blueprints WHERE workspace_id = $1 AND code = ANY($2) AND kind = 'record' AND status = 'published' AND deleted_at IS NULL ORDER BY code, version DESC",
                )
                .bind(workspace_id)
                .bind(&codes)
                .fetch_all(&self.pool)
                .await?
                .into_iter()
                .collect()
            };
            Ok::<_, RepositoryError>(ExploreNavigationConfiguration { entries, names })
        };
        // The generations belong to this repository's workspace.
        match self
            .generations()
            .filter(|_| workspace_id == self.workspace_id.0)
        {
            Some(generations) => {
                self.cache
                    .fetch(
                        CacheKey::new(
                            "explore_navigation",
                            &[&workspace_id, &generations.attricat_generation],
                        ),
                        &[],
                        Policy::Generation,
                        load,
                    )
                    .await
            }
            None => load().await.map(Arc::new),
        }
    }

    pub async fn configured_explore_navigation(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<ExploreNavigationEntry>, RepositoryError> {
        self.require_workspace_navigation_permission(actor_id, workspace_id)
            .await?;
        let settings: serde_json::Value = sqlx::query_scalar(
            "SELECT COALESCE(settings->'explore_navigation', '[]'::jsonb) FROM workspaces WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(workspace_id)
        .fetch_optional(&self.pool)
        .await?
        .unwrap_or(serde_json::json!([]));
        Ok(serde_json::from_value(settings).unwrap_or_default())
    }

    pub async fn update_explore_navigation(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
        entries: &[ExploreNavigationEntry],
    ) -> Result<(), RepositoryError> {
        self.require_workspace_navigation_permission(actor_id, workspace_id)
            .await?;
        let mut tx = self.pool.begin().await?;
        self.lock_workspace_for_explore_navigation_in_transaction(&mut tx, workspace_id)
            .await?;
        self.validate_explore_navigation_entries_in_transaction(&mut tx, workspace_id, entries)
            .await?;
        self.write_explore_navigation_in_transaction(&mut tx, workspace_id, entries)
            .await?;
        self.commit_mutation(tx).await
    }

    async fn lock_workspace_for_explore_navigation_in_transaction(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        workspace_id: Uuid,
    ) -> Result<serde_json::Value, RepositoryError> {
        sqlx::query_scalar(
            "SELECT COALESCE(settings->'explore_navigation', '[]'::jsonb) FROM workspaces WHERE id = $1 AND deleted_at IS NULL FOR UPDATE",
        )
        .bind(workspace_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(RepositoryError::NotFound("workspace"))
    }

    pub(super) async fn lock_explore_navigation_in_transaction(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        workspace_id: Uuid,
    ) -> Result<Vec<ExploreNavigationEntry>, RepositoryError> {
        let value = self
            .lock_workspace_for_explore_navigation_in_transaction(tx, workspace_id)
            .await?;
        parse_stored_explore_navigation(value).map_err(|()| RepositoryError::SolutionPackPlanStale)
    }

    pub(super) async fn validate_explore_navigation_entries_in_transaction(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        workspace_id: Uuid,
        entries: &[ExploreNavigationEntry],
    ) -> Result<(), RepositoryError> {
        let mut codes = HashSet::new();
        for entry in entries {
            if entry.blueprint_code.is_empty() || !codes.insert(&entry.blueprint_code) {
                return Err(RepositoryError::InvalidCode);
            }
            let blueprint_id: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM blueprints WHERE workspace_id = $1 AND code = $2 AND kind = 'record' AND status = 'published' AND deleted_at IS NULL ORDER BY version DESC LIMIT 1 FOR SHARE",
            )
            .bind(workspace_id)
            .bind(&entry.blueprint_code)
            .fetch_optional(&mut **tx)
            .await?;
            if blueprint_id.is_none() {
                return Err(RepositoryError::NotFound("published record blueprint"));
            }
            let role_codes: HashSet<_> = entry.visible_to_role_codes.iter().collect();
            if role_codes.len() != entry.visible_to_role_codes.len() {
                return Err(RepositoryError::InvalidCode);
            }
            let known_roles: HashSet<String> = sqlx::query_scalar(
                "SELECT code FROM roles WHERE code = ANY($1) AND (is_system OR workspace_id = $2) FOR SHARE",
            )
            .bind(&entry.visible_to_role_codes)
            .bind(workspace_id)
            .fetch_all(&mut **tx)
            .await?
            .into_iter()
            .collect();
            if known_roles.len() != entry.visible_to_role_codes.len() {
                return Err(RepositoryError::NotFound("workspace role"));
            }
        }
        Ok(())
    }

    pub(super) async fn write_explore_navigation_in_transaction(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        workspace_id: Uuid,
        entries: &[ExploreNavigationEntry],
    ) -> Result<(), RepositoryError> {
        let updated = sqlx::query(
            "UPDATE workspaces SET settings = jsonb_set(settings, '{explore_navigation}', $1::jsonb, true), updated_at = now() WHERE id = $2 AND deleted_at IS NULL",
        )
        .bind(serde_json::to_value(entries).expect("navigation entries serialize"))
        .bind(workspace_id)
        .execute(&mut **tx)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(RepositoryError::NotFound("workspace"));
        }
        advance_generation(tx, workspace_id, Generation::Attricat).await?;
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredExploreNavigationEntry {
    blueprint_code: String,
    #[serde(default)]
    visible_to_role_codes: Vec<String>,
}

pub(super) fn parse_stored_explore_navigation(
    value: serde_json::Value,
) -> Result<Vec<ExploreNavigationEntry>, ()> {
    let entries = serde_json::from_value::<Vec<StoredExploreNavigationEntry>>(value)
        .map_err(|_| ())?
        .into_iter()
        .map(|entry| ExploreNavigationEntry {
            blueprint_code: entry.blueprint_code,
            visible_to_role_codes: entry.visible_to_role_codes,
        })
        .collect::<Vec<_>>();
    let mut blueprint_codes = HashSet::new();
    for entry in &entries {
        if entry.blueprint_code.is_empty() || !blueprint_codes.insert(&entry.blueprint_code) {
            return Err(());
        }
        let role_codes = entry.visible_to_role_codes.iter().collect::<HashSet<_>>();
        if role_codes.len() != entry.visible_to_role_codes.len() {
            return Err(());
        }
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[sqlx::test(migrations = "../../apps/api/migrations")]
    async fn workspace_lock_serializes_navigation_mutations(pool: sqlx::PgPool) {
        let workspace_id = AttricatRepository::DEFAULT_WORKSPACE_ID;
        let repository = AttricatRepository::new(pool, AttricatRepository::DEFAULT_WORKSPACE_ID);
        let mut first = repository.pool.begin().await.unwrap();
        repository
            .lock_explore_navigation_in_transaction(&mut first, workspace_id)
            .await
            .unwrap();

        let concurrent_repository = repository.clone();
        let mut concurrent = tokio::spawn(async move {
            let mut tx = concurrent_repository.pool.begin().await.unwrap();
            concurrent_repository
                .lock_explore_navigation_in_transaction(&mut tx, workspace_id)
                .await
                .unwrap();
            tx.commit().await.unwrap();
        });
        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut concurrent)
                .await
                .is_err()
        );
        first.rollback().await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), concurrent)
            .await
            .unwrap()
            .unwrap();
    }
}
