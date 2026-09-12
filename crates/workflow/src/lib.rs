//! Strict, inert workflow definition parsing. Execution is intentionally owned by a later slice.
use catalog_validation::is_valid_code;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkflowError {
    #[error("invalid TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("unsupported format_version {0}")]
    UnsupportedFormatVersion(u32),
    #[error("{0}")]
    Invalid(String),
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct WorkflowDefinition {
    pub format_version: u32,
    pub code: String,
    pub name: String,
    pub triggers: Vec<Trigger>,
    pub actions: Vec<Action>,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Trigger {
    pub event_type: String,
    pub envelope: BTreeMap<String, Value>,
    pub facts: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    SystemTagsAdd {
        tags: Vec<String>,
    },
    SystemTagsRemove {
        tags: Vec<String>,
    },
    SystemMetadataMerge {
        values: BTreeMap<String, Value>,
    },
    SystemMetadataDelete {
        keys: Vec<String>,
    },
    AttributeWrite {
        attribute_code: String,
        value: ScalarSource,
    },
}
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(untagged)]
pub enum ScalarSource {
    Fixed { fixed: Value },
    Event { event_field: String },
}
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct CompiledWorkflow {
    pub code: String,
    pub name: String,
    pub format_version: u32,
    pub triggers: Vec<Trigger>,
    pub actions: Vec<Action>,
    pub raw_definition_hash: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    format_version: u32,
    code: String,
    name: String,
    triggers: Vec<RawTrigger>,
    actions: Vec<RawAction>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTrigger {
    event_type: String,
    #[serde(default)]
    envelope: BTreeMap<String, Value>,
    #[serde(default)]
    facts: BTreeMap<String, Value>,
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum RawAction {
    SystemTagsAdd {
        tags: Vec<String>,
    },
    SystemTagsRemove {
        tags: Vec<String>,
    },
    SystemMetadataMerge {
        values: BTreeMap<String, Value>,
    },
    SystemMetadataDelete {
        keys: Vec<String>,
    },
    AttributeWrite {
        attribute_code: String,
        fixed: Option<Value>,
        event_field: Option<String>,
    },
}

/// Parses and compiles a definition without installing or executing it.
pub fn parse(source: &str) -> Result<WorkflowDefinition, WorkflowError> {
    let raw: Raw = toml::from_str(source)?;
    if raw.format_version != 1 {
        return Err(WorkflowError::UnsupportedFormatVersion(raw.format_version));
    }
    valid_code(&raw.code, "workflow code")?;
    non_empty(&raw.name, "workflow name")?;
    if raw.triggers.is_empty() {
        return Err(WorkflowError::Invalid(
            "at least one trigger is required".into(),
        ));
    }
    if raw.actions.is_empty() {
        return Err(WorkflowError::Invalid(
            "at least one action is required".into(),
        ));
    }
    let triggers = raw
        .triggers
        .into_iter()
        .map(|t| {
            if !CORE_EVENTS.contains(&t.event_type.as_str()) {
                return Err(WorkflowError::Invalid(format!(
                    "unsupported core event type '{}'",
                    t.event_type
                )));
            }
            scalar_map(&t.envelope, "trigger envelope")?;
            scalar_map(&t.facts, "trigger facts")?;
            Ok(Trigger {
                event_type: t.event_type,
                envelope: t.envelope,
                facts: t.facts,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let actions = raw
        .actions
        .into_iter()
        .map(action)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(WorkflowDefinition {
        format_version: raw.format_version,
        code: raw.code,
        name: raw.name,
        triggers,
        actions,
    })
}
pub fn compile(source: &str) -> Result<CompiledWorkflow, WorkflowError> {
    let d = parse(source)?;
    Ok(CompiledWorkflow {
        code: d.code,
        name: d.name,
        format_version: d.format_version,
        triggers: d.triggers,
        actions: d.actions,
        raw_definition_hash: raw_hash(source),
    })
}
pub fn raw_hash(source: &str) -> String {
    format!("{:x}", Sha256::digest(source.as_bytes()))
}
// Keep this explicit allowlist aligned with the v1 core event contract. Workflows
// deliberately do not accept plugin or merely syntactically versioned event types.
const CORE_EVENTS: &[&str] = &[
    "entity.created.v1",
    "entity.updated.v1",
    "entity.deleted.v1",
    "entity.migrated.v1",
    "attribute_value.changed.v1",
    "attribute_value.restored.v1",
    "relationship.changed.v1",
    "blueprint.created.v1",
    "blueprint.revision_created.v1",
    "blueprint.published.v1",
    "context.created.v1",
    "context.updated.v1",
    "context.deleted.v1",
];
fn action(a: RawAction) -> Result<Action, WorkflowError> {
    match a {
        RawAction::SystemTagsAdd { tags } => Ok(Action::SystemTagsAdd {
            tags: strings(tags, "tag")?,
        }),
        RawAction::SystemTagsRemove { tags } => Ok(Action::SystemTagsRemove {
            tags: strings(tags, "tag")?,
        }),
        RawAction::SystemMetadataMerge { values } => {
            if values.is_empty() {
                return Err(WorkflowError::Invalid(
                    "metadata merge values cannot be empty".into(),
                ));
            };
            scalar_map(&values, "metadata merge values")?;
            Ok(Action::SystemMetadataMerge { values })
        }
        RawAction::SystemMetadataDelete { keys } => Ok(Action::SystemMetadataDelete {
            keys: strings(keys, "metadata key")?,
        }),
        RawAction::AttributeWrite {
            attribute_code,
            fixed,
            event_field,
        } => {
            valid_code(&attribute_code, "attribute code")?;
            let value = match (fixed, event_field) {
                (Some(v), None) => {
                    scalar(&v, "attribute fixed value")?;
                    ScalarSource::Fixed { fixed: v }
                }
                (None, Some(field)) => {
                    valid_path(&field, "event_field")?;
                    ScalarSource::Event { event_field: field }
                }
                _ => {
                    return Err(WorkflowError::Invalid(
                        "attribute_write requires exactly one of fixed or event_field".into(),
                    ));
                }
            };
            Ok(Action::AttributeWrite {
                attribute_code,
                value,
            })
        }
    }
}
fn non_empty(v: &str, n: &str) -> Result<(), WorkflowError> {
    if v.trim().is_empty() {
        Err(WorkflowError::Invalid(format!("{n} cannot be empty")))
    } else {
        Ok(())
    }
}
fn valid_code(v: &str, n: &str) -> Result<(), WorkflowError> {
    if !is_valid_code(v) {
        Err(WorkflowError::Invalid(format!("invalid {n}")))
    } else {
        Ok(())
    }
}
fn valid_path(v: &str, n: &str) -> Result<(), WorkflowError> {
    if v.split('.').all(is_valid_code) {
        Ok(())
    } else {
        Err(WorkflowError::Invalid(format!("invalid {n}")))
    }
}
fn scalar(v: &Value, n: &str) -> Result<(), WorkflowError> {
    if v.is_string() || v.is_number() || v.is_boolean() || v.is_null() {
        Ok(())
    } else {
        Err(WorkflowError::Invalid(format!("{n} must be a scalar")))
    }
}
fn scalar_map(m: &BTreeMap<String, Value>, n: &str) -> Result<(), WorkflowError> {
    for (k, v) in m {
        valid_path(k, n)?;
        scalar(v, n)?
    }
    Ok(())
}
fn strings(values: Vec<String>, name: &str) -> Result<Vec<String>, WorkflowError> {
    if values.is_empty() {
        return Err(WorkflowError::Invalid(format!("{name}s cannot be empty")));
    }
    let mut seen = HashSet::new();
    for v in &values {
        non_empty(v, name)?;
        if !seen.insert(v) {
            return Err(WorkflowError::Invalid(format!("duplicate {name}")));
        }
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_safe_workflow_compiles() {
        let s = "format_version = 1\ncode='tag_new'\nname='Tag new'\n[[triggers]]\nevent_type='entity.created.v1'\n[triggers.envelope]\nsource='catalog_api'\n[[actions]]\ntype='system_tags_add'\ntags=['new']\n[[actions]]\ntype='attribute_write'\nattribute_code='title'\nevent_field='facts.title'";
        assert_eq!(compile(s).unwrap().actions.len(), 2)
    }
    #[test]
    fn rejects_unknown_and_unsafe() {
        assert!(
            parse("format_version=1\ncode='x'\nname='x'\nscript='evil'\ntriggers=[]\nactions=[]")
                .is_err()
        );
        assert!(parse("format_version=1\ncode='x'\nname='x'\n[[triggers]]\nevent_type='entity.created.v1'\n[[actions]]\ntype='http'\nurl='x'").is_err());
        assert!(parse("format_version=1\ncode='x'\nname='x'\n[[triggers]]\nevent_type='attribute_value.appended.v1'\n[[actions]]\ntype='system_tags_add'\ntags=['x']").is_err());
    }

    #[test]
    fn accepts_registered_core_event_types() {
        let source = "format_version=1\ncode='x'\nname='x'\n[[triggers]]\nevent_type='attribute_value.changed.v1'\n[[actions]]\ntype='system_tags_add'\ntags=['x']";
        assert!(parse(source).is_ok());
    }
}
