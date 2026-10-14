//! Workspace settings for the catalogue agent, stored under
//! `workspaces.settings.agent`.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AttricatRepository, RepositoryError};

/// How many records one bulk edit may change unless a workspace sets another
/// limit.
pub const DEFAULT_BULK_EDIT_MAX_RECORDS: u32 = 500;
/// The highest limit a workspace may set. Records change one at a time, so
/// the bound keeps one approved edit within a run's time.
pub const MAX_BULK_EDIT_MAX_RECORDS: u32 = 5_000;
/// Who may read and change a workspace's agent settings.
const AGENT_SETTINGS_PERMISSION: &str = "agents.manage";

/// How bulk value edits the agent proposes are approved.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BulkEditPolicy {
    /// One approval covers every record of a `bulk_set_record_values`
    /// proposal.
    #[default]
    PerProposal,
    /// The bulk tool is not offered, so each record's edit is approved on its
    /// own.
    PerRecord,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSettings {
    #[serde(default)]
    pub bulk_edits: BulkEditPolicy,
    #[serde(default = "default_bulk_edit_max_records")]
    pub bulk_edit_max_records: u32,
}

fn default_bulk_edit_max_records() -> u32 {
    DEFAULT_BULK_EDIT_MAX_RECORDS
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            bulk_edits: BulkEditPolicy::default(),
            bulk_edit_max_records: DEFAULT_BULK_EDIT_MAX_RECORDS,
        }
    }
}

impl AgentSettings {
    fn validate(&self) -> Result<(), RepositoryError> {
        if !(1..=MAX_BULK_EDIT_MAX_RECORDS).contains(&self.bulk_edit_max_records) {
            return Err(RepositoryError::InvalidAgentSettings(format!(
                "bulk_edit_max_records must be between 1 and {MAX_BULK_EDIT_MAX_RECORDS}"
            )));
        }
        Ok(())
    }
}

impl AttricatRepository {
    async fn require_agent_settings_permission(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<(), RepositoryError> {
        if self
            .is_authorized(
                actor_id,
                workspace_id,
                AGENT_SETTINGS_PERMISSION,
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

    /// The agent settings, for someone who may change them.
    pub async fn configured_agent_settings(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<AgentSettings, RepositoryError> {
        self.require_agent_settings_permission(actor_id, workspace_id)
            .await?;
        self.agent_settings().await
    }

    /// Replaces the workspace's agent settings.
    pub async fn update_agent_settings(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
        settings: AgentSettings,
    ) -> Result<AgentSettings, RepositoryError> {
        self.require_agent_settings_permission(actor_id, workspace_id)
            .await?;
        settings.validate()?;
        let mut transaction = self.pool.begin().await?;
        let updated = sqlx::query(
            "UPDATE workspaces SET settings = jsonb_set(settings, '{agent}', $1::jsonb, true), updated_at = now() WHERE id = $2 AND deleted_at IS NULL",
        )
        .bind(serde_json::to_value(settings).expect("agent settings serialize"))
        .bind(workspace_id)
        .execute(&mut *transaction)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(RepositoryError::NotFound("workspace"));
        }
        self.commit_mutation(transaction).await?;
        Ok(settings)
    }

    /// The scoped workspace's agent settings. Stored settings are written only
    /// through validation; anything unreadable falls back to the defaults.
    pub async fn agent_settings(&self) -> Result<AgentSettings, RepositoryError> {
        let stored: Option<serde_json::Value> = sqlx::query_scalar(
            "SELECT settings->'agent' FROM workspaces WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(self.workspace_id_for_runtime())
        .fetch_optional(&self.pool)
        .await?
        .flatten();
        Ok(stored
            .and_then(|stored| serde_json::from_value(stored).ok())
            .unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn missing_settings_use_the_defaults() {
        let settings: AgentSettings = serde_json::from_value(json!({})).unwrap();
        assert_eq!(settings, AgentSettings::default());
        assert_eq!(settings.bulk_edits, BulkEditPolicy::PerProposal);
        assert_eq!(settings.bulk_edit_max_records, 500);
        let settings: AgentSettings =
            serde_json::from_value(json!({"bulk_edits": "per_record"})).unwrap();
        assert_eq!(settings.bulk_edits, BulkEditPolicy::PerRecord);
    }

    #[test]
    fn limits_stay_within_bounds() {
        let with = |bulk_edit_max_records| AgentSettings {
            bulk_edit_max_records,
            ..AgentSettings::default()
        };
        assert!(with(0).validate().is_err());
        assert!(with(1).validate().is_ok());
        assert!(with(MAX_BULK_EDIT_MAX_RECORDS).validate().is_ok());
        assert!(with(MAX_BULK_EDIT_MAX_RECORDS + 1).validate().is_err());
        assert!(
            serde_json::from_value::<AgentSettings>(json!({"bulk_edits": "sometimes"})).is_err()
        );
        assert!(serde_json::from_value::<AgentSettings>(json!({"other": true})).is_err());
    }
}
