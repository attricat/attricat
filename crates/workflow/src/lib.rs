//! Strict, inert workflow definition parsing. Execution is owned by the API runtime.
use attricat_validation::{
    is_valid_code,
    predicate::{MAX_SYSTEM_TAGS, MAX_TAG_LENGTH},
};
use cron::Schedule;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    borrow::Cow,
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

/// An automation that applies fixed record actions when one of its triggers fires.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    title = "Attricat workflow definition",
    transform = definition_cross_field_rules
)]
struct Raw {
    /// Definition format version. `1` accepts only record event triggers;
    /// `2` also accepts `manual`, `schedule` and `extension_event` triggers.
    #[schemars(extend("enum" = FORMAT_VERSIONS))]
    format_version: u32,
    /// Stable workflow identifier, unique within its owner.
    #[schemars(regex(pattern = attricat_validation::CODE_PATTERN))]
    code: String,
    /// Human-readable workflow name.
    #[schemars(regex(pattern = NON_BLANK_PATTERN))]
    name: String,
    /// What starts the workflow. At least one.
    #[schemars(length(min = 1))]
    triggers: Vec<RawTrigger>,
    /// Record actions applied in order when a trigger fires. At least one.
    #[schemars(length(min = 1))]
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
/// Runs when a record event occurs. Event triggers have no `type` key.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "EventTrigger", transform = event_trigger_rules)]
struct RawEventTrigger {
    /// Record event that starts the workflow.
    #[schemars(extend("enum" = RECORD_TRIGGER_EVENTS))]
    event_type: String,
    /// Scalar values the event envelope must match: `event_type`,
    /// `aggregate_kind`, `source_kind`, `source_name` or `metadata.<path>`.
    #[serde(default)]
    #[schemars(schema_with = "envelope_schema")]
    envelope: BTreeMap<String, Value>,
    /// Scalar values the event facts must match, keyed `facts.<index>.<field>`.
    #[serde(default)]
    #[schemars(schema_with = "facts_schema")]
    facts: BTreeMap<String, Value>,
    /// Match only events whose changed facts name one of these attribute
    /// codes. Not allowed with `record.migrated.v1`.
    // `default` only keeps a custom-schema `Option` optional in the schema.
    #[serde(default)]
    #[schemars(schema_with = "trigger_attributes_schema")]
    attributes: Option<Vec<String>>,
}
/// Schema of the typed (`format_version = 2`) triggers. [`RawTrigger::V2`]
/// reads them loosely and [`trigger`] applies these rules.
#[derive(JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
#[expect(
    dead_code,
    reason = "describes RawTrigger::V2 for the JSON Schema only"
)]
enum TypedTriggerSchema {
    /// Runs when started on demand for one record. Requires `format_version = 2`.
    Manual,
    /// Runs on a UTC cron schedule for one fixed record. Requires `format_version = 2`.
    Schedule {
        /// Six-field cron expression, including seconds: `sec min hour day month weekday`.
        #[schemars(regex(pattern = SIX_FIELD_CRON_PATTERN))]
        cron: String,
        /// Schedule time zone. Only `UTC` is supported.
        #[schemars(extend("enum" = [SCHEDULE_TIMEZONE]))]
        timezone: String,
        /// UUID of the record the scheduled run acts on.
        #[schemars(regex(pattern = UUID_PATTERN))]
        target_record_id: String,
    },
    /// Reserved: runs when an extension emits a versioned event. Delivery is
    /// not enabled yet. Requires `format_version = 2`.
    ExtensionEvent {
        /// Extension that emits the event.
        #[schemars(
            length(max = MAX_EXTENSION_PROVIDER_BYTES),
            regex(pattern = EXTENSION_PROVIDER_PATTERN)
        )]
        provider: String,
        /// Versioned extension event, such as `plugin.example.changed.v1`.
        #[schemars(regex(pattern = EXTENSION_EVENT_TYPE_PATTERN))]
        event_type: String,
        /// Version of the event payload contract.
        #[schemars(range(min = 1))]
        contract_version: u32,
    },
}
/// What starts the workflow.
impl JsonSchema for RawTrigger {
    fn schema_name() -> Cow<'static, str> {
        "Trigger".into()
    }
    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let mut variants = vec![generator.subschema_for::<RawEventTrigger>().to_value()];
        if let Some(Value::Array(typed)) =
            TypedTriggerSchema::json_schema(generator).remove("oneOf")
        {
            variants.extend(typed);
        }
        json_schema!({ "description": "What starts the workflow.", "oneOf": variants })
    }
}
/// A record change the workflow makes.
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(rename = "Action")]
enum RawAction {
    /// Adds system tags to the record.
    SystemTagsAdd {
        /// Tags to add, each at most 128 bytes, without duplicates.
        #[schemars(schema_with = "system_tags_schema")]
        tags: Vec<String>,
    },
    /// Removes system tags from the record.
    SystemTagsRemove {
        /// Tags to remove, without duplicates.
        #[schemars(schema_with = "system_tags_schema")]
        tags: Vec<String>,
    },
    /// Merges scalar values into the record's system metadata.
    SystemMetadataMerge {
        /// Scalar values keyed by dotted metadata path, such as `review.state`.
        #[schemars(schema_with = "metadata_values_schema")]
        values: BTreeMap<String, Value>,
    },
    /// Deletes keys from the record's system metadata.
    SystemMetadataDelete {
        /// Metadata keys to delete, without duplicates.
        #[schemars(schema_with = "metadata_keys_schema")]
        keys: Vec<String>,
    },
    /// Writes one attribute value. Set exactly one of `fixed` and `event_field`.
    #[schemars(transform = exactly_one_value_source)]
    AttributeWrite {
        /// Attribute to write.
        #[schemars(
            regex(pattern = attricat_validation::CODE_PATTERN),
            extend("x-attricat-reference" = "attribute")
        )]
        attribute_code: String,
        /// Scalar value to write.
        // `default` only keeps a custom-schema `Option` optional in the schema.
        #[serde(default)]
        #[schemars(schema_with = "scalar_field_schema")]
        fixed: Option<Value>,
        /// Event fact to copy, as `facts.<index>.<field>`. Record event
        /// triggers only.
        #[schemars(regex(pattern = EVENT_FIELD_PATTERN))]
        event_field: Option<String>,
    },
    /// Applies fixed actions to every live record whose relationship
    /// attribute targets the trigger record.
    ReferencingRecordsUpdate {
        /// Relationship attribute of the referencing records.
        #[schemars(
            regex(pattern = attricat_validation::CODE_PATTERN),
            extend("x-attricat-reference" = "relationship_attribute")
        )]
        relationship_attribute: String,
        /// Most referencing records to update; more fails the action before
        /// any record changes.
        #[schemars(
            range(min = 1, max = MAX_REFERENCING_TARGETS),
            extend("default" = DEFAULT_REFERENCING_TARGETS)
        )]
        max_targets: Option<u32>,
        /// Actions applied to each referencing record. Attribute writes need
        /// `fixed` values and updates cannot nest.
        #[schemars(schema_with = "nested_actions_schema")]
        actions: Vec<RawAction>,
    },
}

