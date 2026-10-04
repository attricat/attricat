//! Planning of seed resources against the workspace facts the repository
//! collected. A seed only provisions: existing targets are mapped or reported
//! as conflicts, never updated.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use catalog_validation::saved_search;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::SeedSavedSearch;
use crate::solution_packs::{
    BlueprintPublication, MappingKind, Outcome, PlanActionKind, PlanResourceKind, PlannedAction,
    PlannedMapping, SolutionPackError, ValidatedSolutionPack, deterministic_target_id,
    physical_code, resource_code,
};

/// The logical key of the plan action for a context's publication channel.
fn publication_channel_key(context_key: &str) -> String {
    format!("channels/{}", resource_code(context_key))
}

#[derive(Clone, Debug)]
pub struct ExistingContextSnapshot {
    pub id: uuid::Uuid,
    pub code: String,
    /// The publication channel the context already provides, if any.
    pub publication_channel: Option<ExistingPublicationChannel>,
}

/// A workspace publication channel's settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExistingPublicationChannel {
    pub enabled: bool,
    pub required_rule_codes: Vec<String>,
    pub require_valid_entity: bool,
}

impl ExistingPublicationChannel {
    /// Whether two channels have the same settings. Required rules are a set,
    /// so their order and repetition do not matter.
    pub fn same_settings(&self, other: &Self) -> bool {
        fn codes(channel: &ExistingPublicationChannel) -> std::collections::BTreeSet<&str> {
            channel
                .required_rule_codes
                .iter()
                .map(String::as_str)
                .collect()
        }
        self.enabled == other.enabled
            && self.require_valid_entity == other.require_valid_entity
            && codes(self) == codes(other)
    }
}

/// How a prerequisite seed is satisfied by the workspace's completed
/// applications. Resolved by the repository; the planner never searches.
#[derive(Clone, Debug)]
pub enum PrerequisiteResolution {
    Satisfied {
        application_id: uuid::Uuid,
        pack_version: String,
    },
    /// No completed application of the prerequisite pack exists.
    Missing,
    /// Completed applications exist, but none has a version in range.
    Incompatible { versions: Vec<String> },
}

/// Workspace facts for seed resources, collected by the repository in the
/// planning transaction.
#[derive(Clone, Debug, Default)]
pub struct SeedWorkspaceSnapshot {
    /// Explicit context selections keyed by pack-local context key.
    pub existing_contexts: BTreeMap<String, ExistingContextSnapshot>,
    pub rule_codes: BTreeSet<String>,
    pub workflow_codes: BTreeSet<String>,
    pub prerequisites: BTreeMap<String, PrerequisiteResolution>,
}

/// An optional resource that cannot be applied is skipped instead.
fn optional_outcome(required: bool, outcome: Outcome) -> Outcome {
    if required || outcome.0.is_applicable() {
        outcome
    } else {
        (PlanActionKind::Skip, outcome.1)
    }
}

fn available(outcome: Option<&Outcome>) -> bool {
    outcome.is_some_and(|(action, _)| action.provides_target())
}

fn target_absent(resource_kind: PlanResourceKind, code: &str) -> Value {
    serde_json::json!([{"kind": "target_absent", "resource_kind": resource_kind, "code": code}])
}

