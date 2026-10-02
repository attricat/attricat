//! Extension-owned entity annotations.
//!
//! An installed extension owns system tags named `<extension-id>:<local>` and
//! the object stored at `system_metadata[<extension-id>]` once its namespace
//! is claimed. Callers name only local tags and keys; Catalog derives the
//! namespace from the authenticated extension provenance. Claimed namespaces
//! are protected on every other write path, so a whole-field entity update
//! cannot overwrite them by round-tripping an older annotation object.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::EventPublisher;
use super::{CatalogRepository, RepositoryError, entity_commands};
use crate::domain_events::ENTITY_ANNOTATIONS_CHANGED_V1;

/// Maximum add/remove/set/delete operations in one patch.
pub const MAX_ANNOTATION_PATCH_OPERATIONS: usize = 32;
const MAX_LOCAL_NAME_BYTES: usize = 64;
const TAG_SEPARATOR: char = ':';
/// Core-owned names that no extension may claim as an annotation namespace.
/// Core tags never contain `:`, so they cannot collide with extension tags;
/// these names also keep Core's metadata keys out of extension ownership.
const RESERVED_NAMESPACES: &[&str] = &["attricat", "attricat.sample", "catalog", "core", "system"];

/// A bounded, explicit patch of one extension's annotation namespace. Setting
/// a key replaces that key's whole JSON value; JSON `null` is a valid value,
/// so removals are listed separately.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ExtensionAnnotationPatch {
    #[serde(default)]
    pub add_tags: Vec<String>,
    #[serde(default)]
    pub remove_tags: Vec<String>,
    #[serde(default)]
    pub set_metadata: BTreeMap<String, Value>,
    #[serde(default)]
    pub remove_metadata: Vec<String>,
    /// Rejects the patch unless the namespace is still at this revision. A
    /// namespace that has never been written is at revision 0.
    #[serde(default)]
    pub expected_revision: Option<i64>,
}

/// One extension's view of its own annotations on an entity.
#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct ExtensionAnnotations {
    pub tags: Vec<String>,
    pub metadata: Map<String, Value>,
    pub revision: i64,
}

/// Operator inventory of a namespace before or after it is claimed.
#[derive(Clone, Debug, Serialize)]
pub struct ExtensionAnnotationNamespace {
    pub extension_id: String,
    pub claimed: bool,
    pub adopted_legacy: bool,
    pub claimed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Entities carrying tags or metadata in this namespace.
    pub annotated_entities: i64,
}

fn invalid(message: impl Into<String>) -> RepositoryError {
    RepositoryError::InvalidAnnotationPatch(message.into())
}

fn valid_local_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_LOCAL_NAME_BYTES
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
}

fn tag_prefix(extension_id: &str) -> String {
    format!("{extension_id}{TAG_SEPARATOR}")
}

impl ExtensionAnnotationPatch {
    fn validate(&self) -> Result<(), RepositoryError> {
        let operations = self.add_tags.len()
            + self.remove_tags.len()
            + self.set_metadata.len()
            + self.remove_metadata.len();
        if operations == 0 || operations > MAX_ANNOTATION_PATCH_OPERATIONS {
            return Err(invalid(format!(
                "a patch must contain 1-{MAX_ANNOTATION_PATCH_OPERATIONS} operations"
            )));
        }
        if self.expected_revision.is_some_and(|revision| revision < 0) {
            return Err(invalid("expected_revision must not be negative"));
        }
        let names = self
            .add_tags
            .iter()
            .chain(&self.remove_tags)
            .chain(self.set_metadata.keys())
            .chain(&self.remove_metadata);
        if let Some(name) = names.into_iter().find(|name| !valid_local_name(name)) {
            return Err(invalid(format!(
                "local tag or key '{name}' must be 1-{MAX_LOCAL_NAME_BYTES} ASCII letters, numbers, '.', '_' or '-'"
            )));
        }
        let unique =
            |values: &[String]| values.iter().collect::<BTreeSet<_>>().len() == values.len();
        if !unique(&self.add_tags) || !unique(&self.remove_tags) || !unique(&self.remove_metadata) {
            return Err(invalid("a patch must not repeat a tag or key"));
        }
        if self
            .add_tags
            .iter()
            .any(|tag| self.remove_tags.contains(tag))
        {
            return Err(invalid("a tag cannot be both added and removed"));
        }
        if self
            .remove_metadata
            .iter()
            .any(|key| self.set_metadata.contains_key(key))
        {
            return Err(invalid("a metadata key cannot be both set and removed"));
        }
        Ok(())
    }
}