const FORMAT_VERSIONS: [u32; 2] = [1, 2];
const MANUAL_TRIGGER: &str = "manual";
const SCHEDULE_TRIGGER: &str = "schedule";
const EXTENSION_EVENT_TRIGGER: &str = "extension_event";
const SCHEDULE_TIMEZONE: &str = "UTC";
const REFERENCING_RECORDS_UPDATE_ACTION: &str = "referencing_records_update";
/// Matches text with at least one non-whitespace character.
const NON_BLANK_PATTERN: &str = r"\S";
const SIX_FIELD_CRON_PATTERN: &str = r"^\s*\S+(?:\s+\S+){5}\s*$";
/// The textual forms [`uuid::Uuid::parse_str`] accepts.
const UUID_PATTERN: &str = "^(?:[0-9A-Fa-f]{32}|[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}|\\{[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}\\}|urn:uuid:[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12})$";
const EXTENSION_PROVIDER_PATTERN: &str = r"^[A-Za-z0-9._-]+$";
const EXTENSION_EVENT_PREFIX: &str = "plugin.";
const EXTENSION_EVENT_SUFFIX: &str = ".v1";
const EXTENSION_EVENT_TYPE_PATTERN: &str = r"^plugin\.(?:[\s\S]*\.)?v1$";
/// A dotted path of codes, such as a metadata key.
const CODE_PATH_PATTERN: &str = r"^[A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)*$";
/// `facts.<index>.<field>`, where the index is a `usize`.
const EVENT_FIELD_PATTERN: &str = r"^facts\.\+?[0-9]+(?:\.[A-Za-z0-9_-]+)+$";
const ENVELOPE_METADATA_PATTERN: &str = r"^metadata\.[A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)*$";