/// Plans prerequisite seeds. These actions are evidence that the reused
/// blueprints come from a reviewed, completed application.
pub(crate) fn plan_prerequisites(
    pack: &ValidatedSolutionPack,
    prefix: &str,
    publication: BlueprintPublication,
    workspace_id: uuid::Uuid,
    seed: &SeedWorkspaceSnapshot,
) -> (
    Vec<PlannedMapping>,
    Vec<PlannedAction>,
    BTreeMap<String, Outcome>,
) {
    let mut mappings = Vec::new();
    let mut actions = Vec::new();
    let mut outcomes = BTreeMap::new();
    for prerequisite in &pack.manifest().prerequisites {
        let resolution = seed
            .prerequisites
            .get(&prerequisite.key)
            .cloned()
            .unwrap_or(PrerequisiteResolution::Missing);
        let (target_id, snapshot, outcome, preconditions) = match &resolution {
            PrerequisiteResolution::Satisfied {
                application_id,
                pack_version,
            } => (
                *application_id,
                serde_json::json!({
                    "application_id": application_id,
                    "pack_id": prerequisite.id,
                    "pack_version": pack_version,
                }),
                (PlanActionKind::Map, "prerequisite_satisfied"),
                serde_json::json!([{
                    "kind": "prerequisite_application",
                    "application_id": application_id,
                    "pack_id": prerequisite.id,
                    "pack_version": pack_version,
                }]),
            ),
            PrerequisiteResolution::Missing => (
                deterministic_target_id(workspace_id, pack, prefix, publication, &prerequisite.key),
                serde_json::json!({"pack_id": prerequisite.id, "available_versions": []}),
                (PlanActionKind::Blocked, "prerequisite_missing"),
                serde_json::json!([]),
            ),
            PrerequisiteResolution::Incompatible { versions } => (
                deterministic_target_id(workspace_id, pack, prefix, publication, &prerequisite.key),
                serde_json::json!({"pack_id": prerequisite.id, "available_versions": versions}),
                (PlanActionKind::Blocked, "prerequisite_incompatible"),
                serde_json::json!([]),
            ),
        };
        outcomes.insert(prerequisite.key.clone(), outcome);
        mappings.push(PlannedMapping {
            resource_kind: PlanResourceKind::Prerequisite,
            logical_key: prerequisite.key.clone(),
            target_id,
            target_code: prerequisite.id.clone(),
            target_version: None,
            mapping_kind: MappingKind::Existing,
            snapshot: snapshot.clone(),
        });
        actions.push(PlannedAction {
            resource_kind: PlanResourceKind::Prerequisite,
            logical_key: prerequisite.key.clone(),
            action: outcome.0,
            reason_code: outcome.1,
            summary: serde_json::json!({
                "pack_id": prerequisite.id,
                "version_requirement": prerequisite.version,
                "resolution": snapshot,
            }),
            normalized_payload: None,
            preconditions,
        });
    }
    (mappings, actions, outcomes)
}

/// Context mappings and actions, plus each context's outcome and mapping by key.
pub(crate) type PlannedContexts = (
    Vec<PlannedMapping>,
    Vec<PlannedAction>,
    BTreeMap<String, (Outcome, PlannedMapping)>,
);

