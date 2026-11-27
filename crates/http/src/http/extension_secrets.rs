//! Secrets of installed extensions: management of named, write-only values a
//! component reads through `secrets.get.v1` or names in mediated requests.

use std::collections::HashMap;

use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{
    auth::ScopedRepository,
    error::ApiError,
    extractors::{ApiJson, ApiPath},
};
use crate::{
    extensions::{Manifest, SecretDeclaration},
    repository::ExtensionSecret,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SecretRequest {
    value: String,
}

/// A secret the installed release declares, one an operator has set, or
/// both. Never carries a value.
#[derive(Serialize)]
pub(super) struct SecretResponse {
    name: String,
    /// Whether the installed release's manifest declares the secret.
    declared: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    required: bool,
    /// Whether an operator has set a value.
    set: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    created_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<DateTime<Utc>>,
}

fn declarations(manifest: &serde_json::Value) -> Result<Vec<SecretDeclaration>, ApiError> {
    let manifest: Manifest = serde::Deserialize::deserialize(manifest)
        .map_err(|_| ApiError::invalid_input("installed extension manifest is invalid".into()))?;
    Ok(manifest
        .server
        .map(|server| server.secrets)
        .unwrap_or_default())
}

/// Declared secrets in manifest order, then the undeclared ones that are set,
/// by name.
fn merge(declared: Vec<SecretDeclaration>, stored: Vec<ExtensionSecret>) -> Vec<SecretResponse> {
    let mut stored: HashMap<String, ExtensionSecret> = stored
        .into_iter()
        .map(|secret| (secret.name.clone(), secret))
        .collect();
    let mut secrets: Vec<SecretResponse> = declared
        .into_iter()
        .map(|declaration| {
            let set = stored.remove(&declaration.name);
            SecretResponse {
                declared: true,
                title: Some(declaration.title),
                description: declaration.description,
                required: declaration.required,
                set: set.is_some(),
                created_at: set.as_ref().map(|secret| secret.created_at),
                updated_at: set.as_ref().map(|secret| secret.updated_at),
                name: declaration.name,
            }
        })
        .collect();
    let mut undeclared: Vec<ExtensionSecret> = stored.into_values().collect();
    undeclared.sort_by(|left, right| left.name.cmp(&right.name));
    secrets.extend(undeclared.into_iter().map(|secret| SecretResponse {
        name: secret.name,
        declared: false,
        title: None,
        description: None,
        required: false,
        set: true,
        created_at: Some(secret.created_at),
        updated_at: Some(secret.updated_at),
    }));
    secrets
}

/// `GET /extensions/{extension_id}/secrets`: the installed release's declared
/// secrets and every secret set for the extension, by name only.
pub(super) async fn list(
    ScopedRepository(repository): ScopedRepository,
    ApiPath(extension_id): ApiPath<String>,
) -> Result<axum::Json<Vec<SecretResponse>>, ApiError> {
    let installed = repository.installed_extension(&extension_id).await?;
    let declared = declarations(&installed.manifest)?;
    let stored = repository.extension_secrets(&extension_id).await?;
    Ok(axum::Json(merge(declared, stored)))
}

/// `PUT /extensions/{extension_id}/secrets/{name}`: sets or replaces a value.
pub(super) async fn put(
    ScopedRepository(repository): ScopedRepository,
    ApiPath((extension_id, name)): ApiPath<(String, String)>,
    ApiJson(input): ApiJson<SecretRequest>,
) -> Result<StatusCode, ApiError> {
    repository
        .put_extension_secret(&extension_id, &name, &input.value)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /extensions/{extension_id}/secrets/{name}`
pub(super) async fn delete(
    ScopedRepository(repository): ScopedRepository,
    ApiPath((extension_id, name)): ApiPath<(String, String)>,
) -> Result<StatusCode, ApiError> {
    if repository
        .delete_extension_secret(&extension_id, &name)
        .await?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("extension secret"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declaration(name: &str, required: bool) -> SecretDeclaration {
        SecretDeclaration {
            name: name.into(),
            title: name.to_uppercase(),
            description: None,
            required,
        }
    }

    fn stored(name: &str) -> ExtensionSecret {
        ExtensionSecret {
            name: name.into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn lists_declared_secrets_first_then_undeclared_ones_by_name() {
        let merged = merge(
            vec![
                declaration("token", true),
                declaration("signing-key", false),
            ],
            vec![stored("zeta"), stored("signing-key"), stored("alpha")],
        );
        let names: Vec<_> = merged.iter().map(|secret| secret.name.as_str()).collect();
        assert_eq!(names, ["token", "signing-key", "alpha", "zeta"]);
        assert!(merged[0].declared && merged[0].required && !merged[0].set);
        assert!(merged[1].declared && merged[1].set && merged[1].updated_at.is_some());
        assert!(!merged[2].declared && merged[2].set && merged[2].title.is_none());
    }
}
