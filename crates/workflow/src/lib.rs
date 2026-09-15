//! Strict, inert workflow definition parsing. Execution is owned by the API runtime.
use catalog_validation::is_valid_code;
use cron::Schedule;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    str::FromStr,
};
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

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct WorkflowDefinition {
    pub format_version: u32,
    pub code: String,
    pub name: String,
    pub triggers: Vec<Trigger>,
    pub actions: Vec<Action>,
}
/// A deliberately closed trigger set.  v1 event syntax is retained for stored revisions.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Trigger {
    Event {
        event_type: String,
        envelope: BTreeMap<String, Value>,
        facts: BTreeMap<String, Value>,
    },
    /// A management caller supplies one UUID entity target; no arbitrary payload is accepted.
    Manual,
    /// Six-field UTC cron only. UTC is intentional: it eliminates DST duplicate/skipped local times.
    Schedule {
        cron: String,
        timezone: String,
        target_entity_id: String,
    },
    /// Reserved contract shape; delivery is intentionally not enabled until the extension dispatcher
    /// has a dynamic, grant-rechecked consumer path.
    ExtensionEvent {
        provider: String,
        event_type: String,
        contract_version: u32,
    },
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
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
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum ScalarSource {
    Fixed { fixed: Value },
    Event { event_field: String },
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
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
#[serde(untagged, deny_unknown_fields)]
enum RawTrigger {
    V1(RawEventTrigger),
    V2 {
        #[serde(rename = "type")]
        kind: String,
        cron: Option<String>,
        timezone: Option<String>,
        target_entity_id: Option<String>,
        provider: Option<String>,
        event_type: Option<String>,
        contract_version: Option<u32>,
    },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEventTrigger {
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

pub fn parse(source: &str) -> Result<WorkflowDefinition, WorkflowError> {
    let raw: Raw = toml::from_str(source)?;
    if !(raw.format_version == 1 || raw.format_version == 2) {
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
    let format_version = raw.format_version;
    let triggers = raw
        .triggers
        .into_iter()
        .map(|trigger_raw| trigger(trigger_raw, format_version))
        .collect::<Result<Vec<_>, _>>()?;
    let actions = raw
        .actions
        .into_iter()
        .map(action)
        .collect::<Result<Vec<_>, _>>()?;
    // Synthetic triggers never contain event facts, so dynamic reads are forbidden.
    if triggers
        .iter()
        .any(|t| matches!(t, Trigger::Manual | Trigger::Schedule { .. }))
        && actions.iter().any(|a| {
            matches!(
                a,
                Action::AttributeWrite {
                    value: ScalarSource::Event { .. },
                    ..
                }
            )
        })
    {
        return Err(WorkflowError::Invalid(
            "manual and schedule workflows require fixed action values".into(),
        ));
    }
    Ok(WorkflowDefinition {
        format_version: raw.format_version,
        code: raw.code,
        name: raw.name,
        triggers,
        actions,
    })
}
fn trigger(raw: RawTrigger, version: u32) -> Result<Trigger, WorkflowError> {
    match raw {
        RawTrigger::V1(RawEventTrigger {
            event_type,
            envelope,
            facts,
        }) => {
            if !ENTITY_TRIGGER_EVENTS.contains(&event_type.as_str()) {
                return Err(WorkflowError::Invalid(format!(
                    "unsupported entity workflow event type '{event_type}'"
                )));
            }
            envelope_map(&envelope)?;
            facts_map(&facts)?;
            Ok(Trigger::Event {
                event_type,
                envelope,
                facts,
            })
        }
        RawTrigger::V2 {
            kind,
            cron,
            timezone,
            target_entity_id,
            provider,
            event_type,
            contract_version,
        } => {
            if version != 2 {
                return Err(WorkflowError::Invalid(
                    "typed triggers require format_version = 2".into(),
                ));
            }
            match kind.as_str() {
                "manual" => {
                    if cron.is_none()
                        && timezone.is_none()
                        && target_entity_id.is_none()
                        && provider.is_none()
                        && event_type.is_none()
                        && contract_version.is_none()
                    {
                        Ok(Trigger::Manual)
                    } else {
                        Err(WorkflowError::Invalid(
                            "manual trigger has no fields".into(),
                        ))
                    }
                }
                "schedule" => {
                    let cron = required(cron, "schedule cron")?;
                    let timezone = required(timezone, "schedule timezone")?;
                    let target_entity_id = required(target_entity_id, "schedule target_entity_id")?;
                    if timezone != "UTC" {
                        return Err(WorkflowError::Invalid(
                            "schedule timezone must be UTC".into(),
                        ));
                    }
                    parse_six_field_cron(&cron)?;
                    if uuid::Uuid::parse_str(&target_entity_id).is_err() {
                        return Err(WorkflowError::Invalid(
                            "schedule target_entity_id must be a UUID".into(),
                        ));
                    }
                    if provider.is_some() || event_type.is_some() || contract_version.is_some() {
                        return Err(WorkflowError::Invalid(
                            "schedule has unsupported fields".into(),
                        ));
                    }
                    Ok(Trigger::Schedule {
                        cron,
                        timezone,
                        target_entity_id,
                    })
                }
                "extension_event" => {
                    let provider = required(provider, "extension provider")?;
                    if provider.len() > 128
                        || !provider
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
                    {
                        return Err(WorkflowError::Invalid("invalid extension provider".into()));
                    }
                    let event_type = required(event_type, "extension event_type")?;
                    if !event_type.starts_with("plugin.") || !event_type.ends_with(".v1") {
                        return Err(WorkflowError::Invalid(
                            "extension event_type must be a versioned plugin event".into(),
                        ));
                    }
                    let contract_version = contract_version.ok_or_else(|| {
                        WorkflowError::Invalid("extension contract_version is required".into())
                    })?;
                    if contract_version == 0
                        || cron.is_some()
                        || timezone.is_some()
                        || target_entity_id.is_some()
                    {
                        return Err(WorkflowError::Invalid(
                            "invalid extension_event trigger".into(),
                        ));
                    }
                    Ok(Trigger::ExtensionEvent {
                        provider,
                        event_type,
                        contract_version,
                    })
                }
                _ => Err(WorkflowError::Invalid("unsupported trigger type".into())),
            }
        }
    }
}
/// Parses the only schedule syntax accepted by the workflow contract.
///
/// The cron crate also accepts alternative field counts, so validate the
/// whitespace-delimited contract before delegating expression parsing.
pub fn parse_six_field_cron(value: &str) -> Result<Schedule, WorkflowError> {
    if value.split_whitespace().count() != 6 {
        return Err(WorkflowError::Invalid(
            "schedule cron must be a valid six-field UTC cron expression".into(),
        ));
    }
    Schedule::from_str(value).map_err(|_| {
        WorkflowError::Invalid("schedule cron must be a valid six-field UTC cron expression".into())
    })
}

fn required(value: Option<String>, name: &str) -> Result<String, WorkflowError> {
    let v = value.ok_or_else(|| WorkflowError::Invalid(format!("{name} is required")))?;
    non_empty(&v, name)?;
    Ok(v)
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
const ENTITY_TRIGGER_EVENTS: &[&str] = &[
    "entity.created.v1",
    "entity.updated.v1",
    "entity.migrated.v1",
    "attribute_value.changed.v1",
    "attribute_value.restored.v1",
    "relationship.changed.v1",
];
const ENVELOPE_FIELDS: &[&str] = &["event_type", "aggregate_kind", "source_kind", "source_name"];
fn action(a: RawAction) -> Result<Action, WorkflowError> {
    match a {
        RawAction::SystemTagsAdd { tags } => Ok(Action::SystemTagsAdd {
            tags: limited_strings(tags, "tag", 100)?,
        }),
        RawAction::SystemTagsRemove { tags } => Ok(Action::SystemTagsRemove {
            tags: limited_strings(tags, "tag", 100)?,
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
            keys: limited_strings(keys, "metadata key", 100)?,
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
                    valid_event_field(&field)?;
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
fn envelope_map(m: &BTreeMap<String, Value>) -> Result<(), WorkflowError> {
    for (key, value) in m {
        let valid = ENVELOPE_FIELDS.contains(&key.as_str())
            || key
                .strip_prefix("metadata.")
                .is_some_and(|p| !p.is_empty() && p.split('.').all(is_valid_code));
        if !valid {
            return Err(WorkflowError::Invalid(format!(
                "unsupported trigger envelope field '{key}'"
            )));
        }
        scalar(value, "trigger envelope")?
    }
    Ok(())
}
fn facts_map(m: &BTreeMap<String, Value>) -> Result<(), WorkflowError> {
    for (key, value) in m {
        let parts: Vec<_> = key.split('.').collect();
        if parts.len() < 3
            || parts[0] != "facts"
            || parts[1].parse::<usize>().is_err()
            || !parts[2..].iter().all(|p| is_valid_code(p))
        {
            return Err(WorkflowError::Invalid(format!(
                "unsupported trigger fact path '{key}'"
            )));
        }
        scalar(value, "trigger facts")?
    }
    Ok(())
}
fn valid_event_field(v: &str) -> Result<(), WorkflowError> {
    let p: Vec<_> = v.split('.').collect();
    if p.len() >= 3
        && p[0] == "facts"
        && p[1].parse::<usize>().is_ok()
        && p[2..].iter().all(|p| is_valid_code(p))
    {
        Ok(())
    } else {
        Err(WorkflowError::Invalid(
            "event_field must be a facts.<index>.<field> path".into(),
        ))
    }
}
fn limited_strings(
    values: Vec<String>,
    name: &str,
    max: usize,
) -> Result<Vec<String>, WorkflowError> {
    if values.is_empty() {
        return Err(WorkflowError::Invalid(format!("{name}s cannot be empty")));
    }
    if values.len() > max {
        return Err(WorkflowError::Invalid(format!(
            "at most {max} {name}s are allowed"
        )));
    }
    let mut seen = HashSet::new();
    for v in &values {
        non_empty(v, name)?;
        if name == "tag" && v.len() > 128 {
            return Err(WorkflowError::Invalid(
                "tag must be no longer than 128 bytes".into(),
            ));
        }
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
    fn v1_remains_compatible() {
        assert!(parse("format_version=1\ncode='x'\nname='x'\n[[triggers]]\nevent_type='entity.updated.v1'\n[[actions]]\ntype='system_tags_add'\ntags=['x']").is_ok())
    }
    #[test]
    fn v2_manual_and_utc_schedule_are_strict() {
        assert!(parse("format_version=2\ncode='x'\nname='x'\n[[triggers]]\ntype='manual'\n[[actions]]\ntype='system_tags_add'\ntags=['x']").is_ok());
        assert!(parse("format_version=2\ncode='x'\nname='x'\n[[triggers]]\ntype='schedule'\ncron='0 */5 * * * *'\ntimezone='UTC'\ntarget_entity_id='00000000-0000-0000-0000-000000000001'\n[[actions]]\ntype='system_tags_add'\ntags=['x']").is_ok());
        assert!(parse("format_version=2\ncode='x'\nname='x'\n[[triggers]]\ntype='schedule'\ncron='0 */5 * * * *'\ntimezone='America/New_York'\ntarget_entity_id='00000000-0000-0000-0000-000000000001'\n[[actions]]\ntype='system_tags_add'\ntags=['x']").is_err());
        for cron in ["*/5 * * * *", "0 0 */5 * * * *"] {
            assert!(parse(&format!("format_version=2\ncode='x'\nname='x'\n[[triggers]]\ntype='schedule'\ncron='{cron}'\ntimezone='UTC'\ntarget_entity_id='00000000-0000-0000-0000-000000000001'\n[[actions]]\ntype='system_tags_add'\ntags=['x']")).is_err());
        }
    }
    #[test]
    fn extension_contract_is_explicit_but_not_generic() {
        assert!(parse("format_version=2\ncode='x'\nname='x'\n[[triggers]]\ntype='extension_event'\nprovider='example'\nevent_type='plugin.example.changed.v1'\ncontract_version=1\n[[actions]]\ntype='system_tags_add'\ntags=['x']").is_ok());
        assert!(parse("format_version=2\ncode='x'\nname='x'\n[[triggers]]\ntype='extension_event'\nprovider='example'\nevent_type='entity.updated.v1'\ncontract_version=1\n[[actions]]\ntype='system_tags_add'\ntags=['x']").is_err())
    }
}
