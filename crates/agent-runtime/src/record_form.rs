//! A record's form as an agent sees it: the editable scalar fields in one
//! context, their saved values and the user's unsaved draft, and how a
//! proposal of new values for them is checked. Proposals are draft-only; the
//! user applies them to the form and saves separately.
use std::collections::{BTreeMap, HashSet};

use attricat_domain::attribute_options::ContextEditable;
use attricat_domain::constants::DEFAULT_CONTEXT_CODE;
use attricat_domain::value_type::ValueType;
use attricat_repository::attricat_read_service::AttricatReadService;
use attricat_validation::validate_json_schema;
use chrono::{DateTime, NaiveDate, NaiveTime};
use serde::Serialize;
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    agent_tools::{ToolDefinition, ToolFunction},
    agents::MAX_CONVERSATION_MESSAGE_BYTES,
    model::FormAttributeValue,
    repository::{AttricatRepository, RepositoryError},
};

/// The tool that proposes values for a record form.
pub const PROPOSE_TOOL: &str = "propose_record_form_values";
/// Most unsaved field values a request may carry.
pub const MAX_DRAFT_VALUES: usize = 256;
/// Longest explanation kept with a proposal.
pub const MAX_EXPLANATION_CHARS: usize = 2048;
/// Largest integer a browser number field represents exactly (2^53 - 1).
const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;
/// Length of a `YYYY-MM-DD` date.
const ISO_DATE_LEN: usize = 10;

