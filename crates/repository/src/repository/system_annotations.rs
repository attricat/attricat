//! Validation and patching of record system tags and system metadata,
//! shared by record writes, workflow actions and extension annotations.

use super::{RepositoryError, SAMPLE_SYSTEM_TAG};
use crate::model::{Record, SystemAnnotationPatch};
use serde_json::{Map, Value};
use std::collections::HashSet;

pub(super) fn validate_system_annotations(
    tags: &[String],
    metadata: &Value,
) -> Result<(), RepositoryError> {
    validate_system_tags(tags)?;
    validate_system_metadata(metadata)
}

pub(super) fn validate_system_tag_update(
    current: &[String],
    requested: &[String],
) -> Result<(), RepositoryError> {
    let marker = SAMPLE_SYSTEM_TAG;
    if requested.iter().any(|tag| tag == marker) && !current.iter().any(|tag| tag == marker) {
        return Err(RepositoryError::InvalidSystemTags);
    }
    validate_system_tags(
        &requested
            .iter()
            .filter(|tag| tag.as_str() != marker)
            .cloned()
            .collect::<Vec<_>>(),
    )
}

/// The effective changes of one [`SystemAnnotationPatch`].
pub(super) struct TagMetadataChanges {
    pub added_tags: Vec<String>,
    pub removed_tags: Vec<String>,
    pub set_keys: Vec<String>,
    pub removed_keys: Vec<String>,
}

impl TagMetadataChanges {
    pub fn is_empty(&self) -> bool {
        self.added_tags.is_empty()
            && self.removed_tags.is_empty()
            && self.set_keys.is_empty()
            && self.removed_keys.is_empty()
    }
}

/// Applies `patch` in place: tag removals, then additions (kept in order,
/// without duplicates), then metadata sets and key removals. Reports only
/// what actually changed. Callers validate the result.
pub(super) fn apply_tag_metadata_patch(
    tags: &mut Vec<String>,
    metadata: &mut Map<String, Value>,
    patch: &SystemAnnotationPatch,
) -> TagMetadataChanges {
    let removed_tags = patch
        .remove_tags
        .iter()
        .filter(|tag| tags.contains(tag))
        .cloned()
        .collect();
    tags.retain(|tag| !patch.remove_tags.contains(tag));
    let mut added_tags = Vec::new();
    for tag in &patch.add_tags {
        if !tags.contains(tag) {
            tags.push(tag.clone());
            added_tags.push(tag.clone());
        }
    }
    let mut set_keys = Vec::new();
    for (key, value) in &patch.set_metadata {
        if metadata.get(key) != Some(value) {
            set_keys.push(key.clone());
        }
        metadata.insert(key.clone(), value.clone());
    }
    let removed_keys = patch
        .remove_metadata
        .iter()
        .filter(|key| metadata.remove(*key).is_some())
        .cloned()
        .collect();
    TagMetadataChanges {
        added_tags,
        removed_tags,
        set_keys,
        removed_keys,
    }
}

/// A record's tags and metadata after one or more patches. A field stays
/// `None` until a patch changes it, so the resulting record update replaces
/// only the fields that were patched.
#[derive(Default)]
pub(super) struct PatchedAnnotations {
    pub system_tags: Option<Vec<String>>,
    pub system_metadata: Option<Value>,
}

impl PatchedAnnotations {
    /// Applies `patch` on top of `record`'s annotations and earlier patches.
    pub fn apply(
        &mut self,
        record: &Record,
        patch: &SystemAnnotationPatch,
    ) -> Result<(), RepositoryError> {
        if patch.changes_tags() {
            let tags = self
                .system_tags
                .get_or_insert_with(|| record.system_tags.clone());
            apply_tag_metadata_patch(tags, &mut Map::new(), patch);
        }
        if patch.changes_metadata() {
            let mut metadata = self
                .system_metadata
                .as_ref()
                .unwrap_or(&record.system_metadata)
                .as_object()
                .cloned()
                .ok_or(RepositoryError::InvalidSystemMetadata)?;
            apply_tag_metadata_patch(&mut Vec::new(), &mut metadata, patch);
            self.system_metadata = Some(Value::Object(metadata));
        }
        Ok(())
    }
}

/// Longest stored system tag, including an extension namespace prefix.
pub(super) use attricat_validation::predicate::{
    MAX_SYSTEM_TAGS, MAX_TAG_LENGTH as MAX_SYSTEM_TAG_BYTES,
};
pub(super) const MAX_SYSTEM_METADATA_BYTES: usize = 64 * 1024;

pub(super) fn validate_system_tags(tags: &[String]) -> Result<(), RepositoryError> {
    if tags.len() > MAX_SYSTEM_TAGS
        || tags.iter().any(|tag| {
            tag.trim().is_empty() || tag.len() > MAX_SYSTEM_TAG_BYTES || tag == SAMPLE_SYSTEM_TAG
        })
        || tags.iter().collect::<HashSet<_>>().len() != tags.len()
    {
        return Err(RepositoryError::InvalidSystemTags);
    }
    Ok(())
}

pub(super) fn validate_system_metadata(metadata: &Value) -> Result<(), RepositoryError> {
    if !metadata.is_object()
        || serde_json::to_vec(metadata)
            .map_or(true, |value| value.len() > MAX_SYSTEM_METADATA_BYTES)
    {
        return Err(RepositoryError::InvalidSystemMetadata);
    }
    Ok(())
}
