use axum::{
    Json,
    body::to_bytes,
    extract::{
        FromRequest, Multipart, Path, Query, Request, State,
        rejection::{BytesRejection, PathRejection, QueryRejection},
    },
    http::{HeaderMap, StatusCode, header},
};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AppState, auth::ScopedRepository, error::ApiError};
use crate::{
    repository::{
        CreateSolutionPackPlanRequest, SolutionPackApplication, SolutionPackApplicationSummary,
        SolutionPackCheckRun, SolutionPackCheckRunSummary, SolutionPackPlan,
    },
    solution_pack_sample_data::SAMPLE_AUTOMATION_WARNING,
    solution_packs::{
        BlueprintMappingRequest, BlueprintPublication, MAX_SOLUTION_PACK_ARCHIVE_BYTES,
        MAX_SOLUTION_PACK_BLUEPRINTS, MAX_SOLUTION_PACK_CHECK_RUN_RESPONSE_BYTES,
        MAX_SOLUTION_PACK_INSPECTION_RESPONSE_BYTES, MAX_SOLUTION_PACK_PLAN_RESPONSE_BYTES,
        MAX_SOLUTION_PACK_PRESENTATION_ASSETS, PresentationAssetMappingRequest,
        SolutionPackExtensionRequirement, SolutionPackResource, ValidatedSolutionPack,
    },
};

#[derive(Serialize)]
pub(super) struct InspectionResponse {
    archive_sha256: String,
    manifest: ManifestSummary,
    resources: ResourceSummaries,
    extensions: Vec<ExtensionRequirementSummary>,
    guidance: GuidanceInspectionSummary,
    sample_data: Option<SampleDataInspectionSummary>,
    warnings: Vec<&'static str>,
}

#[derive(Serialize)]
struct SampleDataInspectionSummary {
    key: String,
    classification: &'static str,
    entity_count: usize,
    scalar_fact_count: usize,
    relationship_fact_count: usize,
    canonical_sha256: String,
}

#[derive(Serialize)]
struct ManifestSummary {
    manifest_version: u32,
    id: String,
    name: String,
    version: String,
    description: String,
    catalog: CatalogSummary,
}

#[derive(Serialize)]
struct CatalogSummary {
    host_api: String,
}

#[derive(Serialize)]
struct ResourceSummaries {
    blueprints: Vec<BlueprintSummary>,
    workspace_settings: Vec<WorkspaceSettingSummary>,
    presentation_assets: Vec<PresentationAssetSummary>,
}

#[derive(Serialize)]
struct PresentationAssetSummary {
    key: String,
    purpose: String,
    media_type: String,
    required: bool,
    source_sha256: String,
    stored_sha256: String,
    source_byte_size: usize,
    stored_byte_size: usize,
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(Serialize)]
struct BlueprintSummary {
    key: String,
    code: String,
    required: bool,
    sha256: String,
    includes: Vec<BlueprintIncludeSummary>,
}

#[derive(Serialize)]
struct BlueprintIncludeSummary {
    alias: String,
    key: String,
}

#[derive(Serialize)]
struct WorkspaceSettingSummary {
    key: String,
    required: bool,
    sha256: String,
    kind: String,
    entry_count: usize,
}

#[derive(Serialize)]
struct ExtensionRequirementSummary {
    key: String,
    id: String,
    version: String,
    required: bool,
    configuration_template: Option<ConfigurationTemplateSummary>,
}

#[derive(Serialize)]
struct ConfigurationTemplateSummary {
    path: String,
    sha256: String,
}

#[derive(Serialize)]
struct GuidanceInspectionSummary {
    readme_bytes: Option<usize>,
    release_notes_bytes: Option<usize>,
    setup_checklist_items: usize,
    checks: Vec<CheckInspectionSummary>,
}

#[derive(Serialize)]
struct CheckInspectionSummary {
    key: String,
    title: String,
    predicate_type: String,
}

/// Decompresses and validates a solution-pack archive off the async executor.
async fn unpack_solution_pack(
    archive: impl Into<Bytes>,
) -> Result<ValidatedSolutionPack, ApiError> {
    let archive = archive.into();
    super::run_blocking(move || ValidatedSolutionPack::from_tar_zst(&archive))
        .await
        .map_err(|error| ApiError::invalid_input(error.to_string()))
}

