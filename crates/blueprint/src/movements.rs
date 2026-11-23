//! Balances and the movement records that change them, such as a stock
//! level and the goods receipts and issues that add to and subtract from it.
//!
//! A `balance` attribute holds a number only movements write. A record
//! blueprint's `movements` say how creating one of its records changes the
//! balance of another record, found through one of that record's unique keys.
//! The repository applies a movement inside the create transaction of the
//! movement record, so the balance and its documents never disagree.

use std::collections::{BTreeMap, HashMap, HashSet};

use attricat_domain::attribute_options::{Cardinality, ContextEditable, MovementDirection};
use attricat_domain::value_type::ValueType;
use rust_decimal::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{BlueprintError, BlueprintKind, EffectiveAttribute, MAX_UNIQUE_KEY_ATTRIBUTES};

/// Movements a record blueprint may declare.
pub const MAX_MOVEMENTS: usize = 4;

/// A compiled `balance: { … }` declaration, stored as JSON on the attribute
/// row. Only movements write the attribute; it starts at zero.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct BalancePolicy {
    /// The lowest value a movement may leave. No limit when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<Decimal>,
}

impl BalancePolicy {
    /// Validates a declaration for an attribute of `value_type`; the error
    /// says what is wrong in words for the blueprint author.
    pub fn new(value_type: ValueType, min: Option<&serde_json::Number>) -> Result<Self, String> {
        if !matches!(value_type, ValueType::Number | ValueType::Integer) {
            return Err("only number and integer attributes can be balances".into());
        }
        let min = min
            .map(|min| {
                min.to_string()
                    .parse::<Decimal>()
                    .map_err(|_| format!("min '{min}' is not a decimal number"))
            })
            .transpose()?;
        if value_type == ValueType::Integer && min.is_some_and(|min| !min.fract().is_zero()) {
            return Err("an integer balance needs a whole-number min".into());
        }
        Ok(Self { min })
    }
}

/// How creating a record of this blueprint changes a balance on another
/// record.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MovementDefinition {
    /// Stable movement identifier, unique within the blueprint.
    #[schemars(regex(pattern = attricat_validation::CODE_PATTERN))]
    pub code: String,
    /// `in` adds the moved amount to the target balance; `out` subtracts it.
    #[schemars(with = "String", extend("enum" = MovementDirection::codes()))]
    pub direction: MovementDirection,
    /// Number or integer attribute of this record holding the quantity. It
    /// must be greater than zero.
    #[schemars(extend("x-attricat-reference" = "attribute"))]
    pub quantity: String,
    /// Multiplies the quantity by a number on a linked record, such as a unit
    /// conversion. Without a link the factor is 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub factor: Option<MovementFactor>,
    /// Number attribute of this record in which core stores the moved
    /// amount, in the balance's unit. Clients cannot write it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-attricat-reference" = "attribute"))]
    pub result: Option<String>,
    /// The record whose balance changes.
    pub target: MovementTarget,
    /// A relationship to another record of this blueprint that this record
    /// reverses. A reversal moves the original's amount the other way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reversal: Option<MovementReversal>,
}

/// `factor: { … }` of a movement.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MovementFactor {
    /// Single-target relationship of this record to the record holding the
    /// factor.
    #[schemars(extend("x-attricat-reference" = "relationship_attribute"))]
    pub relationship: String,
    /// Number or integer attribute of the linked record. It must be greater
    /// than zero.
    #[schemars(regex(pattern = attricat_validation::CODE_PATTERN))]
    pub attribute: String,
}

/// `target: { … }` of a movement.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MovementTarget {
    /// Single-target relationship of this record in which core stores the
    /// target. Clients cannot write it.
    #[schemars(extend("x-attricat-reference" = "relationship_attribute"))]
    pub relationship: String,
    /// Record blueprint of the target.
    #[schemars(
        regex(pattern = attricat_validation::CODE_PATTERN),
        extend("x-attricat-reference" = "blueprint")
    )]
    pub blueprint: String,
    /// Balance attribute of the target blueprint that the movement changes.
    #[schemars(regex(pattern = attricat_validation::CODE_PATTERN))]
    pub balance: String,
    /// Unique key of the target blueprint that finds the target.
    #[schemars(regex(pattern = attricat_validation::CODE_PATTERN))]
    pub key: String,
    /// Every attribute of the key, mapped to the attribute of this record
    /// that holds its value.
    #[schemars(length(min = 1, max = MAX_UNIQUE_KEY_ATTRIBUTES))]
    pub values: BTreeMap<String, String>,
    /// Create the target, with a zero balance, when no record has the key
    /// values. Otherwise the movement fails.
    #[serde(default)]
    pub create_missing: bool,
}

/// `reversal: { … }` of a movement.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MovementReversal {
    /// Single-target relationship of this record to the record of this
    /// blueprint it reverses.
    #[schemars(extend("x-attricat-reference" = "relationship_attribute"))]
    pub relationship: String,
}

