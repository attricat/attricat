//! `default_from`: an attribute that copies a related record's value while it
//! is empty, such as an order line's `unit_price_net` from
//! `item.sale_price_net`.
//!
//! Every record write that can give a relationship a new target (create,
//! update, relationship replace and append) calls
//! [`AttricatRepository::apply_defaults_from`] in its transaction, after the
//! request's own values are written and before the record is validated. So the
//! API, imports, workflows, extensions and solution-pack seeds behave alike;
//! blueprint migrations and history restores do not default.
//!
//! The rule, for each attribute with a `default_from` path whose first
//! relationship gained a target in context `C`:
//!
//! - the attribute holds no direct value in `C` and the request neither wrote
//!   nor removed it there; an attribute editable only in `default` defaults
//!   only from a `default` relationship;
//! - each later hop and the leaf resolve in `C` with their own context
//!   fallback, reading only records the writer may read;
//! - the leaf has the attribute's value type (an `integer` may fill a
//!   `number`) and satisfies its value schema;
//! - the record's status does not lock the attribute in `C`.
//!
//! When any step finds nothing the attribute stays empty and the write
//! continues. The value is a snapshot: it is never copied again when the
//! source changes, and a value the writer supplies always wins.

use super::record_values::{ContextTree, RecordState, Selection, load_records};
use super::values::NativeValue;
use super::write_context::{WriteAttribute, WriteContext};
use super::*;
use attricat_domain::attribute_options::ContextEditable;
use attricat_domain::constants::DEFAULT_CONTEXT_CODE;
use attricat_domain::model::DefaultValuePreview;
use attricat_events::FACT_ORIGIN_DEFAULT_FROM;
use attricat_validation::validate_json_schema;
use std::collections::{BTreeSet, HashMap};

/// Relationship targets by `(attribute, context)`.
pub(super) type PriorTargets = HashMap<(Uuid, Uuid), BTreeSet<Uuid>>;

/// `(attribute, context)` keys of scalar values.
pub(super) type ValueKeys = HashSet<(Uuid, Uuid)>;

impl AttricatRepository {
    /// Copies `default_from` values into `record` for the relationships whose
    /// targets are not all in `before`, the relationship targets before this
    /// write (empty for a new record). `supplied` holds the scalar keys the
    /// request wrote or removed, which are never defaulted. Returns the keys
    /// it wrote, for [`mark_defaulted`].
    pub(super) async fn apply_defaults_from(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        write: &WriteContext,
        record: &Record,
        before: &PriorTargets,
        supplied: &ValueKeys,
    ) -> Result<ValueKeys, RepositoryError> {
        let candidates: Vec<(&WriteAttribute, Vec<&str>)> = write
            .attributes()
            .iter()
            .filter(|attribute| attribute.ensure_client_writable().is_ok())
            .filter_map(|attribute| {
                let path = attribute.default_from.as_deref()?;
                Some((attribute, path.split('.').collect()))
            })
            .collect();
        if candidates.is_empty() {
            return Ok(ValueKeys::new());
        }
        let after = load_records(
            transaction,
            self.workspace_id.0,
            Selection::Records(&[record.id]),
            None,
            RecordState::After,
        )
        .await?
        .remove(&record.id)
        .ok_or(RepositoryError::NotFound("record"))?;
        let mut values = Vec::new();
        let mut defaulted = ValueKeys::new();
        for (attribute, path) in candidates {
            let Some(relationship) = write.by_code(path[0]) else {
                continue;
            };
            let Some(links) = after.attributes.get(path[0]) else {
                continue;
            };
            let own = after.attributes.get(&attribute.code);
            // Sorted, so a write in several contexts defaults in one order.
            let mut contexts: Vec<_> = links.by_context.iter().collect();
            contexts.sort_by_key(|(context_id, _)| **context_id);
            for (&context_id, direct) in contexts {
                let targets = relationship_target_ids(&direct.value);
                let previous = before.get(&(relationship.id, context_id));
                let gained = targets
                    .iter()
                    .any(|target| previous.is_none_or(|previous| !previous.contains(target)));
                // `cardinality: one`: a gained target is the only one.
                let [target] = targets.as_slice() else {
                    continue;
                };
                if !gained
                    || supplied.contains(&(attribute.id, context_id))
                    || own.is_some_and(|own| own.by_context.contains_key(&context_id))
                    || !defaults_in(&write.tree, attribute.context_editable, context_id)
                {
                    continue;
                }
                let Some(value) = self
                    .default_value(
                        transaction,
                        &write.tree,
                        DefaultTarget {
                            value_type: attribute.value_type,
                            value_schema: attribute.value_schema.as_ref(),
                            hops: &path[1..],
                        },
                        context_id,
                        *target,
                    )
                    .await?
                else {
                    continue;
                };
                // A field the record's status locks keeps its (empty) value
                // rather than failing the write.
                match self
                    .ensure_attribute_unlocked(transaction, record, &attribute.code, context_id)
                    .await
                {
                    Ok(()) => {}
                    Err(RepositoryError::RecordLocked { .. }) => continue,
                    Err(error) => return Err(error),
                }
                defaulted.insert((attribute.id, context_id));
                values.push(NewAttributeValue::Scalar {
                    attribute_id: Some(attribute.id),
                    attribute_code: None,
                    context_id: Some(context_id),
                    value,
                });
            }
        }
        if !values.is_empty() {
            self.insert_values_in(transaction, write, record, values)
                .await?;
        }
        Ok(defaulted)
    }