#[derive(Debug, Error)]
pub enum RecordFormError {
    #[error("{0}")]
    Invalid(String),
    #[error("{0} was not found")]
    NotFound(&'static str),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

/// Rejects a draft larger than one conversation message.
pub fn check_draft_values(draft_values: &BTreeMap<String, String>) -> Result<(), String> {
    let bytes = draft_values
        .iter()
        .map(|(code, value)| code.len() + value.len())
        .sum::<usize>();
    if draft_values.len() > MAX_DRAFT_VALUES || bytes > MAX_CONVERSATION_MESSAGE_BYTES {
        return Err(format!(
            "draft_values may hold at most {MAX_DRAFT_VALUES} fields and {MAX_CONVERSATION_MESSAGE_BYTES} bytes"
        ));
    }
    Ok(())
}

pub fn propose_tool_definition() -> ToolDefinition {
    ToolDefinition {
        kind: "function",
        function: ToolFunction {
            name: PROPOSE_TOOL,
            description: "Propose editable record form field values. This does not save catalog data.",
            parameters: json!({
                "type": "object",
                "required": ["fields", "explanation"],
                "properties": {
                    "fields": {
                        "type": "object",
                        "additionalProperties": { "type": "string" }
                    },
                    "explanation": {"type": "string"}
                },
                "additionalProperties": false
            }),
        },
    }
}

#[derive(Serialize)]
pub(crate) struct RecordFormField {
    pub(crate) code: String,
    pub(crate) name: String,
    pub(crate) value_type: ValueType,
    pub(crate) value_schema: Option<Value>,
}

/// The editable part of a record's form in one context.
pub struct RecordForm {
    pub record_id: Uuid,
    pub context_id: Option<Uuid>,
    pub is_default_context: bool,
    pub(crate) fields: Vec<RecordFormField>,
    current_values: Vec<FormAttributeValue>,
    draft_values: BTreeMap<String, String>,
}

/// Values checked against the form, ready to show the user.
#[derive(Debug, PartialEq)]
pub struct Proposal {
    pub fields: BTreeMap<String, String>,
    pub explanation: String,
    /// Proposed fields that are not editable or whose value does not fit.
    pub skipped: Vec<String>,
}

impl RecordForm {
    /// Loads the record's editable scalar fields and their values in
    /// `context_id`. The caller has checked that the user may read the record.
    pub async fn load(
        repository: &AttricatRepository,
        record_id: Uuid,
        context_id: Option<Uuid>,
        draft_values: BTreeMap<String, String>,
    ) -> Result<Self, RecordFormError> {
        let (record, values) = AttricatReadService::new(repository)
            .record_with_values(record_id)
            .await?;
        let blueprint = repository
            .get_blueprint_revision(record.blueprint_id, record.blueprint_version)
            .await?
            .ok_or(RecordFormError::NotFound("blueprint version"))?;
        let reusable_attributes = repository.record_reusable_attributes(record_id).await?;
        let reusable_values = repository.reusable_form_values(record_id).await?;
        let is_default_context = match context_id {
            Some(id) => {
                repository
                    .get_context_by_id(id)
                    .await?
                    .ok_or_else(|| RecordFormError::Invalid("unknown context".to_owned()))?
                    .code
                    == DEFAULT_CONTEXT_CODE
            }
            None => true,
        };
        let mut fields = Vec::new();
        for attribute in &blueprint.attributes {
            if editable(
                attribute.readonly,
                attribute.value_type,
                attribute.context_editable,
                is_default_context,
            ) {
                fields.push(RecordFormField {
                    code: attribute.code.clone(),
                    name: attribute.code.clone(),
                    value_type: attribute.value_type,
                    value_schema: attribute.value_schema.clone(),
                });
            }
        }
        for attribute in &reusable_attributes {
            if editable(
                attribute.readonly,
                attribute.value_type,
                attribute.context_editable,
                is_default_context,
            ) {
                fields.push(RecordFormField {
                    code: attribute.code.clone(),
                    name: attribute.name.clone(),
                    value_type: attribute.value_type,
                    value_schema: attribute.value_schema.clone(),
                });
            }
        }
        let allowed = fields
            .iter()
            .map(|field| field.code.as_str())
            .collect::<HashSet<_>>();
        let current_values = editable_scalar_values(
            values.iter().chain(reusable_values.iter()),
            context_id,
            &allowed,
        )
        .into_iter()
        .cloned()
        .collect();
        let draft_values = draft_values
            .into_iter()
            .filter(|(code, _)| allowed.contains(code.as_str()))
            .collect();
        Ok(Self {
            record_id,
            context_id,
            is_default_context,
            fields,
            current_values,
            draft_values,
        })
    }

    /// The form as the agent provider sees it.
    pub fn describe(&self) -> Value {
        json!({
            "record_id": self.record_id,
            "context_id": self.context_id,
            "editable_fields": self.fields,
            "current_values": self.current_values,
            "unsaved_draft_values": self.draft_values,
        })
    }

    /// Checks proposed values against the form; `None` when the arguments
    /// carry no `fields` object.
    pub fn proposal(&self, arguments: &Value) -> Option<Proposal> {
        let explanation = arguments
            .get("explanation")
            .and_then(Value::as_str)
            .unwrap_or("")
            .chars()
            .take(MAX_EXPLANATION_CHARS)
            .collect();
        let (fields, skipped) = proposed_fields(arguments)?
            .into_iter()
            .partition::<BTreeMap<_, _>, _>(|(code, value)| {
                self.fields
                    .iter()
                    .any(|field| field.code == *code && value_is_valid(field, value))
            });
        Some(Proposal {
            fields,
            explanation,
            skipped: skipped.into_keys().collect(),
        })
    }

    /// A conversation message showing `proposal` with values to apply. Each
    /// field keeps the draft value it was proposed against, so the user is
    /// told when the form changed since.
    pub fn proposal_message(&self, proposal: &Proposal) -> Value {
        let base_values = proposal
            .fields
            .keys()
            .map(|code| (code.clone(), self.draft_values.get(code).cloned()))
            .collect::<BTreeMap<_, _>>();
        json!({"draft_proposal": {
            "fields": proposal.fields,
            "explanation": proposal.explanation,
            "base_values": base_values,
        }})
    }
}

/// The proposed values as form text. Models often send numbers and booleans as
/// JSON scalars despite the tool's string schema; other values are skipped.
fn proposed_fields(arguments: &Value) -> Option<BTreeMap<String, String>> {
    let fields = arguments.get("fields")?.as_object()?;
    Some(
        fields
            .iter()
            .filter_map(|(code, value)| {
                let text = match value {
                    Value::String(text) => text.clone(),
                    Value::Number(number) => number.to_string(),
                    Value::Bool(flag) => flag.to_string(),
                    _ => return None,
                };
                Some((code.clone(), text))
            })
            .collect(),
    )
}

/// Only writable scalar values can be proposed; non-default contexts may
/// change only attributes that are editable outside the default context.
fn editable(
    readonly: bool,
    value_type: ValueType,
    context_editable: ContextEditable,
    is_default_context: bool,
) -> bool {
    !readonly
        && value_type != ValueType::Relationship
        && value_type != ValueType::File
        && (is_default_context || context_editable != ContextEditable::Default)
}

/// Parse a suggestion exactly as the browser will parse a scalar form field,
/// then apply the attribute's JSON schema before offering it to the user.
fn value_is_valid(field: &RecordFormField, proposed: &str) -> bool {
    let value = proposed.trim();
    if value.is_empty() {
        return false;
    }
    let native = match field.value_type {
        ValueType::String => Some(Value::String(value.to_owned())),
        ValueType::Number => value
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number),
        ValueType::Integer => value
            .parse::<i64>()
            .ok()
            .filter(|value| (-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(value))
            .map(|value| json!(value)),
        ValueType::Boolean => value.parse::<bool>().ok().map(|value| json!(value)),
        ValueType::Date => NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .ok()
            .filter(|_| value.len() == ISO_DATE_LEN)
            .map(|_| json!(value)),
        ValueType::Datetime => DateTime::parse_from_rfc3339(value)
            .ok()
            .map(|_| json!(value)),
        ValueType::Time => value.split_once(' ').and_then(|(time, zone)| {
            NaiveTime::parse_from_str(time, "%H:%M:%S%.f").ok()?;
            zone.parse::<chrono_tz::Tz>().ok()?;
            Some(json!({"time": time, "time_zone": zone}))
        }),
        ValueType::Json => serde_json::from_str(value).ok(),
        _ => None,
    };
    let Some(native) = native else { return false };
    field.value_schema.as_ref().is_none_or(|schema| {
        validate_json_schema(schema, &native).is_ok_and(|errors| errors.is_empty())
    })
}

fn editable_scalar_values<'a>(
    values: impl Iterator<Item = &'a FormAttributeValue>,
    context_id: Option<Uuid>,
    allowed: &HashSet<&str>,
) -> Vec<&'a FormAttributeValue> {
    values
        .filter(|value| matches!(value, FormAttributeValue::Scalar { attribute_code, context_id: value_context, .. }
            if *value_context == context_id && allowed.contains(attribute_code.as_str())))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(fields: Vec<RecordFormField>, draft: &[(&str, &str)]) -> RecordForm {
        RecordForm {
            record_id: Uuid::nil(),
            context_id: None,
            is_default_context: true,
            fields,
            current_values: Vec::new(),
            draft_values: draft
                .iter()
                .map(|(code, value)| ((*code).to_owned(), (*value).to_owned()))
                .collect(),
        }
    }

