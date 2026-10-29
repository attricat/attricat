//! Field notes: the badges and short notes extensions show beside a record's
//! fields and files. The host asks every provider once per record and value
//! context, checks each answer against the shared contract, drops anything
//! that does not belong to the record, and renders the rest itself; no
//! extension code runs beside a field.

use std::{
    collections::{BTreeMap, HashSet},
    sync::{Arc, LazyLock},
    time::Duration,
};

use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::task::JoinSet;
use uuid::Uuid;

use super::{
    AppState,
    auth::ScopedRepository,
    error::ApiError,
    extractors::{ApiPath, ApiQuery},
};
use crate::{
    constants::DEFAULT_CONTEXT_CODE,
    extensions::{FieldNotesSource, UiContributionKind, UiOutlet, validate_schema},
    repository::{AttricatRepository, ExtensionRuntimeInstallation},
};

/// The answer contract every field-notes source shares.
static CONTRACT: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!(
        "../../../../contracts/extension-field-notes-v1.schema.json"
    ))
    .expect("the field notes contract is valid JSON")
});
/// How long the host waits for one extension's notes before leaving them out.
const FIELD_NOTES_TIMEOUT: Duration = Duration::from_secs(2);
/// The tone a badge has when its extension names none.
const DEFAULT_BADGE_TONE: &str = "neutral";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FieldNotesQuery {
    context_id: Option<Uuid>,
}

#[derive(Serialize)]
pub(super) struct FieldNotesResponse {
    /// One entry per extension that answered, in extension ID order.
    extensions: Vec<ExtensionFieldNotes>,
    /// A provider is still starting, typically compiling its component on
    /// first use; asking again shortly may return its notes.
    incomplete: bool,
}

#[derive(Serialize)]
struct ExtensionFieldNotes {
    extension_id: String,
    extension_name: String,
    attributes: BTreeMap<String, AttributeNote>,
    files: BTreeMap<String, FileNote>,
}

#[derive(Deserialize)]
struct Notes {
    #[serde(default)]
    attributes: BTreeMap<String, AttributeNote>,
    #[serde(default)]
    files: BTreeMap<String, FileNote>,
}

