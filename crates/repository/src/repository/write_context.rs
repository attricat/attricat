//! Per-operation state for one record write.
//!
//! A record write resolves contexts and attributes for every value, then
//! validates statuses, principals and checks against the same definitions.
//! [`WriteContext`] loads the workspace context tree and the record's
//! applicable attributes once, after the record row lock, so those steps read
//! memory instead of repeating the same queries per value and per validator.
//!
//! Published blueprint attributes are immutable, and record-scoped attributes
//! change only under the record lock the writer already holds.

use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

use serde_json::Value;
use sqlx::{PgConnection, Postgres, Transaction};
use tokio::sync::OnceCell;
use uuid::Uuid;

use std::sync::Mutex;

use super::{
    CatalogRepository, Record, RepositoryError,
    checks::EnabledRule,
    generations::WritePrefetch,
    record_values::{ContextTree, RecordState, RecordValues, load_record},
    validate_attribute_selector_code,
};

/// One attribute that applies to the record: a column of its blueprint
/// revision or an attribute attached to the record itself.
#[derive(Clone, Debug, sqlx::FromRow)]
pub(super) struct WriteAttribute {
    pub id: Uuid,
    pub code: String,
    pub value_type: String,
    pub value_schema: Option<Value>,
    pub target_blueprint_codes: Vec<String>,
    pub cardinality: Option<String>,
    pub target_cardinality: Option<String>,
    pub context_editable: String,
    pub context_fallback: String,
    pub default_value: Option<Value>,
    /// Attached to the record itself rather than a blueprint column.
    pub record_scoped: bool,
}

impl WriteAttribute {
    fn declares(&self, key: &str) -> bool {
        self.value_schema
            .as_ref()
            .is_some_and(|schema| schema.get(key).is_some())
    }
}

#[derive(Debug)]
pub(super) struct WriteContext {
    pub tree: Arc<ContextTree>,
    attributes: Vec<WriteAttribute>,
    /// The revision's enabled rules, when taken from a matching prefetch.
    pub enabled_rules: Option<Vec<EnabledRule>>,
    /// Read on first use by a relationship write.
    family: OnceCell<FamilyConstraints>,
    /// State derived from the record's values, shared by the validators of
    /// one write. Every value write through this context clears it.
    derived: Mutex<Derived>,
}

#[derive(Debug, Default)]
struct Derived {
    preview: Option<Value>,
    after: Option<Option<RecordValues>>,
}

/// Relationship constraints of the record's blueprint family. They follow
/// the latest published revision, so they are read once per operation
/// rather than per relationship target.
#[derive(Clone, Debug, Default)]
pub(super) struct FamilyConstraints {
    /// Hierarchy kind per relationship field code.
    pub hierarchies: BTreeMap<String, super::structural_constraints::Hierarchy>,
    /// Field codes some published revision limits to one source per target.
    pub target_one_codes: HashSet<String>,
}

impl WriteContext {
    pub(super) async fn load(
        transaction: &mut Transaction<'_, Postgres>,
        workspace_id: Uuid,
        record: &Record,
    ) -> Result<Self, RepositoryError> {
        Self::load_with(transaction, workspace_id, record, None).await
    }

    /// [`Self::load`] taking the context tree and enabled rules from
    /// `prefetch` when the transaction read the same generations.
    pub(super) async fn load_with(
        transaction: &mut Transaction<'_, Postgres>,
        workspace_id: Uuid,
        record: &Record,
        prefetch: Option<&WritePrefetch>,
    ) -> Result<Self, RepositoryError> {
        let tree = match prefetch {
            Some(prefetch) => prefetch.tree.clone(),
            None => Arc::new(ContextTree::load(transaction, workspace_id).await?),
        };
        let enabled_rules = prefetch.map(|prefetch| {
            prefetch
                .rules
                .get(&(record.blueprint_id, record.blueprint_version))
                .cloned()
                .unwrap_or_default()
        });
        let attributes = Self::load_attributes(transaction, record).await?;
        Ok(Self {
            tree,
            enabled_rules,
            attributes,
            family: OnceCell::new(),
            derived: Mutex::default(),
        })
    }