/// Plans contexts, parents first. Their publication channels are planned
/// after the rules a channel can require.
pub(crate) fn plan_contexts(
    pack: &ValidatedSolutionPack,
    prefix: &str,
    publication: BlueprintPublication,
    workspace_id: uuid::Uuid,
    physical_codes: &BTreeSet<String>,
    blueprint_codes: &BTreeSet<String>,
    seed: &SeedWorkspaceSnapshot,
) -> Result<PlannedContexts, SolutionPackError> {
    let resources = pack
        .manifest()
        .resources
        .contexts
        .iter()
        .map(|resource| (resource.key.as_str(), resource))
        .collect::<BTreeMap<_, _>>();
    let mut ordered = Vec::new();
    let mut visited = BTreeSet::new();
    fn visit<'a>(
        key: &'a str,
        pack: &'a ValidatedSolutionPack,
        visited: &mut BTreeSet<&'a str>,
        ordered: &mut Vec<&'a str>,
    ) {
        if !visited.insert(key) {
            return;
        }
        if let Some(parent) = pack
            .context(key)
            .and_then(|context| context.parent.as_deref())
        {
            visit(parent, pack, visited, ordered);
        }
        ordered.push(key);
    }
    for key in resources.keys() {
        visit(key, pack, &mut visited, &mut ordered);
    }

    let mut planned = BTreeMap::<String, (Outcome, PlannedMapping)>::new();
    let mut mappings = Vec::new();
    let mut actions = Vec::new();
    let mut generated = HashMap::<String, usize>::new();
    for key in &ordered {
        if !seed.existing_contexts.contains_key(*key) {
            *generated.entry(physical_code(prefix, key)?).or_default() += 1;
        }
    }
    for key in ordered {
        let resource = resources[key];
        let context = pack.context(key).expect("validated context exists");
        let parent = context
            .parent
            .as_ref()
            .map(|parent| planned.get(parent).expect("parents are planned first"));
        let (mapping, outcome, payload, preconditions) = if let Some(existing) =
            seed.existing_contexts.get(key)
        {
            (
                PlannedMapping {
                    resource_kind: PlanResourceKind::Context,
                    logical_key: key.to_owned(),
                    target_id: existing.id,
                    target_code: existing.code.clone(),
                    target_version: None,
                    mapping_kind: MappingKind::Existing,
                    snapshot: serde_json::json!({"id": existing.id, "code": existing.code}),
                },
                (PlanActionKind::Map, "existing_context_selected"),
                None,
                serde_json::json!([{"kind": "existing_context", "id": existing.id, "code": existing.code}]),
            )
        } else {
            let code = physical_code(prefix, key)?;
            let mapping = PlannedMapping {
                resource_kind: PlanResourceKind::Context,
                logical_key: key.to_owned(),
                target_id: deterministic_target_id(workspace_id, pack, prefix, publication, key),
                target_code: code.clone(),
                target_version: None,
                mapping_kind: MappingKind::Create,
                snapshot: serde_json::json!({"code": code}),
            };
            let outcome = if generated[&code] > 1 || blueprint_codes.contains(&code) {
                (PlanActionKind::Conflict, "duplicate_target_code")
            } else if physical_codes.contains(&code) {
                (PlanActionKind::Conflict, "target_code_exists")
            } else if parent.is_some_and(|(outcome, _)| !available(Some(outcome))) {
                (PlanActionKind::Blocked, "dependency_not_creatable")
            } else {
                (PlanActionKind::Create, "target_absent")
            };
            let payload = serde_json::json!({
                "code": code,
                "data": context.data,
                "parent_id": parent.map(|(_, mapping)| mapping.target_id),
            });
            let preconditions = target_absent(PlanResourceKind::Context, &code);
            (mapping, outcome, Some(payload), preconditions)
        };
        let outcome = optional_outcome(resource.required, outcome);
        actions.push(PlannedAction {
            resource_kind: PlanResourceKind::Context,
            logical_key: key.to_owned(),
            action: outcome.0,
            reason_code: outcome.1,
            summary: serde_json::json!({
                "target_code": mapping.target_code,
                "required": resource.required,
                "parent": context.parent,
                "publication_channel": context.publication_channel.as_ref().map(|channel| channel.enabled),
            }),
            normalized_payload: payload.filter(|_| outcome.0 == PlanActionKind::Create),
            preconditions: if matches!(outcome.0, PlanActionKind::Create | PlanActionKind::Conflict | PlanActionKind::Map) {
                preconditions
            } else {
                serde_json::json!([])
            },
        });
        mappings.push(mapping.clone());
        planned.insert(key.to_owned(), (outcome, mapping));
    }

    Ok((mappings, actions, planned))
}

/// Inputs shared by the rule, workflow, and saved-search planners.
pub(crate) struct DependentPlanInput<'a> {
    pub pack: &'a ValidatedSolutionPack,
    pub prefix: &'a str,
    pub publication: BlueprintPublication,
    pub workspace_id: uuid::Uuid,
    pub seed: &'a SeedWorkspaceSnapshot,
    pub blueprint_outcomes: &'a HashMap<String, Outcome>,
    pub blueprint_mappings: &'a BTreeMap<String, PlannedMapping>,
    pub contexts: &'a BTreeMap<String, (Outcome, PlannedMapping)>,
}

impl DependentPlanInput<'_> {
    fn blueprint_available(&self, key: &str) -> bool {
        available(self.blueprint_outcomes.get(key))
    }

    fn context_available(&self, key: Option<&str>) -> bool {
        key.is_none_or(|key| available(self.contexts.get(key).map(|(outcome, _)| outcome)))
    }

    fn context_mapping(&self, key: Option<&str>) -> Option<&PlannedMapping> {
        key.map(|key| &self.contexts[key].1)
    }

    fn target_id(&self, key: &str) -> uuid::Uuid {
        deterministic_target_id(
            self.workspace_id,
            self.pack,
            self.prefix,
            self.publication,
            key,
        )
    }
}

