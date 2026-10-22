//! Connector page v2 and the withdrawn feed (attricat:host 1.1).
//!
//! A v2 row carries what an exporter needs to build a destination listing
//! without other reads: the record ID and content revision, values resolved
//! through the run context's fallback chain (not only those set directly in
//! it), relationship targets, file references, and optionally the resolved
//! values of related records (for variants and parent/child listings).
//! The withdrawn feed lists records unpublished from, or deleted in, the run's
//! channel since a cursor, so an exporter can close them out on the destination.

use std::collections::HashMap;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use uuid::Uuid;

use super::{
    AttricatRepository, ExtensionAttricatPageRequest, FormAttributeValue, RepositoryError,
    record_values::resolve_on_path,
};
use crate::model::{Attribute, AttributeValue, Record};

/// The most relationship attributes one page may expand.
pub const MAX_EXPANDED_RELATIONSHIPS: usize = 4;
/// The most related records expanded per row and attribute.
pub const MAX_EXPANDED_TARGETS: usize = 50;
pub const MAX_WITHDRAWN_PAGE: u32 = 100;
const MAX_WITHDRAWN_CURSOR_BYTES: usize = 512;

/// A v2 connector page request; the host supplies the scope.
#[derive(Clone, Debug)]
pub struct ConnectorPageV2Request {
    pub blueprint_id: Uuid,
    pub blueprint_version: i64,
    pub context_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub cursor: Option<String>,
    pub limit: u32,
    pub expand: Vec<String>,
    /// Confines the page to these records (a resync or triggered run).
    pub record_ids: Option<Vec<Uuid>>,
}

/// One record's values resolved in a context.
#[derive(Default)]
struct ResolvedValues {
    values: Map<String, Value>,
    relationships: Map<String, Value>,
    files: Map<String, Value>,
}

/// Resolves `values` (and current file values) of one record in the context
/// `path`, honoring each attribute's fallback setting.
fn resolve_values(
    attributes: &[Attribute],
    values: &[AttributeValue],
    files: &[FormAttributeValue],
    path: &[Uuid],
) -> ResolvedValues {
    let mut resolved = ResolvedValues::default();
    for attribute in attributes {
        let inherit = attribute.context_fallback != "none";
        match attribute.value_type.as_str() {
            "relationship" => {
                if let Some((_, targets)) = resolve_on_path(path, inherit, |context| {
                    let targets: Vec<Value> = values
                        .iter()
                        .filter(|value| {
                            value.attribute_id == attribute.id
                                && value.context_id == Some(context)
                                && value.active
                        })
                        .filter_map(|value| value.relationship_target_record_id)
                        .map(|target| json!(target))
                        .collect();
                    (!targets.is_empty()).then_some(targets)
                }) {
                    resolved
                        .relationships
                        .insert(attribute.code.clone(), Value::Array(targets));
                }
            }
            "file" => {
                if let Some((_, references)) = resolve_on_path(path, inherit, |context| {
                    files.iter().find_map(|value| match value {
                        FormAttributeValue::File {
                            attribute_code,
                            context_id,
                            files,
                        } if attribute_code == &attribute.code
                            && *context_id == Some(context)
                            && !files.is_empty() =>
                        {
                            Some(
                                files
                                    .iter()
                                    .map(|file| {
                                        json!({
                                            "file_id": file.id,
                                            "filename": file.filename,
                                            "mime_type": file.mime_type,
                                            "byte_size": file.byte_size,
                                            "sha256": file.sha256,
                                        })
                                    })
                                    .collect::<Vec<_>>(),
                            )
                        }
                        _ => None,
                    })
                }) {
                    resolved
                        .files
                        .insert(attribute.code.clone(), Value::Array(references));
                }
            }
            _ => {
                if let Some((_, value)) = resolve_on_path(path, inherit, |context| {
                    values
                        .iter()
                        .find(|value| {
                            value.attribute_id == attribute.id
                                && value.context_id == Some(context)
                                && value.relationship_target_record_id.is_none()
                        })
                        .map(|value| value.value.clone())
                }) {
                    resolved.values.insert(attribute.code.clone(), value);
                }
            }
        }
    }
    resolved
}

#[derive(Serialize, Deserialize)]
struct WithdrawnCursor {
    workspace_id: Uuid,
    blueprint_id: Uuid,
    channel_id: Option<Uuid>,
    after_sequence: i64,
}

