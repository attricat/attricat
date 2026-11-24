//! Movements: records whose creation or status change changes a balance on
//! another record, such as goods receipts and issues changing a stock level
//! or an approved production order reserving its material lines' stock.
//!
//! The blueprint compiler checks a movement against its own blueprint's
//! attributes. Publication checks it against the blueprints it names, at
//! their latest published revisions: the target must declare the balance
//! and the unique key that finds it, with key values of matching types, and
//! the factor's blueprint must hold a number.
//!
//! A movement without `apply_on` applies inside the create transaction of
//! its record ([`AttricatRepository::create_record_in_transaction`]), which
//! every create path shares, so the balance and its documents never
//! disagree:
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
//!
//! A movement with `apply_on` applies in the transaction of the status
//! change that triggers it, on its own record
//! ([`AttricatRepository::apply_status_movements_in`]) or, through a trigger
//! relationship, on the lines linked to that record
//! ([`AttricatRepository::apply_line_movements_in`]). Each movement applies
//! once, recorded in `record_movement_applications`, and a reversal
//! ([`AttricatRepository::prepare_reversal_in`]) moves back what applied.

use std::collections::{BTreeSet, HashMap, HashSet};

use attricat_blueprint::{
    BalancePolicy, CompiledBlueprint, EffectiveAttribute, MovementDefinition, MovementTrigger,
};
use attricat_domain::attribute_options::{Cardinality, MovementDirection};
use attricat_domain::value_type::ValueType;
use rust_decimal::Decimal;
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::record_commands::{
    ChosenIdRecordCreate, CreateFileValues, ValueOrigin, lock_record_writes,
};
use super::record_values::{DirectValue, RecordState, RecordValues, load_record};
use super::status::StatusChange;
use super::structural_constraints::{EnforcedUniqueKey, UniqueKeyScope, key_hash};
use super::write_context::WriteContext;
use super::{AttricatRepository, Record, RepositoryError};
use crate::model::{NewAttributeValue, UpdateRecordFormRequest};

/// The latest published revision of a record blueprint, after the selected
/// columns: workspace `$1`, blueprint code `$2`.
macro_rules! latest_published_record_blueprint_sql {
    () => {
        r#" FROM blueprints
               WHERE workspace_id = $1 AND code = $2 AND kind = 'record'
                 AND status = 'published' AND deleted_at IS NULL
               ORDER BY version DESC LIMIT 1"#
    };
}

/// The codes of a record's applied movements: workspace `$1`, record `$2`.
macro_rules! applied_movements_sql {
    () => {
        r#"SELECT movement_code FROM record_movement_applications
               WHERE workspace_id = $1 AND record_id = $2"#
    };
}

/// The live record a unique key finds in a context: workspace `$1`,
/// blueprint `$2`, key `$3`, context `$4` and key hash `$5`.
macro_rules! target_by_key_sql {
    () => {
        r#"SELECT e.id FROM record_unique_key_values k
               JOIN records e ON e.id = k.record_id AND e.workspace_id = k.workspace_id
               WHERE k.workspace_id = $1 AND k.blueprint_id = $2 AND k.key_code = $3
                 AND k.context_id = $4 AND k.key_hash = $5 AND e.deleted_at IS NULL"#
    };
}

/// Audit action of a target's balance change, audited against the target
/// with the movement record in its metadata.
const MOVEMENT_BALANCE_AUDIT_ACTION: &str = "attricat.records.movement.balance_update";
/// Audit action of a target a movement created because none had its key.
const MOVEMENT_TARGET_AUDIT_ACTION: &str = "attricat.records.movement.target_create";
/// Audit action of a line whose movements its order's status applied.
const MOVEMENT_LINE_AUDIT_ACTION: &str = "attricat.records.movement.line_apply";
/// Metadata key naming the movement in its audit entries.
const MOVEMENT_AUDIT_METADATA_KEY: &str = "movement";
/// Lines one status transition applies the movements of.
const MAX_MOVEMENT_LINES: usize = 200;
/// Entries one page of a target's movement history holds.
pub const MAX_MOVEMENT_LEDGER_PAGE: i64 = 500;

/// What publication needs from an attribute of another blueprint.
#[derive(sqlx::FromRow)]
struct ReferencedAttribute {
    code: String,
    value_type: String,
    cardinality: Option<String>,
    target_blueprint_code: Option<String>,
    target_blueprint_codes: Vec<String>,
    balance: Option<serde_json::Value>,
    value_schema: Option<serde_json::Value>,
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
            self.validate_movement_triggers(transaction, compiled, movement)
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
                // Target lists compare as sets: their order is not meaningful.
                || target_attribute.targets().iter().collect::<BTreeSet<_>>()
                    == source.target_blueprints.iter().collect::<BTreeSet<_>>();
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