/// The part of an entity's annotations that belongs to claimed namespaces.
fn protected_slice(
    tags: &[String],
    metadata: &Value,
    namespaces: &[String],
) -> (BTreeSet<String>, BTreeMap<String, Value>) {
    let protected_tags = tags
        .iter()
        .filter(|tag| {
            namespaces
                .iter()
                .any(|namespace| tag.starts_with(&tag_prefix(namespace)))
        })
        .cloned()
        .collect();
    let protected_metadata = namespaces
        .iter()
        .filter_map(|namespace| {
            metadata
                .get(namespace)
                .map(|value| (namespace.clone(), value.clone()))
        })
        .collect();
    (protected_tags, protected_metadata)
}

/// Reads an extension's local annotations from a stored entity.
pub(crate) fn own_annotations(
    extension_id: &str,
    tags: &[String],
    metadata: &Value,
    revision: i64,
) -> ExtensionAnnotations {
    let prefix = tag_prefix(extension_id);
    ExtensionAnnotations {
        tags: tags
            .iter()
            .filter_map(|tag| tag.strip_prefix(&prefix).map(str::to_owned))
            .collect(),
        metadata: metadata
            .get(extension_id)
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default(),
        revision,
    }
}

impl CatalogRepository {
    pub(crate) async fn claimed_annotation_namespaces(
        &self,
        connection: &mut sqlx::PgConnection,
    ) -> Result<Vec<String>, RepositoryError> {
        Ok(sqlx::query_scalar(
            "SELECT extension_id FROM extension_annotation_namespaces WHERE workspace_id=$1 ORDER BY extension_id",
        )
        .bind(self.workspace_id.0)
        .fetch_all(connection)
        .await?)
    }

    /// Rejects a generic write that would change any claimed namespace. Unrelated
    /// tags and metadata keys remain freely editable through their usual path.
    pub(crate) async fn ensure_annotation_namespaces_unchanged(
        &self,
        connection: &mut sqlx::PgConnection,
        before_tags: &[String],
        before_metadata: &Value,
        after_tags: &[String],
        after_metadata: &Value,
    ) -> Result<(), RepositoryError> {
        let namespaces = self.claimed_annotation_namespaces(connection).await?;
        if namespaces.is_empty() {
            return Ok(());
        }
        let before = protected_slice(before_tags, before_metadata, &namespaces);
        let after = protected_slice(after_tags, after_metadata, &namespaces);
        if before == after {
            return Ok(());
        }
        let changed = namespaces
            .into_iter()
            .find(|namespace| {
                protected_slice(
                    before_tags,
                    before_metadata,
                    std::slice::from_ref(namespace),
                ) != protected_slice(after_tags, after_metadata, std::slice::from_ref(namespace))
            })
            .unwrap_or_default();
        Err(RepositoryError::ProtectedAnnotationNamespace(changed))
    }

    /// Removes claimed namespaces from annotations copied to a new entity. An
    /// extension's facts about one entity never describe a duplicate.
    pub(crate) async fn without_claimed_annotations(
        &self,
        tags: Vec<String>,
        mut metadata: Value,
    ) -> Result<(Vec<String>, Value), RepositoryError> {
        let mut connection = self.pool.acquire().await?;
        let namespaces = self.claimed_annotation_namespaces(&mut connection).await?;
        let tags = tags
            .into_iter()
            .filter(|tag| {
                !namespaces
                    .iter()
                    .any(|namespace| tag.starts_with(&tag_prefix(namespace)))
            })
            .collect();
        if let Some(object) = metadata.as_object_mut() {
            for namespace in &namespaces {
                object.remove(namespace);
            }
        }
        Ok((tags, metadata))
    }

    async fn namespace_annotation_count(
        &self,
        connection: &mut sqlx::PgConnection,
        extension_id: &str,
    ) -> Result<i64, RepositoryError> {
        Ok(sqlx::query_scalar(
            "SELECT COUNT(*) FROM entities WHERE workspace_id=$1 AND (system_metadata ? $2 OR EXISTS (SELECT 1 FROM unnest(system_tags) AS tag WHERE starts_with(tag, $3)))",
        )
        .bind(self.workspace_id.0)
        .bind(extension_id)
        .bind(tag_prefix(extension_id))
        .fetch_one(connection)
        .await?)
    }

