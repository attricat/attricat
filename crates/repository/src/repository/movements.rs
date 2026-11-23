//! Movements: records whose creation changes a balance on another record,
//! such as goods receipts and issues changing a stock level.
//!
//! The blueprint compiler checks a movement against its own blueprint's
//! attributes. Publication checks it against the blueprints it names, at
//! their latest published revisions: the target must declare the balance
//! and the unique key that finds it, with key values of matching types, and
//! the factor's blueprint must hold a number.
//!
//! A movement applies inside the create transaction of its record
//! ([`AttricatRepository::create_record_in_transaction`]), which every
//! create path shares, so the balance and its documents never disagree:
//!
//! 1. Before the record is inserted, [`AttricatRepository::prepare_movements_in`]
//!    reads the quantity and factor from the submitted default-context
//!    values, finds the target through its unique key and locks its row,
//!    creating it with a zero balance when the movement allows. Target rows
//!    are record rows, so they are locked before the new record's counters
//!    and unique keys, as the lock order requires.
//! 2. The record is written with the target and the moved amount, which
//!    only core can set.
//! 3. After the record validates, [`AttricatRepository::apply_movements_in`]
//!    changes each target's balance through the ordinary update seam
//!    (validation, status locks, audit and `record.updated.v1`) and refuses
//!    a result below the balance's `min`, which rolls back the whole create.

use std::collections::{BTreeSet, HashMap};

use attricat_blueprint::{
    BalancePolicy, CompiledBlueprint, EffectiveAttribute, MovementDefinition,
};
use attricat_domain::attribute_options::{Cardinality, MovementDirection};
use attricat_domain::value_type::ValueType;
use rust_decimal::Decimal;
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::record_commands::{ChosenIdRecordCreate, CreateFileValues, lock_record_writes};
use super::record_values::{DirectValue, RecordState, RecordValues, load_record};
use super::structural_constraints::{EnforcedUniqueKey, UniqueKeyScope, key_hash};
use super::write_context::WriteContext;
use super::{AttricatRepository, Record, RepositoryError};
use crate::model::{NewAttributeValue, UpdateRecordFormRequest};

/// Audit action of a target's balance change, audited against the target
/// with the movement record in its metadata.
pub(super) const MOVEMENT_BALANCE_AUDIT_ACTION: &str = "attricat.records.movement.balance_update";
/// Audit action of a target a movement created because none had its key.
pub(super) const MOVEMENT_TARGET_AUDIT_ACTION: &str = "attricat.records.movement.target_create";

/// What publication needs from an attribute of another blueprint.
#[derive(sqlx::FromRow)]
struct ReferencedAttribute {
    code: String,
    value_type: String,
    cardinality: Option<String>,
    target_blueprint_code: Option<String>,
    target_blueprint_codes: Vec<String>,
    balance: Option<serde_json::Value>,
}

impl ReferencedAttribute {
    fn value_type(&self) -> Option<ValueType> {
        ValueType::parse(&self.value_type)
    }

    fn single_relationship(&self) -> bool {
        self.value_type() == Some(ValueType::Relationship)
            && self.cardinality.as_deref() == Some(Cardinality::One.as_str())
    }

    /// Every allowed target; a single target is stored only in
    /// `target_blueprint_code`.
    fn targets(&self) -> Vec<String> {
        if self.target_blueprint_codes.is_empty() {
            self.target_blueprint_code.iter().cloned().collect()
        } else {
            self.target_blueprint_codes.clone()
        }
    }
}

/// The latest published revision of a record blueprint, as publication of
/// a blueprint that references it sees it.
struct PublishedBlueprint {
    unique_keys: Vec<EnforcedUniqueKey>,
    attributes: HashMap<String, ReferencedAttribute>,
}

impl AttricatRepository {
    /// Checks the movements and target scopes of a revision about to be
    /// published against the blueprints they reference.
    pub(super) async fn validate_publication_movements(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        compiled: &CompiledBlueprint,
    ) -> Result<(), RepositoryError> {
        for movement in &compiled.movements {
            self.validate_movement_targets(transaction, compiled, movement)
                .await?;
        }
        for attribute in &compiled.attributes {
            self.validate_target_scope(transaction, attribute).await?;
        }
        Ok(())
    }

