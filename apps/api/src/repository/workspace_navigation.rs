use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{CatalogRepository, RepositoryError};

#[derive(Clone, Debug, Deserialize, Serialize)]
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
        let mut codes = HashSet::new();
        for entry in entries {
            if entry.blueprint_code.is_empty() || !codes.insert(&entry.blueprint_code) {
                return Err(RepositoryError::InvalidCode);
            }
            let blueprint_exists: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM blueprints WHERE workspace_id = $1 AND code = $2 AND kind = 'entity' AND status = 'published' AND deleted_at IS NULL)",
            )
            .bind(workspace_id)
            .bind(&entry.blueprint_code)
            .fetch_one(&mut *tx)
            .await?;
            if !blueprint_exists {
                return Err(RepositoryError::NotFound("published entity blueprint"));
            }
            let role_codes: HashSet<_> = entry.visible_to_role_codes.iter().collect();
            if role_codes.len() != entry.visible_to_role_codes.len() {
                return Err(RepositoryError::InvalidCode);
            }
            let known_roles: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM roles WHERE code = ANY($1) AND (is_system OR workspace_id = $2)",
            )
            .bind(&entry.visible_to_role_codes)
            .bind(workspace_id)
            .fetch_one(&mut *tx)
            .await?;
            if known_roles != entry.visible_to_role_codes.len() as i64 {
                return Err(RepositoryError::NotFound("workspace role"));
            }
        }
        sqlx::query(
            "UPDATE workspaces SET settings = jsonb_set(settings, '{explore_navigation}', $1::jsonb, true), updated_at = now() WHERE id = $2 AND deleted_at IS NULL",
        )
        .bind(serde_json::to_value(entries).expect("navigation entries serialize"))
        .bind(workspace_id)
        .execute(&mut *tx)
        .await?;
        self.commit_mutation(tx).await
    }
}
