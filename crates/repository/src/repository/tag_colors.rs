//! Workspace colors for record system tags, stored under
//! `workspaces.settings.tag_colors` as a map from tag to [`TagColor`] code.
//! Core tags start with a default color; the stored map holds only the
//! workspace's own choices, which override those defaults.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::system_annotations::validate_system_tag_filter;
use super::{AttricatRepository, NEEDS_ATTENTION_SYSTEM_TAG, RepositoryError};
use attricat_domain::{permissions::Permission, tag_colors::TagColor};

/// The colors Core tags have until a workspace chooses another.
pub const DEFAULT_TAG_COLORS: [(&str, TagColor); 1] =
    [(NEEDS_ATTENTION_SYSTEM_TAG, TagColor::Yellow)];
/// The most tags one workspace may color, besides the defaults.
pub const MAX_TAG_COLORS: usize = 200;

/// A workspace's tag colors. A tag's color is its override, else its default;
/// a tag in neither has no color.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct TagColors {
    pub defaults: BTreeMap<String, TagColor>,
    pub overrides: BTreeMap<String, TagColor>,
}

/// One change to a workspace's tag colors.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TagColorsPatch {
    /// Colors to give tags. A Core tag's default color clears its override.
    #[serde(default)]
    pub set: BTreeMap<String, TagColor>,
    /// Tags whose override to remove, so they return to their default color
    /// or to none.
    #[serde(default)]
    pub reset: Vec<String>,
}

fn default_tag_colors() -> BTreeMap<String, TagColor> {
    DEFAULT_TAG_COLORS
        .iter()
        .map(|(tag, color)| ((*tag).to_owned(), *color))
        .collect()
}

/// Stored overrides, written only through [`apply_patch`]. An entry that no
/// longer parses, such as a retired color, is dropped rather than failing
/// every reader.
fn parse_stored_overrides(stored: Option<serde_json::Value>) -> BTreeMap<String, TagColor> {
    let Some(serde_json::Value::Object(stored)) = stored else {
        return BTreeMap::new();
    };
    stored
        .into_iter()
        .filter_map(|(tag, color)| Some((tag, TagColor::parse(color.as_str()?)?)))
        .collect()
}

/// `overrides` after `patch`, which is checked as a whole first.
fn apply_patch(
    mut overrides: BTreeMap<String, TagColor>,
    patch: TagColorsPatch,
) -> Result<BTreeMap<String, TagColor>, RepositoryError> {
    let set_tags: Vec<String> = patch.set.keys().cloned().collect();
    validate_system_tag_filter(&set_tags)?;
    validate_system_tag_filter(&patch.reset)?;
    if let Some(tag) = patch.reset.iter().find(|tag| patch.set.contains_key(*tag)) {
        return Err(RepositoryError::InvalidTagColors(format!(
            "tag '{tag}' is both set and reset"
        )));
    }
    let defaults = default_tag_colors();
    for tag in &patch.reset {
        overrides.remove(tag);
    }
    for (tag, color) in patch.set {
        if defaults.get(&tag) == Some(&color) {
            overrides.remove(&tag);
        } else {
            overrides.insert(tag, color);
        }
    }
    if overrides.len() > MAX_TAG_COLORS {
        return Err(RepositoryError::InvalidTagColors(format!(
            "a workspace may color at most {MAX_TAG_COLORS} tags"
        )));
    }
    Ok(overrides)
}