/// `target_scope: { … }` on a relationship attribute: the record picker
/// offers only targets whose `attribute` links to the same record as this
/// record's `equals`. Record checks, not the scope, guard saved values.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TargetScope {
    /// Single-target relationship attribute of the target blueprint.
    #[schemars(regex(pattern = attricat_validation::CODE_PATTERN))]
    pub attribute: String,
    /// Single-target relationship attribute of this record.
    #[schemars(extend("x-attricat-reference" = "relationship_attribute"))]
    pub equals: String,
}

impl MovementDefinition {
    /// Attributes core writes on the movement record.
    pub fn managed_attributes(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.target.relationship.as_str()).chain(self.result.as_deref())
    }

    /// Attributes of the movement record the movement reads. A movement
    /// applies once, so changing them later would leave the balance wrong.
    pub fn input_attributes(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.quantity.as_str())
            .chain(
                self.factor
                    .iter()
                    .map(|factor| factor.relationship.as_str()),
            )
            .chain(self.target.values.values().map(String::as_str))
            .chain(
                self.reversal
                    .iter()
                    .map(|reversal| reversal.relationship.as_str()),
            )
    }
}

/// Structural checks that need only the declaration: codes, limits and that
/// every attribute it names is declared by the blueprint.
pub(crate) fn validate_movement_declarations(
    movements: &[MovementDefinition],
    kind: &BlueprintKind,
    attribute_codes: &HashSet<String>,
) -> Result<(), BlueprintError> {
    if movements.is_empty() {
        return Ok(());
    }
    if *kind != BlueprintKind::Record {
        return Err(BlueprintError::InvalidMovement {
            movement: movements[0].code.clone(),
            message: "only record blueprints can declare movements".into(),
        });
    }
    if movements.len() > MAX_MOVEMENTS {
        return Err(BlueprintError::InvalidMovement {
            movement: movements[MAX_MOVEMENTS].code.clone(),
            message: format!("a blueprint declares at most {MAX_MOVEMENTS} movements"),
        });
    }
    let mut movement_codes = HashSet::new();
    let mut managed = HashSet::new();
    for movement in movements {
        let invalid = |message: String| BlueprintError::InvalidMovement {
            movement: movement.code.clone(),
            message,
        };
        crate::parser::validate_code(&movement.code, "movement code")?;
        if !movement_codes.insert(movement.code.as_str()) {
            return Err(invalid("the movement code is duplicated".into()));
        }
        crate::parser::validate_code(&movement.target.blueprint, "movement target blueprint")?;
        crate::parser::validate_code(&movement.target.balance, "movement target balance")?;
        crate::parser::validate_code(&movement.target.key, "movement target key")?;
        if let Some(factor) = &movement.factor {
            crate::parser::validate_code(&factor.attribute, "movement factor attribute")?;
        }
        if movement.target.values.is_empty()
            || movement.target.values.len() > MAX_UNIQUE_KEY_ATTRIBUTES
        {
            return Err(invalid(format!(
                "target values map 1 to {MAX_UNIQUE_KEY_ATTRIBUTES} key attributes"
            )));
        }
        for key_attribute in movement.target.values.keys() {
            crate::parser::validate_code(key_attribute, "movement target key attribute")?;
        }
        for attribute in movement
            .managed_attributes()
            .chain(movement.input_attributes())
        {
            if !attribute_codes.contains(attribute) {
                return Err(invalid(format!("unknown attribute '{attribute}'")));
            }
        }
        for attribute in movement.managed_attributes() {
            // One writer per managed attribute, and never an input: a value
            // core writes cannot also be what the movement reads.
            if !managed.insert(attribute.to_owned())
                || movement.input_attributes().any(|input| input == attribute)
            {
                return Err(invalid(format!(
                    "attribute '{attribute}' is written by core and cannot be used twice"
                )));
            }
        }
    }
    for movement in movements {
        if let Some(input) = movement
            .input_attributes()
            .find(|input| managed.contains(*input))
        {
            return Err(BlueprintError::InvalidMovement {
                movement: movement.code.clone(),
                message: format!("attribute '{input}' is written by another movement"),
            });
        }
    }
    Ok(())
}

