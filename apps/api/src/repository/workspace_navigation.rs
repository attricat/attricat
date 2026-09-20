use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::{CatalogRepository, RepositoryError};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ExploreNavigationEntry {
    pub blueprint_code: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub visible_to_role_codes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ExploreNavigationItem {
    pub blueprint_code: String,
    pub blueprint_name: String,
}

impl CatalogRepository {
    async fn require_workspace_navigation_permission(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<(), RepositoryError> {
        if self
            .is_authorized(
                actor_id,
                workspace_id,
                "workspace_navigation.manage",
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
        // Stored configuration is written only by the validated method below.
        // Treat malformed legacy data as no navigation rather than failing sidebar rendering.
        let settings: serde_json::Value = sqlx::query_scalar(
            "SELECT COALESCE(settings->'explore_navigation', '[]'::jsonb) FROM workspaces WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(workspace_id)
        .fetch_optional(&self.pool)
        .await?
        .unwrap_or(serde_json::json!([]));
        let entries: Vec<ExploreNavigationEntry> =
            serde_json::from_value(settings).unwrap_or_default();
        let held_roles: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT r.code FROM workspace_memberships m JOIN role_grants g ON g.membership_id = m.id AND g.workspace_id = m.workspace_id JOIN roles r ON r.id = g.role_id WHERE m.user_id = $1 AND m.workspace_id = $2 AND m.state = 'active'",
        )
        .bind(actor_id)
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;
        let held_roles: HashSet<_> = held_roles.into_iter().collect();
        let mut items = Vec::new();
        for entry in entries {
            if !entry.visible_to_role_codes.is_empty()
                && !entry
                    .visible_to_role_codes
                    .iter()
                    .any(|role| held_roles.contains(role))
            {
                continue;
            }
            if !self
                .is_authorized(
                    actor_id,
                    workspace_id,
                    "entities.read",
                    None,
                    Some(&entry.blueprint_code),
                )
                .await?
            {
                continue;
            }
            let blueprint_name: Option<String> = sqlx::query_scalar(
                "SELECT name FROM blueprints WHERE workspace_id = $1 AND code = $2 AND kind = 'entity' AND status = 'published' AND deleted_at IS NULL ORDER BY version DESC LIMIT 1",
            )
            .bind(workspace_id)
            .bind(&entry.blueprint_code)
            .fetch_optional(&self.pool)
            .await?;
            if let Some(blueprint_name) = blueprint_name {
                items.push(ExploreNavigationItem {
                    blueprint_code: entry.blueprint_code,
                    blueprint_name,
                });
            }
        }
        Ok(items)
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
                "SELECT id FROM blueprints WHERE workspace_id = $1 AND code = $2 AND kind = 'entity' AND status = 'published' AND deleted_at IS NULL ORDER BY version DESC LIMIT 1 FOR SHARE",
            )
            .bind(workspace_id)
            .bind(&entry.blueprint_code)
            .fetch_optional(&mut **tx)
            .await?;
            if blueprint_id.is_none() {
                return Err(RepositoryError::NotFound("published entity blueprint"));
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

    #[sqlx::test(migrations = "./migrations")]
    async fn workspace_lock_serializes_navigation_mutations(pool: sqlx::PgPool) {
        let workspace_id = CatalogRepository::DEFAULT_WORKSPACE_ID;
        let repository = CatalogRepository::new(pool);
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