/// TOML values a trigger filter or written value may hold.
fn scalar_schema() -> Value {
    json!({ "type": ["string", "number", "boolean"] })
}
fn scalar_field_schema(_: &mut SchemaGenerator) -> Schema {
    Schema::try_from(scalar_schema()).expect("scalar schema is an object")
}
fn envelope_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": "object",
        "default": {},
        "propertyNames": {
            "anyOf": [{ "enum": ENVELOPE_FIELDS }, { "pattern": ENVELOPE_METADATA_PATTERN }]
        },
        "additionalProperties": scalar_schema(),
        "x-attricat-key-suggestions": ENVELOPE_FIELDS,
    })
}
fn facts_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": "object",
        "default": {},
        "propertyNames": { "pattern": EVENT_FIELD_PATTERN },
        "additionalProperties": scalar_schema(),
    })
}
fn trigger_attributes_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": "array",
        "minItems": 1,
        "maxItems": MAX_TRIGGER_ATTRIBUTES,
        "uniqueItems": true,
        "items": {
            "type": "string",
            "pattern": attricat_validation::CODE_PATTERN,
            "x-attricat-reference": "attribute",
        },
    })
}
fn system_tags_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": "array",
        "minItems": 1,
        "maxItems": MAX_SYSTEM_TAGS,
        "uniqueItems": true,
        "items": {
            "type": "string",
            "maxLength": MAX_TAG_LENGTH,
            "pattern": NON_BLANK_PATTERN,
        },
    })
}
fn metadata_values_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": "object",
        "minProperties": 1,
        "propertyNames": { "pattern": CODE_PATH_PATTERN },
        "additionalProperties": scalar_schema(),
    })
}
fn metadata_keys_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": "array",
        "minItems": 1,
        "maxItems": MAX_METADATA_KEYS_PER_ACTION,
        "uniqueItems": true,
        "items": { "type": "string", "pattern": NON_BLANK_PATTERN },
    })
}
fn nested_actions_schema(generator: &mut SchemaGenerator) -> Schema {
    let action = generator.subschema_for::<RawAction>();
    json_schema!({
        "type": "array",
        "minItems": 1,
        "maxItems": MAX_NESTED_ACTIONS,
        "items": {
            "$ref": action.get("$ref"),
            "not": {
                "anyOf": [
                    {
                        "properties": { "type": { "const": REFERENCING_RECORDS_UPDATE_ACTION } },
                        "required": ["type"],
                    },
                    { "required": ["event_field"] },
                ]
            },
        },
    })
}
fn exactly_one_value_source(schema: &mut Schema) {
    schema.insert("if".into(), json!({ "required": ["fixed"] }));
    schema.insert(
        "then".into(),
        json!({ "not": { "required": ["event_field"] } }),
    );
    schema.insert("else".into(), json!({ "required": ["event_field"] }));
}
fn event_trigger_rules(schema: &mut Schema) {
    schema.insert(
        "if".into(),
        json!({
            "properties": { "event_type": { "const": attricat_events::RECORD_MIGRATED_V1 } },
            "required": ["event_type"],
        }),
    );
    schema.insert(
        "then".into(),
        json!({ "not": { "required": ["attributes"] } }),
    );
}
fn definition_cross_field_rules(schema: &mut Schema) {
    schema.insert(
        "allOf".into(),
        json!([
            {
                "description": "Format version 1 accepts only record event triggers.",
                "if": {
                    "properties": { "format_version": { "const": 1 } },
                    "required": ["format_version"],
                },
                "then": {
                    "properties": { "triggers": { "items": { "not": { "required": ["type"] } } } }
                },
            },
            {
                "description": "Manual and schedule triggers carry no event facts, so attribute writes need fixed values.",
                "if": {
                    "properties": {
                        "triggers": {
                            "contains": {
                                "properties": {
                                    "type": { "enum": [MANUAL_TRIGGER, SCHEDULE_TRIGGER] }
                                },
                                "required": ["type"],
                            }
                        }
                    },
                    "required": ["triggers"],
                },
                "then": {
                    "properties": {
                        "actions": { "items": { "not": { "required": ["event_field"] } } }
                    }
                },
            },
        ]),
    );
}