/// Visits the blueprint codes named by a TOML predicate, rewriting it only
/// when a code changes so the authored definition is otherwise preserved.
pub(crate) fn visit_toml_predicate_blueprint_codes(
    predicate: &mut toml::Value,
    visit: &mut dyn FnMut(&mut String) -> Result<(), SolutionPackError>,
) -> Result<(), SolutionPackError> {
    let original = serde_json::to_value(&*predicate)
        .map_err(|_| SolutionPackError::Invalid("predicate is not valid TOML".into()))?;
    let mut rewritten = original.clone();
    catalog_validation::predicate::visit_predicate_blueprint_codes(&mut rewritten, visit)?;
    if rewritten != original {
        *predicate = toml::Value::try_from(rewritten)
            .map_err(|_| SolutionPackError::Invalid("predicate is not valid TOML".into()))?;
    }
    Ok(())
}

/// The plan's physical code for a pack blueprint named by its pack-local code.
pub(crate) fn physical_blueprint_code(
    mappings: &BTreeMap<String, PlannedMapping>,
    code: &str,
) -> Result<String, SolutionPackError> {
    mappings
        .iter()
        .find(|(key, mapping)| {
            mapping.resource_kind == PlanResourceKind::Blueprint && resource_code(key) == code
        })
        .map(|(_, mapping)| mapping.target_code.clone())
        .ok_or_else(|| SolutionPackError::Invalid(format!("blueprint code '{code}' is not mapped")))
}

/// Sets a native definition's code and, for a rule, rewrites the blueprint
/// codes its predicate names to the plan's physical codes.
fn replace_codes(
    definition: &str,
    code: &str,
    blueprints: Option<&BTreeMap<String, PlannedMapping>>,
) -> Result<String, SolutionPackError> {
    let mut value: toml::Value = toml::from_str(definition)
        .map_err(|_| SolutionPackError::Invalid("validated definition is invalid".into()))?;
    let table = value
        .as_table_mut()
        .expect("validated definition is a table");
    table.insert("code".into(), toml::Value::String(code.to_owned()));
    if let Some(blueprints) = blueprints
        && let Some(predicate) = table.get_mut("predicate")
    {
        visit_toml_predicate_blueprint_codes(predicate, &mut |code| {
            *code = physical_blueprint_code(blueprints, code)?;
            Ok(())
        })?;
    }
    toml::to_string(&value)
        .map_err(|_| SolutionPackError::Invalid("validated definition is invalid".into()))
}

/// Plans rules, workflows, saved searches and context publication channels.
pub(crate) fn plan_dependents(
    input: &DependentPlanInput<'_>,
) -> Result<(Vec<PlannedMapping>, Vec<PlannedAction>), SolutionPackError> {
    let mut planned = (Vec::new(), Vec::new());
    let rule_outcomes = plan_rules(input, &mut planned)?;
    plan_workflows(input, &mut planned)?;
    plan_saved_searches(input, &mut planned);
    plan_publication_channels(input, &rule_outcomes, &mut planned);
    Ok(planned)
}

type Planned = (Vec<PlannedMapping>, Vec<PlannedAction>);

/// The precondition that a created resource's target is still absent.
fn target_absent_precondition(
    action: PlanActionKind,
    resource_kind: PlanResourceKind,
    code: &str,
) -> Value {
    if matches!(action, PlanActionKind::Create | PlanActionKind::Conflict) {
        target_absent(resource_kind, code)
    } else {
        serde_json::json!([])
    }
}

/// A mapping for a seed resource the plan creates at its first revision.
fn created_mapping(
    input: &DependentPlanInput<'_>,
    resource_kind: PlanResourceKind,
    logical_key: &str,
    code: &str,
) -> PlannedMapping {
    PlannedMapping {
        resource_kind,
        logical_key: logical_key.to_owned(),
        target_id: input.target_id(logical_key),
        target_code: code.to_owned(),
        target_version: Some(1),
        mapping_kind: MappingKind::Create,
        snapshot: serde_json::json!({"code": code, "version": 1}),
    }
}