    fn derived(&self) -> std::sync::MutexGuard<'_, Derived> {
        self.derived
            .lock()
            .expect("write context lock is not poisoned")
    }

    /// Forgets state derived from values; call after writing a value.
    pub(super) fn values_changed(&self) {
        *self.derived() = Derived::default();
    }

    /// The record's preview projection for its current values.
    pub(super) async fn preview(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        record_id: Uuid,
    ) -> Result<Value, RepositoryError> {
        if let Some(preview) = self.derived().preview.clone() {
            return Ok(preview);
        }
        let preview = CatalogRepository::build_preview_projection(transaction, record_id).await?;
        self.derived().preview = Some(preview.clone());
        Ok(preview)
    }

    /// The record's current values, as `load_record(.., RecordState::After)`.
    pub(super) async fn after_record(
        &self,
        connection: &mut PgConnection,
        workspace_id: Uuid,
        record_id: Uuid,
    ) -> Result<Option<RecordValues>, RepositoryError> {
        if let Some(record) = self.derived().after.clone() {
            return Ok(record);
        }
        let record = load_record(connection, workspace_id, record_id, RecordState::After).await?;
        self.derived().after = Some(record.clone());
        Ok(record)
    }

    pub(super) async fn family_constraints(
        &self,
        connection: &mut PgConnection,
        workspace_id: Uuid,
        blueprint_id: Uuid,
    ) -> Result<&FamilyConstraints, RepositoryError> {
        self.family
            .get_or_try_init(|| async {
                let hierarchies = sqlx::query_as::<_, (String, String)>(
                    r#"SELECT a.code, a.hierarchy
                       FROM attributes a
                       WHERE a.workspace_id = $1 AND a.hierarchy IS NOT NULL AND a.deleted_at IS NULL
                         AND (a.blueprint_id, a.blueprint_version) = (
                             SELECT b.id, b.version FROM blueprints b
                             WHERE b.workspace_id = $1 AND b.id = $2 AND b.status = 'published' AND b.deleted_at IS NULL
                             ORDER BY b.version DESC LIMIT 1)"#,
                )
                .bind(workspace_id)
                .bind(blueprint_id)
                .fetch_all(&mut *connection)
                .await?
                .into_iter()
                .map(|(code, hierarchy)| {
                    Ok((code, super::structural_constraints::Hierarchy::parse(&hierarchy)?))
                })
                .collect::<Result<_, RepositoryError>>()?;
                let target_one_codes = sqlx::query_scalar::<_, String>(
                    "SELECT DISTINCT a.code FROM attributes a JOIN blueprints b ON b.id = a.blueprint_id AND b.version = a.blueprint_version WHERE a.blueprint_id = $1 AND a.target_cardinality = 'one' AND a.workspace_id = $2 AND a.deleted_at IS NULL AND b.status = 'published' AND b.deleted_at IS NULL",
                )
                .bind(blueprint_id)
                .bind(workspace_id)
                .fetch_all(&mut *connection)
                .await?
                .into_iter()
                .collect();
                Ok::<_, RepositoryError>(FamilyConstraints {
                    hierarchies,
                    target_one_codes,
                })
            })
            .await
    }

    pub(super) fn by_id(&self, id: Uuid) -> Option<&WriteAttribute> {
        self.attributes.iter().find(|attribute| attribute.id == id)
    }

    async fn load_attributes(
        connection: &mut PgConnection,
        record: &Record,
    ) -> Result<Vec<WriteAttribute>, RepositoryError> {
        Ok(sqlx::query_as::<_, WriteAttribute>(
            r#"SELECT id, code, value_type, value_schema,
                      CASE WHEN cardinality(target_blueprint_codes) > 0 THEN target_blueprint_codes WHEN target_blueprint_code IS NULL THEN '{}'::text[] ELSE ARRAY[target_blueprint_code] END AS target_blueprint_codes,
                      cardinality, target_cardinality, context_editable, context_fallback, default_value,
                      record_id IS NOT NULL AS record_scoped
               FROM attributes
               WHERE ((blueprint_id = $1 AND blueprint_version = $2) OR record_id = $3)
                 AND deleted_at IS NULL
               ORDER BY record_id IS NOT NULL, position, id"#,
        )
        .bind(record.blueprint_id)
        .bind(record.blueprint_version)
        .bind(record.id)
        .fetch_all(connection)
        .await?)
    }

    pub(super) fn attributes(&self) -> &[WriteAttribute] {
        &self.attributes
    }

    /// The attribute named by exactly one of an ID or a code, with the same
    /// errors as resolving it in SQL.
    pub(super) fn attribute(
        &self,
        attribute_id: Option<Uuid>,
        attribute_code: Option<&str>,
    ) -> Result<&WriteAttribute, RepositoryError> {
        match (attribute_id, attribute_code) {
            (Some(id), None) => self.attributes.iter().find(|attribute| attribute.id == id),
            (None, Some(code)) => {
                validate_attribute_selector_code(code)?;
                self.by_code(code)
            }
            _ => return Err(RepositoryError::InvalidAttributeSelector),
        }
        .ok_or(RepositoryError::AttributeNotApplicable)
    }

    pub(super) fn by_code(&self, code: &str) -> Option<&WriteAttribute> {
        self.attributes
            .iter()
            .find(|attribute| attribute.code == code)
    }

    /// The `default` context, or an explicitly named existing context.
    pub(super) fn resolve_context(
        &self,
        context_id: Option<Uuid>,
    ) -> Result<Uuid, RepositoryError> {
        let context_id = match context_id {
            Some(context_id) => context_id,
            None => self.default_context_id()?,
        };
        self.tree
            .get(context_id)
            .map(|node| node.id)
            .ok_or(RepositoryError::InvalidContext)
    }

    pub(super) fn default_context_id(&self) -> Result<Uuid, RepositoryError> {
        self.tree
            .nodes()
            .iter()
            .find(|node| node.code == "default")
            .map(|node| node.id)
            .ok_or(RepositoryError::InvalidContext)
    }

    /// Attributes editable only in `default` reject every other context.
    pub(super) fn ensure_editable(
        &self,
        context_id: Option<Uuid>,
        context_editable: &str,
    ) -> Result<(), RepositoryError> {
        if context_editable == "default" {
            let context_id = context_id.ok_or(RepositoryError::InvalidContext)?;
            if self
                .tree
                .get(context_id)
                .is_none_or(|node| node.code != "default")
            {
                return Err(RepositoryError::DefaultContextOnly);
            }
        }
        Ok(())
    }

    /// `(id, code, value_schema, context_fallback)` of status attributes.
    pub(super) fn status_attributes(&self) -> Vec<(Uuid, String, Value, String)> {
        self.attributes
            .iter()
            .filter(|attribute| attribute.declares(catalog_validation::status::STATUS_KEY))
            .map(|attribute| {
                (
                    attribute.id,
                    attribute.code.clone(),
                    attribute.value_schema.clone().unwrap_or(Value::Null),
                    attribute.context_fallback.clone(),
                )
            })
            .collect()
    }

    /// `(code, value_schema)` of principal (assignment) attributes.
    pub(super) fn principal_attributes(&self) -> Vec<(String, Value)> {
        self.attributes
            .iter()
            .filter(|attribute| attribute.declares(catalog_validation::principal::PRINCIPAL_KEY))
            .map(|attribute| {
                (
                    attribute.code.clone(),
                    attribute.value_schema.clone().unwrap_or(Value::Null),
                )
            })
            .collect()
    }
}
