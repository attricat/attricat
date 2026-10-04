use serde_json::Value;
use uuid::Uuid;

use crate::extensions::{ConfigurationScope, validate_schema};

use super::{CatalogRepository, RepositoryError};

/// A manifest-declared extension value associated with a blueprint revision or
/// an attribute. It is never merged into Catalog's blueprint definition.
#[derive(Clone, Debug)]
pub struct ExtensionConfigurationScope {
    pub kind: ConfigurationScope,
    pub blueprint_id: Uuid,
    pub blueprint_version: i64,
    pub attribute_id: Option<Uuid>,
}

impl CatalogRepository {
    pub async fn extension_scoped_configuration_get(
        &self,
        extension_id: &str,
        expected_release_id: Uuid,
        scope: &ExtensionConfigurationScope,
    ) -> Result<Option<Value>, RepositoryError> {
        self.require_scoped_configuration_access(extension_id, expected_release_id, scope)
            .await?;
        Ok(sqlx::query_scalar(
            "SELECT configuration FROM extension_scoped_configuration WHERE workspace_id = $1 AND extension_id = $2 AND installed_release_id = $3 AND scope_kind = $4 AND blueprint_id = $5 AND blueprint_version = $6 AND attribute_id IS NOT DISTINCT FROM $7",
        )
        .bind(self.extension_workspace())
        .bind(extension_id)
        .bind(expected_release_id)
        .bind(scope_kind(&scope.kind))
        .bind(scope.blueprint_id)
        .bind(scope.blueprint_version)
        .bind(scope.attribute_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn extension_scoped_configuration_set(
        &self,
        extension_id: &str,
        expected_release_id: Uuid,
        scope: &ExtensionConfigurationScope,
        configuration: Value,
    ) -> Result<(), RepositoryError> {
        let installation = self
            .require_scoped_configuration_access(extension_id, expected_release_id, scope)
            .await?;
        let declaration = installation
            .manifest
            .scoped_configuration
            .as_ref()
            .ok_or_else(|| {
                RepositoryError::InvalidExtension(
                    "extension does not declare scoped configuration".into(),
                )
            })?;
        validate_schema(&declaration.schema, &configuration)
            .map_err(|error| RepositoryError::InvalidExtension(error.to_string()))?;
        // Lifecycle and grant mutations lock this installation row. Hold that
        // lock through the write and recheck access after acquiring it, so a
        // stale invocation cannot restore configuration for a revoked release.
        let mut transaction = self.pool.begin().await?;
        let installation_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT i.id FROM extension_installations i JOIN workspaces w ON w.id = i.workspace_id WHERE i.workspace_id = $1 AND i.extension_id = $2 AND i.installed_release_id = $3 AND i.state = 'enabled' AND w.extensions_enabled FOR UPDATE OF i FOR SHARE OF w",
        )
        .bind(self.extension_workspace())
        .bind(extension_id)
        .bind(expected_release_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let installation_id = installation_id.ok_or_else(|| {
            RepositoryError::InvalidExtension("extension invocation is no longer authorized".into())
        })?;
        let granted: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM extension_grants WHERE installation_id = $1 AND grant_kind = 'capability' AND grant_id = 'configuration.write')",
        )
        .bind(installation_id)
        .fetch_one(&mut *transaction)
        .await?;
        if !granted {
            return Err(RepositoryError::InvalidExtension(
                "scoped configuration access is denied".into(),
            ));
        }
        sqlx::query(
            "INSERT INTO extension_scoped_configuration (workspace_id, extension_id, installed_release_id, scope_kind, blueprint_id, blueprint_version, attribute_id, configuration, configuration_version) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT (workspace_id, extension_id, scope_kind, blueprint_id, blueprint_version, attribute_id) DO UPDATE SET installed_release_id = EXCLUDED.installed_release_id, configuration = EXCLUDED.configuration, configuration_version = EXCLUDED.configuration_version, updated_at = clock_timestamp()",
        )
        .bind(self.extension_workspace())
        .bind(extension_id)
        .bind(expected_release_id)
        .bind(scope_kind(&scope.kind))
        .bind(scope.blueprint_id)
        .bind(scope.blueprint_version)
        .bind(scope.attribute_id)
        .bind(configuration)
        .bind(declaration.version as i32)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    async fn require_scoped_configuration_access(
        &self,
        extension_id: &str,
        expected_release_id: Uuid,
        scope: &ExtensionConfigurationScope,
    ) -> Result<super::ExtensionRuntimeInstallation, RepositoryError> {
        let installation = self
            .runtime_extension_installation(extension_id, expected_release_id)
            .await?
            .ok_or_else(|| {
                RepositoryError::InvalidExtension(
                    "extension invocation is no longer authorized".into(),
                )
            })?;
        if !installation
            .capability_grants
            .contains("configuration.write")
        {
            return Err(RepositoryError::InvalidExtension(
                "scoped configuration access is denied".into(),
            ));
        }
        let declaration = installation
            .manifest
            .scoped_configuration
            .as_ref()
            .ok_or_else(|| {
                RepositoryError::InvalidExtension(
                    "extension does not declare scoped configuration".into(),
                )
            })?;
        if !declaration.scopes.contains(&scope.kind)
            || scope.blueprint_version <= 0
            || (scope.kind == ConfigurationScope::Blueprint) != scope.attribute_id.is_none()
        {
            return Err(RepositoryError::InvalidExtension(
                "invalid extension configuration scope".into(),
            ));
        }
        let blueprint_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM blueprints WHERE id = $1 AND version = $2 AND workspace_id = $3 AND deleted_at IS NULL)",
        )
        .bind(scope.blueprint_id)
        .bind(scope.blueprint_version)
        .bind(self.extension_workspace())
        .fetch_one(&self.pool)
        .await?;
        if !blueprint_exists {
            return Err(RepositoryError::NotFound("blueprint revision"));
        }
        if let Some(attribute_id) = scope.attribute_id {
            let owns_attribute: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM attributes WHERE id = $1 AND blueprint_id = $2 AND blueprint_version = $3 AND workspace_id = $4 AND deleted_at IS NULL)",
            )
            .bind(attribute_id)
            .bind(scope.blueprint_id)
            .bind(scope.blueprint_version)
            .bind(self.extension_workspace())
            .fetch_one(&self.pool)
            .await?;
            if !owns_attribute {
                return Err(RepositoryError::NotFound("blueprint attribute"));
            }
        }
        Ok(installation)
    }
}

fn scope_kind(scope: &ConfigurationScope) -> &'static str {
    match scope {
        ConfigurationScope::Blueprint => "blueprint",
        ConfigurationScope::Attribute => "attribute",
    }
}