/// Default and hard upper bound for `referencing_records_update` targets.
pub(crate) const DEFAULT_REFERENCING_TARGETS: u32 = 100;
pub(crate) const MAX_REFERENCING_TARGETS: u32 = 500;
const MAX_NESTED_ACTIONS: usize = 20;
const MAX_TRIGGER_ATTRIBUTES: usize = 100;
const MAX_EXTENSION_PROVIDER_BYTES: usize = 128;
/// Keys one system metadata delete action may remove.
const MAX_METADATA_KEYS_PER_ACTION: usize = 100;
/// Fields of a schedule cron expression, including seconds.
const CRON_FIELDS: usize = 6;

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
                    if event_type == attricat_events::RECORD_MIGRATED_V1 {
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
                MANUAL_TRIGGER => {
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
                SCHEDULE_TRIGGER => {
                    let cron = required(cron, "schedule cron")?;
                    let timezone = required(timezone, "schedule timezone")?;
                    let target_record_id = required(target_record_id, "schedule target_record_id")?;
                    if timezone != SCHEDULE_TIMEZONE {
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
                EXTENSION_EVENT_TRIGGER => {
                    let provider = required(provider, "extension provider")?;
                    if provider.len() > MAX_EXTENSION_PROVIDER_BYTES
                        || !provider
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
                    {
                        return Err(WorkflowError::Invalid("invalid extension provider".into()));
                    }
                    let event_type = required(event_type, "extension event_type")?;
                    if !event_type.starts_with(EXTENSION_EVENT_PREFIX)
                        || !event_type.ends_with(EXTENSION_EVENT_SUFFIX)
                    {
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
    let invalid = || {
        WorkflowError::Invalid("schedule cron must be a valid six-field UTC cron expression".into())
    };
    if value.split_whitespace().count() != CRON_FIELDS {
        return Err(invalid());
    }
    Schedule::from_str(value).map_err(|_| invalid())
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
const RECORD_TRIGGER_EVENTS: &[&str] = attricat_events::RECORD_TRIGGER_EVENT_TYPES_V1;
const ENVELOPE_FIELDS: &[&str] = &["event_type", "aggregate_kind", "source_kind", "source_name"];
fn action(a: RawAction) -> Result<Action, WorkflowError> {
    match a {
        RawAction::SystemTagsAdd { tags } => Ok(Action::SystemTagsAdd {
            tags: system_tags(tags)?,
        }),
        RawAction::SystemTagsRemove { tags } => Ok(Action::SystemTagsRemove {
            tags: system_tags(tags)?,
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
            keys: limited_strings(keys, "metadata key", MAX_METADATA_KEYS_PER_ACTION)?,
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
        if !seen.insert(v) {
            return Err(WorkflowError::Invalid(format!("duplicate {name}")));
        }
    }
    Ok(values)
}
/// A tag action's tags: no more than a record may carry, each within the
/// stored system-tag length.
fn system_tags(tags: Vec<String>) -> Result<Vec<String>, WorkflowError> {
    let tags = limited_strings(tags, "tag", MAX_SYSTEM_TAGS)?;
    if tags.iter().any(|tag| tag.len() > MAX_TAG_LENGTH) {
        return Err(WorkflowError::Invalid(format!(
            "tag must be no longer than {MAX_TAG_LENGTH} bytes"
        )));
    }
    Ok(tags)
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

    /// The JSON Schema for workflow definition TOML, published as
    /// `contracts/workflow-definition-v1.schema.json` for the web editor.
    fn definition_json_schema() -> serde_json::Value {
        schemars::generate::SchemaSettings::draft2020_12()
            .into_generator()
            .into_root_schema_for::<Raw>()
            .to_value()
    }

    const CONTRACT: &str = "contracts/workflow-definition-v1.schema.json";
    const UPDATE_CONTRACTS: &str = "UPDATE_CONTRACTS";

    #[test]
    fn definition_schema_contract_is_current() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .join(CONTRACT);
        let mut rendered = serde_json::to_string_pretty(&definition_json_schema()).unwrap();
        rendered.push('\n');
        if std::env::var_os(UPDATE_CONTRACTS).is_some() {
            std::fs::write(&path, rendered).unwrap();
            return;
        }
        assert!(
            std::fs::read_to_string(&path).unwrap_or_default() == rendered,
            "{CONTRACT} is stale; run `just contracts`"
        );
    }

    const EVENT_WORKFLOW: &str = "format_version=2\ncode='review-license'\nname='Review license'\n[[triggers]]\nevent_type='attribute_value.changed.v1'\nattributes=['license']\n[triggers.envelope]\nsource_kind='api'\n[triggers.facts]\n'facts.0.attribute_code'='license'\n[[actions]]\ntype='attribute_write'\nattribute_code='status'\nevent_field='facts.0.after_value'\n[[actions]]\ntype='referencing_records_update'\nrelationship_attribute='license'\nmax_targets=25\n[[actions.actions]]\ntype='attribute_write'\nattribute_code='status'\nfixed='in_review'\n[[actions.actions]]\ntype='system_tags_add'\ntags=['needs-review']\n[[actions]]\ntype='system_metadata_merge'\nvalues={'review.state'='open'}";
    const SCHEDULE_WORKFLOW: &str = "format_version=2\ncode='x'\nname='x'\n[[triggers]]\ntype='schedule'\ncron='0 */5 * * * *'\ntimezone='UTC'\ntarget_record_id='00000000-0000-0000-0000-000000000001'\n[[triggers]]\ntype='extension_event'\nprovider='example'\nevent_type='plugin.example.changed.v1'\ncontract_version=1\n[[actions]]\ntype='system_tags_add'\ntags=['x']\n[[actions]]\ntype='system_metadata_delete'\nkeys=['review.state']";

    #[test]
    fn schema_matches_parser_for_representative_definitions() {
        let schema = definition_json_schema();
        let fixed_write = "type='attribute_write'\nattribute_code='status'\nfixed='done'";
        for (definition, accepted) in [
            (EVENT_WORKFLOW.to_owned(), true),
            (SCHEDULE_WORKFLOW.to_owned(), true),
            (EVENT_WORKFLOW.replace("format_version=2", "format_version=1"), true),
            (EVENT_WORKFLOW.replace("format_version=2", "format_version=3"), false),
            (EVENT_WORKFLOW.replace("name='Review license'\n", ""), false),
            (EVENT_WORKFLOW.replace("name='Review license'", "name=' '"), false),
            (format!("{EVENT_WORKFLOW}\nlabel='Review'"), false),
            (
                EVENT_WORKFLOW.replace("attribute_value.changed.v1", "record.migrated.v1"),
                false,
            ),
            (
                EVENT_WORKFLOW.replace("attribute_value.changed.v1", "record.deleted.v1"),
                false,
            ),
            (EVENT_WORKFLOW.replace("attributes=['license']\n", ""), true),
            (
                EVENT_WORKFLOW
                    .replace("attributes=['license']\n", "")
                    .replace("attribute_value.changed.v1", "record.migrated.v1"),
                true,
            ),
            (EVENT_WORKFLOW.replace("source_kind=", "origin="), false),
            (EVENT_WORKFLOW.replace("'facts.0.attribute_code'", "'facts.x.attribute_code'"), false),
            (EVENT_WORKFLOW.replace("max_targets=25", "max_targets=501"), false),
            (
                EVENT_WORKFLOW.replace("fixed='in_review'", "event_field='facts.0.after_value'"),
                false,
            ),
            (
                EVENT_WORKFLOW.replace("event_field='facts.0.after_value'", "fixed='x'\nevent_field='facts.0.after_value'"),
                false,
            ),
            (EVENT_WORKFLOW.replace("type='system_tags_add'", "type='system_tags_set'"), false),
            (EVENT_WORKFLOW.replace("tags=['needs-review']", "tags=['a','a']"), false),
            (EVENT_WORKFLOW.replace("{'review.state'='open'}", "{}"), false),
            (SCHEDULE_WORKFLOW.replace("format_version=2", "format_version=1"), false),
            (SCHEDULE_WORKFLOW.replace("timezone='UTC'", "timezone='Europe/Warsaw'"), false),
            (SCHEDULE_WORKFLOW.replace("cron='0 */5 * * * *'", "cron='*/5 * * * *'"), false),
            (SCHEDULE_WORKFLOW.replace("00000000-0000-0000-0000-000000000001", "record-1"), false),
            (SCHEDULE_WORKFLOW.replace("plugin.example.changed.v1", "record.updated.v1"), false),
            (SCHEDULE_WORKFLOW.replace("contract_version=1", "contract_version=0"), false),
            (SCHEDULE_WORKFLOW.replace("keys=['review.state']", "keys=[]"), false),
            (
                SCHEDULE_WORKFLOW.replace("type='system_tags_add'\ntags=['x']", fixed_write),
                true,
            ),
            (
                SCHEDULE_WORKFLOW.replace(
                    "type='system_tags_add'\ntags=['x']",
                    "type='attribute_write'\nattribute_code='status'\nevent_field='facts.0.after_value'",
                ),
                false,
            ),
            (
                SCHEDULE_WORKFLOW.replace("type='schedule'\ncron='0 */5 * * * *'\ntimezone='UTC'\ntarget_record_id='00000000-0000-0000-0000-000000000001'", "type='manual'"),
                true,
            ),
            (
                SCHEDULE_WORKFLOW.replace("type='schedule'\ncron='0 */5 * * * *'\ntimezone='UTC'\ntarget_record_id='00000000-0000-0000-0000-000000000001'", "type='manual'\nattributes=['a']"),
                false,
            ),
        ] {
            assert_eq!(parse(&definition).is_ok(), accepted, "{definition}");
            let instance =
                serde_json::to_value(toml::from_str::<toml::Value>(&definition).unwrap()).unwrap();
            let violations = attricat_validation::validate_json_schema(&schema, &instance).unwrap();
            assert_eq!(
                violations.is_empty(),
                accepted,
                "{definition}: {violations:?}"
            );
        }
    }
}