/// Validates and summarizes an uploaded solution-pack archive without storing
/// the archive or applying any resources to the workspace.
pub(super) async fn inspect(
    ScopedRepository(_repository): ScopedRepository,
    headers: HeaderMap,
    archive: Result<Bytes, BytesRejection>,
) -> Result<(StatusCode, Json<InspectionResponse>), ApiError> {
    require_zstd(&headers)?;

    let archive = archive.map_err(ApiError::from_bytes_rejection)?;
    let pack = unpack_solution_pack(archive).await?;
    let manifest = pack.manifest();
    let blueprints = manifest
        .resources
        .blueprints
        .iter()
        .map(|resource| blueprint_summary(&pack, resource))
        .collect();
    let response = InspectionResponse {
        archive_sha256: pack.archive_sha256().to_owned(),
        manifest: ManifestSummary {
            manifest_version: manifest.manifest_version,
            id: manifest.id.clone(),
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            description: manifest.description.clone(),
            catalog: CatalogSummary {
                host_api: manifest.catalog.host_api.clone(),
            },
        },
        resources: ResourceSummaries {
            blueprints,
            presentation_assets: manifest
                .resources
                .presentation_assets
                .iter()
                .map(|resource| {
                    let asset = pack
                        .presentation_asset(&resource.key)
                        .expect("validated presentation asset is available");
                    PresentationAssetSummary {
                        key: resource.key.clone(),
                        purpose: resource.purpose.clone(),
                        media_type: resource.media_type.clone(),
                        required: resource.required,
                        source_sha256: asset.source_sha256.clone(),
                        stored_sha256: asset.stored_sha256.clone(),
                        source_byte_size: asset.source_byte_size,
                        stored_byte_size: asset.stored_bytes.len(),
                        width: asset.width,
                        height: asset.height,
                    }
                })
                .collect(),
            workspace_settings: manifest
                .resources
                .workspace_settings
                .iter()
                .map(|resource| {
                    let (kind, entry_count) = if resource.key == "workspace/explore-navigation" {
                        (
                            "explore_navigation",
                            pack.explore_navigation()
                                .expect("validated workspace setting is available")
                                .entries
                                .len(),
                        )
                    } else if resource.key == "workspace/lexicon" {
                        (
                            "lexicon",
                            pack.lexicon()
                                .expect("validated workspace setting is available")
                                .len(),
                        )
                    } else {
                        (
                            "extension_layout",
                            pack.extension_layout()
                                .expect("validated workspace setting is available")
                                .entries
                                .len(),
                        )
                    };
                    WorkspaceSettingSummary {
                        key: resource.key.clone(),
                        required: resource.required,
                        sha256: resource.sha256.clone(),
                        kind: kind.to_owned(),
                        entry_count,
                    }
                })
                .collect(),
        },
        extensions: manifest
            .extensions
            .iter()
            .map(extension_requirement_summary)
            .collect(),
        guidance: GuidanceInspectionSummary {
            readme_bytes: pack.guidance().readme_markdown.as_ref().map(String::len),
            release_notes_bytes: pack
                .guidance()
                .release_notes_markdown
                .as_ref()
                .map(String::len),
            setup_checklist_items: pack
                .guidance()
                .setup_checklist
                .as_ref()
                .map_or(0, |checklist| checklist.items.len()),
            checks: pack
                .checks()
                .iter()
                .map(|check| CheckInspectionSummary {
                    key: check.key.clone(),
                    title: check.title.clone(),
                    predicate_type: check.predicate.predicate_type().to_owned(),
                })
                .collect(),
        },
        sample_data: pack
            .sample_data()
            .map(|sample| SampleDataInspectionSummary {
                key: "sample-data/default".to_owned(),
                classification: "synthetic",
                entity_count: sample.declaration.entities.len(),
                scalar_fact_count: sample.scalar_fact_count,
                relationship_fact_count: sample.relationship_fact_count,
                canonical_sha256: sample.canonical_sha256.clone(),
            }),
        warnings: pack
            .sample_data()
            .map(|_| vec![SAMPLE_AUTOMATION_WARNING])
            .unwrap_or_default(),
    };
    ensure_response_size(&response)?;
    Ok((StatusCode::OK, Json(response)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreatePlanQuery {
    prefix: String,
    blueprint_publication: BlueprintPublication,
    from_application: Option<Uuid>,
    #[serde(default)]
    include_sample_data: bool,
}

/// Validates an uploaded archive again and persists an immutable dry-run. Raw
/// zstd remains the no-choice protocol; multipart adds only explicit blueprint
/// selections and still streams the archive into a bounded buffer.
pub(super) async fn create_plan(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    query: Result<Query<CreatePlanQuery>, QueryRejection>,
    request: Request,
) -> Result<(StatusCode, Json<SolutionPackPlan>), ApiError> {
    let Query(query) = query.map_err(ApiError::from_query_rejection)?;
    let (archive, mappings, asset_mappings) = parse_plan_request(request).await?;
    if query.from_application.is_some() && (!mappings.is_empty() || !asset_mappings.is_empty()) {
        return Err(ApiError::invalid_input(
            "from_application and explicit mappings are mutually exclusive".into(),
        ));
    }
    let pack = unpack_solution_pack(archive).await?;
    let plan = repository
        .create_solution_pack_plan(
            &pack,
            CreateSolutionPackPlanRequest {
                prefix: &query.prefix,
                publication: query.blueprint_publication,
                blueprint_mappings: &mappings,
                asset_mappings: &asset_mappings,
                prior_application_id: query.from_application,
                include_sample_data: query.include_sample_data,
            },
            state.object_store.as_ref(),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(plan)))
}

async fn parse_plan_request(
    request: Request,
) -> Result<
    (
        Bytes,
        Vec<BlueprintMappingRequest>,
        Vec<PresentationAssetMappingRequest>,
    ),
    ApiError,
> {
    let media_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim);
    if media_type.is_some_and(|value| value.eq_ignore_ascii_case("application/zstd")) {
        let bytes = to_bytes(request.into_body(), MAX_SOLUTION_PACK_ARCHIVE_BYTES)
            .await
            .map_err(|_| ApiError::payload_too_large())?;
        return Ok((bytes, Vec::new(), Vec::new()));
    }
    if !media_type.is_some_and(|value| value.eq_ignore_ascii_case("multipart/form-data")) {
        return Err(ApiError::unsupported_media_type());
    }

    let mut multipart = Multipart::from_request(request, &())
        .await
        .map_err(|error| {
            if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
                ApiError::payload_too_large()
            } else {
                ApiError::invalid_input("invalid solution-pack multipart request".into())
            }
        })?;
    let mut archive = None;
    let mut mappings = Vec::new();
    let mut asset_mappings = Vec::new();
    let mut metadata_bytes = 0usize;
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|error| multipart_error(error, "invalid solution-pack multipart request"))?
    {
        match field.name() {
            Some("archive") => {
                if archive.is_some() {
                    return Err(ApiError::invalid_input(
                        "multipart request contains duplicate archive parts".into(),
                    ));
                }
                if field.content_type() != Some("application/zstd") {
                    return Err(ApiError::unsupported_media_type());
                }
                let mut bytes = Vec::new();
                while let Some(chunk) = field
                    .chunk()
                    .await
                    .map_err(|error| multipart_error(error, "invalid solution-pack archive part"))?
                {
                    if bytes.len().saturating_add(chunk.len()) > MAX_SOLUTION_PACK_ARCHIVE_BYTES {
                        return Err(ApiError::payload_too_large());
                    }
                    bytes.extend_from_slice(&chunk);
                }
                archive = Some(Bytes::from(bytes));
            }
            Some("asset_map") => {
                if asset_mappings.len() >= MAX_SOLUTION_PACK_PRESENTATION_ASSETS {
                    return Err(ApiError::invalid_input(
                        "too many presentation asset mappings".into(),
                    ));
                }
                let mut bytes = Vec::new();
                while let Some(chunk) = field.chunk().await.map_err(|error| {
                    multipart_error(error, "invalid presentation asset mapping part")
                })? {
                    if metadata_bytes
                        .saturating_add(bytes.len())
                        .saturating_add(chunk.len())
                        > 64 * 1024
                    {
                        return Err(ApiError::invalid_input(
                            "mapping metadata exceeds the size limit".into(),
                        ));
                    }
                    bytes.extend_from_slice(&chunk);
                }
                metadata_bytes += bytes.len();
                let text = std::str::from_utf8(&bytes).map_err(|_| {
                    ApiError::invalid_input("presentation asset mapping must be UTF-8".into())
                })?;
                asset_mappings.push(serde_json::from_str(text).map_err(|_| {
                    ApiError::invalid_input(
                        "presentation asset mapping must be a {key,id} JSON object".into(),
                    )
                })?);
            }
            Some("blueprint_map") => {
                if mappings.len() >= MAX_SOLUTION_PACK_BLUEPRINTS {
                    return Err(ApiError::invalid_input(
                        "too many blueprint mappings".into(),
                    ));
                }
                let mut bytes = Vec::new();
                while let Some(chunk) = field
                    .chunk()
                    .await
                    .map_err(|error| multipart_error(error, "invalid blueprint mapping part"))?
                {
                    if metadata_bytes
                        .saturating_add(bytes.len())
                        .saturating_add(chunk.len())
                        > 64 * 1024
                    {
                        return Err(ApiError::invalid_input(
                            "blueprint mapping metadata exceeds the size limit".into(),
                        ));
                    }
                    bytes.extend_from_slice(&chunk);
                }
                metadata_bytes += bytes.len();
                let text = std::str::from_utf8(&bytes).map_err(|_| {
                    ApiError::invalid_input("blueprint mapping must be UTF-8".into())
                })?;
                mappings.push(serde_json::from_str(text).map_err(|_| {
                    ApiError::invalid_input(
                        "blueprint mapping must be a {key,code} JSON object".into(),
                    )
                })?);
            }
            _ => {
                return Err(ApiError::invalid_input(
                    "unknown solution-pack multipart part".into(),
                ));
            }
        }
    }
    let archive = archive.ok_or_else(|| {
        ApiError::invalid_input("multipart request is missing the archive part".into())
    })?;
    Ok((archive, mappings, asset_mappings))
}