    async fn validate_movement_targets(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        compiled: &CompiledBlueprint,
        movement: &MovementDefinition,
    ) -> Result<(), RepositoryError> {
        let invalid = |message: String| {
            RepositoryError::InvalidBlueprintDefinition(format!(
                "movement '{}' is invalid: {message}",
                movement.code
            ))
        };
        let target_code = &movement.target.blueprint;
        let target = self
            .published_blueprint(transaction, target_code)
            .await?
            .ok_or_else(|| {
                invalid(format!(
                    "target blueprint '{target_code}' has no published revision"
                ))
            })?;
        if !target
            .attributes
            .get(&movement.target.balance)
            .is_some_and(|attribute| attribute.balance.is_some())
        {
            return Err(invalid(format!(
                "'{}' is not a balance attribute of '{target_code}'",
                movement.target.balance
            )));
        }
        let key = target
            .unique_keys
            .iter()
            .find(|key| key.code == movement.target.key)
            .ok_or_else(|| {
                invalid(format!(
                    "'{target_code}' has no unique key '{}'",
                    movement.target.key
                ))
            })?;
        if key.scope != UniqueKeyScope::Workspace {
            return Err(invalid(format!(
                "unique key '{}' must have workspace scope",
                key.code
            )));
        }
        let key_attributes: BTreeSet<&str> = key.attributes.iter().map(String::as_str).collect();
        let mapped: BTreeSet<&str> = movement.target.values.keys().map(String::as_str).collect();
        if key_attributes != mapped {
            return Err(invalid(format!(
                "target values must map exactly the attributes of unique key '{}': {}",
                key.code,
                key.attributes.join(", ")
            )));
        }
        let own = |code: &str| {
            compiled
                .attributes
                .iter()
                .find(|attribute| attribute.code == code)
        };
        for (key_attribute, source) in &movement.target.values {
            let (Some(target_attribute), Some(source)) =
                (target.attributes.get(key_attribute), own(source))
            else {
                return Err(invalid(format!("unknown key attribute '{key_attribute}'")));
            };
            let same_targets = source.value_type != ValueType::Relationship
                || target_attribute.targets() == source.target_blueprints;
            if target_attribute.value_type() != Some(source.value_type) || !same_targets {
                return Err(invalid(format!(
                    "'{}' and key attribute '{key_attribute}' of '{target_code}' must have the same type and targets",
                    source.code
                )));
            }
        }
        if let Some(factor) = &movement.factor {
            let relationship = own(&factor.relationship)
                .ok_or_else(|| invalid(format!("unknown attribute '{}'", factor.relationship)))?;
            let factor_code = &relationship.target_blueprints[0];
            let factor_blueprint = self
                .published_blueprint(transaction, factor_code)
                .await?
                .ok_or_else(|| {
                    invalid(format!(
                        "factor blueprint '{factor_code}' has no published revision"
                    ))
                })?;
            if !factor_blueprint
                .attributes
                .get(&factor.attribute)
                .and_then(ReferencedAttribute::value_type)
                .is_some_and(|value_type| {
                    matches!(value_type, ValueType::Number | ValueType::Integer)
                })
            {
                return Err(invalid(format!(
                    "factor '{}' must be a number or integer attribute of '{factor_code}'",
                    factor.attribute
                )));
            }
        }
        Ok(())
    }

    /// A scope names a relationship of the target blueprint that links to
    /// the same blueprint as the scoped record's `equals`. A target without a
    /// published revision has no records to offer yet, so it is not checked.
    async fn validate_target_scope(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        attribute: &EffectiveAttribute,
    ) -> Result<(), RepositoryError> {
        let Some(scope) = &attribute.target_scope else {
            return Ok(());
        };
        let Some(target_code) = attribute.target_blueprint.as_deref() else {
            return Ok(());
        };
        let Some(target) = self.published_blueprint(transaction, target_code).await? else {
            return Ok(());
        };
        if !target
            .attributes
            .get(&scope.attribute)
            .is_some_and(ReferencedAttribute::single_relationship)
        {
            return Err(RepositoryError::InvalidBlueprintDefinition(format!(
                "relationship attribute '{}' has an invalid target_scope: '{}' is not a relationship of '{target_code}' with cardinality: one",
                attribute.code, scope.attribute
            )));
        }
        Ok(())
    }