impl AttricatRepository {
    /// The scoped workspace's tag colors, which every catalog reader needs to
    /// render tags.
    pub async fn tag_colors(&self) -> Result<TagColors, RepositoryError> {
        let stored: Option<serde_json::Value> = sqlx::query_scalar(
            "SELECT settings->'tag_colors' FROM workspaces WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(self.workspace_id())
        .fetch_optional(&self.pool)
        .await?
        .flatten();
        Ok(TagColors {
            defaults: default_tag_colors(),
            overrides: parse_stored_overrides(stored),
        })
    }

    /// Applies `patch` to the workspace's tag colors. The row lock keeps two
    /// administrators' changes to different tags from overwriting each other.
    pub async fn update_tag_colors(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
        patch: TagColorsPatch,
    ) -> Result<TagColors, RepositoryError> {
        if !self
            .is_authorized(
                actor_id,
                workspace_id,
                Permission::TagColorsManage,
                None,
                None,
            )
            .await?
        {
            return Err(RepositoryError::NotFound("permission"));
        }
        let mut transaction = self.pool.begin().await?;
        let stored: Option<Option<serde_json::Value>> = sqlx::query_scalar(
            "SELECT settings->'tag_colors' FROM workspaces WHERE id = $1 AND deleted_at IS NULL FOR UPDATE",
        )
        .bind(workspace_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(stored) = stored else {
            return Err(RepositoryError::NotFound("workspace"));
        };
        let overrides = apply_patch(parse_stored_overrides(stored), patch)?;
        sqlx::query(
            "UPDATE workspaces SET settings = jsonb_set(settings, '{tag_colors}', $1::jsonb, true), updated_at = now() WHERE id = $2",
        )
        .bind(serde_json::to_value(&overrides).expect("tag colors serialize"))
        .bind(workspace_id)
        .execute(&mut *transaction)
        .await?;
        self.commit_mutation(transaction).await?;
        Ok(TagColors {
            defaults: default_tag_colors(),
            overrides,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn patch(value: serde_json::Value) -> TagColorsPatch {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn patches_set_and_reset_overrides() {
        let overrides = apply_patch(
            BTreeMap::new(),
            patch(json!({"set": {"reviewed": "green", NEEDS_ATTENTION_SYSTEM_TAG: "red"}})),
        )
        .unwrap();
        assert_eq!(overrides.get("reviewed"), Some(&TagColor::Green));
        assert_eq!(
            overrides.get(NEEDS_ATTENTION_SYSTEM_TAG),
            Some(&TagColor::Red)
        );

        let overrides = apply_patch(overrides, patch(json!({"reset": ["reviewed"]}))).unwrap();
        assert!(!overrides.contains_key("reviewed"));
        // Choosing a Core tag's default again leaves nothing to store.
        let overrides = apply_patch(
            overrides,
            patch(json!({"set": {NEEDS_ATTENTION_SYSTEM_TAG: "yellow"}})),
        )
        .unwrap();
        assert!(overrides.is_empty());
    }

    #[test]
    fn invalid_patches_are_rejected() {
        let rejects = |value| apply_patch(BTreeMap::new(), patch(value)).is_err();
        assert!(rejects(json!({"set": {" ": "green"}})));
        assert!(rejects(json!({"set": {"x".repeat(129): "green"}})));
        assert!(rejects(json!({"set": {"a": "green"}, "reset": ["a"]})));
        assert!(rejects(json!({"reset": ["a", "a"]})));
        let full: BTreeMap<_, _> = (0..MAX_TAG_COLORS)
            .map(|index| (format!("tag-{index}"), TagColor::Blue))
            .collect();
        assert!(apply_patch(full.clone(), patch(json!({"set": {"tag-0": "red"}}))).is_ok());
        assert!(apply_patch(full, patch(json!({"set": {"one-more": "red"}}))).is_err());
        assert!(serde_json::from_value::<TagColorsPatch>(json!({"set": {"a": "grey"}})).is_err());
        assert!(serde_json::from_value::<TagColorsPatch>(json!({"other": []})).is_err());
    }

    #[test]
    fn unreadable_stored_entries_are_dropped() {
        let overrides = parse_stored_overrides(Some(json!({"a": "blue", "b": "plaid", "c": 3})));
        assert_eq!(
            overrides,
            BTreeMap::from([("a".to_owned(), TagColor::Blue)])
        );
        assert!(parse_stored_overrides(Some(json!([]))).is_empty());
        assert!(parse_stored_overrides(None).is_empty());
    }
}
