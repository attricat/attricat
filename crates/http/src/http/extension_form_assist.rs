//! Form assists: extension commands that propose values in an open record
//! form when fields they watch change. The host owns the form. It passes the
//! command a strict context (the watched and proposed fields only), checks
//! the answer against the shared contract and each attribute, and returns
//! the proposals for the web app to apply as unsaved changes to fields the
//! user has not edited. A failing, slow, disabled or ungranted assist
//! proposes nothing; the form never depends on it. Writes outside the form
//! do not call assists.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::{Arc, LazyLock},
    time::Duration,
};

use attricat_domain::{model::Attribute, permissions::Permission, value_type::ValueType};
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    AppState,
    auth::{ActiveWorkspace, AuthenticatedPrincipal, ScopedRepository},
    error::ApiError,
    extractors::{ApiJson, ApiQuery},
};
use crate::{
    constants::DEFAULT_CONTEXT_CODE,
    extensions::{
        ContributionView, FormAssistDeclaration, MAX_FORM_ASSIST_FIELDS, UiContribution,
        UiContributionKind, UiOutlet, validate_schema,
    },
    repository::{AttricatRepository, ExtensionRuntimeInstallation, accepted_attribute_value},
};

/// The answer contract every form assist shares.
static CONTRACT: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!(
        "../../../../contracts/extension-form-assist-v1.schema.json"
    ))
    .expect("the form assist contract is valid JSON")
});
/// How long the host waits for an assist before the form goes on without it.
const FORM_ASSIST_TIMEOUT: Duration = Duration::from_secs(3);
/// The version of the context the host passes to an assist's command.
const FORM_ASSIST_CONTEXT_VERSION: u32 = 1;
/// The most targets of one relationship field passed to an assist.
const MAX_FORM_ASSIST_TARGETS: usize = 100;

/// How a form assist call ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Outcome {
    /// The command answered; its valid proposals are returned.
    Answered,
    /// The command failed or broke the contract.
    Failed,
    /// The command did not answer in time, such as while its component is
    /// first compiled; asking again later may answer.
    TimedOut,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FormAssistsQuery {
    blueprint_id: Uuid,
    blueprint_version: i64,
}

#[derive(Serialize)]
pub(super) struct FormAssistsResponse {
    assists: Vec<FormAssistSummary>,
}