    async fn published_blueprint(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        code: &str,
    ) -> Result<Option<PublishedBlueprint>, RepositoryError> {
        let Some((blueprint_id, version, unique_keys)) =
            sqlx::query_as::<_, (Uuid, i64, serde_json::Value)>(
                r#"SELECT id, version, unique_keys FROM blueprints
                   WHERE workspace_id = $1 AND code = $2 AND kind = 'record'
                     AND status = 'published' AND deleted_at IS NULL
                   ORDER BY version DESC LIMIT 1"#,
            )
            .bind(self.workspace_id.0)
            .bind(code)
            .fetch_optional(&mut **transaction)
            .await?
        else {
            return Ok(None);
        };
        let unique_keys = serde_json::from_value(unique_keys)
            .map_err(|error| RepositoryError::InvalidBlueprintDefinition(error.to_string()))?;
        let attributes = sqlx::query_as::<_, ReferencedAttribute>(
            r#"SELECT code, value_type, cardinality, target_blueprint_code,
                      target_blueprint_codes, balance
               FROM attributes
               WHERE workspace_id = $1 AND blueprint_id = $2 AND blueprint_version = $3
                 AND deleted_at IS NULL"#,
        )
        .bind(self.workspace_id.0)
        .bind(blueprint_id)
        .bind(version)
        .fetch_all(&mut **transaction)
        .await?
        .into_iter()
        .map(|attribute| (attribute.code.clone(), attribute))
        .collect();
        Ok(Some(PublishedBlueprint {
            unique_keys,
            attributes,
        }))
    }
}

/// A movement of a record about to be created, with its target found and
/// locked.
pub(super) struct PreparedMovement {
    code: String,
    direction: MovementDirection,
    quantity: String,
    target_id: Uuid,
    target_relationship: String,
    balance: String,
    result: Option<String>,
    amount: Decimal,
    /// A reversal's inputs the record did not submit, copied from the
    /// record it reverses.
    copied: Vec<NewAttributeValue>,
}

impl PreparedMovement {
    /// The values core sets on the movement record: its target and, when
    /// declared, the moved amount.
    pub(super) fn record_values(&self, default_context_id: Uuid) -> Vec<NewAttributeValue> {
        let mut values = vec![NewAttributeValue::Relationship {
            attribute_id: None,
            attribute_code: Some(self.target_relationship.clone()),
            context_id: Some(default_context_id),
            target_record_id: self.target_id,
        }];
        if let Some(result) = &self.result {
            values.push(NewAttributeValue::Scalar {
                attribute_id: None,
                attribute_code: Some(result.clone()),
                context_id: Some(default_context_id),
                value: decimal_json(self.amount),
            });
        }
        values.extend(self.copied.iter().cloned());
        values
    }
}

/// The default-context values a create submits, by attribute code.
struct SubmittedValues(HashMap<String, Submitted>);

#[derive(Clone)]
enum Submitted {
    Scalar(Value),
    Relationship(Uuid),
}

impl SubmittedValues {
    /// The last value submitted for each attribute in the default context,
    /// as the create writes it.
    fn new(
        values: &[NewAttributeValue],
        codes: &HashMap<Uuid, String>,
        default_context_id: Uuid,
    ) -> Self {
        let mut submitted = HashMap::new();
        for value in values {
            let (attribute_id, attribute_code, context_id, value) = match value {
                NewAttributeValue::Scalar {
                    attribute_id,
                    attribute_code,
                    context_id,
                    value,
                } => (
                    attribute_id,
                    attribute_code,
                    context_id,
                    Submitted::Scalar(value.clone()),
                ),
                NewAttributeValue::Relationship {
                    attribute_id,
                    attribute_code,
                    context_id,
                    target_record_id,
                } => (
                    attribute_id,
                    attribute_code,
                    context_id,
                    Submitted::Relationship(*target_record_id),
                ),
            };
            if context_id.is_some_and(|id| id != default_context_id) {
                continue;
            }
            let code = attribute_code
                .clone()
                .or_else(|| attribute_id.and_then(|id| codes.get(&id).cloned()));
            if let Some(code) = code {
                submitted.insert(code, value);
            }
        }
        Self(submitted)
    }

    fn get(&self, code: &str) -> Option<&Submitted> {
        self.0.get(code)
    }

    fn relationship(&self, code: &str) -> Option<Uuid> {
        match self.get(code) {
            Some(Submitted::Relationship(target)) => Some(*target),
            _ => None,
        }
    }

    fn decimal(&self, code: &str) -> Option<Decimal> {
        match self.get(code) {
            Some(Submitted::Scalar(Value::Number(number))) => number.to_string().parse().ok(),
            _ => None,
        }
    }
}

/// The record IDs of a relationship's direct value.
fn linked_ids(value: &DirectValue) -> Vec<Uuid> {
    match &value.value {
        Value::Array(targets) => targets
            .iter()
            .filter_map(Value::as_str)
            .filter_map(|target| Uuid::parse_str(target).ok())
            .collect(),
        Value::String(target) => Uuid::parse_str(target).into_iter().collect(),
        _ => Vec::new(),
    }
}