    /// A trigger through a relationship names a status of the linked
    /// blueprint, and options of that status.
    async fn validate_movement_triggers(
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
        for trigger in &movement.apply_on {
            let Some(relationship) = &trigger.relationship else {
                continue;
            };
            let Some(linked) = compiled
                .attributes
                .iter()
                .find(|attribute| &attribute.code == relationship)
                .and_then(|attribute| attribute.target_blueprints.first())
            else {
                return Err(invalid(format!("unknown relationship '{relationship}'")));
            };
            let schema = if linked == &compiled.code {
                compiled
                    .attributes
                    .iter()
                    .find(|attribute| attribute.code == trigger.attribute)
                    .and_then(|attribute| attribute.value_schema.clone())
            } else {
                self.published_blueprint(transaction, linked)
                    .await?
                    .ok_or_else(|| {
                        invalid(format!("blueprint '{linked}' has no published revision"))
                    })?
                    .attributes
                    .get(&trigger.attribute)
                    .and_then(|attribute| attribute.value_schema.clone())
            };
            let options: Vec<String> = schema
                .as_ref()
                .filter(|schema| {
                    schema
                        .get(attricat_validation::status::STATUS_KEY)
                        .is_some()
                })
                .map(|schema| {
                    attricat_validation::status::status_option_labels(schema)
                        .into_iter()
                        .map(|(code, _)| code.to_owned())
                        .collect()
                })
                .ok_or_else(|| {
                    invalid(format!(
                        "'{}' is not a status attribute of '{linked}'",
                        trigger.attribute
                    ))
                })?;
            for option in std::iter::once(&trigger.to).chain(&trigger.from) {
                if !options.contains(option) {
                    return Err(invalid(format!(
                        "'{option}' is not an option of status '{}' of '{linked}'",
                        trigger.attribute
                    )));
                }
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
            sqlx::query_as::<_, (Uuid, i64, serde_json::Value)>(concat!(
                "SELECT id, version, unique_keys",
                latest_published_record_blueprint_sql!()
            ))
            .bind(self.workspace_id.0)
            .bind(code)
            .fetch_optional(&mut **transaction)
            .await?
        else {
            return Ok(None);
        };
        let unique_keys = serde_json::from_value(unique_keys)
            .map_err(RepositoryError::invalid_blueprint_definition)?;
        let attributes = sqlx::query_as::<_, ReferencedAttribute>(
            r#"SELECT code, value_type, cardinality, target_blueprint_code,
                      target_blueprint_codes, balance, value_schema
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

/// A movement about to apply, with its target found and locked.
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
    fn record_values(&self, default_context_id: Uuid) -> Vec<NewAttributeValue> {
        let mut values = vec![relationship_value(
            &self.target_relationship,
            default_context_id,
            self.target_id,
        )];
        if let Some(result) = &self.result {
            values.push(scalar_value(
                result,
                default_context_id,
                decimal_json(self.amount),
            ));
        }
        values.extend(self.copied.iter().cloned());
        values
    }
}

/// The values core sets for several movements of one record, each attribute
/// once: movements that share a target link write it once.
pub(super) fn movement_record_values(
    movements: &[PreparedMovement],
    default_context_id: Uuid,
) -> Vec<NewAttributeValue> {
    let mut written = HashSet::new();
    movements
        .iter()
        .flat_map(|movement| movement.record_values(default_context_id))
        .filter(|value| {
            let code = match value {
                NewAttributeValue::Scalar { attribute_code, .. }
                | NewAttributeValue::Relationship { attribute_code, .. } => attribute_code.clone(),
            };
            code.is_none_or(|code| written.insert(code))
        })
        .collect()
}

/// A movement's default-context inputs, by attribute code: submitted by a
/// create, or saved on a record whose status applies it.
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

    /// A saved record's default-context values. Numbers keep their exact
    /// decimal text.
    fn saved(record: &RecordValues, default_context_id: Uuid) -> Self {
        let mut submitted = HashMap::new();
        for (code, attribute) in &record.attributes {
            if attribute.record_scoped {
                continue;
            }
            let Some(direct) = attribute.by_context.get(&default_context_id) else {
                continue;
            };
            let value = if attribute.value_type == ValueType::Relationship {
                match linked_ids(direct).first() {
                    Some(target) => Submitted::Relationship(*target),
                    None => continue,
                }
            } else {
                Submitted::Scalar(direct.key_input())
            };
            submitted.insert(code.clone(), value);
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
            Some(Submitted::Scalar(Value::String(text))) => text.parse().ok(),
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
    let decimal = |value: &Value| match value {
        Value::Number(number) => number.to_string().parse::<Decimal>().ok(),
        Value::String(text) => text.parse::<Decimal>().ok(),
        _ => None,
    };
    match (given, original) {
        (Submitted::Relationship(given), Submitted::Relationship(original)) => given == original,
        (Submitted::Scalar(given), Submitted::Scalar(original)) => {
            given == original || (decimal(given).is_some() && decimal(given) == decimal(original))
        }
        _ => false,
    }
}

/// A decimal as the JSON number the value pipeline parses back into it.
fn decimal_json(value: Decimal) -> Value {
    serde_json::from_str(&value.normalize().to_string()).expect("a decimal is a JSON number")
}

/// Whether a status change in the default context is one of a movement's
/// own-status triggers.
fn own_trigger_matches(
    trigger: &MovementTrigger,
    change: &StatusChange,
    default_context_id: Uuid,
) -> bool {
    trigger.relationship.is_none() && transition_matches(trigger, change, default_context_id)
}

fn transition_matches(
    trigger: &MovementTrigger,
    change: &StatusChange,
    default_context_id: Uuid,
) -> bool {
    change.context_id == default_context_id
        && change.attribute_code == trigger.attribute
        && change.after.as_str() == Some(trigger.to.as_str())
        && trigger
            .from
            .as_deref()
            .is_none_or(|from| change.before.as_str() == Some(from))
}

/// A movement application of the record a reversal reverses.
#[derive(sqlx::FromRow)]
struct Application {
    applied_at: chrono::DateTime<chrono::Utc>,
    movement_code: String,
    target_record_id: Uuid,
    balance_code: String,
    amount: Decimal,
    direction: String,
}

/// One row of a target's movement history.
#[derive(sqlx::FromRow)]
struct LedgerRow {
    sequence: i64,
    record_id: Uuid,
    record_blueprint: String,
    movement_code: String,
    balance_code: String,
    amount: Decimal,
    #[sqlx(try_from = "String")]
    direction: MovementDirection,
    applied_at: chrono::DateTime<chrono::Utc>,
}

/// A blueprint revision whose movements a related record's status applies,
/// such as order lines applied by their order's approval.
struct LineBlueprint {
    blueprint_id: Uuid,
    movements: Vec<MovementDefinition>,
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
            Some(movements) => parse_movements(movements),
            None => Ok(Vec::new()),
        }
    }

    /// The revision's attribute codes by ID and value types by code.
    async fn revision_value_types(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
        blueprint_version: i64,
    ) -> Result<(HashMap<Uuid, String>, HashMap<String, ValueType>), RepositoryError> {
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
        let value_types = attributes
            .iter()
            .filter_map(|(_, code, value_type)| {
                ValueType::parse(value_type).map(|value_type| (code.clone(), value_type))
            })
            .collect();
        Ok((codes, value_types))
    }

    /// Resolves the movements a record applies as it is created, from its
    /// submitted `values`: those without status triggers, with their
    /// targets found through their unique keys and locked, or created. A
    /// reversal instead moves back what the record it reverses moved. The
    /// caller holds the relationship lock and has not inserted the record
    /// yet. A sample record's missing target is created as a sample too, so
    /// removing samples removes it.
    pub(super) async fn prepare_movements_in(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
        blueprint_version: i64,
        values: &[NewAttributeValue],
        movements: Vec<MovementDefinition>,
        sample: bool,
    ) -> Result<Vec<PreparedMovement>, RepositoryError> {
        let default_context_id = self.default_movement_context(transaction).await?;
        let (codes, value_types) = self
            .revision_value_types(transaction, blueprint_id, blueprint_version)
            .await?;
        let submitted = SubmittedValues::new(values, &codes, default_context_id);
        if let Some((reversal, original_id)) = movements.iter().find_map(|movement| {
            let reversal = movement.reversal.as_ref()?;
            Some((
                reversal.relationship.clone(),
                submitted.relationship(&reversal.relationship)?,
            ))
        }) {
            return self
                .prepare_reversal_in(
                    transaction,
                    blueprint_id,
                    &movements,
                    &reversal,
                    original_id,
                    &submitted,
                    default_context_id,
                )
                .await;
        }
        let mut prepared = Vec::new();
        for movement in movements
            .iter()
            .filter(|movement| movement.applies_on_create())
        {
            prepared.push(
                self.resolve_movement(
                    transaction,
                    movement,
                    &submitted,
                    &value_types,
                    default_context_id,
                    sample,
                )
                .await?,
            );
        }
        Ok(prepared)
    }

    async fn default_movement_context(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
    ) -> Result<Uuid, RepositoryError> {
        self.resolve_context_id(transaction, None)
            .await?
            .ok_or(RepositoryError::InvalidContext)
    }

    /// The amount a movement moves and its target, locked or created.
    async fn resolve_movement(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        movement: &MovementDefinition,
        inputs: &SubmittedValues,
        value_types: &HashMap<String, ValueType>,
        default_context_id: Uuid,
        sample: bool,
    ) -> Result<PreparedMovement, RepositoryError> {
        let quantity = inputs
            .decimal(&movement.quantity)
            .filter(|quantity| *quantity > Decimal::ZERO)
            .ok_or_else(|| RepositoryError::MovementQuantityInvalid {
                attribute: movement.quantity.clone(),
            })?;
        let factor = match &movement.factor {
            Some(factor) => match inputs.relationship(&factor.relationship) {
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
                movement,
                inputs,
                value_types,
                default_context_id,
                sample,
            )
            .await?;
        Ok(PreparedMovement {
            code: movement.code.clone(),
            direction: movement.direction,
            quantity: movement.quantity.clone(),
            target_id,
            target_relationship: movement.target.relationship.clone(),
            balance: movement.target.balance.clone(),
            result: movement.result.clone(),
            amount,
            copied: Vec::new(),
        })
    }

    /// A reversal of `original_id`: every movement that applied to the
    /// original, moved back the other way in reverse order. The original and
    /// its targets are locked, in ID order, so two reversals of one record
    /// cannot both pass the check that none exists. The reversal's inputs are
    /// the original's: submitted ones must match, and missing ones are
    /// copied.
    #[allow(clippy::too_many_arguments)]
    async fn prepare_reversal_in(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
        movements: &[MovementDefinition],
        reversal: &str,
        original_id: Uuid,
        submitted: &SubmittedValues,
        default_context_id: Uuid,
    ) -> Result<Vec<PreparedMovement>, RepositoryError> {
        let applications = sqlx::query_as::<_, Application>(
            r#"SELECT applied_at, movement_code, target_record_id, balance_code, amount, direction
               FROM record_movement_applications
               WHERE workspace_id = $1 AND record_id = $2"#,
        )
        .bind(self.workspace_id.0)
        .bind(original_id)
        .fetch_all(&mut **transaction)
        .await?;
        // Latest first; one save applied its movements in declaration order.
        let declared = |code: &str| movements.iter().position(|movement| movement.code == code);
        let mut applications = applications;
        applications.sort_by(|first, second| {
            (second.applied_at, declared(&second.movement_code))
                .cmp(&(first.applied_at, declared(&first.movement_code)))
        });
        if applications.is_empty() {
            return Err(RepositoryError::MovementNotApplied {
                record_id: original_id,
            });
        }
        // A set iterates in ID order, the order rows are locked in.
        let rows: BTreeSet<Uuid> = std::iter::once(original_id)
            .chain(
                applications
                    .iter()
                    .map(|application| application.target_record_id),
            )
            .collect();
        for row in rows {
            self.lock_record(transaction, row).await?;
        }
        let original_inputs = self
            .saved_inputs(transaction, original_id, default_context_id)
            .await?;
        if original_inputs.relationship(reversal).is_some() {
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
        .bind(reversal)
        .bind(blueprint_id)
        .fetch_optional(&mut **transaction)
        .await?;
        if let Some(reversed_by) = reversed_by {
            return Err(RepositoryError::MovementAlreadyReversed {
                record_id: original_id,
                reversed_by,
            });
        }
        let mut copied_codes = HashSet::new();
        let mut prepared = Vec::with_capacity(applications.len());
        for application in applications {
            let movement = movements
                .iter()
                .find(|movement| movement.code == application.movement_code)
                .ok_or_else(|| {
                    RepositoryError::InvalidBlueprintDefinition(format!(
                        "movement '{}' that applied to record {original_id} is no longer declared",
                        application.movement_code
                    ))
                })?;
            let direction = MovementDirection::parse(&application.direction)
                .ok_or(RepositoryError::InvalidStoredAttributeValue)?;
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
                let Some(original_value) = original_inputs.get(code) else {
                    if submitted.get(code).is_some() {
                        return Err(RepositoryError::MovementReversalMismatch {
                            attribute: code.to_owned(),
                        });
                    }
                    continue;
                };
                match submitted.get(code) {
                    Some(given) if !same_input(given, original_value) => {
                        return Err(RepositoryError::MovementReversalMismatch {
                            attribute: code.to_owned(),
                        });
                    }
                    Some(_) => {}
                    None if copied_codes.insert(code.to_owned()) => {
                        copied.push(match original_value.clone() {
                            Submitted::Relationship(target_record_id) => {
                                relationship_value(code, default_context_id, target_record_id)
                            }
                            Submitted::Scalar(value) => scalar_value(
                                code,
                                default_context_id,
                                // Exact decimal text goes back as a number.
                                match &value {
                                    Value::String(text) => text
                                        .parse::<Decimal>()
                                        .ok()
                                        .filter(|_| movement.quantity == code)
                                        .map_or(value.clone(), decimal_json),
                                    _ => value,
                                },
                            ),
                        });
                    }
                    None => {}
                }
            }
            prepared.push(PreparedMovement {
                code: movement.code.clone(),
                direction: direction.reversed(),
                quantity: movement.quantity.clone(),
                target_id: application.target_record_id,
                target_relationship: movement.target.relationship.clone(),
                balance: application.balance_code,
                result: movement.result.clone(),
                amount: application.amount,
                copied,
            });
        }
        Ok(prepared)
    }

    /// A record's values before this transaction and as this write leaves
    /// them; a record without values reads as empty.
    async fn before_and_after(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        write: &WriteContext,
        record_id: Uuid,
    ) -> Result<(RecordValues, RecordValues), RepositoryError> {
        let before = load_record(
            transaction,
            self.workspace_id.0,
            record_id,
            RecordState::Before,
        )
        .await?
        .unwrap_or_default();
        let after = write
            .after_record(transaction, self.workspace_id.0, record_id)
            .await?
            .unwrap_or_default();
        Ok((before, after))
    }

    /// The movement inputs `record_id` holds in the default context, as
    /// saved in this transaction.
    async fn saved_inputs(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        record_id: Uuid,
        default_context_id: Uuid,
    ) -> Result<SubmittedValues, RepositoryError> {
        let values = load_record(
            transaction,
            self.workspace_id.0,
            record_id,
            RecordState::After,
        )
        .await?
        .ok_or(RepositoryError::NotFound("record"))?;
        Ok(SubmittedValues::saved(&values, default_context_id))
    }

    /// Codes of the movements that applied to `record_id`, sorted.
    pub async fn applied_movement_codes(
        &self,
        record_id: Uuid,
    ) -> Result<Vec<String>, RepositoryError> {
        Ok(
            sqlx::query_scalar(concat!(applied_movements_sql!(), " ORDER BY movement_code"))
                .bind(self.workspace_id.0)
                .bind(record_id)
                .fetch_all(&self.pool)
                .await?,
        )
    }

    /// The movements that changed `target_id`'s balances after sequence
    /// `after`, in sequence order, at most `limit` (1 to
    /// [`MAX_MOVEMENT_LEDGER_PAGE`]). A missing target is
    /// [`RepositoryError::NotFound`], a bad page [`RepositoryError::InvalidPage`].
    /// Replaying them from the start rebuilds every balance of the target,
    /// which is what derived views such as stock valuation read.
    pub async fn movement_ledger(
        &self,
        target_id: Uuid,
        after: i64,
        limit: i64,
    ) -> Result<attricat_domain::model::MovementLedgerPage, RepositoryError> {
        use attricat_domain::model::{MovementLedgerEntry, MovementLedgerPage};
        if after < 0 || !(1..=MAX_MOVEMENT_LEDGER_PAGE).contains(&limit) {
            return Err(RepositoryError::InvalidPage(format!(
                "limit must be 1-{MAX_MOVEMENT_LEDGER_PAGE} and after must not be negative"
            )));
        }
        if !self.record_exists(target_id).await? {
            return Err(RepositoryError::NotFound("record"));
        }
        let rows: Vec<LedgerRow> = sqlx::query_as(
            r#"SELECT m.sequence, m.record_id, b.code AS record_blueprint, m.movement_code,
                      m.balance_code, m.amount, m.direction, m.applied_at
               FROM record_movement_applications m
               JOIN records r ON r.id = m.record_id AND r.workspace_id = m.workspace_id
               JOIN blueprints b ON b.id = r.blueprint_id AND b.version = r.blueprint_version
                AND b.workspace_id = r.workspace_id
               WHERE m.workspace_id = $1 AND m.target_record_id = $2 AND m.sequence > $3
               ORDER BY m.sequence LIMIT $4"#,
        )
        .bind(self.workspace_id.0)
        .bind(target_id)
        .bind(after)
        .bind(limit + 1)
        .fetch_all(&self.pool)
        .await?;
        // A reader who may read the target but not a movement record sees
        // that entry without the record, so the history stays gapless and
        // still rebuilds the balance.
        let readable = self
            .actor_readable_record_ids(&rows.iter().map(|row| row.record_id).collect::<Vec<_>>())
            .await?;
        let mut items: Vec<MovementLedgerEntry> = rows
            .into_iter()
            .map(|row| {
                let shown = Self::actor_may_read(&readable, &row.record_id);
                MovementLedgerEntry {
                    sequence: row.sequence,
                    record_id: shown.then_some(row.record_id),
                    record_blueprint: shown.then_some(row.record_blueprint),
                    movement_code: shown.then_some(row.movement_code),
                    balance: row.balance_code,
                    amount: row.amount.normalize().to_string(),
                    direction: row.direction,
                    applied_at: row.applied_at,
                }
            })
            .collect();
        let next_after =
            super::record_search::next_page_cursor(&mut items, limit, |entry| entry.sequence);
        Ok(MovementLedgerPage { items, next_after })
    }

    /// The line relationships to `record_id` whose lines' movements its
    /// status applied, such as a production order's approved material lines.
    pub async fn settled_lines(
        &self,
        record_id: Uuid,
    ) -> Result<Vec<attricat_domain::model::SettledLines>, RepositoryError> {
        let rows: Vec<(String, String, Value)> = sqlx::query_as(
            r#"SELECT DISTINCT b.code, a.code, b.movements
               FROM record_movement_applications m
               JOIN records r ON r.id = m.record_id AND r.workspace_id = m.workspace_id
                AND r.deleted_at IS NULL
               JOIN blueprints b ON b.id = r.blueprint_id AND b.version = r.blueprint_version
                AND b.workspace_id = r.workspace_id
               JOIN attribute_values av ON av.record_id = m.record_id
                AND av.workspace_id = m.workspace_id AND av.active
                AND av.relationship_target_record_id = $2
               JOIN attributes a ON a.id = av.attribute_id AND a.workspace_id = av.workspace_id
               WHERE m.workspace_id = $1"#,
        )
        .bind(self.workspace_id.0)
        .bind(record_id)
        .fetch_all(&self.pool)
        .await?;
        let mut settled = BTreeSet::new();
        for (blueprint, relationship, movements) in rows {
            let movements = parse_movements(movements)?;
            // Only the link the movements trigger through settles them.
            if movements.iter().any(|movement| {
                movement
                    .apply_on
                    .iter()
                    .any(|trigger| trigger.relationship.as_deref() == Some(relationship.as_str()))
            }) {
                settled.insert(attricat_domain::model::SettledLines {
                    blueprint,
                    relationship,
                });
            }
        }
        Ok(settled.into_iter().collect())
    }

    /// Codes of the record's movements that applied before this transaction.
    async fn applied_movements_before(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        record_id: Uuid,
    ) -> Result<HashSet<String>, RepositoryError> {
        self.applied_movements_where(
            transaction,
            record_id,
            concat!(applied_movements_sql!(), " AND applied_at < now()"),
        )
        .await
    }

    /// Codes of the record's movements that applied, in any transaction.
    async fn applied_movements(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        record_id: Uuid,
    ) -> Result<HashSet<String>, RepositoryError> {
        self.applied_movements_where(transaction, record_id, applied_movements_sql!())
            .await
    }

    /// Codes the applications `sql`, a filter of `applied_movements_sql!`,
    /// selects for the record.
    async fn applied_movements_where(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        record_id: Uuid,
        sql: &'static str,
    ) -> Result<HashSet<String>, RepositoryError> {
        Ok(sqlx::query_scalar::<_, String>(sql)
            .bind(self.workspace_id.0)
            .bind(record_id)
            .fetch_all(&mut **transaction)
            .await?
            .into_iter()
            .collect())
    }

    /// Applies the movements a save's status transitions trigger: the
    /// record's own, and those of records that link to it through a
    /// relationship trigger, such as the lines of an approved order. Each
    /// movement applies once; one whose `requires` have not applied is
    /// skipped. Called from record validation, so every write path applies
    /// them, in the write's transaction.
    pub(super) async fn apply_status_movements_in(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        write: &WriteContext,
        record: &Record,
        changes: &[StatusChange],
    ) -> Result<(), RepositoryError> {
        if changes.is_empty() {
            return Ok(());
        }
        let default_context_id = write.default_context_id()?;
        let movements = self
            .revision_movements(transaction, record.blueprint_id, record.blueprint_version)
            .await?;
        let triggered: Vec<&MovementDefinition> = movements
            .iter()
            .filter(|movement| {
                movement.apply_on.iter().any(|trigger| {
                    changes
                        .iter()
                        .any(|change| own_trigger_matches(trigger, change, default_context_id))
                })
            })
            .collect();
        if !triggered.is_empty() {
            let inputs = self
                .saved_inputs(transaction, record.id, default_context_id)
                .await?;
            // A reversal applies its movements when it is created; its own
            // status never applies them again.
            let reverses = movements.iter().any(|movement| {
                movement
                    .reversal
                    .as_ref()
                    .is_some_and(|reversal| inputs.relationship(&reversal.relationship).is_some())
            });
            if !reverses {
                let prepared = self
                    .prepare_triggered_in(
                        transaction,
                        record,
                        &triggered,
                        &inputs,
                        default_context_id,
                    )
                    .await?;
                if !prepared.is_empty() {
                    self.insert_values_with(
                        transaction,
                        write,
                        record,
                        movement_record_values(&prepared, default_context_id),
                        ValueOrigin::Movement,
                    )
                    .await?;
                    write.values_changed();
                    self.apply_movements_in(transaction, record, &prepared, default_context_id)
                        .await?;
                }
            }
        }
        self.apply_line_movements_in(transaction, record, changes, default_context_id)
            .await
    }

    /// Resolves triggered movements in declaration order, skipping those that
    /// applied already or whose `requires` have not applied.
    async fn prepare_triggered_in(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        record: &Record,
        triggered: &[&MovementDefinition],
        inputs: &SubmittedValues,
        default_context_id: Uuid,
    ) -> Result<Vec<PreparedMovement>, RepositoryError> {
        let mut applied = self.applied_movements(transaction, record.id).await?;
        let (_, value_types) = self
            .revision_value_types(transaction, record.blueprint_id, record.blueprint_version)
            .await?;
        let mut prepared = Vec::new();
        for movement in triggered {
            if applied.contains(&movement.code)
                || !movement
                    .requires
                    .iter()
                    .all(|required| applied.contains(required))
            {
                continue;
            }
            prepared.push(
                self.resolve_movement(
                    transaction,
                    movement,
                    inputs,
                    &value_types,
                    default_context_id,
                    record.is_sample,
                )
                .await?,
            );
            applied.insert(movement.code.clone());
        }
        Ok(prepared)
    }

    /// Blueprints whose movements a status of `record`'s blueprint applies
    /// through a relationship trigger, at their latest published revisions.
    async fn line_blueprints(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
        blueprint_version: i64,
    ) -> Result<Vec<LineBlueprint>, RepositoryError> {
        let candidates: Vec<(Uuid, i64, Value)> = sqlx::query_as(
            r#"SELECT DISTINCT ON (b.id) b.id, b.version, b.movements
               FROM blueprints b
               WHERE b.workspace_id = $1 AND b.status = 'published' AND b.deleted_at IS NULL
                 AND b.kind = 'record' AND b.movements <> '[]'::jsonb
               ORDER BY b.id, b.version DESC"#,
        )
        .bind(self.workspace_id.0)
        .fetch_all(&mut **transaction)
        .await?;
        let own_code: String = sqlx::query_scalar(
            "SELECT code FROM blueprints WHERE workspace_id = $1 AND id = $2 AND version = $3",
        )
        .bind(self.workspace_id.0)
        .bind(blueprint_id)
        .bind(blueprint_version)
        .fetch_one(&mut **transaction)
        .await?;
        let mut lines = Vec::new();
        for (blueprint_id, version, movements) in candidates {
            let movements = parse_movements(movements)?;
            let relationships = trigger_relationships(&movements);
            if relationships.is_empty() {
                continue;
            }
            // The relationship must link to this record's blueprint.
            let links_here: bool = sqlx::query_scalar(
                r#"SELECT EXISTS (
                       SELECT 1 FROM attributes
                       WHERE workspace_id = $1 AND blueprint_id = $2 AND blueprint_version = $3
                         AND code = ANY($4) AND deleted_at IS NULL
                         AND (target_blueprint_code = $5 OR $5 = ANY(target_blueprint_codes)))"#,
            )
            .bind(self.workspace_id.0)
            .bind(blueprint_id)
            .bind(version)
            .bind(
                relationships
                    .iter()
                    .map(|code| code.to_string())
                    .collect::<Vec<_>>(),
            )
            .bind(&own_code)
            .fetch_one(&mut **transaction)
            .await?;
            if links_here {
                lines.push(LineBlueprint {
                    blueprint_id,
                    movements,
                });
            }
        }
        Ok(lines)
    }

    /// The records of `line` that link to `record_id` through `relationship`
    /// in the default context, in ID order.
    async fn linked_lines(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        line: &LineBlueprint,
        relationship: &str,
        record_id: Uuid,
        default_context_id: Uuid,
    ) -> Result<Vec<Uuid>, RepositoryError> {
        let lines: Vec<Uuid> = sqlx::query_scalar(
            r#"SELECT DISTINCT av.record_id FROM attribute_values av
               JOIN attributes a ON a.id = av.attribute_id AND a.workspace_id = av.workspace_id
               JOIN records r ON r.id = av.record_id AND r.workspace_id = av.workspace_id
                AND r.deleted_at IS NULL
               WHERE av.workspace_id = $1 AND av.relationship_target_record_id = $2
                 AND av.active AND av.context_id = $3 AND a.code = $4 AND r.blueprint_id = $5
               ORDER BY av.record_id
               LIMIT $6"#,
        )
        .bind(self.workspace_id.0)
        .bind(record_id)
        .bind(default_context_id)
        .bind(relationship)
        .bind(line.blueprint_id)
        .bind(MAX_MOVEMENT_LINES as i64 + 1)
        .fetch_all(&mut **transaction)
        .await?;
        if lines.len() > MAX_MOVEMENT_LINES {
            return Err(RepositoryError::MovementLinesExceeded {
                limit: MAX_MOVEMENT_LINES,
            });
        }
        Ok(lines)
    }

    /// Applies the movements of the records linked to `record` whose
    /// relationship triggers its status changes match, such as the material
    /// lines of an approved production order. All of them apply, or the
    /// save fails.
    async fn apply_line_movements_in(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        record: &Record,
        changes: &[StatusChange],
        default_context_id: Uuid,
    ) -> Result<(), RepositoryError> {
        for line in self
            .line_blueprints(transaction, record.blueprint_id, record.blueprint_version)
            .await?
        {
            let mut by_relationship: HashMap<&str, Vec<&MovementDefinition>> = HashMap::new();
            for movement in &line.movements {
                for trigger in &movement.apply_on {
                    if let Some(relationship) = trigger.relationship.as_deref()
                        && changes
                            .iter()
                            .any(|change| transition_matches(trigger, change, default_context_id))
                    {
                        let triggered = by_relationship.entry(relationship).or_default();
                        if !triggered.iter().any(|known| known.code == movement.code) {
                            triggered.push(movement);
                        }
                    }
                }
            }
            for (relationship, triggered) in by_relationship {
                // Declaration order, whatever order the triggers matched in.
                let mut triggered = triggered;
                triggered.sort_by_key(|movement| {
                    line.movements
                        .iter()
                        .position(|declared| declared.code == movement.code)
                });
                for line_id in self
                    .linked_lines(
                        transaction,
                        &line,
                        relationship,
                        record.id,
                        default_context_id,
                    )
                    .await?
                {
                    let line_record = self.lock_record(transaction, line_id).await?;
                    let inputs = self
                        .saved_inputs(transaction, line_id, default_context_id)
                        .await?;
                    let prepared = self
                        .prepare_triggered_in(
                            transaction,
                            &line_record,
                            &triggered,
                            &inputs,
                            default_context_id,
                        )
                        .await?;
                    if prepared.is_empty() {
                        continue;
                    }
                    let (_, changes, event) = self
                        .update_record_from_movement_in(
                            transaction,
                            line_id,
                            UpdateRecordFormRequest {
                                expected_updated_at: None,
                                values: movement_record_values(&prepared, default_context_id),
                                relationships: Vec::new(),
                                remove_values: Vec::new(),
                                system_tags: None,
                                system_metadata: None,
                            },
                        )
                        .await?;
                    self.for_movement_target(
                        MOVEMENT_LINE_AUDIT_ACTION,
                        line_id,
                        &prepared[0].code,
                    )
                    .stage_record_mutation(transaction, changes, event)
                    .await?;
                    self.apply_movements_in(
                        transaction,
                        &line_record,
                        &prepared,
                        default_context_id,
                    )
                    .await?;
                }
            }
        }
        Ok(())
    }

    /// The status attributes of `record_id` that `values` write; only these
    /// can trigger movements.
    pub(super) async fn written_status_codes(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        record_id: Uuid,
        values: &[NewAttributeValue],
    ) -> Result<Vec<String>, RepositoryError> {
        let (ids, codes): (Vec<Uuid>, Vec<String>) = values
            .iter()
            .filter_map(|value| match value {
                NewAttributeValue::Scalar {
                    attribute_id,
                    attribute_code,
                    ..
                } => Some((attribute_id, attribute_code)),
                NewAttributeValue::Relationship { .. } => None,
            })
            .fold(
                (Vec::new(), Vec::new()),
                |(mut ids, mut codes), (id, code)| {
                    ids.extend(*id);
                    codes.extend(code.clone());
                    (ids, codes)
                },
            );
        if ids.is_empty() && codes.is_empty() {
            return Ok(Vec::new());
        }
        Ok(sqlx::query_scalar(
            r#"SELECT DISTINCT a.code FROM attributes a
               JOIN records r ON r.blueprint_id = a.blueprint_id
                AND r.blueprint_version = a.blueprint_version AND r.workspace_id = a.workspace_id
               WHERE r.workspace_id = $1 AND r.id = $2 AND a.deleted_at IS NULL
                 AND (a.id = ANY($3) OR a.code = ANY($4))
                 AND a.value_schema ? $5"#,
        )
        .bind(self.workspace_id.0)
        .bind(record_id)
        .bind(&ids)
        .bind(&codes)
        .bind(attricat_validation::status::STATUS_KEY)
        .fetch_all(&mut **transaction)
        .await?)
    }

    /// Locks, in ID order, the rows a status change of `record_id` may move:
    /// the record, the targets of its status-triggered movements and its
    /// lines with theirs. The update seam locks the record first otherwise,
    /// against the lock order. Only a write of a trigger status attribute
    /// needs it.
    pub(super) async fn pre_lock_movement_rows(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        record_id: Uuid,
        written: &[String],
    ) -> Result<(), RepositoryError> {
        if written.is_empty() {
            return Ok(());
        }
        let Some((blueprint_id, blueprint_version)) = sqlx::query_as::<_, (Uuid, i64)>(
            r#"SELECT blueprint_id, blueprint_version
               FROM records WHERE workspace_id = $1 AND id = $2 AND deleted_at IS NULL"#,
        )
        .bind(self.workspace_id.0)
        .bind(record_id)
        .fetch_optional(&mut **transaction)
        .await?
        else {
            return Ok(());
        };
        let movements = self
            .revision_movements(transaction, blueprint_id, blueprint_version)
            .await?;
        let own: Vec<&MovementDefinition> = movements
            .iter()
            .filter(|movement| {
                movement.apply_on.iter().any(|trigger| {
                    trigger.relationship.is_none() && written.contains(&trigger.attribute)
                })
            })
            .collect();
        let lines: Vec<LineBlueprint> = self
            .line_blueprints(transaction, blueprint_id, blueprint_version)
            .await?
            .into_iter()
            .filter(|line| {
                line.movements.iter().any(|movement| {
                    movement.apply_on.iter().any(|trigger| {
                        trigger.relationship.is_some() && written.contains(&trigger.attribute)
                    })
                })
            })
            .collect();
        if own.is_empty() && lines.is_empty() {
            return Ok(());
        }
        let default_context_id = self.default_movement_context(transaction).await?;
        // Movements write their target links: the relationship lock comes
        // before any record row.
        self.lock_relationship_cardinality_writes(transaction)
            .await?;
        let mut rows = BTreeSet::from([record_id]);
        self.collect_movement_targets(
            transaction,
            (record_id, blueprint_id, blueprint_version),
            &own,
            default_context_id,
            &mut rows,
        )
        .await?;
        for line in &lines {
            let triggered: Vec<&MovementDefinition> = line
                .movements
                .iter()
                .filter(|movement| {
                    movement
                        .apply_on
                        .iter()
                        .any(|trigger| trigger.relationship.is_some())
                })
                .collect();
            let relationships = trigger_relationships(triggered.iter().copied());
            for relationship in relationships {
                for line_id in self
                    .linked_lines(
                        transaction,
                        line,
                        relationship,
                        record_id,
                        default_context_id,
                    )
                    .await?
                {
                    rows.insert(line_id);
                    let Some((line_blueprint, line_version)) = sqlx::query_as::<_, (Uuid, i64)>(
                        "SELECT blueprint_id, blueprint_version FROM records WHERE workspace_id = $1 AND id = $2",
                    )
                    .bind(self.workspace_id.0)
                    .bind(line_id)
                    .fetch_optional(&mut **transaction)
                    .await?
                    else {
                        continue;
                    };
                    self.collect_movement_targets(
                        transaction,
                        (line_id, line_blueprint, line_version),
                        &triggered,
                        default_context_id,
                        &mut rows,
                    )
                    .await?;
                }
            }
        }
        for row in rows {
            self.lock_record(transaction, row).await?;
        }
        Ok(())
    }

    /// Adds the existing targets of `owner`'s `movements`, found from its
    /// saved values, to `rows`.
    async fn collect_movement_targets(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        (owner_id, blueprint_id, blueprint_version): (Uuid, Uuid, i64),
        movements: &[&MovementDefinition],
        default_context_id: Uuid,
        rows: &mut BTreeSet<Uuid>,
    ) -> Result<(), RepositoryError> {
        if movements.is_empty() {
            return Ok(());
        }
        let inputs = self
            .saved_inputs(transaction, owner_id, default_context_id)
            .await?;
        let (_, value_types) = self
            .revision_value_types(transaction, blueprint_id, blueprint_version)
            .await?;
        for movement in movements {
            if let Some(target) = self
                .movement_target_lookup(
                    transaction,
                    movement,
                    &inputs,
                    &value_types,
                    default_context_id,
                )
                .await?
            {
                rows.insert(target);
            }
        }
        Ok(())
    }

    /// Rejects a change to a value a movement read or set when it applied:
    /// it applied once, so the balance would no longer match. A movement
    /// target's key values are kept the same way: movements found the target
    /// by them. Values are compared in the default context, the only one
    /// these values are editable in. A movement that has not applied yet,
    /// such as one waiting for an order's approval, leaves its values free.
    pub(super) async fn ensure_movement_values_unchanged(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        write: &WriteContext,
        record: &Record,
    ) -> Result<(), RepositoryError> {
        if write.new_record {
            return Ok(());
        }
        // Movement flags are only ever set on blueprint attributes, never
        // record-scoped ones.
        let has_movements = write
            .attributes()
            .iter()
            .any(|attribute| attribute.movement_input || attribute.movement_managed);
        let mut guarded: Vec<(String, bool)> = write
            .attributes()
            .iter()
            .filter(|attribute| {
                attribute.movement_key && !attribute.movement_input && !attribute.movement_managed
            })
            .map(|attribute| (attribute.code.clone(), true))
            .collect();
        if has_movements {
            let applied = self
                .applied_movements_before(transaction, record.id)
                .await?;
            if !applied.is_empty() {
                let movements = self
                    .revision_movements(transaction, record.blueprint_id, record.blueprint_version)
                    .await?;
                let codes: BTreeSet<String> = movements
                    .iter()
                    .filter(|movement| applied.contains(&movement.code))
                    .flat_map(|movement| {
                        movement
                            .input_attributes()
                            .chain(movement.managed_attributes())
                            .map(str::to_owned)
                            .collect::<Vec<_>>()
                    })
                    .collect();
                guarded.extend(codes.into_iter().map(|code| (code, false)));
            }
        }
        if guarded.is_empty() {
            return Ok(());
        }
        let default_context_id = write.default_context_id()?;
        let (before, after) = self.before_and_after(transaction, write, record.id).await?;
        let input = |values: &RecordValues, code: &str| {
            default_value(values, code, default_context_id).map(DirectValue::key_input)
        };
        // A target key may be set when it has no value yet, as a target
        // created earlier in this transaction is, but never changed.
        if let Some((code, target_key)) = guarded.into_iter().find(|(code, target_key)| {
            let previous = input(&before, code);
            (previous.is_some() || !target_key) && previous != input(&after, code)
        }) {
            return Err(if target_key {
                RepositoryError::MovementTargetKeyImmutable { attribute: code }
            } else {
                RepositoryError::MovementImmutable { attribute: code }
            });
        }
        Ok(())
    }

    /// The lines of a record whose status applied their movements are
    /// settled: none can be added, moved to it or away from it. A line's own
    /// applied values are guarded by
    /// [`Self::ensure_movement_values_unchanged`].
    pub(super) async fn ensure_movement_lines_open(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        write: &WriteContext,
        record: &Record,
    ) -> Result<(), RepositoryError> {
        if !write
            .attributes()
            .iter()
            .any(|attribute| attribute.movement_input)
        {
            return Ok(());
        }
        let movements = self
            .revision_movements(transaction, record.blueprint_id, record.blueprint_version)
            .await?;
        let relationships = trigger_relationships(&movements);
        if relationships.is_empty() {
            return Ok(());
        }
        let default_context_id = write.default_context_id()?;
        let (before, after) = self.before_and_after(transaction, write, record.id).await?;
        for relationship in relationships {
            let linked = |values: &RecordValues| {
                default_value(values, relationship, default_context_id)
                    .and_then(|value| linked_ids(value).first().copied())
            };
            let (previous, current) = (linked(&before), linked(&after));
            if previous == current && !write.new_record {
                continue;
            }
            for order_id in [previous, current].into_iter().flatten() {
                self.ensure_lines_open_for(
                    transaction,
                    record.blueprint_id,
                    relationship,
                    order_id,
                )
                .await?;
            }
        }
        Ok(())
    }

    /// Refuses a change to the lines of `order_id` of one blueprint once
    /// its status applied any of their movements before this transaction.
    /// Lines of another blueprint settle separately, so an approved order's
    /// output lines stay open until its completion applies them.
    async fn ensure_lines_open_for(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        line_blueprint_id: Uuid,
        relationship: &str,
        order_id: Uuid,
    ) -> Result<(), RepositoryError> {
        let settled: bool = sqlx::query_scalar(
            r#"SELECT EXISTS (
                   SELECT 1 FROM record_movement_applications m
                   JOIN attribute_values av ON av.record_id = m.record_id
                    AND av.workspace_id = m.workspace_id AND av.active
                    AND av.relationship_target_record_id = $2
                   JOIN attributes a ON a.id = av.attribute_id AND a.workspace_id = av.workspace_id
                    AND a.code = $3
                   JOIN records line ON line.id = m.record_id AND line.workspace_id = m.workspace_id
                    AND line.blueprint_id = $4 AND line.deleted_at IS NULL
                   WHERE m.workspace_id = $1 AND m.applied_at < now())"#,
        )
        .bind(self.workspace_id.0)
        .bind(order_id)
        .bind(relationship)
        .bind(line_blueprint_id)
        .fetch_one(&mut **transaction)
        .await?;
        if settled {
            return Err(RepositoryError::MovementLinesLocked { order_id });
        }
        Ok(())
    }

    /// A record a movement applied to is reversed, never deleted; a line of
    /// a settled order stays with it; and a record that still holds a
    /// balance keeps it. Solution-pack samples are removed together, so they
    /// are exempt.
    pub(super) async fn ensure_movement_deletable(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        record: &Record,
    ) -> Result<(), RepositoryError> {
        if record.is_sample {
            return Ok(());
        }
        if !self
            .applied_movements(transaction, record.id)
            .await?
            .is_empty()
        {
            return Err(RepositoryError::MovementRecordUndeletable);
        }
        let movements = self
            .revision_movements(transaction, record.blueprint_id, record.blueprint_version)
            .await?;
        let relationships = trigger_relationships(&movements);
        if !relationships.is_empty() {
            let default_context_id = self.default_movement_context(transaction).await?;
            let inputs = self
                .saved_inputs(transaction, record.id, default_context_id)
                .await?;
            for relationship in relationships {
                if let Some(order_id) = inputs.relationship(relationship) {
                    self.ensure_lines_open_for(
                        transaction,
                        record.blueprint_id,
                        relationship,
                        order_id,
                    )
                    .await?;
                }
            }
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
        let default_context_id = self.default_movement_context(transaction).await?;
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

    /// The target blueprint, unique key and normalized key hash a movement's
    /// inputs find its target by, and the values a missing target is
    /// created with.
    async fn movement_target_key(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        movement: &MovementDefinition,
        inputs: &SubmittedValues,
        value_types: &HashMap<String, ValueType>,
        default_context_id: Uuid,
    ) -> Result<TargetKey, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let blueprint = &movement.target.blueprint;
        let missing = || RepositoryError::MovementTargetMissing {
            blueprint: blueprint.clone(),
            key: movement.target.key.clone(),
        };
        let (blueprint_id, version): (Uuid, i64) = sqlx::query_as(concat!(
            "SELECT id, version",
            latest_published_record_blueprint_sql!()
        ))
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
            blueprint_id,
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
            let (input, create_value) = match inputs.get(source).ok_or_else(missing)? {
                Submitted::Relationship(target) => (
                    Value::String(target.to_string()),
                    relationship_value(key_attribute, default_context_id, *target),
                ),
                Submitted::Scalar(value) => (
                    value.clone(),
                    scalar_value(key_attribute, default_context_id, value.clone()),
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
        Ok(TargetKey {
            blueprint_id,
            version,
            key_code: key.code,
            hash: key_hash(&Value::Array(components)),
            create_values,
        })
    }

    /// The existing target of a movement, without locking it.
    async fn movement_target_lookup(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        movement: &MovementDefinition,
        inputs: &SubmittedValues,
        value_types: &HashMap<String, ValueType>,
        default_context_id: Uuid,
    ) -> Result<Option<Uuid>, RepositoryError> {
        let Ok(key) = self
            .movement_target_key(
                transaction,
                movement,
                inputs,
                value_types,
                default_context_id,
            )
            .await
        else {
            // Missing inputs fail later, when the movement applies.
            return Ok(None);
        };
        self.target_by_key(transaction, &key, default_context_id, false)
            .await
    }

    /// The live record `key` finds in the default context, locked for
    /// update when `lock` is set.
    async fn target_by_key(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        key: &TargetKey,
        default_context_id: Uuid,
        lock: bool,
    ) -> Result<Option<Uuid>, RepositoryError> {
        let sql = if lock {
            concat!(target_by_key_sql!(), " FOR UPDATE OF e")
        } else {
            target_by_key_sql!()
        };
        Ok(sqlx::query_scalar(sql)
            .bind(self.workspace_id.0)
            .bind(key.blueprint_id)
            .bind(&key.key_code)
            .bind(default_context_id)
            .bind(&key.hash)
            .fetch_optional(&mut **transaction)
            .await?)
    }

    /// Finds the movement's target by its unique key and locks it, or
    /// creates it with the key values when the movement allows. Concurrent
    /// movements to one missing target serialize on the key, so only the
    /// first creates it.
    async fn movement_target_in(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        movement: &MovementDefinition,
        inputs: &SubmittedValues,
        value_types: &HashMap<String, ValueType>,
        default_context_id: Uuid,
        sample: bool,
    ) -> Result<Uuid, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let key = self
            .movement_target_key(
                transaction,
                movement,
                inputs,
                value_types,
                default_context_id,
            )
            .await?;
        // The record-writer lock comes before any record row lock.
        lock_record_writes(transaction, workspace_id, false).await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!(
                "movement-target-key:{workspace_id}:{}:{}:{}",
                key.blueprint_id, key.key_code, key.hash
            ))
            .execute(&mut **transaction)
            .await?;
        let found = self
            .target_by_key(transaction, &key, default_context_id, true)
            .await?;
        if let Some(target_id) = found {
            return Ok(target_id);
        }
        if !movement.target.create_missing {
            return Err(RepositoryError::MovementTargetMissing {
                blueprint: movement.target.blueprint.clone(),
                key: movement.target.key.clone(),
            });
        }
        // A target has no movements of its own to resolve first; boxing
        // breaks the async recursion through the create seam.
        let (target, changes, event) = Box::pin(self.create_record_in_transaction(
            transaction,
            ChosenIdRecordCreate {
                record_id: Uuid::new_v4(),
                blueprint_id: key.blueprint_id,
                blueprint_version: key.version,
                values: key.create_values,
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

    /// Changes each target's balance by its movement's amount, records the
    /// application, and stages the targets' updates.
    pub(super) async fn apply_movements_in(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        record: &Record,
        movements: &[PreparedMovement],
        default_context_id: Uuid,
    ) -> Result<(), RepositoryError> {
        for movement in movements {
            self.apply_balance_in(transaction, record, movement, default_context_id)
                .await?;
            // The balance change above holds the target's row lock until
            // commit, so the target's next sequence is the order its
            // balances changed in.
            sqlx::query(
                r#"INSERT INTO record_movement_applications
                       (workspace_id, record_id, movement_code, target_record_id, balance_code,
                        amount, direction, sequence)
                   VALUES ($1, $2, $3, $4, $5, $6, $7,
                           (SELECT COALESCE(MAX(sequence), 0) + 1 FROM record_movement_applications
                            WHERE workspace_id = $1 AND target_record_id = $4))"#,
            )
            .bind(self.workspace_id.0)
            .bind(record.id)
            .bind(&movement.code)
            .bind(movement.target_id)
            .bind(&movement.balance)
            .bind(movement.amount)
            .bind(movement.direction.as_str())
            .execute(&mut **transaction)
            .await?;
        }
        Ok(())
    }

    /// One movement's balance change on its locked target. The balance's
    /// `min` applies to it minus its `reserved` balance, and a change to a
    /// reserved balance keeps the balance holding it at its `min`.
    async fn apply_balance_in(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        record: &Record,
        movement: &PreparedMovement,
        default_context_id: Uuid,
    ) -> Result<(), RepositoryError> {
        let balances: Vec<(String, String, sqlx::types::Json<BalancePolicy>)> = sqlx::query_as(
            r#"SELECT a.code, a.value_type, a.balance FROM attributes a
               JOIN records r ON r.blueprint_id = a.blueprint_id
                AND r.blueprint_version = a.blueprint_version AND r.workspace_id = a.workspace_id
               WHERE r.workspace_id = $1 AND r.id = $2 AND a.balance IS NOT NULL
                 AND a.record_id IS NULL AND a.deleted_at IS NULL"#,
        )
        .bind(self.workspace_id.0)
        .bind(movement.target_id)
        .fetch_all(&mut **transaction)
        .await?;
        let policy_of = |code: &str| {
            balances
                .iter()
                .find(|(balance, _, _)| balance == code)
                .map(|(_, value_type, policy)| (value_type.as_str(), &policy.0))
        };
        let (value_type, policy) = policy_of(&movement.balance).ok_or_else(|| {
            RepositoryError::InvalidBlueprintDefinition(format!(
                "movement '{}' changes '{}', which is not a balance of its target's revision",
                movement.code, movement.balance
            ))
        })?;
        let current = self
            .default_context_number(
                transaction,
                movement.target_id,
                &movement.balance,
                default_context_id,
            )
            .await?
            .unwrap_or(Decimal::ZERO);
        let balance = match movement.direction {
            MovementDirection::In => current.checked_add(movement.amount),
            MovementDirection::Out => current.checked_sub(movement.amount),
        }
        .ok_or_else(|| RepositoryError::MovementQuantityInvalid {
            attribute: movement.quantity.clone(),
        })?;
        if ValueType::parse(value_type) == Some(ValueType::Integer) && !balance.fract().is_zero() {
            return Err(RepositoryError::MovementQuantityInvalid {
                attribute: movement.quantity.clone(),
            });
        }
        // This balance, less what its reserved balance holds.
        if let Some(minimum) = policy.min {
            let held = match &policy.reserved {
                Some(reserved) => Some(
                    self.default_context_number(
                        transaction,
                        movement.target_id,
                        reserved,
                        default_context_id,
                    )
                    .await?
                    .unwrap_or(Decimal::ZERO),
                ),
                None => None,
            };
            if balance - held.unwrap_or(Decimal::ZERO) < minimum {
                return Err(RepositoryError::BalanceBelowMinimum(Box::new(
                    super::error::BalanceShortage {
                        attribute: movement.balance.clone(),
                        record_id: movement.target_id,
                        available: current - held.unwrap_or(Decimal::ZERO),
                        requested: movement.amount,
                        minimum,
                        reserved: held,
                    },
                )));
            }
        }
        // A balance that holds part of another, such as reserved stock,
        // cannot hold more than the other allows.
        for (holder, _, holder_policy) in &balances {
            if holder_policy.0.reserved.as_deref() != Some(movement.balance.as_str()) {
                continue;
            }
            let Some(minimum) = holder_policy.0.min else {
                continue;
            };
            let total = self
                .default_context_number(transaction, movement.target_id, holder, default_context_id)
                .await?
                .unwrap_or(Decimal::ZERO);
            if total - balance < minimum {
                return Err(RepositoryError::BalanceBelowMinimum(Box::new(
                    super::error::BalanceShortage {
                        attribute: holder.clone(),
                        record_id: movement.target_id,
                        available: total - current,
                        requested: movement.amount,
                        minimum,
                        reserved: Some(current),
                    },
                )));
            }
        }
        let (_, changes, event) = self
            .update_record_from_movement_in(
                transaction,
                movement.target_id,
                UpdateRecordFormRequest {
                    expected_updated_at: None,
                    values: vec![scalar_value(
                        &movement.balance,
                        default_context_id,
                        decimal_json(balance),
                    )],
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
                MOVEMENT_AUDIT_METADATA_KEY.to_owned(),
                json!({
                    "code": movement.code,
                    "record_id": record.id,
                    "direction": movement.direction.as_str(),
                    "balance": movement.balance,
                    "amount": movement.amount.normalize().to_string(),
                }),
            );
        }
        repository
            .stage_record_mutation(transaction, changes, event)
            .await
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
                metadata.insert(
                    MOVEMENT_AUDIT_METADATA_KEY.to_owned(),
                    json!({ "code": movement }),
                );
            }
        }
        repository
    }
}

/// What a movement finds its target by.
struct TargetKey {
    blueprint_id: Uuid,
    version: i64,
    key_code: String,
    hash: String,
    create_values: Vec<NewAttributeValue>,
}

/// Decodes a revision's stored movements.
fn parse_movements(movements: Value) -> Result<Vec<MovementDefinition>, RepositoryError> {
    serde_json::from_value(movements).map_err(RepositoryError::invalid_blueprint_definition)
}

/// The relationships whose records' statuses trigger `movements`.
fn trigger_relationships<'a>(
    movements: impl IntoIterator<Item = &'a MovementDefinition>,
) -> BTreeSet<&'a str> {
    movements
        .into_iter()
        .flat_map(|movement| &movement.apply_on)
        .filter_map(|trigger| trigger.relationship.as_deref())
        .collect()
}

/// The value `values` hold for attribute `code` in the default context.
fn default_value<'a>(
    values: &'a RecordValues,
    code: &str,
    default_context_id: Uuid,
) -> Option<&'a DirectValue> {
    values
        .attributes
        .get(code)
        .and_then(|attribute| attribute.by_context.get(&default_context_id))
}

/// A scalar value core writes for attribute `code` in the default context.
fn scalar_value(code: &str, default_context_id: Uuid, value: Value) -> NewAttributeValue {
    NewAttributeValue::Scalar {
        attribute_id: None,
        attribute_code: Some(code.to_owned()),
        context_id: Some(default_context_id),
        value,
    }
}

/// A link core writes for relationship `code` in the default context.
fn relationship_value(
    code: &str,
    default_context_id: Uuid,
    target_record_id: Uuid,
) -> NewAttributeValue {
    NewAttributeValue::Relationship {
        attribute_id: None,
        attribute_code: Some(code.to_owned()),
        context_id: Some(default_context_id),
        target_record_id,
    }
}
