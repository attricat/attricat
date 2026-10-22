//! Connector page v2 and the withdrawn feed (attricat:host 1.1).
//!
//! A v2 row carries what an exporter needs to build a destination listing
//! without other reads: the record ID and content revision, values resolved
//! through the run context's fallback chain (not only those set directly in
//! it), relationship targets, file references, and optionally the resolved
//! values of related records (for variants and parent/child listings).
//! The withdrawn feed lists records unpublished from, or deleted in, the run's
//! channel since a cursor, so an exporter can close them out on the destination.

use std::collections::{HashMap, HashSet};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use uuid::Uuid;

use super::{
    AttricatRepository, ExtensionAttricatPageRequest, RepositoryError,
    record_values::resolve_on_path,
};
use crate::model::{Attribute, AttributeValue, Record};
use crate::persistence_rows::{Db, IntoDomain};

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

/// A file reference: record, attribute code, context, and the file's ID,
/// name, media type, size and SHA-256.
type FileReferenceRow = (
    Uuid,
    String,
    Option<Uuid>,
    Uuid,
    String,
    String,
    i64,
    String,
);

/// One record's current file references by attribute code and context.
type FileReferences = HashMap<(String, Option<Uuid>), Vec<Value>>;

/// Resolves `values` (and current file values) of one record in the context
/// `path`, honoring each attribute's fallback setting.
fn resolve_values(
    attributes: &[Attribute],
    values: &[AttributeValue],
    files: &FileReferences,
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
                    files
                        .get(&(attribute.code.clone(), Some(context)))
                        .filter(|references| !references.is_empty())
                        .cloned()
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
    /// Values (as of `snapshot_at`) and current file references of
    /// `records`, read in one query each.
    async fn values_and_files_of(
        &self,
        record_ids: &[Uuid],
        snapshot_at: DateTime<Utc>,
    ) -> Result<
        (
            HashMap<Uuid, Vec<AttributeValue>>,
            HashMap<Uuid, FileReferences>,
        ),
        RepositoryError,
    > {
        let mut values: HashMap<Uuid, Vec<AttributeValue>> = HashMap::new();
        let mut files: HashMap<Uuid, FileReferences> = HashMap::new();
        if record_ids.is_empty() {
            return Ok((values, files));
        }
        for value in self
            .extension_attricat_values_of_at(record_ids, snapshot_at)
            .await?
        {
            values.entry(value.record_id).or_default().push(value);
        }
        let rows: Vec<FileReferenceRow> = sqlx::query_as(
            "SELECT av.record_id,a.code,av.context_id,f.id,f.display_filename,f.mime_type,f.byte_size,f.sha256 \
             FROM attribute_values av \
             JOIN records rec ON rec.id=av.record_id AND rec.workspace_id=av.workspace_id \
             JOIN attributes a ON a.id=av.attribute_id AND a.blueprint_id=rec.blueprint_id AND a.blueprint_version=rec.blueprint_version AND a.value_type='file' \
             JOIN attribute_file_references r ON r.attribute_value_id=av.id \
             JOIN files f ON f.id=r.file_id AND f.workspace_id=av.workspace_id AND f.deleted_at IS NULL \
             WHERE av.workspace_id=$1 AND av.record_id=ANY($2) \
             ORDER BY av.record_id,a.position,av.context_id,r.position",
        )
        .bind(self.workspace_id.0)
        .bind(record_ids)
        .fetch_all(&self.pool)
        .await?;
        for (record_id, code, context_id, file_id, filename, mime_type, byte_size, sha256) in rows {
            files
                .entry(record_id)
                .or_default()
                .entry((code, context_id))
                .or_default()
                .push(json!({
                    "file_id": file_id,
                    "filename": filename,
                    "mime_type": mime_type,
                    "byte_size": byte_size,
                    "sha256": sha256,
                }));
        }
        Ok((values, files))
    }

    /// Which of `record_ids` are currently published in `channel_id`.
    async fn published_of(
        &self,
        record_ids: &[Uuid],
        channel_id: Uuid,
    ) -> Result<HashSet<Uuid>, RepositoryError> {
        let published: Vec<Uuid> = sqlx::query_scalar("SELECT record_id FROM record_channel_publications WHERE workspace_id=$1 AND record_id=ANY($2) AND context_id=$3 AND published_at IS NOT NULL")
            .bind(self.workspace_id.0).bind(record_ids).bind(channel_id)
            .fetch_all(&self.pool).await?;
        Ok(published.into_iter().collect())
    }

    /// Whether `record_id` is currently published in `channel_id`.
    async fn published_in(
        &self,
        record_id: Uuid,
        channel_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        Ok(self
            .published_of(&[record_id], channel_id)
            .await?
            .contains(&record_id))
    }

    /// Live records among `record_ids`, by ID.
    async fn records_of(
        &self,
        record_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, Record>, RepositoryError> {
        if record_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let records: Vec<Record> = sqlx::query_as::<_, Db<Record>>(
            "SELECT id, blueprint_id, blueprint_version, projections, system_tags, system_metadata, ('attricat.sample'=ANY(system_tags)) AS is_sample, created_at, updated_at, deleted_at, content_revision \
             FROM records WHERE id = ANY($1) AND workspace_id = $2 AND deleted_at IS NULL",
        )
        .bind(record_ids)
        .bind(self.workspace_id.0)
        .fetch_all(&self.pool)
        .await?
        .into_domain();
        Ok(records
            .into_iter()
            .map(|record| (record.id, record))
            .collect())
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
        // Every page row and expanded target is read in a fixed number of
        // queries, not per row.
        let page_ids: Vec<Uuid> = page.records.iter().map(|record| record.id).collect();
        let (mut values, mut files) = self
            .values_and_files_of(&page_ids, page.snapshot_at)
            .await?;
        let no_files = FileReferences::new();
        let resolved_rows: Vec<ResolvedValues> = page
            .records
            .iter()
            .map(|record| {
                resolve_values(
                    &attributes,
                    values.get(&record.id).map_or(&[][..], Vec::as_slice),
                    files.get(&record.id).unwrap_or(&no_files),
                    &path,
                )
            })
            .collect();
        let mut target_ids: Vec<Uuid> = resolved_rows
            .iter()
            .flat_map(|resolved| {
                request.expand.iter().flat_map(|code| {
                    resolved
                        .relationships
                        .get(code)
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .take(MAX_EXPANDED_TARGETS)
                        .filter_map(|target| target.as_str()?.parse().ok())
                })
            })
            .collect();
        target_ids.sort_unstable();
        target_ids.dedup();
        let targets = self.records_of(&target_ids).await?;
        let unread: Vec<Uuid> = target_ids
            .iter()
            .copied()
            .filter(|id| targets.contains_key(id) && !page_ids.contains(id))
            .collect();
        let (target_values, target_files) =
            self.values_and_files_of(&unread, page.snapshot_at).await?;
        values.extend(target_values);
        files.extend(target_files);
        let published = match request.channel_id {
            Some(channel) if !targets.is_empty() => {
                let ids: Vec<Uuid> = targets.keys().copied().collect();
                Some(self.published_of(&ids, channel).await?)
            }
            Some(_) => Some(HashSet::new()),
            None => None,
        };
        let mut target_attributes: HashMap<(Uuid, i64), Vec<Attribute>> = HashMap::new();
        for target in targets.values() {
            let key = (target.blueprint_id, target.blueprint_version);
            if let std::collections::hash_map::Entry::Vacant(entry) = target_attributes.entry(key) {
                entry.insert(self.list_attributes(key.0, key.1).await?);
            }
        }
        let mut expanded: HashMap<Uuid, Value> = HashMap::new();
        let mut rows = Vec::with_capacity(page.records.len());
        for (record, resolved) in page.records.iter().zip(resolved_rows) {
            let mut related = Map::new();
            for code in &request.expand {
                let Some(target_refs) = resolved.relationships.get(code).and_then(Value::as_array)
                else {
                    continue;
                };
                let mut items = Vec::new();
                for target in target_refs.iter().take(MAX_EXPANDED_TARGETS) {
                    let Some(target) = target
                        .as_str()
                        .and_then(|id| id.parse::<Uuid>().ok())
                        .and_then(|id| targets.get(&id))
                    else {
                        continue;
                    };
                    let item = expanded.entry(target.id).or_insert_with(|| {
                        let target_resolved = resolve_values(
                            &target_attributes[&(target.blueprint_id, target.blueprint_version)],
                            values.get(&target.id).map_or(&[][..], Vec::as_slice),
                            files.get(&target.id).unwrap_or(&no_files),
                            &path,
                        );
                        json!({
                            "record_id": target.id,
                            "blueprint_id": target.blueprint_id,
                            "blueprint_version": target.blueprint_version,
                            "content_revision": target.content_revision,
                            "published": published.as_ref().map(|published| published.contains(&target.id)),
                            "values": target_resolved.values,
                            "relationships": target_resolved.relationships,
                            "files": target_resolved.files,
                        })
                    });
                    items.push(item.clone());
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
        let resolved = resolve_values(
            &[title, local],
            &values,
            &FileReferences::new(),
            &[child, parent],
        );
        assert_eq!(resolved.values.get("title"), Some(&json!("Inherited")));
        assert!(resolved.values.get("sku").is_none());
    }
}