/// Plans rules and returns each rule's outcome and physical code by key.
fn plan_rules(
    input: &DependentPlanInput<'_>,
    (mappings, actions): &mut Planned,
) -> Result<BTreeMap<String, (Outcome, String)>, SolutionPackError> {
    let mut rule_outcomes = BTreeMap::new();
    for resource in &input.pack.manifest().resources.rules {
        let rule = input.pack.rule(&resource.key).expect("validated rule");
        let code = physical_code(input.prefix, &resource.key)?;
        let blueprint_action = input
            .blueprint_outcomes
            .get(&rule.blueprint)
            .map(|(action, _)| *action);
        let outcome = if !input.blueprint_available(&rule.blueprint)
            || !rule
                .referenced_blueprints
                .iter()
                .all(|key| input.blueprint_available(key))
            || !input.context_available(rule.context.as_deref())
        {
            (PlanActionKind::Blocked, "dependency_not_creatable")
        } else if blueprint_action == Some(PlanActionKind::Create)
            && input.publication != BlueprintPublication::Publish
        {
            (PlanActionKind::Blocked, "blueprint_not_published")
        } else if input.seed.rule_codes.contains(&code) {
            (PlanActionKind::Conflict, "target_code_exists")
        } else {
            (PlanActionKind::Create, "target_absent")
        };
        let (action, reason_code) = optional_outcome(resource.required, outcome);
        rule_outcomes.insert(resource.key.clone(), ((action, reason_code), code.clone()));
        let blueprint = &input.blueprint_mappings[&rule.blueprint];
        let context = input.context_mapping(rule.context.as_deref());
        // An existing blueprint may have live entities, and an enforcing rule
        // is only enabled there after an operator's dry run, as for any rule.
        let enable_deferred = rule.enabled
            && rule.compiled.enforcement.is_some()
            && blueprint_action == Some(PlanActionKind::Map);
        let enabled = rule.enabled && !enable_deferred;
        let normalized_payload = if action == PlanActionKind::Create {
            Some(serde_json::json!({
                "definition": replace_codes(&rule.definition, &code, Some(input.blueprint_mappings))?,
                "blueprint_id": blueprint.target_id,
                "blueprint_version": blueprint.target_version,
                "context_id": context.map(|context| context.target_id),
                "enabled": enabled,
            }))
        } else {
            None
        };
        mappings.push(created_mapping(
            input,
            PlanResourceKind::Rule,
            &resource.key,
            &code,
        ));
        actions.push(PlannedAction {
            resource_kind: PlanResourceKind::Rule,
            logical_key: resource.key.clone(),
            action,
            reason_code,
            summary: serde_json::json!({
                "target_code": code,
                "required": resource.required,
                "blueprint": rule.blueprint,
                "blueprint_code": blueprint.target_code,
                "context": rule.context,
                "context_code": context.map(|context| context.target_code.clone()),
                "enabled": enabled,
                "requested_enabled": rule.enabled,
                "enable_deferred_reason": enable_deferred.then_some("enforcing_rule_requires_dry_run"),
                "severity": rule.compiled.severity,
                "predicate": serde_json::to_value(&rule.compiled.predicate)
                    .expect("rule predicate serializes")["type"],
            }),
            normalized_payload,
            preconditions: target_absent_precondition(action, PlanResourceKind::Rule, &code),
        });
    }
    Ok(rule_outcomes)
}