    fn field(code: &str, value_type: ValueType) -> RecordFormField {
        RecordFormField {
            code: code.into(),
            name: code.into(),
            value_type,
            value_schema: None,
        }
    }

    #[test]
    fn only_editable_scalar_values_in_the_requested_context_reach_the_provider() {
        let context = Uuid::new_v4();
        let values = [
            FormAttributeValue::Scalar {
                attribute_code: "name".into(),
                context_id: Some(context),
                value: json!("ok"),
            },
            FormAttributeValue::Scalar {
                attribute_code: "private".into(),
                context_id: Some(context),
                value: json!("secret"),
            },
            FormAttributeValue::Scalar {
                attribute_code: "name".into(),
                context_id: Some(Uuid::new_v4()),
                value: json!("other"),
            },
        ];
        let allowed = HashSet::from(["name"]);
        let filtered = editable_scalar_values(values.iter(), Some(context), &allowed);
        assert_eq!(filtered.len(), 1);
        assert_eq!(serde_json::to_value(filtered[0]).unwrap()["value"], "ok");
    }

    #[test]
    fn suggestions_must_match_the_browser_scalar_type_and_attribute_schema() {
        let field = RecordFormField {
            code: "state".into(),
            name: "State".into(),
            value_type: ValueType::String,
            value_schema: Some(json!({"type": "string", "enum": ["active", "inactive"]})),
        };
        assert!(value_is_valid(&field, "active"));
        assert!(!value_is_valid(&field, "discontinued"));
        let integer = RecordFormField {
            value_type: ValueType::Integer,
            value_schema: None,
            ..field
        };
        assert!(value_is_valid(&integer, "42"));
        assert!(!value_is_valid(&integer, "1.5"));
        assert!(!value_is_valid(&integer, "9223372036854775808"));
        assert!(!value_is_valid(&integer, "-9223372036854775808"));
        let date = RecordFormField {
            value_type: ValueType::Date,
            value_schema: None,
            ..integer
        };
        assert!(value_is_valid(&date, "2026-09-23"));
        assert!(!value_is_valid(&date, "2026-9-23"));
    }