/// Whether a submitted reversal input is the original's value; numbers
/// compare by value, so `2` and `2.0` match.
fn same_input(given: &Submitted, original: &Submitted) -> bool {
    match (given, original) {
        (Submitted::Relationship(given), Submitted::Relationship(original)) => given == original,
        (Submitted::Scalar(Value::Number(given)), Submitted::Scalar(Value::Number(original))) => {
            given.to_string().parse::<Decimal>().ok()
                == original.to_string().parse::<Decimal>().ok()
        }
        (Submitted::Scalar(given), Submitted::Scalar(original)) => given == original,
        _ => false,
    }
}

/// A decimal as the JSON number the value pipeline parses back into it.
fn decimal_json(value: Decimal) -> Value {
    serde_json::from_str(&value.normalize().to_string()).expect("a decimal is a JSON number")
}

impl AttricatRepository {
    /// The movements of a record blueprint revision; empty for most.
    pub(super) async fn revision_movements(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
        blueprint_version: i64,
    ) -> Result<Vec<MovementDefinition>, RepositoryError> {
        let movements: Option<Value> = sqlx::query_scalar(
            "SELECT movements FROM blueprints WHERE workspace_id = $1 AND id = $2 AND version = $3",
        )
        .bind(self.workspace_id.0)
        .bind(blueprint_id)
        .bind(blueprint_version)
        .fetch_optional(&mut **transaction)
        .await?;
        match movements {
            Some(movements) => serde_json::from_value(movements)
                .map_err(|error| RepositoryError::InvalidBlueprintDefinition(error.to_string())),
            None => Ok(Vec::new()),
        }
    }

    /// Resolves every movement of a record about to be created from its
    /// submitted `values`: the moved amount, and the target, found through
    /// its unique key and locked, or created. A reversal instead moves back
    /// what the record it reverses moved. The caller holds the relationship
    /// lock and has not inserted the record yet. A sample record's missing
    /// target is created as a sample too, so removing samples removes it.
    pub(super) async fn prepare_movements_in(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
        blueprint_version: i64,
        values: &[NewAttributeValue],
        movements: Vec<MovementDefinition>,
        sample: bool,
    ) -> Result<Vec<PreparedMovement>, RepositoryError> {
        let default_context_id = self
            .resolve_context_id(transaction, None)
            .await?
            .ok_or(RepositoryError::InvalidContext)?;
        let attributes: Vec<(Uuid, String, String)> = sqlx::query_as(
            r#"SELECT id, code, value_type FROM attributes
               WHERE workspace_id = $1 AND blueprint_id = $2 AND blueprint_version = $3
                 AND record_id IS NULL AND deleted_at IS NULL"#,
        )
        .bind(self.workspace_id.0)
        .bind(blueprint_id)
        .bind(blueprint_version)
        .fetch_all(&mut **transaction)
        .await?;
        let codes = attributes
            .iter()
            .map(|(id, code, _)| (*id, code.clone()))
            .collect();
        let value_types: HashMap<String, ValueType> = attributes
            .iter()
            .filter_map(|(_, code, value_type)| {
                ValueType::parse(value_type).map(|value_type| (code.clone(), value_type))
            })
            .collect();
        let submitted = SubmittedValues::new(values, &codes, default_context_id);
        let mut prepared = Vec::with_capacity(movements.len());
        for movement in movements {
            if let Some(reversal) = &movement.reversal
                && let Some(original_id) = submitted.relationship(&reversal.relationship)
            {
                prepared.push(
                    self.prepare_reversal_in(
                        transaction,
                        blueprint_id,
                        movement,
                        original_id,
                        &submitted,
                        default_context_id,
                    )
                    .await?,
                );
                continue;
            }
            let quantity = submitted
                .decimal(&movement.quantity)
                .filter(|quantity| *quantity > Decimal::ZERO)
                .ok_or_else(|| RepositoryError::MovementQuantityInvalid {
                    attribute: movement.quantity.clone(),
                })?;
            let factor = match &movement.factor {
                Some(factor) => match submitted.relationship(&factor.relationship) {
                    Some(linked) => self
                        .default_context_number(
                            transaction,
                            linked,
                            &factor.attribute,
                            default_context_id,
                        )
                        .await?
                        .filter(|factor| *factor > Decimal::ZERO)
                        .ok_or_else(|| RepositoryError::MovementFactorInvalid {
                            attribute: factor.attribute.clone(),
                        })?,
                    None => Decimal::ONE,
                },
                None => Decimal::ONE,
            };
            let amount = quantity.checked_mul(factor).ok_or_else(|| {
                RepositoryError::MovementQuantityInvalid {
                    attribute: movement.quantity.clone(),
                }
            })?;
            let target_id = self
                .movement_target_in(
                    transaction,
                    &movement,
                    &submitted,
                    &value_types,
                    default_context_id,
                    sample,
                )
                .await?;
            prepared.push(PreparedMovement {
                code: movement.code,
                direction: movement.direction,
                quantity: movement.quantity,
                target_id,
                target_relationship: movement.target.relationship,
                balance: movement.target.balance,
                result: movement.result,
                amount,
                copied: Vec::new(),
            });
        }
        Ok(prepared)
    }