    /// Claims a namespace on an extension's first annotation write. Existing
    /// unowned data under the same name is never claimed implicitly.
    async fn claim_annotation_namespace(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        extension_id: &str,
        adopt_legacy: bool,
    ) -> Result<bool, RepositoryError> {
        if RESERVED_NAMESPACES.contains(&extension_id) {
            return Err(RepositoryError::ReservedAnnotationNamespace(
                extension_id.to_owned(),
            ));
        }
        // Serialize first claims for a namespace so the legacy scan and the
        // claim cannot race another first writer or an adoption.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!(
                "extension-annotation-namespace:{}:{extension_id}",
                self.workspace_id.0
            ))
            .execute(&mut **transaction)
            .await?;
        let claimed: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM extension_annotation_namespaces WHERE workspace_id=$1 AND extension_id=$2)",
        )
        .bind(self.workspace_id.0)
        .bind(extension_id)
        .fetch_one(&mut **transaction)
        .await?;
        if claimed {
            return Ok(false);
        }
        if !adopt_legacy
            && self
                .namespace_annotation_count(&mut *transaction, extension_id)
                .await?
                > 0
        {
            return Err(RepositoryError::AnnotationNamespaceAdoptionRequired(
                extension_id.to_owned(),
            ));
        }
        sqlx::query(
            "INSERT INTO extension_annotation_namespaces(workspace_id,extension_id,adopted_legacy,claimed_by_user_id) VALUES($1,$2,$3,$4)",
        )
        .bind(self.workspace_id.0)
        .bind(extension_id)
        .bind(adopt_legacy)
        .bind(self.audit_context.as_ref().and_then(|audit| audit.actor_user_id))
        .execute(&mut **transaction)
        .await?;
        Ok(true)
    }

    /// Operator inventory of a namespace's existing annotations.
    pub async fn extension_annotation_namespace(
        &self,
        extension_id: &str,
    ) -> Result<ExtensionAnnotationNamespace, RepositoryError> {
        let mut connection = self.pool.acquire().await?;
        let claim: Option<(bool, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
            "SELECT adopted_legacy, claimed_at FROM extension_annotation_namespaces WHERE workspace_id=$1 AND extension_id=$2",
        )
        .bind(self.workspace_id.0)
        .bind(extension_id)
        .fetch_optional(&mut *connection)
        .await?;
        Ok(ExtensionAnnotationNamespace {
            extension_id: extension_id.to_owned(),
            claimed: claim.is_some(),
            adopted_legacy: claim.as_ref().is_some_and(|claim| claim.0),
            claimed_at: claim.map(|claim| claim.1),
            annotated_entities: self
                .namespace_annotation_count(&mut connection, extension_id)
                .await?,
        })
    }

    /// Explicitly adopts existing unowned annotations for an installed
    /// extension. This is the only path that claims a namespace with legacy
    /// data; it never renames or deletes the existing values.
    pub async fn adopt_extension_annotation_namespace(
        &self,
        extension_id: &str,
    ) -> Result<ExtensionAnnotationNamespace, RepositoryError> {
        self.installed_extension(extension_id).await?;
        let mut transaction = self.pool.begin().await?;
        if self
            .claim_annotation_namespace(&mut transaction, extension_id, true)
            .await?
        {
            self.write_audit_event(&mut transaction).await?;
        }
        transaction.commit().await?;
        self.extension_annotation_namespace(extension_id).await
    }

    /// Applies one extension's namespace patch inside the caller's transaction.
    /// The entity row lock serializes it with every other entity writer; the
    /// current state is read under that lock, so disjoint writers never lose
    /// each other's changes.
    pub(crate) async fn apply_extension_annotation_patch(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        extension_id: &str,
        entity_id: Uuid,
        patch: &ExtensionAnnotationPatch,
    ) -> Result<i64, RepositoryError> {
        patch.validate()?;
        self.ensure_actor_may(&mut *transaction, "entities.read", entity_id)
            .await?;
        self.claim_annotation_namespace(transaction, extension_id, false)
            .await?;
        let entity = self.lock_entity(transaction, entity_id).await?;
        let revision: i64 = sqlx::query_scalar(
            "SELECT revision FROM entity_extension_annotation_revisions WHERE workspace_id=$1 AND entity_id=$2 AND extension_id=$3",
        )
        .bind(self.workspace_id.0)
        .bind(entity_id)
        .bind(extension_id)
        .fetch_optional(&mut **transaction)
        .await?
        .unwrap_or(0);
        if let Some(expected) = patch.expected_revision
            && expected != revision
        {
            return Err(RepositoryError::AnnotationRevisionConflict {
                expected,
                actual: revision,
            });
        }
        let prefix = tag_prefix(extension_id);
        let mut tags = entity.system_tags.clone();
        let removed_tags = patch
            .remove_tags
            .iter()
            .filter(|tag| tags.contains(&format!("{prefix}{tag}")))
            .cloned()
            .collect::<Vec<_>>();
        tags.retain(|tag| {
            !tag.strip_prefix(&prefix)
                .is_some_and(|local| patch.remove_tags.iter().any(|removed| removed == local))
        });
        let mut added_tags = Vec::new();
        for tag in &patch.add_tags {
            let qualified = format!("{prefix}{tag}");
            if !tags.contains(&qualified) {
                tags.push(qualified);
                added_tags.push(tag.clone());
            }
        }
        let mut metadata = entity
            .system_metadata
            .as_object()
            .cloned()
            .ok_or(RepositoryError::InvalidSystemMetadata)?;
        let mut namespace = match metadata.remove(extension_id) {
            None => Map::new(),
            Some(Value::Object(namespace)) => namespace,
            Some(_) => {
                return Err(invalid(
                    "the namespace metadata value is not an object; an operator must repair it",
                ));
            }
        };
        let mut set_keys = Vec::new();
        for (key, value) in &patch.set_metadata {
            if namespace.get(key) != Some(value) {
                set_keys.push(key.clone());
            }
            namespace.insert(key.clone(), value.clone());
        }
        let removed_keys = patch
            .remove_metadata
            .iter()
            .filter(|key| namespace.remove(*key).is_some())
            .cloned()
            .collect::<Vec<_>>();
        if !namespace.is_empty() {
            metadata.insert(extension_id.to_owned(), Value::Object(namespace));
        }
        let metadata = Value::Object(metadata);
        if added_tags.is_empty()
            && removed_tags.is_empty()
            && set_keys.is_empty()
            && removed_keys.is_empty()
        {
            return Ok(revision);
        }
        entity_commands::validate_system_tag_update(&entity.system_tags, &tags)?;
        entity_commands::validate_system_metadata(&metadata)?;
        self.ensure_task_fence(transaction).await?;
        // Annotation bookkeeping is not catalog data. Leaving `updated_at`
        // unchanged keeps extensions from invalidating their own output and
        // avoids spurious conflicts with concurrent entity editors.
        sqlx::query("UPDATE entities SET system_tags=$2, system_metadata=$3 WHERE id=$1 AND workspace_id=$4")
            .bind(entity_id)
            .bind(&tags)
            .bind(&metadata)
            .bind(self.workspace_id.0)
            .execute(&mut **transaction)
            .await?;
        let next_revision = revision + 1;
        sqlx::query(
            "INSERT INTO entity_extension_annotation_revisions(workspace_id,entity_id,extension_id,revision) VALUES($1,$2,$3,$4) ON CONFLICT (workspace_id,entity_id,extension_id) DO UPDATE SET revision=EXCLUDED.revision, updated_at=now()",
        )
        .bind(self.workspace_id.0)
        .bind(entity_id)
        .bind(extension_id)
        .bind(next_revision)
        .execute(&mut **transaction)
        .await?;
        let summary = json!({
            "extension_id": extension_id,
            "revision": next_revision,
            "tags_added": added_tags,
            "tags_removed": removed_tags,
            "metadata_keys_set": set_keys,
            "metadata_keys_removed": removed_keys,
        });
        let mut audited = self.clone();
        if let Some(audit) = audited.audit_context.as_mut()
            && let Some(metadata) = audit.metadata.as_object_mut()
        {
            metadata.insert("annotations".to_owned(), summary.clone());
            metadata.insert("entity_id".to_owned(), json!(entity_id));
        }
        audited.write_audit_event(transaction).await?;
        let mut payload = summary;
        payload["entity_id"] = json!(entity_id);
        payload["blueprint_id"] = json!(entity.blueprint_id);
        payload["blueprint_version"] = json!(entity.blueprint_version);
        let event = self.core_event(ENTITY_ANNOTATIONS_CHANGED_V1, "entity", entity_id, payload);
        self.enqueue_event(transaction, event).await?;
        Ok(next_revision)
    }

    /// Applies an operator repair or cleanup patch to one extension namespace.
    /// It uses the same validation, revision, audit and outbox path as the
    /// extension, but is authorized by the operator's own request.
    pub async fn repair_extension_annotations(
        &self,
        extension_id: &str,
        entity_id: Uuid,
        patch: ExtensionAnnotationPatch,
    ) -> Result<ExtensionAnnotations, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let claimed: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM extension_annotation_namespaces WHERE workspace_id=$1 AND extension_id=$2)",
        )
        .bind(self.workspace_id.0)
        .bind(extension_id)
        .fetch_one(&mut *transaction)
        .await?;
        if !claimed {
            return Err(RepositoryError::NotFound("claimed annotation namespace"));
        }
        self.apply_extension_annotation_patch(&mut transaction, extension_id, entity_id, &patch)
            .await?;
        transaction.commit().await?;
        self.extension_annotations(extension_id, entity_id)
            .await?
            .ok_or(RepositoryError::NotFound("entity"))
    }

    /// Reads one extension's annotations and current revision for an entity.
    pub async fn extension_annotations(
        &self,
        extension_id: &str,
        entity_id: Uuid,
    ) -> Result<Option<ExtensionAnnotations>, RepositoryError> {
        let row: Option<(Vec<String>, Value, Option<i64>)> = sqlx::query_as(
            "SELECT e.system_tags, e.system_metadata, r.revision FROM entities e LEFT JOIN entity_extension_annotation_revisions r ON r.workspace_id=e.workspace_id AND r.entity_id=e.id AND r.extension_id=$3 WHERE e.id=$1 AND e.workspace_id=$2 AND e.deleted_at IS NULL",
        )
        .bind(entity_id)
        .bind(self.workspace_id.0)
        .bind(extension_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(tags, metadata, revision)| {
            own_annotations(extension_id, &tags, &metadata, revision.unwrap_or(0))
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patch() -> ExtensionAnnotationPatch {
        ExtensionAnnotationPatch {
            add_tags: vec!["generated".into()],
            ..Default::default()
        }
    }

    #[test]
    fn patches_are_bounded_and_unambiguous() {
        assert!(patch().validate().is_ok());
        assert!(ExtensionAnnotationPatch::default().validate().is_err());
        let mut contradictory = patch();
        contradictory.remove_tags = vec!["generated".into()];
        assert!(contradictory.validate().is_err());
        let set_and_remove = ExtensionAnnotationPatch {
            set_metadata: [("last".to_owned(), Value::Null)].into(),
            remove_metadata: vec!["last".into()],
            ..Default::default()
        };
        assert!(set_and_remove.validate().is_err());
        let mut qualified = patch();
        qualified.add_tags = vec!["other:generated".into()];
        assert!(qualified.validate().is_err());
        let mut repeated = patch();
        repeated.add_tags.push("generated".into());
        assert!(repeated.validate().is_err());
        let oversized = ExtensionAnnotationPatch {
            add_tags: (0..=MAX_ANNOTATION_PATCH_OPERATIONS)
                .map(|index| format!("tag-{index}"))
                .collect(),
            ..Default::default()
        };
        assert!(oversized.validate().is_err());
        let mut negative = patch();
        negative.expected_revision = Some(-1);
        assert!(negative.validate().is_err());
    }

    #[test]
    fn null_metadata_values_are_settable() {
        let value = ExtensionAnnotationPatch {
            set_metadata: [("cleared".to_owned(), Value::Null)].into(),
            ..Default::default()
        };
        assert!(value.validate().is_ok());
    }

    #[test]
    fn protected_slices_only_cover_claimed_namespaces() {
        let tags = vec![
            "acme.docs:generated".to_owned(),
            "acme.docsx:other".to_owned(),
            "plain".to_owned(),
        ];
        let metadata = json!({"acme.docs": {"run": 1}, "plain": true});
        let (protected_tags, protected_metadata) =
            protected_slice(&tags, &metadata, &["acme.docs".to_owned()]);
        assert_eq!(
            protected_tags.into_iter().collect::<Vec<_>>(),
            vec!["acme.docs:generated"]
        );
        assert_eq!(
            protected_metadata.keys().collect::<Vec<_>>(),
            vec!["acme.docs"]
        );
    }

    #[test]
    fn own_annotations_strip_the_namespace() {
        let annotations = own_annotations(
            "acme.docs",
            &["acme.docs:generated".into(), "other:x".into()],
            &json!({"acme.docs": {"template": 2}, "other": {}}),
            4,
        );
        assert_eq!(annotations.tags, vec!["generated"]);
        assert_eq!(annotations.metadata["template"], 2);
        assert_eq!(annotations.revision, 4);
    }
}