fn plan_workflows(
    input: &DependentPlanInput<'_>,
    (mappings, actions): &mut Planned,
) -> Result<(), SolutionPackError> {
    for resource in &input.pack.manifest().resources.workflows {
        let workflow = input
            .pack
            .workflow(&resource.key)
            .expect("validated workflow");
        let code = physical_code(input.prefix, &resource.key)?;
        let outcome = if input.seed.workflow_codes.contains(&code) {
            (PlanActionKind::Conflict, "target_code_exists")
        } else {
            (PlanActionKind::Create, "target_absent")
        };
        let (action, reason_code) = optional_outcome(resource.required, outcome);
        let normalized_payload = if action == PlanActionKind::Create {
            Some(serde_json::json!({
                "definition": replace_codes(&workflow.definition, &code, None)?,
                "enabled": workflow.enabled,
            }))
        } else {
            None
        };
        mappings.push(created_mapping(
            input,
            PlanResourceKind::Workflow,
            &resource.key,
            &code,
        ));
        actions.push(PlannedAction {
            resource_kind: PlanResourceKind::Workflow,
            logical_key: resource.key.clone(),
            action,
            reason_code,
            summary: serde_json::json!({
                "target_code": code,
                "required": resource.required,
                "enabled": workflow.enabled,
                "triggers": workflow.compiled.triggers.iter().map(|trigger| {
                    serde_json::to_value(trigger).expect("workflow trigger serializes")["type"].clone()
                }).collect::<Vec<_>>(),
                "action_count": workflow.compiled.actions.len(),
            }),
            normalized_payload,
            preconditions: target_absent_precondition(action, PlanResourceKind::Workflow, &code),
        });
    }
    Ok(())
}

fn plan_saved_searches(input: &DependentPlanInput<'_>, (mappings, actions): &mut Planned) {
    for resource in &input.pack.manifest().resources.saved_searches {
        let search = input
            .pack
            .saved_search(&resource.key)
            .expect("validated saved search");
        let blueprint = &input.blueprint_mappings[&search.blueprint];
        let context = input.context_mapping(search.context.as_deref());
        let dependencies_available = search
            .referenced_blueprints
            .iter()
            .all(|key| input.blueprint_available(key))
            && input.context_available(search.context.as_deref());
        // Physical codes can make a state that was valid with logical keys
        // invalid, for example longer than the size limit. Apply validates
        // the same state, so the plan blocks it instead of failing apply.
        let state = dependencies_available
            .then(|| physical_search_state(search, input.blueprint_mappings, context));
        let invalid_state = state.as_ref().and_then(|state| {
            saved_search::validate_state(saved_search::EXPLORER_SEARCH_KIND, state).err()
        });
        let outcome = if !dependencies_available {
            (PlanActionKind::Blocked, "dependency_not_creatable")
        } else if invalid_state.is_some() {
            (PlanActionKind::Blocked, "saved_search_state_invalid")
        } else {
            (PlanActionKind::Create, "target_absent")
        };
        let (action, reason_code) = optional_outcome(resource.required, outcome);
        let normalized_payload = (action == PlanActionKind::Create).then(|| {
            serde_json::json!({
                "name": search.name,
                "description": search.description,
                "visibility": "workspace",
                "state": state,
            })
        });
        let mut summary = serde_json::json!({
            "required": resource.required,
            "name": search.name,
            "blueprint": search.blueprint,
            "blueprint_code": blueprint.target_code,
            "context": search.context,
            "visibility": "workspace",
        });
        if let Some(reason) = invalid_state {
            summary["invalid_state_reason"] = Value::String(reason);
        }
        mappings.push(PlannedMapping {
            resource_kind: PlanResourceKind::SavedSearch,
            logical_key: resource.key.clone(),
            target_id: input.target_id(&resource.key),
            target_code: "saved_search".to_owned(),
            target_version: None,
            mapping_kind: MappingKind::Create,
            snapshot: serde_json::json!({}),
        });
        actions.push(PlannedAction {
            resource_kind: PlanResourceKind::SavedSearch,
            logical_key: resource.key.clone(),
            action,
            reason_code,
            summary,
            normalized_payload,
            preconditions: target_absent_precondition(
                action,
                PlanResourceKind::SavedSearch,
                "saved_search",
            ),
        });
    }
}

