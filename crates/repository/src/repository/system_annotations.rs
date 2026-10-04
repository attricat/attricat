//! Validation and patching of entity system tags and system metadata,
//! shared by entity writes, workflow actions and extension annotations.

use super::RepositoryError;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashSet};

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
    let marker = "attricat.sample";
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

/// A tag and metadata patch, applied by [`apply_tag_metadata_patch`].
#[derive(Default)]
pub(super) struct TagMetadataPatch {
    pub add_tags: Vec<String>,
    pub remove_tags: Vec<String>,
    pub set_metadata: BTreeMap<String, Value>,
    pub remove_metadata: Vec<String>,
}

/// The effective changes of one [`TagMetadataPatch`].
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
    patch: &TagMetadataPatch,
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

/// Longest stored system tag, including an extension namespace prefix.
pub(super) const MAX_SYSTEM_TAG_BYTES: usize = 128;

pub(super) fn validate_system_tags(tags: &[String]) -> Result<(), RepositoryError> {
    if tags.len() > 100
        || tags.iter().any(|tag| {
            tag.trim().is_empty() || tag.len() > MAX_SYSTEM_TAG_BYTES || tag == "attricat.sample"
        })
        || tags.iter().collect::<HashSet<_>>().len() != tags.len()
    {
        return Err(RepositoryError::InvalidSystemTags);
    }
    Ok(())
}

pub(super) fn validate_system_metadata(metadata: &Value) -> Result<(), RepositoryError> {
    if !metadata.is_object()
        || serde_json::to_vec(metadata).map_or(true, |value| value.len() > 64 * 1024)
    {
        return Err(RepositoryError::InvalidSystemMetadata);
    }
    Ok(())
}