/// Type checks against the effective attributes. The target and factor
/// blueprints are checked when the revision is published, against their
/// latest published revisions.
pub(crate) fn validate_movement_attributes(
    blueprint_code: &str,
    movements: &[MovementDefinition],
    attributes: &[EffectiveAttribute],
) -> Result<(), BlueprintError> {
    let by_code: HashMap<&str, &EffectiveAttribute> = attributes
        .iter()
        .map(|attribute| (attribute.code.as_str(), attribute))
        .collect();
    for movement in movements {
        let invalid = |message: String| BlueprintError::InvalidMovement {
            movement: movement.code.clone(),
            message,
        };
        let attribute = |code: &str| {
            by_code
                .get(code)
                .copied()
                .ok_or_else(|| invalid(format!("unknown attribute '{code}'")))
        };
        let single_relationship = |code: &str, role: &str| {
            let attribute = attribute(code)?;
            if attribute.value_type != ValueType::Relationship
                || attribute.cardinality != Some(Cardinality::One)
            {
                return Err(invalid(format!(
                    "{role} '{code}' must be a relationship with cardinality: one"
                )));
            }
            Ok(attribute)
        };
        // A movement applies once, from the values the record is created
        // with, so it reads them where every context sees them.
        let default_context_only = |attribute: &EffectiveAttribute| {
            if attribute.context_editable != ContextEditable::Default {
                return Err(invalid(format!(
                    "attribute '{}' is read by the movement; set context_editable: default",
                    attribute.code
                )));
            }
            Ok(())
        };

        let quantity = attribute(&movement.quantity)?;
        if !matches!(quantity.value_type, ValueType::Number | ValueType::Integer) {
            return Err(invalid(format!(
                "quantity '{}' must be a number or integer attribute",
                movement.quantity
            )));
        }
        default_context_only(quantity)?;
        if let Some(factor) = &movement.factor {
            let relationship = single_relationship(&factor.relationship, "factor relationship")?;
            if relationship.target_blueprints.len() != 1 {
                return Err(invalid(format!(
                    "factor relationship '{}' must target exactly one blueprint",
                    factor.relationship
                )));
            }
            default_context_only(relationship)?;
        }
        if let Some(result) = &movement.result {
            let result = attribute(result)?;
            // An integer can hold the amount only when nothing can make it
            // fractional.
            let fits = result.value_type == ValueType::Number
                || (result.value_type == ValueType::Integer
                    && quantity.value_type == ValueType::Integer
                    && movement.factor.is_none());
            if !fits || result.generated.is_some() || result.balance.is_some() {
                return Err(invalid(format!(
                    "result '{}' must be a plain number attribute (integer only for an integer quantity without a factor)",
                    result.code
                )));
            }
        }
        let target = single_relationship(&movement.target.relationship, "target relationship")?;
        if target.target_blueprints != [movement.target.blueprint.clone()] {
            return Err(invalid(format!(
                "target relationship '{}' must target exactly blueprint '{}'",
                movement.target.relationship, movement.target.blueprint
            )));
        }
        for source in movement.target.values.values() {
            let source = attribute(source)?;
            if matches!(source.value_type, ValueType::Json | ValueType::File)
                || (source.value_type == ValueType::Relationship
                    && source.cardinality != Some(Cardinality::One))
            {
                return Err(invalid(format!(
                    "key value '{}' must be a scalar or a relationship with cardinality: one",
                    source.code
                )));
            }
            default_context_only(source)?;
        }
        if let Some(reversal) = &movement.reversal {
            // A reversal moves back what the original moved, which it reads
            // from the original's result.
            if movement.result.is_none() {
                return Err(invalid(
                    "a movement with a reversal must declare result".into(),
                ));
            }
            let relationship =
                single_relationship(&reversal.relationship, "reversal relationship")?;
            if relationship.target_blueprints != [blueprint_code.to_owned()] {
                return Err(invalid(format!(
                    "reversal relationship '{}' must target exactly this blueprint '{blueprint_code}'",
                    reversal.relationship
                )));
            }
            default_context_only(relationship)?;
        }
    }
    Ok(())
}

/// Core writes the managed attributes of a movement record in the default
/// context only, so the UI shows them read-only.
pub(crate) fn protect_managed_attributes(
    movements: &[MovementDefinition],
    attributes: &mut [EffectiveAttribute],
) {
    let managed: HashSet<&str> = movements
        .iter()
        .flat_map(MovementDefinition::managed_attributes)
        .collect();
    for attribute in attributes
        .iter_mut()
        .filter(|attribute| managed.contains(attribute.code.as_str()))
    {
        attribute.readonly = true;
        attribute.context_editable = ContextEditable::Default;
    }
}

/// `target_scope` compares single-target relationships: the picker filters
/// targets by the record this record's `equals` links to.
pub(crate) fn validate_target_scopes(
    attributes: &[EffectiveAttribute],
) -> Result<(), BlueprintError> {
    for attribute in attributes {
        let Some(scope) = &attribute.target_scope else {
            continue;
        };
        let invalid = |message: String| BlueprintError::InvalidTargetScope {
            code: attribute.code.clone(),
            message,
        };
        if attribute.value_type != ValueType::Relationship || attribute.target_blueprints.len() != 1
        {
            return Err(invalid(
                "only a relationship with exactly one target blueprint can be scoped".into(),
            ));
        }
        let equals = attributes
            .iter()
            .find(|candidate| candidate.code == scope.equals)
            .ok_or_else(|| invalid(format!("unknown attribute '{}'", scope.equals)))?;
        if equals.code == attribute.code
            || equals.value_type != ValueType::Relationship
            || equals.cardinality != Some(Cardinality::One)
        {
            return Err(invalid(format!(
                "equals '{}' must be another relationship with cardinality: one",
                scope.equals
            )));
        }
    }
    Ok(())
}