fn multipart_error(
    error: axum::extract::multipart::MultipartError,
    message: &'static str,
) -> ApiError {
    if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        ApiError::payload_too_large()
    } else {
        ApiError::invalid_input(message.into())
    }
}

pub(super) async fn apply_plan(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    plan_id: Result<Path<Uuid>, PathRejection>,
) -> Result<Json<SolutionPackApplication>, ApiError> {
    let Path(plan_id) = plan_id.map_err(ApiError::from_path_rejection)?;
    let application = repository
        .apply_solution_pack_plan(plan_id, state.object_store.as_ref())
        .await?;
    ensure_application_response_size(&application)?;
    Ok(Json(application))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListApplicationsQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

pub(super) async fn list_applications(
    ScopedRepository(repository): ScopedRepository,
    query: Result<Query<ListApplicationsQuery>, QueryRejection>,
) -> Result<Json<Vec<SolutionPackApplicationSummary>>, ApiError> {
    let Query(query) = query.map_err(ApiError::from_query_rejection)?;
    let limit = query.limit.unwrap_or(25);
    let offset = query.offset.unwrap_or(0);
    if !(1..=100).contains(&limit) || !(0..=10_000).contains(&offset) {
        return Err(ApiError::invalid_input(
            "limit must be 1-100 and offset must be 0-10000".to_owned(),
        ));
    }
    let applications = repository
        .list_solution_pack_applications(limit, offset)
        .await?;
    ensure_application_response_size(&applications)?;
    Ok(Json(applications))
}

pub(super) async fn get_application(
    ScopedRepository(repository): ScopedRepository,
    application_id: Result<Path<Uuid>, PathRejection>,
) -> Result<Json<SolutionPackApplication>, ApiError> {
    let Path(application_id) = application_id.map_err(ApiError::from_path_rejection)?;
    let application = repository
        .get_solution_pack_application(application_id)
        .await?
        .ok_or_else(|| ApiError::not_found("solution-pack application"))?;
    ensure_application_response_size(&application)?;
    Ok(Json(application))
}

pub(super) async fn abandon_application(
    ScopedRepository(repository): ScopedRepository,
    application_id: Result<Path<Uuid>, PathRejection>,
    body: Result<Bytes, BytesRejection>,
) -> Result<Json<SolutionPackApplication>, ApiError> {
    let Path(application_id) = application_id.map_err(ApiError::from_path_rejection)?;
    if !body.map_err(ApiError::from_bytes_rejection)?.is_empty() {
        return Err(ApiError::invalid_input(
            "solution-pack abandonment body must be empty".to_owned(),
        ));
    }
    let application = repository
        .abandon_solution_pack_application(application_id)
        .await?;
    ensure_application_response_size(&application)?;
    Ok(Json(application))
}

pub(super) async fn rerun_checks(
    ScopedRepository(repository): ScopedRepository,
    application_id: Result<Path<Uuid>, PathRejection>,
    body: Result<Bytes, BytesRejection>,
) -> Result<(StatusCode, Json<SolutionPackCheckRun>), ApiError> {
    let Path(application_id) = application_id.map_err(ApiError::from_path_rejection)?;
    if !body.map_err(ApiError::from_bytes_rejection)?.is_empty() {
        return Err(ApiError::invalid_input(
            "solution-pack check rerun body must be empty".to_owned(),
        ));
    }
    let run = repository
        .rerun_solution_pack_checks(application_id)
        .await?;
    ensure_check_response_size(&run)?;
    Ok((StatusCode::CREATED, Json(run)))
}

pub(super) async fn list_check_runs(
    ScopedRepository(repository): ScopedRepository,
    application_id: Result<Path<Uuid>, PathRejection>,
    query: Result<Query<ListApplicationsQuery>, QueryRejection>,
) -> Result<Json<Vec<SolutionPackCheckRunSummary>>, ApiError> {
    let Path(application_id) = application_id.map_err(ApiError::from_path_rejection)?;
    let Query(query) = query.map_err(ApiError::from_query_rejection)?;
    let limit = query.limit.unwrap_or(25);
    let offset = query.offset.unwrap_or(0);
    if !(1..=100).contains(&limit) || !(0..=10_000).contains(&offset) {
        return Err(ApiError::invalid_input(
            "limit must be 1-100 and offset must be 0-10000".to_owned(),
        ));
    }
    let runs = repository
        .list_solution_pack_check_runs(application_id, limit, offset)
        .await?;
    ensure_check_response_size(&runs)?;
    Ok(Json(runs))
}

pub(super) async fn get_check_run(
    ScopedRepository(repository): ScopedRepository,
    path: Result<Path<(Uuid, Uuid)>, PathRejection>,
) -> Result<Json<SolutionPackCheckRun>, ApiError> {
    let Path((application_id, run_id)) = path.map_err(ApiError::from_path_rejection)?;
    let run = repository
        .get_solution_pack_check_run(application_id, run_id)
        .await?
        .ok_or_else(|| ApiError::not_found("solution-pack check run"))?;
    ensure_check_response_size(&run)?;
    Ok(Json(run))
}

pub(super) async fn get_plan(
    ScopedRepository(repository): ScopedRepository,
    plan_id: Result<Path<Uuid>, PathRejection>,
) -> Result<Json<SolutionPackPlan>, ApiError> {
    let Path(plan_id) = plan_id.map_err(ApiError::from_path_rejection)?;
    let plan = repository
        .get_solution_pack_plan(plan_id)
        .await?
        .ok_or_else(|| ApiError::not_found("solution-pack plan"))?;
    ensure_plan_response_size(&plan)?;
    Ok(Json(plan))
}

fn require_zstd(headers: &HeaderMap) -> Result<(), ApiError> {
    let media_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim);
    if !media_type.is_some_and(|value| value.eq_ignore_ascii_case("application/zstd")) {
        return Err(ApiError::unsupported_media_type());
    }
    Ok(())
}

