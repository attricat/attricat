//! Workspace settings for the catalogue agent, stored under
//! `workspaces.settings.agent`.

use serde::{Deserialize, Serialize};

use super::{AttricatRepository, RepositoryError};

/// How many records one bulk edit may change unless a workspace sets another
/// limit.
pub const DEFAULT_BULK_EDIT_MAX_RECORDS: u32 = 500;

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

impl AttricatRepository {
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
}
