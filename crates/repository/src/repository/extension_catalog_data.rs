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

pub const MAX_EXTENSION_CATALOG_PAGE_SIZE: u32 = 100;
pub const MAX_EXTENSION_LOOKUP_VALUE_BYTES: usize = 512;

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
        let workspace_id = self.workspace_id.ok_or_else(|| {
            RepositoryError::InvalidExtension("extension catalog reads require a workspace".into())
        })?;
        let cursor = request
            .cursor
            .as_deref()
            .map(decode_extension_cursor)
            .transpose()?;
        let (snapshot_at, created_at, entity_id) = if let Some(cursor) = cursor {
            if cursor.workspace_id != workspace_id
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
            (Utc::now(), None, None)
        };
        if let Some(context_id) = request.context_id {
            let exists: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM attribute_contexts WHERE id=$1 AND workspace_id=$2 AND deleted_at IS NULL",
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
            "SELECT e.id,e.blueprint_id,e.blueprint_version,e.projections,e.system_tags,e.system_metadata,e.created_at,e.updated_at,e.deleted_at \
             FROM entities e \
             WHERE e.workspace_id=$1 AND e.deleted_at IS NULL AND e.blueprint_id=$2 AND e.blueprint_version=$3 \
               AND e.created_at <= $4 \
               AND ($5::timestamptz IS NULL OR (e.created_at,e.id) > ($5,$6)) \
               AND ($7::uuid IS NULL OR EXISTS (SELECT 1 FROM entity_channel_publications p WHERE p.workspace_id=e.workspace_id AND p.entity_id=e.id AND p.context_id=$7)) \
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
        let workspace_id = self.workspace_id.ok_or_else(|| {
            RepositoryError::InvalidExtension("extension lookup requires a workspace".into())
        })?;
        Ok(sqlx::query_as::<_, Db<Entity>>(
            "SELECT e.id,e.blueprint_id,e.blueprint_version,e.projections,e.system_tags,e.system_metadata,e.created_at,e.updated_at,e.deleted_at \
             FROM entities e JOIN attribute_values v ON v.entity_id=e.id AND v.workspace_id=e.workspace_id AND v.active \
             WHERE e.workspace_id=$1 AND e.deleted_at IS NULL AND e.blueprint_id=$2 AND e.blueprint_version=$3 \
               AND v.attribute_id=$4 AND v.relationship_target_entity_id IS NULL AND v.value = to_jsonb($5::text) LIMIT 2",
        ).bind(workspace_id).bind(blueprint_id).bind(blueprint_version).bind(attribute_id).bind(value)
        .fetch_all(&self.pool).await?.into_domain().into_iter().next())
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