impl AttricatRepository {
    async fn resolved_record(
        &self,
        record: &Record,
        path: &[Uuid],
        attributes: &[Attribute],
        snapshot_at: DateTime<Utc>,
    ) -> Result<ResolvedValues, RepositoryError> {
        let values = self
            .extension_attricat_values_at(record.id, snapshot_at)
            .await?;
        let files = self.file_form_values(record.id).await?;
        Ok(resolve_values(attributes, &values, &files, path))
    }

    /// Whether `record_id` is currently published in `channel_id`.
    async fn published_in(
        &self,
        record_id: Uuid,
        channel_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM record_channel_publications WHERE workspace_id=$1 AND record_id=$2 AND context_id=$3 AND published_at IS NOT NULL)")
            .bind(self.workspace_id.0).bind(record_id).bind(channel_id)
            .fetch_one(&self.pool).await?)
    }

    /// A page of v2 connector rows. Values are as of the page's snapshot;
    /// expanded related records are read as of the same snapshot.
    pub async fn extension_connector_page_v2(
        &self,
        request: ConnectorPageV2Request,
    ) -> Result<Value, RepositoryError> {
        if request.expand.len() > MAX_EXPANDED_RELATIONSHIPS {
            return Err(RepositoryError::InvalidExtension(format!(
                "expand at most {MAX_EXPANDED_RELATIONSHIPS} relationships"
            )));
        }
        let page = self
            .extension_attricat_page_of(
                ExtensionAttricatPageRequest {
                    blueprint_id: request.blueprint_id,
                    blueprint_version: request.blueprint_version,
                    context_id: Some(request.context_id),
                    publication_context_id: request.channel_id,
                    cursor: request.cursor,
                    limit: request.limit,
                },
                request.record_ids.as_deref(),
            )
            .await?;
        let attributes = self
            .list_attributes(request.blueprint_id, request.blueprint_version)
            .await?;
        for code in &request.expand {
            if !attributes
                .iter()
                .any(|attribute| &attribute.code == code && attribute.value_type == "relationship")
            {
                return Err(RepositoryError::InvalidExtension(format!(
                    "'{code}' is not a relationship attribute of the blueprint"
                )));
            }
        }
        let scope = self.preview_scope(request.context_id).await?;
        let path = scope.tree.path(request.context_id, true)?;
        let mut target_attributes: HashMap<(Uuid, i64), Vec<Attribute>> = HashMap::new();
        let mut rows = Vec::with_capacity(page.records.len());
        for record in &page.records {
            let resolved = self
                .resolved_record(record, &path, &attributes, page.snapshot_at)
                .await?;
            let mut related = Map::new();
            for code in &request.expand {
                let Some(targets) = resolved.relationships.get(code).and_then(Value::as_array)
                else {
                    continue;
                };
                let mut items = Vec::new();
                for target in targets.iter().take(MAX_EXPANDED_TARGETS) {
                    let Some(target_id) = target.as_str().and_then(|id| id.parse().ok()) else {
                        continue;
                    };
                    let Some(target) = self.get_record(target_id).await? else {
                        continue;
                    };
                    let key = (target.blueprint_id, target.blueprint_version);
                    if let std::collections::hash_map::Entry::Vacant(entry) =
                        target_attributes.entry(key)
                    {
                        entry.insert(self.list_attributes(key.0, key.1).await?);
                    }
                    let target_resolved = self
                        .resolved_record(&target, &path, &target_attributes[&key], page.snapshot_at)
                        .await?;
                    let published = match request.channel_id {
                        Some(channel) => Some(self.published_in(target.id, channel).await?),
                        None => None,
                    };
                    items.push(json!({
                        "record_id": target.id,
                        "blueprint_id": target.blueprint_id,
                        "blueprint_version": target.blueprint_version,
                        "content_revision": target.content_revision,
                        "published": published,
                        "values": target_resolved.values,
                        "relationships": target_resolved.relationships,
                        "files": target_resolved.files,
                    }));
                }
                related.insert(code.clone(), Value::Array(items));
            }
            rows.push(json!({
                "record_id": record.id,
                "blueprint_version": record.blueprint_version,
                "content_revision": record.content_revision,
                "updated_at": record.updated_at,
                "values": resolved.values,
                "relationships": resolved.relationships,
                "files": resolved.files,
                "related": related,
            }));
        }
        Ok(json!({"rows": rows, "next_cursor": page.next_cursor, "read_at": page.snapshot_at}))
    }

    /// Records of `blueprint_id` withdrawn from `channel_id` (unpublished and
    /// still unpublished) or deleted since the cursor, in event order.
    pub async fn extension_withdrawn_records(
        &self,
        blueprint_id: Uuid,
        channel_id: Option<Uuid>,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<Value, RepositoryError> {
        if limit == 0 || limit > MAX_WITHDRAWN_PAGE {
            return Err(RepositoryError::InvalidExtension(format!(
                "withdrawn page limit must be 1-{MAX_WITHDRAWN_PAGE}"
            )));
        }
        let invalid_cursor =
            || RepositoryError::InvalidExtension("withdrawn cursor is invalid".into());
        let after_sequence = match cursor {
            Some(cursor) if !cursor.is_empty() => {
                if cursor.len() > MAX_WITHDRAWN_CURSOR_BYTES {
                    return Err(invalid_cursor());
                }
                let decoded: WithdrawnCursor = URL_SAFE_NO_PAD
                    .decode(cursor)
                    .ok()
                    .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                    .ok_or_else(invalid_cursor)?;
                if decoded.workspace_id != self.workspace_id.0
                    || decoded.blueprint_id != blueprint_id
                    || decoded.channel_id != channel_id
                {
                    return Err(RepositoryError::InvalidExtension(
                        "withdrawn cursor does not match this run's scope".into(),
                    ));
                }
                decoded.after_sequence
            }
            _ => 0,
        };
        let events: Vec<(i64, String, Uuid, DateTime<Utc>)> = sqlx::query_as(
            "SELECT e.sequence,e.event_type,e.aggregate_id,e.occurred_at FROM domain_events e \
             WHERE e.workspace_id=$1 AND e.aggregate_kind='record' AND e.sequence>$2 \
               AND e.event_type IN ('record.unpublished.v1','record.deleted.v1') \
               AND ($3::uuid IS NULL OR e.event_type<>'record.unpublished.v1' OR e.payload->>'context_id'=$3::text) \
               AND EXISTS (SELECT 1 FROM records r WHERE r.workspace_id=e.workspace_id AND r.id=e.aggregate_id AND r.blueprint_id=$4) \
             ORDER BY e.sequence LIMIT $5",
        )
        .bind(self.workspace_id.0)
        .bind(after_sequence)
        .bind(channel_id)
        .bind(blueprint_id)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;
        let mut items = Vec::with_capacity(events.len());
        let mut last = after_sequence;
        for (sequence, event_type, record_id, occurred_at) in &events {
            last = *sequence;
            let reason = if event_type == "record.deleted.v1" {
                "deleted"
            } else {
                // A record published again since is no longer withdrawn.
                if let Some(channel) = channel_id
                    && self.published_in(*record_id, channel).await?
                {
                    continue;
                }
                "unpublished"
            };
            items.push(json!({
                "record_id": record_id,
                "reason": reason,
                "record_revision": sequence,
                "occurred_at": occurred_at,
            }));
        }
        let next_cursor = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&WithdrawnCursor {
                workspace_id: self.workspace_id.0,
                blueprint_id,
                channel_id,
                after_sequence: last,
            })
            .expect("withdrawn cursor serializes"),
        );
        Ok(json!({
            "records": items,
            "next_cursor": next_cursor,
            "more": events.len() == limit as usize,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attribute(code: &str, value_type: &str, fallback: &str) -> Attribute {
        Attribute {
            id: Uuid::new_v4(),
            blueprint_id: Uuid::nil(),
            blueprint_version: 1,
            code: code.into(),
            name: None,
            description: None,
            value_type: value_type.into(),
            value_schema: None,
            extension_type: None,
            default_value: None,
            file_policy: None,
            target_blueprint_code: None,
            target_blueprint_codes: Vec::new(),
            cardinality: None,
            target_cardinality: None,
            hierarchy: None,
            tags: json!([]),
            context_fallback: fallback.into(),
            context_editable: "all".into(),
            readonly: false,
            position: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            deleted_at: None,
        }
    }

    #[test]
    fn values_resolve_through_the_context_path() {
        let (child, parent) = (Uuid::new_v4(), Uuid::new_v4());
        let title = attribute("title", "string", "parent");
        let local = attribute("sku", "string", "none");
        let value = |attribute: &Attribute, context: Uuid, value: Value| AttributeValue {
            id: Uuid::new_v4(),
            record_id: Uuid::nil(),
            attribute_id: attribute.id,
            value,
            relationship_target_record_id: None,
            active: true,
            context_id: Some(context),
            created_at: Utc::now(),
        };
        let values = vec![
            value(&title, parent, json!("Inherited")),
            value(&local, parent, json!("not inherited")),
        ];
        let resolved = resolve_values(&[title, local], &values, &[], &[child, parent]);
        assert_eq!(resolved.values.get("title"), Some(&json!("Inherited")));
        assert!(resolved.values.get("sku").is_none());
    }
}
