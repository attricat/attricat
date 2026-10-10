//! Strict, inert workflow definition parsing. Execution is owned by the API runtime.
use attricat_validation::is_valid_code;
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
        /// Optional changed-attribute filter: the event matches only when one
        /// of its payload facts names one of these attribute codes. Empty means
        /// no filter, which keeps stored revisions without the field unchanged.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        attributes: Vec<String>,
    },
    /// A management caller supplies one UUID record target; no arbitrary payload is accepted.
    Manual,
    /// Six-field UTC cron only. UTC is intentional: it eliminates DST duplicate/skipped local times.
    Schedule {
        cron: String,
        timezone: String,
        target_record_id: String,
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
    /// Applies fixed local actions to every live record whose
    /// `relationship_attribute` currently targets the trigger record. Each
    /// target is a separate, idempotent record write; more than `max_targets`
    /// referencing records fails the action before any target is changed.
    ReferencingRecordsUpdate {
        relationship_attribute: String,
        max_targets: u32,
        actions: Vec<Action>,
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
        target_record_id: Option<String>,
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
    attributes: Option<Vec<String>>,
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
    ReferencingRecordsUpdate {
        relationship_attribute: String,
        max_targets: Option<u32>,
        actions: Vec<RawAction>,
    },
}

/// Default and hard upper bound for `referencing_records_update` targets.
pub const DEFAULT_REFERENCING_TARGETS: u32 = 100;
pub const MAX_REFERENCING_TARGETS: u32 = 500;
const MAX_NESTED_ACTIONS: usize = 20;
const MAX_TRIGGER_ATTRIBUTES: usize = 100;

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
            attributes,
        }) => {
            if !RECORD_TRIGGER_EVENTS.contains(&event_type.as_str()) {
                return Err(WorkflowError::Invalid(format!(
                    "unsupported record workflow event type '{event_type}'"
                )));
            }
            envelope_map(&envelope)?;
            facts_map(&facts)?;
            let attributes = match attributes {
                None => Vec::new(),
                Some(codes) => {
                    if event_type == "record.migrated.v1" {
                        return Err(WorkflowError::Invalid(
                            "record.migrated.v1 carries no attribute facts, so it cannot filter on attributes".into(),
                        ));
                    }
                    let codes =
                        limited_strings(codes, "trigger attribute", MAX_TRIGGER_ATTRIBUTES)?;
                    for code in &codes {
                        valid_code(code, "trigger attribute code")?;
                    }
                    codes
                }
            };
            Ok(Trigger::Event {
                event_type,
                envelope,
                facts,
                attributes,
            })
        }
        RawTrigger::V2 {
            kind,
            cron,
            timezone,
            target_record_id,
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
                        && target_record_id.is_none()
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
                    let target_record_id = required(target_record_id, "schedule target_record_id")?;
                    if timezone != "UTC" {
                        return Err(WorkflowError::Invalid(
                            "schedule timezone must be UTC".into(),
                        ));
                    }
                    parse_six_field_cron(&cron)?;
                    if uuid::Uuid::parse_str(&target_record_id).is_err() {
                        return Err(WorkflowError::Invalid(
                            "schedule target_record_id must be a UUID".into(),
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
                        target_record_id,
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
                        || target_record_id.is_some()
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
const RECORD_TRIGGER_EVENTS: &[&str] = &[
    "record.created.v1",
    "record.updated.v1",
    "record.migrated.v1",
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
        RawAction::ReferencingRecordsUpdate {
            relationship_attribute,
            max_targets,
            actions,
        } => {
            valid_code(&relationship_attribute, "relationship_attribute")?;
            let max_targets = max_targets.unwrap_or(DEFAULT_REFERENCING_TARGETS);
            if max_targets == 0 || max_targets > MAX_REFERENCING_TARGETS {
                return Err(WorkflowError::Invalid(format!(
                    "max_targets must be between 1 and {MAX_REFERENCING_TARGETS}"
                )));
            }
            if actions.is_empty() || actions.len() > MAX_NESTED_ACTIONS {
                return Err(WorkflowError::Invalid(format!(
                    "referencing_records_update requires 1 to {MAX_NESTED_ACTIONS} actions"
                )));
            }
            let actions = actions
                .into_iter()
                .map(|nested| match nested {
                    RawAction::ReferencingRecordsUpdate { .. } => Err(WorkflowError::Invalid(
                        "referencing_records_update cannot be nested".into(),
                    )),
                    RawAction::AttributeWrite {
                        event_field: Some(_),
                        ..
                    } => Err(WorkflowError::Invalid(
                        "referencing_records_update attribute writes require fixed values".into(),
                    )),
                    nested => action(nested),
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Action::ReferencingRecordsUpdate {
                relationship_attribute,
                max_targets,
                actions,
            })
        }
    }
}

/// Whether a record event's payload facts name at least one of `attributes`.
/// An empty filter always matches. Facts exist only for values that actually
/// changed, so an unchanged write never satisfies a filter.
pub fn changed_attributes_match(attributes: &[String], payload: &Value) -> bool {
    attributes.is_empty()
        || payload
            .get("facts")
            .and_then(Value::as_array)
            .is_some_and(|facts| {
                facts.iter().any(|fact| {
                    fact.get("attribute_code")
                        .and_then(Value::as_str)
                        .is_some_and(|code| attributes.iter().any(|wanted| wanted == code))
                })
            })
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
        assert!(parse("format_version=1\ncode='x'\nname='x'\n[[triggers]]\nevent_type='record.updated.v1'\n[[actions]]\ntype='system_tags_add'\ntags=['x']").is_ok())
    }
    #[test]
    fn v2_manual_and_utc_schedule_are_strict() {
        assert!(parse("format_version=2\ncode='x'\nname='x'\n[[triggers]]\ntype='manual'\n[[actions]]\ntype='system_tags_add'\ntags=['x']").is_ok());
        assert!(parse("format_version=2\ncode='x'\nname='x'\n[[triggers]]\ntype='schedule'\ncron='0 */5 * * * *'\ntimezone='UTC'\ntarget_record_id='00000000-0000-0000-0000-000000000001'\n[[actions]]\ntype='system_tags_add'\ntags=['x']").is_ok());
        assert!(parse("format_version=2\ncode='x'\nname='x'\n[[triggers]]\ntype='schedule'\ncron='0 */5 * * * *'\ntimezone='America/New_York'\ntarget_record_id='00000000-0000-0000-0000-000000000001'\n[[actions]]\ntype='system_tags_add'\ntags=['x']").is_err());
        for cron in ["*/5 * * * *", "0 0 */5 * * * *"] {
            assert!(parse(&format!("format_version=2\ncode='x'\nname='x'\n[[triggers]]\ntype='schedule'\ncron='{cron}'\ntimezone='UTC'\ntarget_record_id='00000000-0000-0000-0000-000000000001'\n[[actions]]\ntype='system_tags_add'\ntags=['x']")).is_err());
        }
    }
    #[test]
    fn extension_contract_is_explicit_but_not_generic() {
        assert!(parse("format_version=2\ncode='x'\nname='x'\n[[triggers]]\ntype='extension_event'\nprovider='example'\nevent_type='plugin.example.changed.v1'\ncontract_version=1\n[[actions]]\ntype='system_tags_add'\ntags=['x']").is_ok());
        assert!(parse("format_version=2\ncode='x'\nname='x'\n[[triggers]]\ntype='extension_event'\nprovider='example'\nevent_type='record.updated.v1'\ncontract_version=1\n[[actions]]\ntype='system_tags_add'\ntags=['x']").is_err())
    }

    const HEAD: &str = "format_version=2\ncode='x'\nname='x'\n";

    #[test]
    fn event_trigger_filters_on_changed_attributes() {
        let parsed = parse(&format!("{HEAD}[[triggers]]\nevent_type='attribute_value.changed.v1'\nattributes=['body','title']\n[[actions]]\ntype='system_tags_add'\ntags=['x']")).unwrap();
        let Trigger::Event { attributes, .. } = &parsed.triggers[0] else {
            panic!("expected an event trigger")
        };
        assert_eq!(attributes, &["body".to_owned(), "title".to_owned()]);
        for invalid in [
            "attributes=[]",
            "attributes=['a','a']",
            "attributes=['Not A Code']",
        ] {
            assert!(parse(&format!("{HEAD}[[triggers]]\nevent_type='record.updated.v1'\n{invalid}\n[[actions]]\ntype='system_tags_add'\ntags=['x']")).is_err(), "{invalid}");
        }
        assert!(parse(&format!("{HEAD}[[triggers]]\nevent_type='record.migrated.v1'\nattributes=['a']\n[[actions]]\ntype='system_tags_add'\ntags=['x']")).is_err());
        assert!(parse(&format!("{HEAD}[[triggers]]\ntype='manual'\nattributes=['a']\n[[actions]]\ntype='system_tags_add'\ntags=['x']")).is_err());
    }

    #[test]
    fn stored_plans_without_attribute_filters_still_deserialize() {
        let stored = serde_json::json!({"type":"event","event_type":"record.updated.v1","envelope":{},"facts":{}});
        let trigger: Trigger = serde_json::from_value(stored.clone()).unwrap();
        assert!(matches!(&trigger, Trigger::Event { attributes, .. } if attributes.is_empty()));
        assert_eq!(serde_json::to_value(&trigger).unwrap(), stored);
    }

    #[test]
    fn changed_attribute_matching_reads_fact_codes() {
        let payload =
            serde_json::json!({"facts":[{"attribute_code":"title"},{"attribute_code":"license"}]});
        assert!(changed_attributes_match(&[], &payload));
        assert!(changed_attributes_match(&["license".into()], &payload));
        assert!(!changed_attributes_match(&["body".into()], &payload));
        assert!(!changed_attributes_match(
            &["body".into()],
            &serde_json::json!({"facts":[]})
        ));
        assert!(!changed_attributes_match(
            &["body".into()],
            &serde_json::json!({})
        ));
    }

    #[test]
    fn referencing_update_is_bounded_and_fixed() {
        let ok = parse(&format!("{HEAD}[[triggers]]\nevent_type='record.updated.v1'\n[[actions]]\ntype='referencing_records_update'\nrelationship_attribute='license'\nmax_targets=25\n[[actions.actions]]\ntype='attribute_write'\nattribute_code='status'\nfixed='in_review'\n[[actions.actions]]\ntype='system_tags_add'\ntags=['needs-review']")).unwrap();
        assert_eq!(
            ok.actions[0],
            Action::ReferencingRecordsUpdate {
                relationship_attribute: "license".into(),
                max_targets: 25,
                actions: vec![
                    Action::AttributeWrite {
                        attribute_code: "status".into(),
                        value: ScalarSource::Fixed {
                            fixed: "in_review".into()
                        },
                    },
                    Action::SystemTagsAdd {
                        tags: vec!["needs-review".into()]
                    },
                ],
            }
        );
        let default = parse(&format!("{HEAD}[[triggers]]\ntype='manual'\n[[actions]]\ntype='referencing_records_update'\nrelationship_attribute='license'\n[[actions.actions]]\ntype='system_tags_add'\ntags=['x']")).unwrap();
        assert!(matches!(
            default.actions[0],
            Action::ReferencingRecordsUpdate {
                max_targets: DEFAULT_REFERENCING_TARGETS,
                ..
            }
        ));
        for invalid in [
            "max_targets=0\n[[actions.actions]]\ntype='system_tags_add'\ntags=['x']",
            "max_targets=501\n[[actions.actions]]\ntype='system_tags_add'\ntags=['x']",
            "actions=[]",
            "[[actions.actions]]\ntype='attribute_write'\nattribute_code='status'\nevent_field='facts.0.after_value'",
            "[[actions.actions]]\ntype='referencing_records_update'\nrelationship_attribute='x'\n[[actions.actions.actions]]\ntype='system_tags_add'\ntags=['x']",
            "unknown=1\n[[actions.actions]]\ntype='system_tags_add'\ntags=['x']",
        ] {
            assert!(parse(&format!("{HEAD}[[triggers]]\nevent_type='record.updated.v1'\n[[actions]]\ntype='referencing_records_update'\nrelationship_attribute='license'\n{invalid}")).is_err(), "{invalid}");
        }
        let compiled = compile(&format!("{HEAD}[[triggers]]\ntype='manual'\n[[actions]]\ntype='referencing_records_update'\nrelationship_attribute='license'\n[[actions.actions]]\ntype='system_tags_add'\ntags=['x']")).unwrap();
        let round_trip: CompiledWorkflow =
            serde_json::from_value(serde_json::to_value(&compiled).unwrap()).unwrap();
        assert_eq!(round_trip, compiled);
    }
}