    /// A reversal of `original_id`: the original's amount moved the other
    /// way on the original's target. Both rows are locked, in ID order, so
    /// two reversals of one record cannot both pass the check that none
    /// exists. The reversal's inputs are the original's: submitted ones must
    /// match, and missing ones are copied.
    async fn prepare_reversal_in(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
        movement: MovementDefinition,
        original_id: Uuid,
        submitted: &SubmittedValues,
        default_context_id: Uuid,
    ) -> Result<PreparedMovement, RepositoryError> {
        let reversal = movement
            .reversal
            .as_ref()
            .expect("the caller found a reversal link");
        let original = self
            .movement_record_values(transaction, original_id)
            .await?;
        let direct = |code: &str| {
            original
                .attributes
                .get(code)
                .and_then(|attribute| attribute.by_context.get(&default_context_id))
        };
        let target_id = direct(&movement.target.relationship)
            .and_then(|value| linked_ids(value).first().copied())
            .ok_or_else(|| RepositoryError::MovementTargetMissing {
                blueprint: movement.target.blueprint.clone(),
                key: movement.target.key.clone(),
            })?;
        let mut rows = [original_id, target_id];
        rows.sort();
        for row in rows {
            self.lock_record(transaction, row).await?;
        }
        if direct(&reversal.relationship).is_some_and(|value| !linked_ids(value).is_empty()) {
            return Err(RepositoryError::MovementReversalOfReversal {
                record_id: original_id,
            });
        }
        let reversed_by: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT av.record_id FROM attribute_values av
               JOIN attributes a ON a.id = av.attribute_id AND a.workspace_id = av.workspace_id
               JOIN records r ON r.id = av.record_id AND r.workspace_id = av.workspace_id
                AND r.deleted_at IS NULL
               WHERE av.workspace_id = $1 AND av.relationship_target_record_id = $2
                 AND av.active AND a.code = $3 AND r.blueprint_id = $4
               LIMIT 1"#,
        )
        .bind(self.workspace_id.0)
        .bind(original_id)
        .bind(&reversal.relationship)
        .bind(blueprint_id)
        .fetch_optional(&mut **transaction)
        .await?;
        if let Some(reversed_by) = reversed_by {
            return Err(RepositoryError::MovementAlreadyReversed {
                record_id: original_id,
                reversed_by,
            });
        }
        let result = movement
            .result
            .as_deref()
            .expect("the compiler requires a result for reversals");
        let amount = direct(result)
            .and_then(|value| value.exact.as_deref()?.parse::<Decimal>().ok())
            .ok_or_else(|| RepositoryError::MovementQuantityInvalid {
                attribute: result.to_owned(),
            })?;
        let inputs = std::iter::once(movement.quantity.as_str())
            .chain(
                movement
                    .factor
                    .iter()
                    .map(|factor| factor.relationship.as_str()),
            )
            .chain(movement.target.values.values().map(String::as_str));
        let mut copied = Vec::new();
        for code in inputs {
            let Some(value) = direct(code) else {
                if submitted.get(code).is_some() {
                    return Err(RepositoryError::MovementReversalMismatch {
                        attribute: code.to_owned(),
                    });
                }
                continue;
            };
            let original_value = match linked_ids(value).first() {
                Some(target) => Submitted::Relationship(*target),
                None => Submitted::Scalar(value.value.clone()),
            };
            match submitted.get(code) {
                Some(given) if !same_input(given, &original_value) => {
                    return Err(RepositoryError::MovementReversalMismatch {
                        attribute: code.to_owned(),
                    });
                }
                Some(_) => {}
                None => copied.push(match original_value {
                    Submitted::Relationship(target_record_id) => NewAttributeValue::Relationship {
                        attribute_id: None,
                        attribute_code: Some(code.to_owned()),
                        context_id: Some(default_context_id),
                        target_record_id,
                    },
                    Submitted::Scalar(value) => NewAttributeValue::Scalar {
                        attribute_id: None,
                        attribute_code: Some(code.to_owned()),
                        context_id: Some(default_context_id),
                        value,
                    },
                }),
            }
        }
        Ok(PreparedMovement {
            code: movement.code,
            direction: movement.direction.reversed(),
            quantity: movement.quantity,
            target_id,
            target_relationship: movement.target.relationship,
            balance: movement.target.balance,
            result: movement.result,
            amount,
            copied,
        })
    }

    async fn movement_record_values(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        record_id: Uuid,
    ) -> Result<RecordValues, RepositoryError> {
        load_record(
            transaction,
            self.workspace_id.0,
            record_id,
            RecordState::After,
        )
        .await?
        .ok_or(RepositoryError::NotFound("record"))
    }

    /// Rejects a change to a value a movement read or set when its record
    /// was created: the movement applied once, so the balance would no
    /// longer match. A movement target's key values are kept the same way:
    /// movements found the target by them. Values are compared in the
    /// default context, the only one these values are editable in.
    pub(super) async fn ensure_movement_values_unchanged(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        write: &WriteContext,
        record: &Record,
    ) -> Result<(), RepositoryError> {
        if write.new_record {
            return Ok(());
        }
        let guarded: Vec<(&str, bool)> = write
            .attributes()
            .iter()
            .filter(|attribute| {
                !attribute.record_scoped
                    && (attribute.movement_input
                        || attribute.movement_managed
                        || attribute.movement_key)
            })
            .map(|attribute| {
                (
                    attribute.code.as_str(),
                    attribute.movement_key
                        && !attribute.movement_input
                        && !attribute.movement_managed,
                )
            })
            .collect();
        if guarded.is_empty() {
            return Ok(());
        }
        let default_context_id = write.default_context_id()?;
        let before = load_record(
            transaction,
            self.workspace_id.0,
            record.id,
            RecordState::Before,
        )
        .await?
        .unwrap_or_default();
        let after = write
            .after_record(transaction, self.workspace_id.0, record.id)
            .await?
            .unwrap_or_default();
        let input = |values: &RecordValues, code: &str| {
            values
                .attributes
                .get(code)
                .and_then(|attribute| attribute.by_context.get(&default_context_id))
                .map(DirectValue::key_input)
        };
        // A target key may be set when it has no value yet, as a target
        // created earlier in this transaction is, but never changed.
        if let Some((code, target_key)) = guarded.into_iter().find(|(code, target_key)| {
            let previous = input(&before, code);
            (previous.is_some() || !target_key) && previous != input(&after, code)
        }) {
            let attribute = code.to_owned();
            return Err(if target_key {
                RepositoryError::MovementTargetKeyImmutable { attribute }
            } else {
                RepositoryError::MovementImmutable { attribute }
            });
        }
        Ok(())
    }

    /// A movement record is reversed, never deleted, and a record that still
    /// holds a balance keeps it. Solution-pack samples are removed together,
    /// so they are exempt.
    pub(super) async fn ensure_movement_deletable(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        record: &Record,
    ) -> Result<(), RepositoryError> {
        if record.is_sample {
            return Ok(());
        }
        if !self
            .revision_movements(transaction, record.blueprint_id, record.blueprint_version)
            .await?
            .is_empty()
        {
            return Err(RepositoryError::MovementRecordUndeletable);
        }
        let balances: Vec<String> = sqlx::query_scalar(
            r#"SELECT code FROM attributes
               WHERE workspace_id = $1 AND blueprint_id = $2 AND blueprint_version = $3
                 AND record_id IS NULL AND balance IS NOT NULL AND deleted_at IS NULL"#,
        )
        .bind(self.workspace_id.0)
        .bind(record.blueprint_id)
        .bind(record.blueprint_version)
        .fetch_all(&mut **transaction)
        .await?;
        if balances.is_empty() {
            return Ok(());
        }
        let default_context_id = self
            .resolve_context_id(transaction, None)
            .await?
            .ok_or(RepositoryError::InvalidContext)?;
        for attribute in balances {
            if let Some(balance) = self
                .default_context_number(transaction, record.id, &attribute, default_context_id)
                .await?
                .filter(|balance| !balance.is_zero())
            {
                return Err(RepositoryError::BalanceRecordNotEmpty { attribute, balance });
            }
        }
        Ok(())
    }

    /// Finds the movement's target by its unique key and locks it, or
    /// creates it with the key values when the movement allows. Concurrent
    /// movements to one missing target serialize on the key, so only the
    /// first creates it.
    async fn movement_target_in(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        movement: &MovementDefinition,
        submitted: &SubmittedValues,
        value_types: &HashMap<String, ValueType>,
        default_context_id: Uuid,
        sample: bool,
    ) -> Result<Uuid, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let blueprint = &movement.target.blueprint;
        let missing = || RepositoryError::MovementTargetMissing {
            blueprint: blueprint.clone(),
            key: movement.target.key.clone(),
        };
        let (target_blueprint_id, target_version): (Uuid, i64) = sqlx::query_as(
            r#"SELECT id, version FROM blueprints
               WHERE workspace_id = $1 AND code = $2 AND kind = 'record'
                 AND status = 'published' AND deleted_at IS NULL
               ORDER BY version DESC LIMIT 1"#,
        )
        .bind(workspace_id)
        .bind(blueprint)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| {
            RepositoryError::InvalidBlueprintDefinition(format!(
                "movement '{}' targets blueprint '{blueprint}', which has no published revision",
                movement.code
            ))
        })?;
        let key = super::structural_constraints::enforced_unique_keys(
            transaction,
            workspace_id,
            target_blueprint_id,
        )
        .await?
        .into_iter()
        .find(|key| key.code == movement.target.key)
        .ok_or_else(|| {
            RepositoryError::InvalidBlueprintDefinition(format!(
                "movement '{}' needs unique key '{}' of '{blueprint}'",
                movement.code, movement.target.key
            ))
        })?;
        let mut components = Vec::with_capacity(key.attributes.len());
        let mut create_values = Vec::with_capacity(key.attributes.len());
        for key_attribute in &key.attributes {
            let source = movement
                .target
                .values
                .get(key_attribute)
                .ok_or_else(missing)?;
            let value_type = *value_types.get(source).ok_or_else(missing)?;
            let (input, create_value) = match submitted.get(source).ok_or_else(missing)? {
                Submitted::Relationship(target) => (
                    Value::String(target.to_string()),
                    NewAttributeValue::Relationship {
                        attribute_id: None,
                        attribute_code: Some(key_attribute.clone()),
                        context_id: Some(default_context_id),
                        target_record_id: *target,
                    },
                ),
                Submitted::Scalar(value) => (
                    value.clone(),
                    NewAttributeValue::Scalar {
                        attribute_id: None,
                        attribute_code: Some(key_attribute.clone()),
                        context_id: Some(default_context_id),
                        value: value.clone(),
                    },
                ),
            };
            components.push(
                attricat_validation::unique_key::normalize_key_component(
                    value_type,
                    &input,
                    key.case_sensitive,
                )
                .ok_or_else(missing)?,
            );
            create_values.push(create_value);
        }
        let hash = key_hash(&Value::Array(components));
        // The record-writer lock comes before any record row lock.
        lock_record_writes(transaction, workspace_id, false).await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!(
                "movement-target-key:{workspace_id}:{target_blueprint_id}:{}:{hash}",
                key.code
            ))
            .execute(&mut **transaction)
            .await?;
        let found: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT e.id FROM record_unique_key_values k
               JOIN records e ON e.id = k.record_id AND e.workspace_id = k.workspace_id
               WHERE k.workspace_id = $1 AND k.blueprint_id = $2 AND k.key_code = $3
                 AND k.context_id = $4 AND k.key_hash = $5 AND e.deleted_at IS NULL
               FOR UPDATE OF e"#,
        )
        .bind(workspace_id)
        .bind(target_blueprint_id)
        .bind(&key.code)
        .bind(default_context_id)
        .bind(&hash)
        .fetch_optional(&mut **transaction)
        .await?;
        if let Some(target_id) = found {
            return Ok(target_id);
        }
        if !movement.target.create_missing {
            return Err(missing());
        }
        // A target has no movements of its own to resolve first; boxing
        // breaks the async recursion through the create seam.
        let (target, changes, event) = Box::pin(self.create_record_in_transaction(
            transaction,
            ChosenIdRecordCreate {
                record_id: Uuid::new_v4(),
                blueprint_id: target_blueprint_id,
                blueprint_version: target_version,
                values: create_values,
                files: CreateFileValues::None,
                system_tags: if sample {
                    vec![super::SAMPLE_SYSTEM_TAG.to_owned()]
                } else {
                    Vec::new()
                },
                system_metadata: json!({}),
                host_sample_marker: sample,
            },
        ))
        .await?;
        self.for_movement_target(MOVEMENT_TARGET_AUDIT_ACTION, target.id, &movement.code)
            .stage_record_mutation(transaction, changes, event)
            .await?;
        Ok(target.id)
    }

    /// Changes each target's balance by its movement's amount once the
    /// movement record validated, and stages the targets' updates.
    pub(super) async fn apply_movements_in(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        record: &Record,
        movements: &[PreparedMovement],
        default_context_id: Uuid,
    ) -> Result<(), RepositoryError> {
        for movement in movements {
            let (policy, value_type): (Option<sqlx::types::Json<BalancePolicy>>, String) =
                sqlx::query_as(
                    r#"SELECT a.balance, a.value_type FROM attributes a
                       JOIN records r ON r.blueprint_id = a.blueprint_id
                        AND r.blueprint_version = a.blueprint_version AND r.workspace_id = a.workspace_id
                       WHERE r.workspace_id = $1 AND r.id = $2 AND a.code = $3
                         AND a.record_id IS NULL AND a.deleted_at IS NULL"#,
                )
                .bind(self.workspace_id.0)
                .bind(movement.target_id)
                .bind(&movement.balance)
                .fetch_optional(&mut **transaction)
                .await?
                .ok_or_else(|| self.not_a_balance(movement))?;
            let policy = policy.ok_or_else(|| self.not_a_balance(movement))?.0;
            let available = self
                .default_context_number(
                    transaction,
                    movement.target_id,
                    &movement.balance,
                    default_context_id,
                )
                .await?
                .unwrap_or(Decimal::ZERO);
            let balance = match movement.direction {
                MovementDirection::In => available.checked_add(movement.amount),
                MovementDirection::Out => available.checked_sub(movement.amount),
            }
            .ok_or_else(|| RepositoryError::MovementQuantityInvalid {
                attribute: movement.quantity.clone(),
            })?;
            let integer = ValueType::parse(&value_type) == Some(ValueType::Integer);
            if integer && !balance.fract().is_zero() {
                return Err(RepositoryError::MovementQuantityInvalid {
                    attribute: movement.quantity.clone(),
                });
            }
            if let Some(minimum) = policy.min
                && balance < minimum
            {
                return Err(RepositoryError::BalanceBelowMinimum {
                    attribute: movement.balance.clone(),
                    record_id: movement.target_id,
                    available,
                    requested: movement.amount,
                    minimum,
                });
            }
            let (_, changes, event) = self
                .update_record_from_movement_in(
                    transaction,
                    movement.target_id,
                    UpdateRecordFormRequest {
                        expected_updated_at: None,
                        values: vec![NewAttributeValue::Scalar {
                            attribute_id: None,
                            attribute_code: Some(movement.balance.clone()),
                            context_id: Some(default_context_id),
                            value: decimal_json(balance),
                        }],
                        relationships: Vec::new(),
                        remove_values: Vec::new(),
                        system_tags: None,
                        system_metadata: None,
                    },
                )
                .await?;
            let mut repository = self.for_movement_target(
                MOVEMENT_BALANCE_AUDIT_ACTION,
                movement.target_id,
                &movement.code,
            );
            if let Some(audit) = repository.audit_context.as_mut()
                && let Value::Object(metadata) = &mut audit.metadata
            {
                metadata.insert(
                    "movement".to_owned(),
                    json!({
                        "code": movement.code,
                        "record_id": record.id,
                        "direction": movement.direction.as_str(),
                        "amount": movement.amount.normalize().to_string(),
                    }),
                );
            }
            repository
                .stage_record_mutation(transaction, changes, event)
                .await?;
        }
        Ok(())
    }

    fn not_a_balance(&self, movement: &PreparedMovement) -> RepositoryError {
        RepositoryError::InvalidBlueprintDefinition(format!(
            "movement '{}' changes '{}', which is not a balance of its target's revision",
            movement.code, movement.balance
        ))
    }

    /// A record's current number or integer in the default context.
    async fn default_context_number(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        record_id: Uuid,
        attribute_code: &str,
        default_context_id: Uuid,
    ) -> Result<Option<Decimal>, RepositoryError> {
        Ok(sqlx::query_scalar::<_, Option<Decimal>>(
            r#"SELECT COALESCE(av.value_number, av.value_integer::numeric)
               FROM attribute_values av
               JOIN attributes a ON a.id = av.attribute_id AND a.workspace_id = av.workspace_id
               WHERE av.workspace_id = $1 AND av.record_id = $2 AND a.code = $3
                 AND av.context_id = $4 AND av.active
                 AND av.relationship_target_record_id IS NULL AND a.record_id IS NULL"#,
        )
        .bind(self.workspace_id.0)
        .bind(record_id)
        .bind(attribute_code)
        .bind(default_context_id)
        .fetch_optional(&mut **transaction)
        .await?
        .flatten())
    }

    /// The repository auditing a movement's write to `target_id` under
    /// `action`, sharing the request and correlation IDs.
    fn for_movement_target(&self, action: &str, target_id: Uuid, movement: &str) -> Self {
        let mut repository = self.clone();
        if let Some(audit) = repository.audit_context.as_mut() {
            audit.action = action.to_owned();
            audit.target = json!({ "type": "record", "id": target_id });
            if let Value::Object(metadata) = &mut audit.metadata {
                metadata.insert("movement".to_owned(), json!({ "code": movement }));
            }
        }
        repository
    }
}