/// The inspection summary is derived from the uploaded archive, so an
/// oversized one is the client's input problem.
fn ensure_response_size(response: &InspectionResponse) -> Result<(), ApiError> {
    ensure_encoded_response_size(
        response,
        MAX_SOLUTION_PACK_INSPECTION_RESPONSE_BYTES,
        ApiError::invalid_input(
            "solution-pack inspection summary exceeds the size limit".to_owned(),
        ),
    )
}

// The remaining guards cover server-stored state (possibly after a committed
// mutation), so exceeding them is a server fault rather than a client error.
fn ensure_application_response_size(response: &impl Serialize) -> Result<(), ApiError> {
    ensure_encoded_response_size(
        response,
        MAX_SOLUTION_PACK_PLAN_RESPONSE_BYTES,
        ApiError::internal("solution-pack application summary exceeds the size limit"),
    )
}

fn ensure_check_response_size(response: &impl Serialize) -> Result<(), ApiError> {
    ensure_encoded_response_size(
        response,
        MAX_SOLUTION_PACK_CHECK_RUN_RESPONSE_BYTES,
        ApiError::internal("solution-pack check response exceeds the size limit"),
    )
}

fn ensure_plan_response_size(response: &SolutionPackPlan) -> Result<(), ApiError> {
    ensure_encoded_response_size(
        response,
        MAX_SOLUTION_PACK_PLAN_RESPONSE_BYTES,
        ApiError::internal("solution-pack plan summary exceeds the size limit"),
    )
}

