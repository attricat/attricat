//! Field notes: the badges, short notes and action states extensions give
//! for records. The host asks every server-side provider once per batch of
//! records and value context, checks each answer against the shared
//! contract, drops anything that does not belong to the records, and renders
//! the rest itself; no extension code runs beside a field. Client-side
//! providers are listed for the web app, which runs them in sandboxed frames.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, hash_map::Entry},
    hash::{DefaultHasher, Hash, Hasher},
    sync::{Arc, LazyLock, Mutex},
    time::{Duration, Instant},
};

use attricat_domain::permissions::Permission;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, HeaderValue, header},
    response::Response,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::task::JoinSet;
use uuid::Uuid;

use super::{
    AppState,
    auth::{ActiveWorkspace, AuthenticatedPrincipal, ScopedRepository},
    conditional,
    error::ApiError,
    extractors::{ApiJson, ApiPath},
};
use crate::{
    constants::DEFAULT_CONTEXT_CODE,
    extension_installer::installed_artifact_key,
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
/// The most records one request may ask about, such as an Explorer page.
pub(super) const MAX_FIELD_NOTE_RECORDS: usize = 100;
/// The most records one provider call asks about, so an answer stays within
/// a command's response limit.
const FIELD_NOTES_PER_CALL: usize = 25;
/// How long an answer is reused for the same record version.
const FIELD_NOTES_CACHE_TTL: Duration = Duration::from_secs(30);
/// How long an answer its extension marked cacheable is reused while the
/// record does not change.
const CACHEABLE_FIELD_NOTES_TTL: Duration = Duration::from_secs(60 * 60);
/// The most answers kept in this process.
const MAX_CACHED_FIELD_NOTES: usize = 10_000;
/// The longest locale tag forwarded to extensions.
const MAX_LOCALE_BYTES: usize = 16;

/// One provider's notes for one record, context and locale: the workspace,
/// release, [`installation_fingerprint`], record, [`RecordScope::fingerprint`],
/// context and locale.
type CacheKey = (Uuid, Uuid, u64, Uuid, u64, Uuid, String);
static CACHE: LazyLock<Mutex<HashMap<CacheKey, (Instant, RecordNotes)>>> =
    LazyLock::new(Default::default);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FieldNotesRequest {
    record_ids: Vec<Uuid>,
    #[serde(default)]
    context_id: Option<Uuid>,
    /// The viewer's language, forwarded so extensions can answer in it.
    #[serde(default)]
    locale: Option<String>,
}

#[derive(Serialize)]
pub(super) struct FieldNotesResponse {
    /// Each readable record's notes, one entry per extension that answered,
    /// in extension ID order.
    records: BTreeMap<Uuid, Vec<ExtensionRecordNotes>>,
    /// A provider is still starting, typically compiling its component on
    /// first use; asking again shortly may return its notes.
    incomplete: bool,
    /// Providers the web app runs itself, in sandboxed frames.
    client_sources: Vec<ClientSource>,
}

#[derive(Serialize)]
struct ClientSource {
    extension_id: String,
    extension_name: String,
    release_id: Uuid,
    /// The granted capabilities its frame may use; only reads are brokered.
    capabilities: Vec<String>,
}

#[derive(Clone, Serialize)]
struct ExtensionRecordNotes {
    extension_id: String,
    extension_name: String,
    #[serde(flatten)]
    notes: RecordNotes,
}

#[derive(Deserialize)]
struct Answer {
    records: BTreeMap<String, RecordNotes>,
    #[serde(default)]
    cacheable: bool,
}

#[derive(Clone, Default, Deserialize, Serialize)]
struct RecordNotes {
    #[serde(default)]
    attributes: BTreeMap<String, AttributeNote>,
    #[serde(default)]
    files: BTreeMap<String, FileNote>,
    #[serde(default)]
    actions: BTreeMap<String, Value>,
}

#[derive(Clone, Deserialize, Serialize)]
struct AttributeNote {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    badge: Option<Badge>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
struct Badge {
    label: String,
    #[serde(default = "default_tone")]
    tone: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tooltip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    action: Option<String>,
}

fn default_tone() -> String {
    DEFAULT_BADGE_TONE.into()
}

#[derive(Clone, Deserialize, Serialize)]
struct FileNote {
    note: String,
}

/// What a record allows notes about.
struct RecordScope {
    blueprint_id: Uuid,
    blueprint_version: i64,
    attribute_codes: HashSet<String>,
    file_ids: HashSet<String>,
    /// Changes with anything answers are filtered against: the record's
    /// version (its values and annotations), attribute codes and files. A
    /// reusable attribute attached to the record does not change its version.
    fingerprint: u64,
}

/// Changes with what a provider answers under: its configuration and grants.
fn installation_fingerprint(installation: &ExtensionRuntimeInstallation) -> u64 {
    let mut hasher = DefaultHasher::new();
    installation.configuration.to_string().hash(&mut hasher);
    for grants in [
        &installation.capability_grants,
        &installation.host_permission_grants,
    ] {
        grants.iter().collect::<BTreeSet<_>>().hash(&mut hasher);
    }
    hasher.finish()
}

fn has_source(installation: &ExtensionRuntimeInstallation, client: bool) -> bool {
    match installation
        .manifest
        .record_field_notes
        .as_ref()
        .map(|notes| &notes.source)
    {
        Some(FieldNotesSource::Client(_)) => client,
        Some(FieldNotesSource::Command(_)) => !client,
        None => false,
    }
}

/// Field notes for the readable records among `record_ids` from every
/// enabled server-side provider. A provider that fails, answers late or
/// breaks the contract is left out; it never fails the response. A late
/// provider keeps running in its own task, so a first call that compiles its
/// component completes for the next request.
pub(super) async fn post(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(caller, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ApiJson(input): ApiJson<FieldNotesRequest>,
) -> Result<Json<FieldNotesResponse>, ApiError> {
    let mut record_ids = input.record_ids;
    record_ids.sort_unstable();
    record_ids.dedup();
    if record_ids.is_empty() || record_ids.len() > MAX_FIELD_NOTE_RECORDS {
        return Err(ApiError::invalid_input(format!(
            "record_ids must contain between 1 and {MAX_FIELD_NOTE_RECORDS} distinct IDs"
        )));
    }
    let locale = input.locale.unwrap_or_default();
    if locale.len() > MAX_LOCALE_BYTES
        || !locale
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(ApiError::invalid_input("locale is invalid".into()));
    }
    let readable = repository
        .authorized_record_ids(caller, workspace, Permission::RecordsRead, &record_ids)
        .await?;
    record_ids.retain(|id| readable.contains(id));

    let providers = repository.enabled_field_note_providers().await?;
    let client_sources = providers
        .iter()
        .filter(|installation| has_source(installation, true))
        .map(|installation| ClientSource {
            extension_id: installation.extension_id.clone(),
            extension_name: installation.manifest.name.clone(),
            release_id: installation.installed_release_id,
            capabilities: installation.capability_grants.iter().cloned().collect(),
        })
        .collect();
    let servers: Vec<_> = providers
        .into_iter()
        .filter(|installation| has_source(installation, false))
        .collect();
    // Only live records are answered; deleted and unknown IDs are omitted.
    let scopes = record_scopes(&repository, &record_ids).await?;
    let mut records: BTreeMap<Uuid, Vec<ExtensionRecordNotes>> =
        scopes.keys().map(|id| (*id, Vec::new())).collect();
    if servers.is_empty() || records.is_empty() {
        return Ok(Json(FieldNotesResponse {
            records,
            incomplete: false,
            client_sources,
        }));
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
    let workspace_id = repository.workspace_id();
    let cache_key = |installation: &ExtensionRuntimeInstallation, record_id: Uuid| -> CacheKey {
        (
            workspace_id,
            installation.installed_release_id,
            installation_fingerprint(installation),
            record_id,
            scopes[&record_id].fingerprint,
            context_id,
            locale.clone(),
        )
    };

    let mut asks = JoinSet::new();
    for installation in servers {
        let installation = Arc::new(installation);
        // Reuse what this provider answered for unchanged records.
        let mut missing = Vec::new();
        {
            let cache = CACHE.lock().expect("field notes cache");
            for record_id in scopes.keys() {
                match cache.get(&cache_key(&installation, *record_id)) {
                    Some((expires, notes)) if *expires > Instant::now() => {
                        records
                            .entry(*record_id)
                            .or_default()
                            .push(ExtensionRecordNotes {
                                extension_id: installation.extension_id.clone(),
                                extension_name: installation.manifest.name.clone(),
                                notes: notes.clone(),
                            });
                    }
                    _ => missing.push(*record_id),
                }
            }
        }
        for batch in missing.chunks(FIELD_NOTES_PER_CALL) {
            let request = json!({
                "records": batch.iter().map(|record_id| json!({
                    "record_id": record_id,
                    "blueprint_id": scopes[record_id].blueprint_id,
                    "blueprint_version": scopes[record_id].blueprint_version,
                })).collect::<Vec<_>>(),
                "context_id": context_id,
                "locale": locale,
            });
            let call = tokio::spawn(ask(
                state.clone(),
                repository.for_extension(&installation.extension_id),
                installation.clone(),
                request,
            ));
            let installation = installation.clone();
            asks.spawn(async move {
                (
                    installation,
                    tokio::time::timeout(FIELD_NOTES_TIMEOUT, call).await,
                )
            });
        }
    }

    let mut incomplete = false;
    while let Some(joined) = asks.join_next().await {
        let Ok((installation, answer)) = joined else {
            continue;
        };
        let answer = match answer {
            Ok(Ok(Ok(answer))) => answer,
            Err(_) => {
                incomplete = true;
                continue;
            }
            Ok(Ok(Err(error))) => {
                tracing::warn!(
                    extension = %installation.extension_id,
                    error = %error,
                    "extension field notes were left out"
                );
                continue;
            }
            Ok(Err(_)) => {
                tracing::warn!(
                    extension = %installation.extension_id,
                    "extension field notes task failed"
                );
                continue;
            }
        };
        let ttl = if answer.cacheable {
            CACHEABLE_FIELD_NOTES_TTL
        } else {
            FIELD_NOTES_CACHE_TTL
        };
        let mut fresh = Vec::new();
        for (record_id, notes) in answer.records {
            let Ok(record_id) = record_id.parse::<Uuid>() else {
                continue;
            };
            let Some(scope) = scopes.get(&record_id) else {
                continue;
            };
            let notes = belonging_to_record(&installation, notes, scope);
            fresh.push((cache_key(&installation, record_id), notes.clone()));
            records
                .entry(record_id)
                .or_default()
                .push(ExtensionRecordNotes {
                    extension_id: installation.extension_id.clone(),
                    extension_name: installation.manifest.name.clone(),
                    notes,
                });
        }
        remember(fresh, ttl);
    }
    for notes in records.values_mut() {
        notes.sort_by(|left, right| left.extension_id.cmp(&right.extension_id));
    }
    Ok(Json(FieldNotesResponse {
        records,
        incomplete,
        client_sources,
    }))
}

/// Each live record's revision, version, attribute codes and current files.
async fn record_scopes(
    repository: &AttricatRepository,
    record_ids: &[Uuid],
) -> Result<BTreeMap<Uuid, RecordScope>, ApiError> {
    let (versions, mut files, mut reusable) = tokio::try_join!(
        async { Ok::<_, ApiError>(repository.record_versions(record_ids).await?) },
        async { Ok(repository.records_file_ids(record_ids).await?) },
        async {
            Ok(repository
                .records_reusable_attribute_codes(record_ids)
                .await?)
        },
    )?;
    let mut revisions: HashMap<(Uuid, i64), HashSet<String>> = HashMap::new();
    let mut scopes = BTreeMap::new();
    for (record_id, blueprint_id, blueprint_version, version) in versions {
        if let Entry::Vacant(entry) = revisions.entry((blueprint_id, blueprint_version)) {
            let blueprint = repository
                .get_blueprint_revision(blueprint_id, blueprint_version)
                .await?
                .ok_or_else(|| ApiError::not_found("blueprint version"))?;
            entry.insert(
                blueprint
                    .attributes
                    .into_iter()
                    .map(|attribute| attribute.code)
                    .collect(),
            );
        }
        let mut attribute_codes = revisions[&(blueprint_id, blueprint_version)].clone();
        attribute_codes.extend(reusable.remove(&record_id).unwrap_or_default());
        let file_ids: HashSet<String> = files
            .remove(&record_id)
            .unwrap_or_default()
            .iter()
            .map(Uuid::to_string)
            .collect();
        let mut hasher = DefaultHasher::new();
        version.hash(&mut hasher);
        attribute_codes
            .iter()
            .collect::<BTreeSet<_>>()
            .hash(&mut hasher);
        file_ids.iter().collect::<BTreeSet<_>>().hash(&mut hasher);
        scopes.insert(
            record_id,
            RecordScope {
                blueprint_id,
                blueprint_version,
                attribute_codes,
                file_ids,
                fingerprint: hasher.finish(),
            },
        );
    }
    Ok(scopes)
}

fn remember(entries: Vec<(CacheKey, RecordNotes)>, ttl: Duration) {
    let mut cache = CACHE.lock().expect("field notes cache");
    let now = Instant::now();
    if cache.len() + entries.len() > MAX_CACHED_FIELD_NOTES {
        cache.retain(|_, (expires, _)| *expires > now);
        if cache.len() + entries.len() > MAX_CACHED_FIELD_NOTES {
            cache.clear();
        }
    }
    for (key, notes) in entries {
        cache.insert(key, (now + ttl, notes));
    }
}

/// Calls the provider's declared command and checks its answer against the
/// command's own response schema and the shared contract.
async fn ask(
    state: AppState,
    repository: AttricatRepository,
    installation: Arc<ExtensionRuntimeInstallation>,
    request: Value,
) -> Result<Answer, String> {
    let Some(FieldNotesSource::Command(command_id)) = installation
        .manifest
        .record_field_notes
        .as_ref()
        .map(|notes| &notes.source)
    else {
        return Err("no server field notes source".into());
    };
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

/// Keeps notes for the record's own attributes and files, states of the
/// extension's own actions, and badge actions that name one of its
/// badge-only actions.
fn belonging_to_record(
    installation: &ExtensionRuntimeInstallation,
    notes: RecordNotes,
    scope: &RecordScope,
) -> RecordNotes {
    let actions_at = |badge_only: bool| -> HashSet<&str> {
        installation
            .manifest
            .ui
            .iter()
            .filter(|item| {
                item.kind == UiContributionKind::Action
                    && (!badge_only || item.outlet == Some(UiOutlet::RecordAttributeAction))
            })
            .map(|item| item.id.as_str())
            .collect()
    };
    let badge_actions = actions_at(true);
    let actions = actions_at(false);
    let attributes = notes
        .attributes
        .into_iter()
        .filter(|(code, _)| scope.attribute_codes.contains(code))
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
    RecordNotes {
        attributes,
        files: notes
            .files
            .into_iter()
            .filter(|(file_id, _)| scope.file_ids.contains(file_id))
            .collect(),
        actions: notes
            .actions
            .into_iter()
            .filter(|(action, _)| actions.contains(action.as_str()))
            .collect(),
    }
}

/// The client component of an enabled extension's client-side field-notes
/// source, which the web app runs headless in a sandboxed frame. Like other
/// artifacts it is immutable per release and revalidated on every use.
pub(super) async fn client_source(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    ApiPath(extension_id): ApiPath<String>,
    request_headers: HeaderMap,
) -> Result<Response, ApiError> {
    let installation = repository
        .enabled_field_note_providers()
        .await?
        .into_iter()
        .find(|installation| installation.extension_id == extension_id)
        .ok_or_else(|| ApiError::not_found("field notes source"))?;
    let Some(FieldNotesSource::Client(artifact)) = installation
        .manifest
        .record_field_notes
        .as_ref()
        .map(|notes| &notes.source)
    else {
        return Err(ApiError::not_found("field notes source"));
    };
    let etag = conditional::entity_tag(&format!(
        "{}/field-notes/{artifact}",
        installation.installed_release_id
    ))
    .ok_or_else(|| ApiError::internal("field notes source tag is invalid"))?;
    if conditional::not_modified(&request_headers, &etag) {
        return Ok(conditional::not_modified_response(
            etag,
            conditional::REVALIDATE,
        ));
    }
    let body = super::extensions::stored_object_body(
        &state,
        &installed_artifact_key(installation.installed_release_id, artifact),
        "extension artifact storage is unavailable",
        "extension artifact could not be loaded",
    )
    .await?;
    let mut response = Response::new(body);
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/javascript; charset=utf-8"),
    );
    headers.insert(header::ETAG, etag);
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(conditional::REVALIDATE),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}