    #[test]
    fn scalar_proposals_become_form_text() {
        let fields = proposed_fields(&json!({"fields": {
            "name": "Golf",
            "founded_year": 1937,
            "price": 24.5,
            "electric": false,
            "notes": null,
            "dimensions": {"length": 4},
        }}))
        .unwrap();
        assert_eq!(
            fields,
            BTreeMap::from([
                ("electric".to_owned(), "false".to_owned()),
                ("founded_year".to_owned(), "1937".to_owned()),
                ("name".to_owned(), "Golf".to_owned()),
                ("price".to_owned(), "24.5".to_owned()),
            ])
        );
        assert_eq!(proposed_fields(&json!({"explanation": "No fields"})), None);
    }

    #[test]
    fn a_proposal_keeps_editable_valid_values_and_names_the_rest() {
        let form = form(
            vec![
                field("name", ValueType::String),
                field("founded_year", ValueType::Integer),
            ],
            &[("name", "VW")],
        );
        let proposal = form
            .proposal(&json!({
                "fields": {"name": "Volkswagen", "founded_year": "soon", "secret": "x"},
                "explanation": "From the fact sheet.",
            }))
            .unwrap();
        assert_eq!(
            proposal,
            Proposal {
                fields: BTreeMap::from([("name".to_owned(), "Volkswagen".to_owned())]),
                explanation: "From the fact sheet.".to_owned(),
                skipped: vec!["founded_year".to_owned(), "secret".to_owned()],
            }
        );
        assert_eq!(
            form.proposal_message(&proposal),
            json!({"draft_proposal": {
                "fields": {"name": "Volkswagen"},
                "explanation": "From the fact sheet.",
                "base_values": {"name": "VW"},
            }})
        );
    }

    #[test]
    fn drafts_are_bounded_like_a_message() {
        assert!(check_draft_values(&BTreeMap::from([("name".into(), "VW".into())])).is_ok());
        let many = (0..=MAX_DRAFT_VALUES)
            .map(|index| (format!("field_{index}"), String::new()))
            .collect();
        assert!(check_draft_values(&many).is_err());
        let large = BTreeMap::from([(
            "notes".to_owned(),
            "x".repeat(MAX_CONVERSATION_MESSAGE_BYTES),
        )]);
        assert!(check_draft_values(&large).is_err());
    }
}
