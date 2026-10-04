//! Bounded, workspace-scoped catalog data access for extension host calls.
//!
//! Cursors carry every filter and the snapshot high-water mark. They are opaque
//! to components and are rejected if replayed by another workspace or with a
//! different request, so an export cannot silently skip or duplicate rows.

use super::*;
use crate::persistence_rows::{Db, IntoDomain};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_EXTENSION_CATALOG_PAGE_SIZE: u32 = 100;
pub const MAX_EXTENSION_LOOKUP_VALUE_BYTES: usize = 512;
pub const MAX_EXTENSION_BATCH_INTENTS: usize = 100;
pub const MAX_EXTENSION_BATCH_KEY_BYTES: usize = 256;
pub const MAX_EXTENSION_INTENT_KEY_BYTES: usize = 128;

/// A bounded, host-owned mutation envelope. The component never receives a
/// repository handle: this payload is validated then committed through the
/// same entity mutation transaction that writes audit and outbox records.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionCatalogBatch {
    pub batch_key: String,
    pub dry_run: bool,
    pub intents: Vec<ExtensionCatalogIntent>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExtensionCatalogIntent {
    Create {
        intent_key: String,
        blueprint_id: Uuid,
        blueprint_version: i64,
        values: Vec<NewAttributeValue>,
        #[serde(default)]
        system_tags: Vec<String>,
        #[serde(default = "empty_object")]
        system_metadata: Value,
    },
    Update {
        intent_key: String,
        entity_id: Uuid,
        #[serde(default)]
        values: Vec<NewAttributeValue>,
        #[serde(default)]
        relationships: Vec<RelationshipTargets>,
    },
    Relationships {
        intent_key: String,
        entity_id: Uuid,
        relationships: Vec<RelationshipTargets>,
    },
    /// Creates when the declared business value is absent, otherwise updates
    /// that one entity. The intent marker makes both branches replay-safe.
    Upsert {
        intent_key: String,
        blueprint_id: Uuid,
        blueprint_version: i64,
        lookup_attribute_id: Uuid,
        lookup_value: String,
        #[serde(default)]
        values: Vec<NewAttributeValue>,
        #[serde(default)]
        relationships: Vec<RelationshipTargets>,
        #[serde(default)]
        system_tags: Vec<String>,
        #[serde(default = "empty_object")]
        system_metadata: Value,
    },
    /// Patches only the calling extension's annotation namespace. Requires
    /// the separately granted `catalog.annotations.write` capability.
    Annotate {
        intent_key: String,
        entity_id: Uuid,
        #[serde(default)]
        add_tags: Vec<String>,
        #[serde(default)]
        remove_tags: Vec<String>,
        #[serde(default)]
        set_metadata: std::collections::BTreeMap<String, Value>,
        #[serde(default)]
        remove_metadata: Vec<String>,
        #[serde(default)]
        expected_revision: Option<i64>,
    },
}

impl ExtensionCatalogIntent {
    pub fn is_annotation(&self) -> bool {
        matches!(self, Self::Annotate { .. })
    }