    /// The values `default_from` would copy into a new or edited record of
    /// a blueprint revision when its `relationship` links to `target` in
    /// `context_id` (the default context when `None`), as the writer may read
    /// them. The record form shows them before it saves; the save applies
    /// the same rule.
    pub async fn preview_defaults_from(
        &self,
        blueprint_id: Uuid,
        blueprint_version: i64,
        context_id: Option<Uuid>,
        relationship: &str,
        target: Uuid,
    ) -> Result<Vec<DefaultValuePreview>, RepositoryError> {
        let attributes = self
            .list_attributes(blueprint_id, blueprint_version)
            .await?;
        let defaulted: Vec<_> = attributes
            .iter()
            .filter(|attribute| attribute.generated.is_none() && attribute.balance.is_none())
            .filter_map(|attribute| {
                let path: Vec<&str> = attribute.default_from.as_deref()?.split('.').collect();
                (path[0] == relationship).then_some((attribute, path))
            })
            .collect();
        if defaulted.is_empty() {
            return Ok(Vec::new());
        }
        let mut transaction = self.pool.begin().await?;
        let tree = ContextTree::load(&mut transaction, self.workspace_id.0).await?;
        let context_id = match context_id {
            Some(context_id) => {
                tree.get(context_id)
                    .ok_or(RepositoryError::InvalidContext)?
                    .id
            }
            None => tree.default_context()?.id,
        };
        let mut previews = Vec::new();
        for (attribute, path) in defaulted {
            if !defaults_in(&tree, attribute.context_editable, context_id) {
                continue;
            }
            if let Some(value) = self
                .default_value(
                    &mut transaction,
                    &tree,
                    DefaultTarget {
                        value_type: attribute.value_type,
                        value_schema: attribute.value_schema.as_ref(),
                        hops: &path[1..],
                    },
                    context_id,
                    target,
                )
                .await?
            {
                previews.push(DefaultValuePreview {
                    attribute_code: attribute.code.clone(),
                    value,
                });
            }
        }
        Ok(previews)
    }

    /// The value to copy into `into` from `target`, the first relationship's
    /// target: the leaf of the later hops, of a type and value the attribute
    /// accepts, in its normalized form.
    async fn default_value(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        tree: &ContextTree,
        into: DefaultTarget<'_>,
        context_id: Uuid,
        target: Uuid,
    ) -> Result<Option<Value>, RepositoryError> {
        Ok(self
            .default_source_value(transaction, tree, into.hops, context_id, target)
            .await?
            .filter(|(value_type, _)| value_type.copies_into(into.value_type))
            .and_then(|(_, value)| accepted(into.value_type, into.value_schema, value)))
    }

    /// The leaf of `hops` (later relationships and the leaf attribute) from
    /// `target` in `context_id`, with its value type, or `None` when a hop
    /// has no single target, a record is unreadable or lacks the attribute,
    /// or the leaf is empty or not scalar.
    async fn default_source_value(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        tree: &ContextTree,
        hops: &[&str],
        context_id: Uuid,
        mut target: Uuid,
    ) -> Result<Option<(ValueType, Value)>, RepositoryError> {
        let (leaf, relationships) = hops.split_last().expect("a path has a leaf");
        for (position, code) in relationships.iter().chain([leaf]).enumerate() {
            if !self.may_read_record(transaction, target).await? {
                return Ok(None);
            }
            let codes = [(*code).to_owned()];
            let Some(values) = load_records(
                transaction,
                self.workspace_id.0,
                Selection::Records(&[target]),
                Some(&codes),
                RecordState::After,
            )
            .await?
            .remove(&target)
            .and_then(|mut record| record.attributes.remove(*code)) else {
                // A target of another blueprint may not declare the leaf.
                return Ok(None);
            };
            let path = tree.path(context_id, values.inherit)?;
            let Some(direct) = values.resolve(&path) else {
                return Ok(None);
            };
            if position == relationships.len() {
                return Ok((!matches!(
                    values.value_type,
                    ValueType::Relationship | ValueType::File
                ))
                .then(|| (values.value_type, direct.value.clone())));
            }
            match relationship_target_ids(&direct.value).as_slice() {
                [next] if values.value_type == ValueType::Relationship => target = *next,
                _ => return Ok(None),
            }
        }
        unreachable!("the loop returns at the leaf")
    }

    /// Whether the interactive actor may read `record_id`; repositories
    /// without an actor are governed by their caller.
    async fn may_read_record(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        record_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        let Some(actor) = self.authorization_actor else {
            return Ok(true);
        };
        Ok(Self::principal_record_ids_on(
            transaction,
            actor,
            self.workspace_id.0,
            Permission::RecordsRead,
            &[record_id],
        )
        .await?
        .contains(&record_id))
    }
}

