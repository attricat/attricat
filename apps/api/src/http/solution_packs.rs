use axum::{
    Json,
    extract::{
        Path, Query,
        rejection::{BytesRejection, PathRejection, QueryRejection},
    },
    http::{HeaderMap, StatusCode, header},
};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{auth::ScopedRepository, error::ApiError};
use crate::{
    repository::{
        SolutionPackApplication, SolutionPackApplicationSummary, SolutionPackCheckRun,
        SolutionPackCheckRunSummary, SolutionPackPlan,
    },
    solution_packs::{
        BlueprintPublication, MAX_SOLUTION_PACK_CHECK_RUN_RESPONSE_BYTES,
        MAX_SOLUTION_PACK_INSPECTION_RESPONSE_BYTES, MAX_SOLUTION_PACK_PLAN_RESPONSE_BYTES,
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

/// Validates and summarizes an uploaded solution-pack archive without storing
/// the archive or applying any resources to the workspace.
pub(super) async fn inspect(
    ScopedRepository(_repository): ScopedRepository,
    headers: HeaderMap,
    archive: Result<Bytes, BytesRejection>,
) -> Result<(StatusCode, Json<InspectionResponse>), ApiError> {
    require_zstd(&headers)?;

    let archive = archive.map_err(ApiError::from_bytes_rejection)?;
    let pack = ValidatedSolutionPack::from_tar_zst(&archive)
        .map_err(|error| ApiError::invalid_input(error.to_string()))?;
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
    };
    ensure_response_size(&response)?;
    Ok((StatusCode::OK, Json(response)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreatePlanQuery {
    prefix: String,
    blueprint_publication: BlueprintPublication,
}

/// Validates an uploaded archive again and persists an immutable create-only dry-run.
pub(super) async fn create_plan(
    ScopedRepository(repository): ScopedRepository,
    query: Result<Query<CreatePlanQuery>, QueryRejection>,
    headers: HeaderMap,
    archive: Result<Bytes, BytesRejection>,
) -> Result<(StatusCode, Json<SolutionPackPlan>), ApiError> {
    require_zstd(&headers)?;
    let Query(query) = query.map_err(ApiError::from_query_rejection)?;
    let archive = archive.map_err(ApiError::from_bytes_rejection)?;
    let pack = ValidatedSolutionPack::from_tar_zst(&archive)
        .map_err(|error| ApiError::invalid_input(error.to_string()))?;
    let plan = repository
        .create_solution_pack_plan(&pack, &query.prefix, query.blueprint_publication)
        .await?;
    // The repository validated this exact public representation before commit.
    Ok((StatusCode::CREATED, Json(plan)))
}

pub(super) async fn apply_plan(
    ScopedRepository(repository): ScopedRepository,
    plan_id: Result<Path<Uuid>, PathRejection>,
) -> Result<Json<SolutionPackApplication>, ApiError> {
    let Path(plan_id) = plan_id.map_err(ApiError::from_path_rejection)?;
    let application = repository.apply_solution_pack_plan(plan_id).await?;
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

fn ensure_response_size(response: &InspectionResponse) -> Result<(), ApiError> {
    ensure_encoded_response_size(
        response,
        MAX_SOLUTION_PACK_INSPECTION_RESPONSE_BYTES,
        "solution-pack inspection summary exceeds the size limit",
    )
}

fn ensure_application_response_size(response: &impl Serialize) -> Result<(), ApiError> {
    ensure_encoded_response_size(
        response,
        MAX_SOLUTION_PACK_PLAN_RESPONSE_BYTES,
        "solution-pack application summary exceeds the size limit",
    )
}

fn ensure_check_response_size(response: &impl Serialize) -> Result<(), ApiError> {
    ensure_encoded_response_size(
        response,
        MAX_SOLUTION_PACK_CHECK_RUN_RESPONSE_BYTES,
        "solution-pack check response exceeds the size limit",
    )
}

fn ensure_plan_response_size(response: &SolutionPackPlan) -> Result<(), ApiError> {
    ensure_encoded_response_size(
        response,
        MAX_SOLUTION_PACK_PLAN_RESPONSE_BYTES,
        "solution-pack plan summary exceeds the size limit",
    )
}

fn ensure_encoded_response_size(
    response: &impl Serialize,
    limit: usize,
    message: &'static str,
) -> Result<(), ApiError> {
    let response_bytes = serde_json::to_vec(response)
        .map_err(|_| ApiError::internal("solution-pack response could not be encoded"))?;
    if response_bytes.len() > limit {
        return Err(ApiError::invalid_input(message.to_owned()));
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
            },
            extensions: Vec::new(),
            guidance: GuidanceInspectionSummary {
                readme_bytes: None,
                release_notes_bytes: None,
                setup_checklist_items: 0,
                checks: Vec::new(),
            },
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