    /// The existing entity an intent targets; creates and upserts have none.
    pub fn target_entity_id(&self) -> Option<Uuid> {
        match self {
            Self::Update { entity_id, .. }
            | Self::Relationships { entity_id, .. }
            | Self::Annotate { entity_id, .. } => Some(*entity_id),
            Self::Create { .. } | Self::Upsert { .. } => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionCatalogIntentStatus {
    Applied,
    AlreadyApplied,
    Validated,
    Rejected,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtensionCatalogIntentOutcome {
    pub intent_key: String,
    pub status: ExtensionCatalogIntentStatus,
    pub entity_id: Option<Uuid>,
    pub error: Option<String>,
    /// The extension namespace revision after an applied annotation intent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotation_revision: Option<i64>,
}

fn empty_object() -> Value {
    Value::Object(Default::default())
}

#[derive(Clone, Debug)]
pub struct ExtensionCatalogPageRequest {
    pub blueprint_id: Uuid,
    pub blueprint_version: i64,
    pub context_id: Option<Uuid>,
    pub publication_context_id: Option<Uuid>,
    pub cursor: Option<String>,
    pub limit: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExtensionCatalogPage {
    pub entities: Vec<Entity>,
    pub next_cursor: Option<String>,
    #[serde(skip_serializing)]
    pub snapshot_at: DateTime<Utc>,
}

/// A stable, ordered page of entity mutation events. `next_cursor` carries the
/// high-water sequence captured by the first request, so events committed after
/// that request are never interleaved into an in-progress catch-up.
#[derive(Clone, Debug, Serialize)]
pub struct ExtensionCatalogChangePage {
    pub events: Vec<crate::domain_events::DomainEvent>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ExtensionCatalogCursor {
    workspace_id: Uuid,
    blueprint_id: Uuid,
    blueprint_version: i64,
    context_id: Option<Uuid>,
    publication_context_id: Option<Uuid>,
    snapshot_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
    entity_id: Uuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ExtensionCatalogChangeCursor {
    workspace_id: Uuid,
    blueprint_id: Uuid,
    blueprint_version: i64,
    high_water_sequence: i64,
    after_sequence: i64,
}

impl CatalogRepository {
    /// Lists an immutable-as-of-start entity set. New entities created after
    /// `snapshot_at` are deliberately left for a later export; updates do not
    /// change ordering, so a page cannot move between cursors.
    pub async fn extension_catalog_page(
        &self,
        request: ExtensionCatalogPageRequest,
    ) -> Result<ExtensionCatalogPage, RepositoryError> {
        if request.limit == 0 || request.limit > MAX_EXTENSION_CATALOG_PAGE_SIZE {
            return Err(RepositoryError::InvalidExtension(format!(
                "catalog page limit must be 1-{MAX_EXTENSION_CATALOG_PAGE_SIZE}"
            )));
        }
        let workspace_id = self.workspace_id.0;
        let cursor = request
            .cursor
            .as_deref()
            .map(decode_extension_cursor)
            .transpose()?;
        let (snapshot_at, created_at, entity_id) = if let Some(cursor) = cursor {
            if cursor.snapshot_at < Utc::now() - chrono::Duration::days(30)
                || cursor.workspace_id != workspace_id
                || cursor.blueprint_id != request.blueprint_id
                || cursor.blueprint_version != request.blueprint_version
                || cursor.context_id != request.context_id
                || cursor.publication_context_id != request.publication_context_id
            {
                return Err(RepositoryError::InvalidExtension(
                    "catalog cursor does not match this workspace or filter".into(),
                ));
            }
            (
                cursor.snapshot_at,
                Some(cursor.created_at),
                Some(cursor.entity_id),
            )
        } else {
            // The database is the ordering authority. Its clock avoids losing a
            // row when the host process clock lags the database clock.
            let snapshot_at: DateTime<Utc> = sqlx::query_scalar("SELECT clock_timestamp()")
                .fetch_one(&self.pool)
                .await?;
            (snapshot_at, None, None)
        };
        if let Some(channel_id) = request.publication_context_id {
            let enabled: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM publication_channels WHERE workspace_id=$1 AND context_id=$2 AND enabled)",
            )
            .bind(workspace_id)
            .bind(channel_id)
            .fetch_one(&self.pool)
            .await?;
            if !enabled {
                return Err(RepositoryError::InvalidExtension(
                    "export publication channel is not enabled".into(),
                ));
            }
        }
        if let Some(context_id) = request.context_id {
            let exists: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM attribute_contexts WHERE id=$1 AND workspace_id=$2",
            )
            .bind(context_id)
            .bind(workspace_id)
            .fetch_optional(&self.pool)
            .await?;
            if exists.is_none() {
                return Err(RepositoryError::NotFound("context"));
            }
        }
        let rows = sqlx::query_as::<_, Db<Entity>>(
            "SELECT e.id,e.blueprint_id,e.blueprint_version,e.projections,e.system_tags,e.system_metadata,('attricat.sample'=ANY(e.system_tags)) AS is_sample,e.created_at,e.updated_at,e.deleted_at \
             FROM entities e \
             WHERE e.workspace_id=$1 AND (e.deleted_at IS NULL OR e.deleted_at > $4) AND e.blueprint_id=$2 AND e.blueprint_version=$3 \
               AND e.created_at <= $4 \
               AND ($5::timestamptz IS NULL OR (e.created_at,e.id) > ($5,$6)) \
               AND ($7::uuid IS NULL OR EXISTS (SELECT 1 FROM entity_channel_publications p WHERE p.workspace_id=e.workspace_id AND p.entity_id=e.id AND p.context_id=$7 AND p.published_at IS NOT NULL)) \
             ORDER BY e.created_at,e.id LIMIT $8",
        )
        .bind(workspace_id).bind(request.blueprint_id).bind(request.blueprint_version)
        .bind(snapshot_at).bind(created_at).bind(entity_id).bind(request.publication_context_id)
        .bind(i64::from(request.limit) + 1).fetch_all(&self.pool).await?;
        let mut entities = rows.into_domain();
        let next_cursor = if entities.len() > request.limit as usize {
            entities.pop();
            entities.last().map(|entity| {
                encode_extension_cursor(&ExtensionCatalogCursor {
                    workspace_id,
                    blueprint_id: request.blueprint_id,
                    blueprint_version: request.blueprint_version,
                    context_id: request.context_id,
                    publication_context_id: request.publication_context_id,
                    snapshot_at,
                    created_at: entity.created_at,
                    entity_id: entity.id,
                })
            })
        } else {
            None
        };
        Ok(ExtensionCatalogPage {
            entities,
            next_cursor,
            snapshot_at,
        })
    }

    /// Reads entity mutation events in sequence order. This is deliberately a
    /// separate cursor from snapshots: a client first completes a snapshot,
    /// then persists the returned change cursor and consumes changes.
    pub async fn extension_catalog_changes(
        &self,
        blueprint_id: Uuid,
        blueprint_version: i64,
        cursor: Option<String>,
        limit: u32,
    ) -> Result<ExtensionCatalogChangePage, RepositoryError> {
        if limit == 0 || limit > MAX_EXTENSION_CATALOG_PAGE_SIZE {
            return Err(RepositoryError::InvalidExtension(format!(
                "catalog change limit must be 1-{MAX_EXTENSION_CATALOG_PAGE_SIZE}"
            )));
        }
        let workspace_id = self.workspace_id.0;
        let cursor = cursor
            .as_deref()
            .map(decode_extension_change_cursor)
            .transpose()?;
        let (high_water_sequence, after_sequence) = if let Some(cursor) = cursor {
            if cursor.workspace_id != workspace_id
                || cursor.blueprint_id != blueprint_id
                || cursor.blueprint_version != blueprint_version
            {
                return Err(RepositoryError::InvalidExtension(
                    "catalog change cursor does not match this workspace or filter".into(),
                ));
            }
            (cursor.high_water_sequence, cursor.after_sequence)
        } else {
            let high_water_sequence: i64 = sqlx::query_scalar(
                "SELECT COALESCE(MAX(sequence), 0) FROM domain_events WHERE workspace_id=$1",
            )
            .bind(workspace_id)
            .fetch_one(&self.pool)
            .await?;
            (high_water_sequence, 0)
        };
        let events = sqlx::query_as::<_, crate::domain_events::DomainEvent>(
            "SELECT id,sequence,workspace_id,occurred_at,event_type,aggregate_kind,aggregate_id,correlation_id,causation_id,source_kind,source_name,metadata,payload \
             FROM domain_events WHERE workspace_id=$1 AND aggregate_kind='entity' AND sequence > $2 AND sequence <= $3 \
               AND payload->>'blueprint_id'=$4 AND (payload->>'blueprint_version')::bigint=$5 \
             ORDER BY sequence LIMIT $6",
        ).bind(workspace_id).bind(after_sequence).bind(high_water_sequence).bind(blueprint_id.to_string()).bind(blueprint_version).bind(i64::from(limit) + 1).fetch_all(&self.pool).await?;
        let mut events = events;
        let next_cursor = if events.len() > limit as usize {
            events.pop();
            events.last().map(|event| {
                encode_extension_change_cursor(&ExtensionCatalogChangeCursor {
                    workspace_id,
                    blueprint_id,
                    blueprint_version,
                    high_water_sequence,
                    after_sequence: event.sequence,
                })
            })
        } else {
            None
        };
        Ok(ExtensionCatalogChangePage {
            events,
            next_cursor,
        })
    }

    /// Resolves one business identifier only within a declared blueprint and
    /// attribute. It intentionally is not a general search endpoint.
    pub async fn extension_catalog_lookup(
        &self,
        blueprint_id: Uuid,
        blueprint_version: i64,
        attribute_id: Uuid,
        value: &str,
    ) -> Result<Option<Entity>, RepositoryError> {
        if value.is_empty() || value.len() > MAX_EXTENSION_LOOKUP_VALUE_BYTES {
            return Err(RepositoryError::InvalidExtension(
                "lookup value must be 1-512 bytes".into(),
            ));
        }
        // Resolve exactly as an upsert with this lookup would, so a read never
        // names a different entity than the one a write would update.
        let mut transaction = self.pool.begin().await?;
        let Some(entity_id) = self
            .extension_lookup(
                &mut transaction,
                blueprint_id,
                blueprint_version,
                attribute_id,
                value,
                super::extension_catalog_commands::ExtensionLookupMode::Read,
            )
            .await?
        else {
            return Ok(None);
        };
        let entity = sqlx::query_as::<_, Db<Entity>>(
            "SELECT id,blueprint_id,blueprint_version,projections,system_tags,system_metadata,('attricat.sample'=ANY(system_tags)) AS is_sample,created_at,updated_at,deleted_at FROM entities WHERE id=$1 AND workspace_id=$2 AND deleted_at IS NULL",
        )
        .bind(entity_id)
        .bind(self.workspace_id.0)
        .fetch_optional(&mut *transaction)
        .await?
        .into_domain();
        transaction.commit().await?;
        Ok(entity)
    }
}

fn decode_extension_cursor(value: &str) -> Result<ExtensionCatalogCursor, RepositoryError> {
    if value.len() > 2048 {
        return Err(RepositoryError::InvalidExtension(
            "catalog cursor is invalid".into(),
        ));
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| RepositoryError::InvalidExtension("catalog cursor is invalid".into()))?;
    serde_json::from_slice(&bytes)
        .map_err(|_| RepositoryError::InvalidExtension("catalog cursor is invalid".into()))
}
fn encode_extension_cursor(cursor: &ExtensionCatalogCursor) -> String {
    URL_SAFE_NO_PAD.encode(serde_json::to_vec(cursor).expect("extension cursor serializes"))
}
fn decode_extension_change_cursor(
    value: &str,
) -> Result<ExtensionCatalogChangeCursor, RepositoryError> {
    if value.len() > 2048 {
        return Err(RepositoryError::InvalidExtension(
            "catalog change cursor is invalid".into(),
        ));
    }
    let bytes = URL_SAFE_NO_PAD.decode(value).map_err(|_| {
        RepositoryError::InvalidExtension("catalog change cursor is invalid".into())
    })?;
    serde_json::from_slice(&bytes)
        .map_err(|_| RepositoryError::InvalidExtension("catalog change cursor is invalid".into()))
}
fn encode_extension_change_cursor(cursor: &ExtensionCatalogChangeCursor) -> String {
    URL_SAFE_NO_PAD.encode(serde_json::to_vec(cursor).expect("extension change cursor serializes"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cursors_are_bounded_and_round_trip() {
        let cursor = ExtensionCatalogCursor {
            workspace_id: Uuid::nil(),
            blueprint_id: Uuid::nil(),
            blueprint_version: 1,
            context_id: None,
            publication_context_id: None,
            snapshot_at: Utc::now(),
            created_at: Utc::now(),
            entity_id: Uuid::nil(),
        };
        assert_eq!(
            decode_extension_cursor(&encode_extension_cursor(&cursor))
                .unwrap()
                .workspace_id,
            Uuid::nil()
        );
        assert!(decode_extension_cursor(&"x".repeat(2049)).is_err());
    }
}