/// Plans the publication channels of pack contexts, after the rules they
/// require. A channel is created like an ordinary channel update; an existing
/// channel must already match exactly.
fn plan_publication_channels(
    input: &DependentPlanInput<'_>,
    rule_outcomes: &BTreeMap<String, (Outcome, String)>,
    (mappings, actions): &mut Planned,
) {
    let required_by_key = input
        .pack
        .manifest()
        .resources
        .contexts
        .iter()
        .map(|resource| (resource.key.as_str(), resource.required))
        .collect::<HashMap<_, _>>();
    for context in input.pack.contexts() {
        let Some(channel) = &context.publication_channel else {
            continue;
        };
        let required = required_by_key[context.key.as_str()];
        let (context_outcome, context_mapping) = &input.contexts[&context.key];
        // A required rule is available when this plan creates it or a rule
        // with its code already exists, such as one an earlier release created.
        let rules_available = channel.required_rules.iter().all(|rule| {
            let (outcome, code) = &rule_outcomes[rule];
            available(Some(outcome)) || input.seed.rule_codes.contains(code)
        });
        let declared = ExistingPublicationChannel {
            enabled: channel.enabled,
            required_rule_codes: channel
                .required_rules
                .iter()
                .map(|rule| rule_outcomes[rule].1.clone())
                .collect(),
            require_valid_entity: channel.require_valid_entity,
        };
        let existing = input
            .seed
            .existing_contexts
            .get(&context.key)
            .and_then(|existing| existing.publication_channel.as_ref());
        let outcome = if !available(Some(context_outcome)) || !rules_available {
            (PlanActionKind::Blocked, "dependency_not_creatable")
        } else if existing.is_some_and(|existing| existing.same_settings(&declared)) {
            (PlanActionKind::Satisfied, "exact_match")
        } else if existing.is_some() {
            (PlanActionKind::Conflict, "publication_channel_mismatch")
        } else {
            (PlanActionKind::Create, "target_absent")
        };
        let (action, reason_code) = optional_outcome(required, outcome);
        let key = publication_channel_key(&context.key);
        let payload = serde_json::json!({
            "context_id": context_mapping.target_id,
            "context_code": context_mapping.target_code,
            "enabled": declared.enabled,
            "required_rule_codes": declared.required_rule_codes,
            "require_valid_entity": declared.require_valid_entity,
        });
        mappings.push(PlannedMapping {
            resource_kind: PlanResourceKind::PublicationChannel,
            logical_key: key.clone(),
            target_id: context_mapping.target_id,
            target_code: context_mapping.target_code.clone(),
            target_version: None,
            mapping_kind: if existing.is_some() {
                MappingKind::Existing
            } else {
                MappingKind::Create
            },
            snapshot: serde_json::json!({
                "context": context.key,
                "enabled": existing.map(|existing| existing.enabled),
            }),
        });
        actions.push(PlannedAction {
            resource_kind: PlanResourceKind::PublicationChannel,
            logical_key: key,
            action,
            reason_code,
            summary: serde_json::json!({
                "context": context.key,
                "context_code": context_mapping.target_code,
                "enabled": declared.enabled,
                "required_rules": channel.required_rules,
                "required_rule_codes": declared.required_rule_codes,
                "require_valid_entity": declared.require_valid_entity,
                "current_enabled": existing.map(|existing| existing.enabled),
                "required": required,
            }),
            normalized_payload: matches!(
                action,
                PlanActionKind::Create | PlanActionKind::Satisfied
            )
            .then_some(payload),
            preconditions: target_absent_precondition(
                action,
                PlanResourceKind::PublicationChannel,
                &context_mapping.target_code,
            ),
        });
    }
}

/// Explorer state with physical blueprint and context codes, normalized by
/// the same function as ordinary saved-search writes.
fn physical_search_state(
    search: &SeedSavedSearch,
    blueprints: &BTreeMap<String, PlannedMapping>,
    context: Option<&PlannedMapping>,
) -> Value {
    let mut state = search.state.clone();
    let object = state.as_object_mut().expect("validated state is an object");
    object.insert(
        "blueprint".into(),
        Value::String(blueprints[&search.blueprint].target_code.clone()),
    );
    match context {
        Some(context) if context.target_code != "default" => {
            object.insert("context".into(), Value::String(context.target_code.clone()));
        }
        _ => {
            object.remove("context");
        }
    }
    if let Some(Value::Array(facets)) = object.get_mut("relationshipFacets") {
        for facet in facets {
            if let Some(Value::String(target)) = facet.get_mut("targetBlueprint") {
                *target = blueprints[target.as_str()].target_code.clone();
            }
        }
    }
    saved_search::normalize_state(&state)
}