#[derive(Serialize)]
struct FormAssistSummary {
    extension_id: String,
    extension_name: String,
    id: String,
    version: u32,
    /// Where the assist runs; always `record_form`.
    outlet: UiOutlet,
    watch: Vec<String>,
    proposes: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FormAssistRequest {
    extension_id: String,
    /// The `form_assist` contribution's ID.
    id: String,
    blueprint_id: Uuid,
    blueprint_version: i64,
    /// The record being edited; absent for a new record.
    #[serde(default)]
    record_id: Option<Uuid>,
    /// The form's value context; the default context when absent.
    #[serde(default)]
    context_id: Option<Uuid>,
    /// The form's working scalar values, by attribute code.
    #[serde(default)]
    values: BTreeMap<String, Value>,
    /// The form's working relationship targets, by attribute code.
    #[serde(default)]
    relationships: BTreeMap<String, Vec<Uuid>>,
    /// Fields the user has edited, which keep their values.
    #[serde(default)]
    edited: BTreeSet<String>,
}

#[derive(Serialize)]
pub(super) struct FormAssistResponse {
    outcome: Outcome,
    proposals: Vec<Proposal>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
struct Proposal {
    attribute_code: String,
    value: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

#[derive(Deserialize)]
struct Answer {
    proposals: Vec<Proposal>,
}

/// The revision's form assists that run now: listed by its detail view,
/// declared by an enabled extension granted `client.form_assist` for that
/// view, with a declared command, and not narrowed away by `when.blueprints`.
/// The detail view's order is the order they run in.
async fn active_assists(
    repository: &AttricatRepository,
    blueprint_id: Uuid,
    blueprint_version: i64,
) -> Result<
    (
        String,
        Vec<(Arc<ExtensionRuntimeInstallation>, UiContribution)>,
    ),
    ApiError,
> {
    let (code, entries) = repository
        .blueprint_view_extensions(blueprint_id, blueprint_version, ContributionView::Detail)
        .await?
        .ok_or_else(|| ApiError::not_found("blueprint version"))?;
    if entries.is_empty() {
        return Ok((code, Vec::new()));
    }
    let providers: HashMap<String, Arc<ExtensionRuntimeInstallation>> = repository
        .enabled_form_assist_providers()
        .await?
        .into_iter()
        .map(|installation| (installation.extension_id.clone(), Arc::new(installation)))
        .collect();
    let assists = entries
        .iter()
        .filter_map(|(extension_id, contribution_id)| {
            let installation = providers.get(extension_id)?;
            let contribution = installation.manifest.ui.iter().find(|contribution| {
                contribution.kind == UiContributionKind::FormAssist
                    && contribution.outlet == Some(UiOutlet::RecordForm)
                    && contribution.view == Some(ContributionView::Detail)
                    && contribution.id == *contribution_id
                    && contribution.when.as_ref().is_none_or(|when| {
                        when.blueprints.is_empty() || when.blueprints.contains(&code)
                    })
            })?;
            Some((installation.clone(), contribution.clone()))
        })
        .collect();
    Ok((code, assists))
}

fn declaration(contribution: &UiContribution) -> &FormAssistDeclaration {
    contribution
        .form_assist
        .as_ref()
        .expect("a validated form_assist declares its fields")
}

/// The form assists a blueprint revision's record form runs, with the fields
/// each watches and proposes.
pub(super) async fn list(
    ScopedRepository(repository): ScopedRepository,
    ApiQuery(query): ApiQuery<FormAssistsQuery>,
) -> Result<Json<FormAssistsResponse>, ApiError> {
    let (_, assists) =
        active_assists(&repository, query.blueprint_id, query.blueprint_version).await?;
    Ok(Json(FormAssistsResponse {
        assists: assists
            .into_iter()
            .map(|(installation, contribution)| {
                let declaration = declaration(&contribution);
                FormAssistSummary {
                    extension_id: installation.extension_id.clone(),
                    extension_name: installation.manifest.name.clone(),
                    id: contribution.id.clone(),
                    version: contribution.version,
                    outlet: UiOutlet::RecordForm,
                    watch: declaration.watch.clone(),
                    proposes: declaration.proposes.clone(),
                }
            })
            .collect(),
    }))
}

/// Calls one form assist with the form's working values and returns its
/// valid proposals. Only the assist's watched and proposed fields reach it,
/// and only relationship targets the caller can read.
pub(super) async fn run(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(caller, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ApiJson(input): ApiJson<FormAssistRequest>,
) -> Result<Json<FormAssistResponse>, ApiError> {
    let (blueprint_code, assists) =
        active_assists(&repository, input.blueprint_id, input.blueprint_version).await?;
    let (installation, contribution) = assists
        .into_iter()
        .find(|(installation, contribution)| {
            installation.extension_id == input.extension_id && contribution.id == input.id
        })
        .ok_or_else(|| ApiError::not_found("form assist"))?;
    let declaration = declaration(&contribution).clone();
    let attributes: HashMap<String, Attribute> = repository
        .list_attributes(input.blueprint_id, input.blueprint_version)
        .await?
        .into_iter()
        .map(|attribute| (attribute.code.clone(), attribute))
        .collect();
    let fields: BTreeSet<&String> = declaration
        .watch
        .iter()
        .chain(&declaration.proposes)
        .collect();
    for (code, kind) in input
        .values
        .keys()
        .map(|code| (code, false))
        .chain(input.relationships.keys().map(|code| (code, true)))
    {
        let relationship = attributes
            .get(code)
            .map(|attribute| attribute.value_type == ValueType::Relationship);
        if !fields.contains(code) || relationship != Some(kind) {
            return Err(ApiError::invalid_input(format!(
                "'{code}' is not a {} field the assist watches or proposes",
                if kind { "relationship" } else { "scalar" }
            )));
        }
    }
    if input
        .relationships
        .values()
        .any(|targets| targets.len() > MAX_FORM_ASSIST_TARGETS)
    {
        return Err(ApiError::invalid_input(format!(
            "a relationship field may pass at most {MAX_FORM_ASSIST_TARGETS} targets"
        )));
    }
    if let Some(record_id) = input.record_id
        && !repository
            .authorized_record_ids(caller, workspace, Permission::RecordsRead, &[record_id])
            .await?
            .contains(&record_id)
    {
        return Err(ApiError::not_found("record"));
    }
    let context_id = match input.context_id {
        Some(context_id) => context_id,
        None => repository
            .list_contexts()
            .await?
            .into_iter()
            .find(|context| context.code == DEFAULT_CONTEXT_CODE)
            .map(|context| context.id)
            .ok_or_else(|| ApiError::not_found("default context"))?,
    };

    // Targets the caller can't read are left out, with their blueprints.
    let mut targets: Vec<Uuid> = input.relationships.values().flatten().copied().collect();
    targets.sort_unstable();
    targets.dedup();
    let readable = repository
        .authorized_record_ids(caller, workspace, Permission::RecordsRead, &targets)
        .await?;
    targets.retain(|id| readable.contains(id));
    let blueprint_codes: HashMap<Uuid, String> = repository
        .record_labels(&targets)
        .await?
        .into_iter()
        .map(|label| (label.id, label.blueprint_code))
        .collect();
    let values: serde_json::Map<String, Value> = fields
        .iter()
        .map(|code| {
            let value = match input.relationships.get(*code) {
                Some(ids) => Value::from(
                    ids.iter()
                        .filter_map(|id| {
                            blueprint_codes.get(id).map(|blueprint_code| {
                                json!({"record_id": id, "blueprint_code": blueprint_code})
                            })
                        })
                        .collect::<Vec<_>>(),
                ),
                None => input.values.get(*code).cloned().unwrap_or(Value::Null),
            };
            ((*code).clone(), value)
        })
        .collect();
    let edited: Vec<&String> = declaration
        .proposes
        .iter()
        .filter(|code| input.edited.contains(*code))
        .collect();
    let request = json!({
        "version": FORM_ASSIST_CONTEXT_VERSION,
        "assist": contribution.id,
        "blueprint": {"code": blueprint_code, "id": input.blueprint_id, "version": input.blueprint_version},
        "record_id": input.record_id,
        "context_id": context_id,
        "values": values,
        "edited": edited,
    });

    let call = tokio::spawn(ask(
        state,
        repository.for_extension(&installation.extension_id),
        installation.clone(),
        declaration.command.clone(),
        request,
    ));
    let answer = match tokio::time::timeout(FORM_ASSIST_TIMEOUT, call).await {
        Ok(Ok(Ok(answer))) => answer,
        Err(_) => {
            return Ok(Json(FormAssistResponse {
                outcome: Outcome::TimedOut,
                proposals: Vec::new(),
            }));
        }
        Ok(Ok(Err(error))) => {
            tracing::warn!(
                extension = %installation.extension_id,
                assist = %contribution.id,
                error = %error,
                "a form assist proposed nothing"
            );
            return Ok(Json(FormAssistResponse {
                outcome: Outcome::Failed,
                proposals: Vec::new(),
            }));
        }
        Ok(Err(_)) => {
            tracing::warn!(
                extension = %installation.extension_id,
                assist = %contribution.id,
                "a form assist task failed"
            );
            return Ok(Json(FormAssistResponse {
                outcome: Outcome::Failed,
                proposals: Vec::new(),
            }));
        }
    };
    Ok(Json(FormAssistResponse {
        outcome: Outcome::Answered,
        proposals: valid_proposals(answer.proposals, &declaration, &attributes, &input.edited),
    }))
}

/// Calls the assist's command and checks its answer against the command's
/// own response schema and the shared contract.
async fn ask(
    state: AppState,
    repository: AttricatRepository,
    installation: Arc<ExtensionRuntimeInstallation>,
    command_id: String,
    request: Value,
) -> Result<Answer, String> {
    let command = installation
        .manifest
        .server
        .as_ref()
        .and_then(|server| {
            server
                .commands
                .iter()
                .find(|command| command.id == command_id)
        })
        .ok_or("the form assist command is not declared")?;
    validate_schema(&command.request_schema, &request).map_err(|error| error.to_string())?;
    let request = request.to_string();
    if request.len() > command.max_request_bytes as usize {
        return Err("the request exceeds the command's byte limit".into());
    }
    let response = state
        .extension_runtime
        .invoke_command(
            &installation,
            repository,
            &command.handler,
            &request,
            command.max_response_bytes,
        )
        .await
        .map_err(|error| error.to_string())?;
    let answer: Value =
        serde_json::from_str(&response).map_err(|_| "the answer is not JSON".to_owned())?;
    validate_schema(&command.response_schema, &answer).map_err(|error| error.to_string())?;
    let violations = attricat_validation::validate_json_schema(&CONTRACT, &answer)?;
    if let Some(violation) = violations.first() {
        return Err(format!(
            "the answer breaks the form assist contract at '{}': {}",
            violation.instance_path, violation.message
        ));
    }
    serde_json::from_value(answer).map_err(|error| error.to_string())
}

/// The proposals the form may apply: for the assist's declared, unedited
/// proposes fields, the first per field, with values their attributes accept
/// in stored form. The rest are dropped.
fn valid_proposals(
    proposals: Vec<Proposal>,
    declaration: &FormAssistDeclaration,
    attributes: &HashMap<String, Attribute>,
    edited: &BTreeSet<String>,
) -> Vec<Proposal> {
    let mut seen = BTreeSet::new();
    proposals
        .into_iter()
        .take(MAX_FORM_ASSIST_FIELDS)
        .filter(|proposal| {
            declaration.proposes.contains(&proposal.attribute_code)
                && !edited.contains(&proposal.attribute_code)
                && seen.insert(proposal.attribute_code.clone())
        })
        .filter_map(|proposal| {
            let attribute = attributes.get(&proposal.attribute_code)?;
            Some(Proposal {
                value: accepted_attribute_value(attribute, proposal.value)?,
                note: proposal
                    .note
                    .map(|note| note.trim().to_owned())
                    .filter(|note| !note.is_empty()),
                attribute_code: proposal.attribute_code,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use attricat_domain::attribute_options::{ContextEditable, ContextFallback};
    use chrono::Utc;

    fn attribute(code: &str, value_type: ValueType, schema: Option<Value>) -> Attribute {
        Attribute {
            id: Uuid::new_v4(),
            blueprint_id: Uuid::nil(),
            blueprint_version: 1,
            code: code.into(),
            name: None,
            description: None,
            value_type,
            value_schema: schema,
            extension_type: None,
            default_value: None,
            default_from: None,
            file_policy: None,
            target_blueprint_code: None,
            target_blueprint_codes: Vec::new(),
            cardinality: None,
            target_cardinality: None,
            hierarchy: None,
            tags: json!([]),
            context_fallback: ContextFallback::Default,
            context_editable: ContextEditable::All,
            readonly: false,
            generated: None,
            balance: None,
            target_scope: None,
            position: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            deleted_at: None,
        }
    }

    #[test]
    fn only_valid_proposals_for_declared_unedited_fields_are_kept() {
        let declaration = FormAssistDeclaration {
            command: "propose".into(),
            watch: vec!["item".into()],
            proposes: vec!["price".into(), "discount".into(), "note".into()],
        };
        let attributes: HashMap<_, _> = [
            attribute("price", ValueType::Number, Some(json!({"minimum": 0}))),
            attribute("discount", ValueType::Integer, None),
            attribute("note", ValueType::String, None),
            attribute("item", ValueType::Relationship, None),
        ]
        .into_iter()
        .map(|attribute| (attribute.code.clone(), attribute))
        .collect();
        let proposal = |code: &str, value: Value| Proposal {
            attribute_code: code.into(),
            value,
            note: Some(" Customer price ".into()),
        };
        let kept = valid_proposals(
            vec![
                proposal("price", json!(12.5)),
                // A second proposal for the same field is dropped.
                proposal("price", json!(13)),
                // Refused by the attribute: not an integer.
                proposal("discount", json!("ten")),
                // Edited by the user.
                proposal("note", json!("typed")),
                // Not declared.
                proposal("item", json!("x")),
            ],
            &declaration,
            &attributes,
            &BTreeSet::from(["note".to_owned()]),
        );
        assert_eq!(
            kept,
            vec![Proposal {
                attribute_code: "price".into(),
                value: json!(12.5),
                note: Some("Customer price".into()),
            }]
        );
        // A value the schema refuses is dropped too.
        assert!(
            valid_proposals(
                vec![proposal("price", json!(-1))],
                &declaration,
                &attributes,
                &BTreeSet::new()
            )
            .is_empty()
        );
    }

    #[test]
    fn the_contract_bounds_answers() {
        let valid = json!({"proposals": [{"attribute_code": "price", "value": 1, "note": "n"}]});
        assert!(
            attricat_validation::validate_json_schema(&CONTRACT, &valid)
                .unwrap()
                .is_empty()
        );
        for invalid in [
            json!({}),
            json!({"proposals": [{"attribute_code": "price"}]}),
            json!({"proposals": [{"attribute_code": "price", "value": null}]}),
            json!({"proposals": [{"attribute_code": "price", "value": 1, "note": "x".repeat(141)}]}),
            json!({"proposals": [], "extra": true}),
        ] {
            assert!(
                !attricat_validation::validate_json_schema(&CONTRACT, &invalid)
                    .unwrap()
                    .is_empty(),
                "{invalid}"
            );
        }
    }
}