#[derive(Deserialize, Serialize)]
struct AttributeNote {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    badge: Option<Badge>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct Badge {
    label: String,
    #[serde(default = "default_tone")]
    tone: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tooltip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    action: Option<String>,
}

fn default_tone() -> String {
    DEFAULT_BADGE_TONE.into()
}

#[derive(Deserialize, Serialize)]
struct FileNote {
    note: String,
}

/// The record's field notes from every enabled provider. A provider that
/// fails, answers late or breaks the contract is left out; it never fails the
/// response. A late provider keeps running in its own task, so a first call
/// that compiles its component completes for the next request. Reading the
/// record is authorized by the route.
pub(super) async fn get(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
    ApiQuery(query): ApiQuery<FieldNotesQuery>,
) -> Result<Json<FieldNotesResponse>, ApiError> {
    let record = repository
        .get_record(record_id)
        .await?
        .ok_or_else(|| ApiError::not_found("record"))?;
    let providers = repository.enabled_field_note_providers().await?;
    if providers.is_empty() {
        return Ok(Json(FieldNotesResponse {
            extensions: Vec::new(),
            incomplete: false,
        }));
    }
    let (blueprint, reusable, files, contexts) = tokio::try_join!(
        async {
            repository
                .get_blueprint_revision(record.blueprint_id, record.blueprint_version)
                .await?
                .ok_or_else(|| ApiError::not_found("blueprint version"))
        },
        async { Ok(repository.record_reusable_attributes(record_id).await?) },
        async { Ok(repository.record_file_ids(record_id).await?) },
        async { Ok(repository.list_contexts().await?) },
    )?;
    // Extensions always receive a concrete context: the page's, or the
    // workspace default the page shows when it has none.
    let context_id = match query.context_id {
        Some(context_id) => context_id,
        None => contexts
            .iter()
            .find(|context| context.code == DEFAULT_CONTEXT_CODE)
            .map(|context| context.id)
            .ok_or_else(|| ApiError::not_found("default context"))?,
    };
    let attribute_codes: HashSet<String> = blueprint
        .attributes
        .iter()
        .map(|attribute| attribute.code.clone())
        .chain(reusable.iter().map(|attribute| attribute.code.clone()))
        .collect();
    let file_ids: HashSet<String> = files.iter().map(Uuid::to_string).collect();
    let request = json!({
        "record_id": record_id,
        "blueprint_id": record.blueprint_id,
        "blueprint_version": record.blueprint_version,
        "context_id": context_id,
    });

    let mut asks = JoinSet::new();
    for installation in providers {
        let installation = Arc::new(installation);
        let call = tokio::spawn(ask(
            state.clone(),
            repository.for_extension(&installation.extension_id),
            installation.clone(),
            request.clone(),
        ));
        asks.spawn(async move {
            (
                installation,
                tokio::time::timeout(FIELD_NOTES_TIMEOUT, call).await,
            )
        });
    }
    let mut extensions = Vec::new();
    let mut incomplete = false;
    while let Some(joined) = asks.join_next().await {
        let Ok((installation, answer)) = joined else {
            continue;
        };
        match answer {
            Ok(Ok(Ok(notes))) => extensions.push(belonging_to_record(
                &installation,
                notes,
                &attribute_codes,
                &file_ids,
            )),
            Err(_) => incomplete = true,
            Ok(Ok(Err(error))) => tracing::warn!(
                extension = %installation.extension_id,
                error = %error,
                "extension field notes were left out"
            ),
            Ok(Err(_)) => tracing::warn!(
                extension = %installation.extension_id,
                "extension field notes task failed"
            ),
        }
    }
    extensions.sort_by(|left, right| left.extension_id.cmp(&right.extension_id));
    Ok(Json(FieldNotesResponse {
        extensions,
        incomplete,
    }))
}

/// Calls the provider's declared command and checks its answer against the
/// command's own response schema and the shared contract.
async fn ask(
    state: AppState,
    repository: AttricatRepository,
    installation: Arc<ExtensionRuntimeInstallation>,
    request: Value,
) -> Result<Notes, String> {
    let source = installation
        .manifest
        .record_field_notes
        .as_ref()
        .ok_or("no field notes source")?;
    let FieldNotesSource::Command(command_id) = &source.source;
    let command = installation
        .manifest
        .server
        .as_ref()
        .and_then(|server| {
            server
                .commands
                .iter()
                .find(|command| command.id == *command_id)
        })
        .ok_or("the field notes command is not declared")?;
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
            "the answer breaks the field notes contract at '{}': {}",
            violation.instance_path, violation.message
        ));
    }
    serde_json::from_value(answer).map_err(|error| error.to_string())
}

/// Keeps notes for the record's own attributes and files, and badge actions
/// that name one of the extension's badge-only actions.
fn belonging_to_record(
    installation: &ExtensionRuntimeInstallation,
    notes: Notes,
    attribute_codes: &HashSet<String>,
    file_ids: &HashSet<String>,
) -> ExtensionFieldNotes {
    let badge_actions: HashSet<&str> = installation
        .manifest
        .ui
        .iter()
        .filter(|item| {
            item.kind == UiContributionKind::Action
                && item.outlet == Some(UiOutlet::RecordAttributeAction)
        })
        .map(|item| item.id.as_str())
        .collect();
    let attributes = notes
        .attributes
        .into_iter()
        .filter(|(code, _)| attribute_codes.contains(code))
        .map(|(code, mut note)| {
            if let Some(badge) = &mut note.badge
                && badge
                    .action
                    .as_deref()
                    .is_some_and(|action| !badge_actions.contains(action))
            {
                badge.action = None;
            }
            (code, note)
        })
        .filter(|(_, note)| note.badge.is_some() || note.note.is_some())
        .collect();
    let files = notes
        .files
        .into_iter()
        .filter(|(file_id, _)| file_ids.contains(file_id))
        .collect();
    ExtensionFieldNotes {
        extension_id: installation.extension_id.clone(),
        extension_name: installation.manifest.name.clone(),
        attributes,
        files,
    }
}
