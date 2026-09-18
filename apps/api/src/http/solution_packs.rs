use axum::{
    Json,
    extract::rejection::BytesRejection,
    http::{HeaderMap, StatusCode, header},
};
use bytes::Bytes;
use serde::Serialize;

use super::{auth::ScopedRepository, error::ApiError};
use crate::solution_packs::{
    MAX_SOLUTION_PACK_INSPECTION_RESPONSE_BYTES, SolutionPackResource, ValidatedSolutionPack,
};

#[derive(Serialize)]
pub(super) struct InspectionResponse {
    archive_sha256: String,
    manifest: ManifestSummary,
    resources: ResourceSummaries,
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
    contexts: Vec<ContextSummary>,
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
struct ContextSummary {
    key: String,
    code: String,
    required: bool,
    sha256: String,
    parent: String,
}

/// Validates and summarizes an uploaded solution-pack archive without storing
/// the archive or applying any resources to the workspace.
pub(super) async fn inspect(
    ScopedRepository(_repository): ScopedRepository,
    headers: HeaderMap,
    archive: Result<Bytes, BytesRejection>,
) -> Result<(StatusCode, Json<InspectionResponse>), ApiError> {
    let media_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim);
    if !media_type.is_some_and(|value| value.eq_ignore_ascii_case("application/zstd")) {
        return Err(ApiError::unsupported_media_type());
    }

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
    let contexts = manifest
        .resources
        .contexts
        .iter()
        .map(|resource| context_summary(&pack, resource))
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
            contexts,
        },
    };
    ensure_response_size(&response)?;
    Ok((StatusCode::OK, Json(response)))
}

fn ensure_response_size(response: &InspectionResponse) -> Result<(), ApiError> {
    let response_bytes = serde_json::to_vec(response).map_err(|_| {
        ApiError::internal("solution-pack inspection response could not be encoded")
    })?;
    if response_bytes.len() > MAX_SOLUTION_PACK_INSPECTION_RESPONSE_BYTES {
        return Err(ApiError::invalid_input(
            "solution-pack inspection summary exceeds the size limit".to_owned(),
        ));
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

fn context_summary(
    pack: &ValidatedSolutionPack,
    resource: &SolutionPackResource,
) -> ContextSummary {
    let context = pack
        .context(&resource.key)
        .expect("validated context resource is available");
    ContextSummary {
        key: resource.key.clone(),
        code: context.code.clone(),
        required: resource.required,
        sha256: resource.sha256.clone(),
        parent: context.parent.clone(),
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
                contexts: Vec::new(),
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
}