/// The attribute a default is copied into, and the path that reaches it
/// after the first relationship.
struct DefaultTarget<'a> {
    value_type: ValueType,
    value_schema: Option<&'a Value>,
    hops: &'a [&'a str],
}

/// Whether an attribute takes defaults in `context_id`: one editable only in
/// `default` defaults only from a `default` relationship.
fn defaults_in(tree: &ContextTree, editable: ContextEditable, context_id: Uuid) -> bool {
    editable != ContextEditable::Default
        || tree
            .get(context_id)
            .is_some_and(|node| node.code == DEFAULT_CONTEXT_CODE)
}

/// `value` in its stored form when `attribute`, a scalar, accepts it by its
/// value type and schema, as a client write would be checked; `None` for a
/// value it refuses and for relationships and files.
pub fn accepted_attribute_value(
    attribute: &attricat_domain::model::Attribute,
    value: Value,
) -> Option<Value> {
    if matches!(
        attribute.value_type,
        ValueType::Relationship | ValueType::File
    ) {
        return None;
    }
    accepted(attribute.value_type, attribute.value_schema.as_ref(), value)
}

/// `value` in its stored form when an attribute of `value_type` and
/// `value_schema` accepts it, as a client write would be checked. A source
/// value the attribute refuses is no default.
fn accepted(value_type: ValueType, value_schema: Option<&Value>, value: Value) -> Option<Value> {
    let native = NativeValue::parse(value_type, value).ok()?;
    let json = native.json();
    value_schema
        .is_none_or(|schema| {
            validate_json_schema(schema, &json).is_ok_and(|errors| errors.is_empty())
        })
        .then_some(json)
}

/// The target IDs of a relationship's direct value.
fn relationship_target_ids(value: &Value) -> Vec<Uuid> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|target| target.as_str()?.parse().ok())
        .collect()
}

/// Marks the audit changes of `defaulted` keys as copied by `default_from`.
pub(super) fn mark_defaulted(changes: &mut [AuditEventChange], defaulted: &ValueKeys) {
    if defaulted.is_empty() {
        return;
    }
    for change in changes {
        if change.relationship_target_record_id.is_none()
            && change
                .context_id
                .is_some_and(|context_id| defaulted.contains(&(change.attribute_id, context_id)))
        {
            change.origin = Some(FACT_ORIGIN_DEFAULT_FROM);
        }
    }
}

/// The `(attribute, context)` keys of the scalar values and removals a
/// request names. Values that do not resolve are left out; writing them fails
/// the request anyway.
pub(super) fn supplied_value_keys(
    write: &WriteContext,
    values: &[NewAttributeValue],
    removals: &[AttributeValueSelector],
) -> ValueKeys {
    let scalars = values.iter().filter_map(|value| match value {
        NewAttributeValue::Scalar {
            attribute_id,
            attribute_code,
            context_id,
            ..
        } => Some((
            write
                .attribute(*attribute_id, attribute_code.as_deref())
                .ok()?
                .id,
            write.resolve_context(*context_id).ok()?,
        )),
        NewAttributeValue::Relationship { .. } => None,
    });
    let removed = removals.iter().filter_map(|selector| {
        Some((
            write.by_code(&selector.attribute_code)?.id,
            write.resolve_context(selector.context_id).ok()?,
        ))
    });
    scalars.chain(removed).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn integers_fill_numbers_but_other_types_must_match() {
        assert!(ValueType::Number.copies_into(ValueType::Number));
        assert!(ValueType::Integer.copies_into(ValueType::Number));
        assert!(!ValueType::Number.copies_into(ValueType::Integer));
        assert!(!ValueType::String.copies_into(ValueType::Number));
        assert!(!ValueType::Date.copies_into(ValueType::Datetime));
    }

    #[test]
    fn relationship_values_list_their_target_ids() {
        let id = Uuid::new_v4();
        assert_eq!(
            relationship_target_ids(&json!([id.to_string(), "not-an-id"])),
            vec![id]
        );
        assert!(relationship_target_ids(&json!(null)).is_empty());
    }

    #[test]
    fn only_defaulted_scalar_changes_are_marked() {
        let (attribute, context, other) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let change = |attribute_id, relationship_target_record_id| AuditEventChange {
            record_id: Uuid::nil(),
            attribute_id,
            attribute_code: "price".into(),
            context_id: Some(context),
            context_code: None,
            relationship_target_record_id,
            change_kind: crate::domain_events::CHANGE_KIND_SET,
            before_value: None,
            after_value: None,
            origin: None,
        };
        let mut changes = vec![
            change(attribute, None),
            change(other, None),
            change(attribute, Some(Uuid::new_v4())),
        ];
        mark_defaulted(&mut changes, &HashSet::from([(attribute, context)]));
        assert_eq!(changes[0].origin, Some(FACT_ORIGIN_DEFAULT_FROM));
        assert_eq!(changes[1].origin, None);
        assert_eq!(changes[2].origin, None);
    }
}