fn ensure_encoded_response_size(
    response: &impl Serialize,
    limit: usize,
    exceeded: ApiError,
) -> Result<(), ApiError> {
    let response_bytes = serde_json::to_vec(response)
        .map_err(|_| ApiError::internal("solution-pack response could not be encoded"))?;
    if response_bytes.len() > limit {
        return Err(exceeded);
    }
    Ok(())
}

fn blueprint_summary(
    pack: &ValidatedSolutionPack,
    resource: &SolutionPackResource,
) -> BlueprintSummary {
    let blueprint = pack
        .blueprint(&resource.key)
        .expect("validated blueprint resource is available");
    BlueprintSummary {
        key: resource.key.clone(),
        code: blueprint.code().to_owned(),
        required: resource.required,
        sha256: resource.sha256.clone(),
        includes: blueprint
            .includes()
            .iter()
            .map(|include| BlueprintIncludeSummary {
                alias: include.alias().to_owned(),
                key: include.key().to_owned(),
            })
            .collect(),
    }
}

fn extension_requirement_summary(
    requirement: &SolutionPackExtensionRequirement,
) -> ExtensionRequirementSummary {
    ExtensionRequirementSummary {
        key: requirement.key.clone(),
        id: requirement.id.clone(),
        version: requirement.version.clone(),
        required: requirement.required,
        configuration_template: requirement.configuration_template.as_ref().map(|template| {
            ConfigurationTemplateSummary {
                path: template.path.clone(),
                sha256: template.sha256.clone(),
            }
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response_with_description(description: String) -> InspectionResponse {
        InspectionResponse {
            archive_sha256: "0".repeat(64),
            manifest: ManifestSummary {
                manifest_version: 1,
                id: "attricat.test".to_owned(),
                name: "Test".to_owned(),
                version: "1.0.0".to_owned(),
                description,
                catalog: CatalogSummary {
                    host_api: "^1.0".to_owned(),
                },
            },
            resources: ResourceSummaries {
                blueprints: Vec::new(),
                workspace_settings: Vec::new(),
                presentation_assets: Vec::new(),
            },
            extensions: Vec::new(),
            guidance: GuidanceInspectionSummary {
                readme_bytes: None,
                release_notes_bytes: None,
                setup_checklist_items: 0,
                checks: Vec::new(),
            },
            sample_data: None,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn inspection_response_size_guard_rejects_oversized_json() {
        assert!(ensure_response_size(&response_with_description("small".into())).is_ok());
        assert!(
            ensure_response_size(&response_with_description(
                "x".repeat(MAX_SOLUTION_PACK_INSPECTION_RESPONSE_BYTES)
            ))
            .is_err()
        );
    }

    #[test]
    fn check_response_guard_accepts_maximum_bounded_result_evidence() {
        let now = chrono::Utc::now();
        let results = (0..64)
            .map(|position| crate::repository::SolutionPackCheckResult {
                position,
                key: format!("checks/check_{position}"),
                title: "x".repeat(200),
                predicate_type: "extension_installed".to_owned(),
                passed: true,
                reason_code: "installed".to_owned(),
                summary: "Check passed.".to_owned(),
                evidence: serde_json::json!({"bounded":"x".repeat(3900)}),
                evaluated_at: now,
            })
            .collect();
        let run = SolutionPackCheckRun {
            summary: SolutionPackCheckRunSummary {
                id: Uuid::nil(),
                application_id: Uuid::nil(),
                request_id: Uuid::nil(),
                correlation_id: Uuid::nil(),
                trigger: "manual".to_owned(),
                total_count: 64,
                passed_count: 64,
                failed_count: 0,
                started_at: now,
                completed_at: now,
            },
            results,
        };
        assert!(serde_json::to_vec(&run).unwrap().len() > 256 * 1024);
        assert!(ensure_check_response_size(&run).is_ok());
    }
}
