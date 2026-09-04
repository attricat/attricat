use std::collections::HashSet;

use chrono::{DateTime, Utc};
use semver::{Version, VersionReq};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, Postgres, Transaction};
use uuid::Uuid;

use crate::extensions::{Manifest, SUPPORTED_HOST_API};

use super::{AuditContext, CatalogRepository, RepositoryError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExtensionState {
    Disabled,
    Enabled,
    Quarantined,
}
impl ExtensionState {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Enabled => "enabled",
            Self::Quarantined => "quarantined",
        }
    }
}

#[derive(Clone, Debug, FromRow)]
pub struct ExtensionInstallation {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub extension_id: String,
    pub installed_release_id: Uuid,
    pub state: String,
    pub configuration: Value,
    pub configuration_version: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, FromRow)]
pub struct ExtensionLifecycleRecord {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub installation_id: Option<Uuid>,
    pub extension_id: String,
    pub installed_release_id: Option<Uuid>,
    pub operation: String,
    pub prior_state: Option<String>,
    pub new_state: Option<String>,
    pub outcome: String,
    pub created_at: DateTime<Utc>,
}

impl CatalogRepository {
    /// Persists an archive release whose extracted artifacts were already
    /// validated and uploaded by the application installer.
    pub(crate) async fn install_extension(
        &self,
        manifest: &Manifest,
        source: &str,
        installed_release_id: Uuid,
    ) -> Result<ExtensionInstallation, RepositoryError> {
        self.validate_extension(manifest)?;
        self.validate_extension_source(source)?;
        let workspace_id = self.extension_workspace();
        let mut transaction = self.pool.begin().await?;
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM extension_installations WHERE workspace_id = $1 AND extension_id = $2 FOR UPDATE)")
            .bind(workspace_id).bind(&manifest.catalog.id).fetch_one(&mut *transaction).await?;
        if exists {
            return Err(RepositoryError::ExtensionAlreadyInstalled);
        }
        self.insert_installed_extension_release(
            &mut transaction,
            installed_release_id,
            manifest,
            source,
        )
        .await?;
        let installation_id = Uuid::new_v4();
        let configuration_version = manifest
            .configuration
            .as_ref()
            .map(|value| value.version as i32);
        let row = sqlx::query_as::<_, ExtensionInstallation>("INSERT INTO extension_installations (id, workspace_id, extension_id, installed_release_id, state, configuration, configuration_version) VALUES ($1, $2, $3, $4, 'disabled', '{}'::jsonb, $5) RETURNING id, workspace_id, extension_id, installed_release_id, state, configuration, configuration_version, created_at, updated_at")
            .bind(installation_id).bind(workspace_id).bind(&manifest.catalog.id).bind(installed_release_id).bind(configuration_version).fetch_one(&mut *transaction).await?;
        self.write_extension_lifecycle(
            &mut transaction,
            &row,
            "install",
            None,
            Some("disabled"),
            source,
            json!({}),
        )
        .await?;
        self.commit_extension_mutation(transaction, "install", &row.extension_id)
            .await?;
        Ok(row)
    }

    pub async fn configure_extension(
        &self,
        extension_id: &str,
        configuration: Value,
    ) -> Result<ExtensionInstallation, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let current = self.lock_extension(&mut transaction, extension_id).await?;
        let manifest = self
            .installed_release_manifest(&mut transaction, current.installed_release_id)
            .await?;
        manifest
            .validate_configuration(&configuration)
            .map_err(|error| RepositoryError::InvalidExtension(error.to_string()))?;
        let version = manifest
            .configuration
            .as_ref()
            .map(|item| item.version as i32);
        let row = sqlx::query_as::<_, ExtensionInstallation>("UPDATE extension_installations SET configuration = $2, configuration_version = $3, updated_at = clock_timestamp() WHERE id = $1 RETURNING id, workspace_id, extension_id, installed_release_id, state, configuration, configuration_version, created_at, updated_at")
            .bind(current.id).bind(configuration).bind(version).fetch_one(&mut *transaction).await?;
        self.write_extension_lifecycle(
            &mut transaction,
            &row,
            "configure",
            Some(&current.state),
            Some(&row.state),
            "configuration",
            json!({}),
        )
        .await?;
        self.commit_extension_mutation(transaction, "configure", &row.extension_id)
            .await?;
        Ok(row)
    }

    /// Grants either a requested capability or a declared host-permission rule.
    /// Optional and required entries use the same durable grant representation;
    /// enablement only requires the required subset.
    pub async fn grant_extension(
        &self,
        extension_id: &str,
        grant_kind: &str,
        grant_id: &str,
    ) -> Result<(), RepositoryError> {
        if !matches!(grant_kind, "capability" | "host_permission") {
            return Err(RepositoryError::InvalidExtension(
                "grant kind must be capability or host_permission".into(),
            ));
        }
        let mut transaction = self.pool.begin().await?;
        let current = self.lock_extension(&mut transaction, extension_id).await?;
        let manifest = self
            .installed_release_manifest(&mut transaction, current.installed_release_id)
            .await?;
        let declared = match grant_kind {
            "capability" => manifest
                .permissions
                .iter()
                .chain(&manifest.optional_permissions)
                .any(|item| item == grant_id),
            _ => manifest
                .host_permissions
                .iter()
                .chain(&manifest.optional_host_permissions)
                .any(|item| item.id == grant_id),
        };
        if !declared {
            return Err(RepositoryError::InvalidExtension(format!(
                "'{grant_id}' is not declared by this release"
            )));
        }
        sqlx::query("INSERT INTO extension_grants (id, installation_id, grant_kind, grant_id) VALUES ($1, $2, $3, $4) ON CONFLICT (installation_id, grant_kind, grant_id) DO NOTHING")
            .bind(Uuid::new_v4()).bind(current.id).bind(grant_kind).bind(grant_id).execute(&mut *transaction).await?;
        self.write_extension_lifecycle(
            &mut transaction,
            &current,
            "grant",
            Some(&current.state),
            Some(&current.state),
            grant_kind,
            json!({"grant_id": grant_id}),
        )
        .await?;
        self.commit_extension_mutation(transaction, "grant", &current.extension_id)
            .await
    }

    pub async fn revoke_extension_grant(
        &self,
        extension_id: &str,
        grant_kind: &str,
        grant_id: &str,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let current = self.lock_extension(&mut transaction, extension_id).await?;
        let manifest = self
            .installed_release_manifest(&mut transaction, current.installed_release_id)
            .await?;
        let required = match grant_kind {
            "capability" => manifest.permissions.iter().any(|item| item == grant_id),
            "host_permission" => manifest
                .host_permissions
                .iter()
                .any(|item| item.id == grant_id),
            _ => {
                return Err(RepositoryError::InvalidExtension(
                    "grant kind must be capability or host_permission".into(),
                ));
            }
        };
        if current.state == "enabled" && required {
            return Err(RepositoryError::InvalidExtensionTransition(
                "disable an extension before revoking a required grant",
            ));
        }
        sqlx::query("DELETE FROM extension_grants WHERE installation_id = $1 AND grant_kind = $2 AND grant_id = $3")
            .bind(current.id).bind(grant_kind).bind(grant_id).execute(&mut *transaction).await?;
        self.write_extension_lifecycle(
            &mut transaction,
            &current,
            "revoke",
            Some(&current.state),
            Some(&current.state),
            grant_kind,
            json!({"grant_id": grant_id}),
        )
        .await?;
        self.commit_extension_mutation(transaction, "revoke", &current.extension_id)
            .await
    }

    pub async fn enable_extension(
        &self,
        extension_id: &str,
    ) -> Result<ExtensionInstallation, RepositoryError> {
        let workspace_id = self.extension_workspace();
        self.dependencies_enabled(workspace_id, extension_id, &mut HashSet::new())
            .await?;
        let mut transaction = self.pool.begin().await?;
        let current = self.lock_extension(&mut transaction, extension_id).await?;
        if current.state == "quarantined" {
            return Err(RepositoryError::InvalidExtensionTransition(
                "a quarantined extension must be remediated to disabled before enabling",
            ));
        }
        if current.state == "enabled" {
            return Err(RepositoryError::InvalidExtensionTransition(
                "an extension is already enabled",
            ));
        }
        self.required_grants_present(&mut transaction, &current)
            .await?;
        let row = self
            .update_extension_state(&mut transaction, &current, ExtensionState::Enabled)
            .await?;
        self.write_extension_lifecycle(
            &mut transaction,
            &row,
            "enable",
            Some(&current.state),
            Some("enabled"),
            "lifecycle",
            json!({}),
        )
        .await?;
        self.commit_extension_mutation(transaction, "enable", &row.extension_id)
            .await?;
        Ok(row)
    }

    pub async fn disable_extension(
        &self,
        extension_id: &str,
    ) -> Result<ExtensionInstallation, RepositoryError> {
        self.transition_extension(extension_id, ExtensionState::Disabled, "disable")
            .await
    }

    /// Quarantine is legal from either installed state and blocks all future
    /// runtime/webhook policy checks until explicit remediation.
    pub async fn quarantine_extension(
        &self,
        extension_id: &str,
        diagnostic_code: &str,
    ) -> Result<ExtensionInstallation, RepositoryError> {
        if diagnostic_code.is_empty() || diagnostic_code.len() > 128 {
            return Err(RepositoryError::InvalidExtension(
                "quarantine diagnostic code is invalid".into(),
            ));
        }
        let mut transaction = self.pool.begin().await?;
        let current = self.lock_extension(&mut transaction, extension_id).await?;
        if current.state == "quarantined" {
            return Err(RepositoryError::InvalidExtensionTransition(
                "an extension is already quarantined",
            ));
        }
        let row = self
            .update_extension_state(&mut transaction, &current, ExtensionState::Quarantined)
            .await?;
        self.write_extension_lifecycle(
            &mut transaction,
            &row,
            "quarantine",
            Some(&current.state),
            Some("quarantined"),
            "runtime",
            json!({"code": diagnostic_code}),
        )
        .await?;
        self.commit_extension_mutation(transaction, "quarantine", &row.extension_id)
            .await?;
        Ok(row)
    }

    /// Persists an already extracted and validated replacement archive release.
    pub(crate) async fn upgrade_extension(
        &self,
        manifest: &Manifest,
        source: &str,
        installed_release_id: Uuid,
    ) -> Result<ExtensionInstallation, RepositoryError> {
        self.validate_extension(manifest)?;
        self.validate_extension_source(source)?;
        let mut transaction = self.pool.begin().await?;
        let current = self
            .lock_extension(&mut transaction, &manifest.catalog.id)
            .await?;
        let prior_manifest = self
            .installed_release_manifest(&mut transaction, current.installed_release_id)
            .await?;
        let prior_version = Version::parse(&prior_manifest.version).map_err(|_| {
            RepositoryError::InvalidExtension("stored extension version is invalid".into())
        })?;
        let next_version = Version::parse(&manifest.version).map_err(|_| {
            RepositoryError::InvalidExtension("extension version is invalid".into())
        })?;
        if next_version <= prior_version {
            return Err(RepositoryError::InvalidExtension(
                "upgrade version must be greater than the installed release".into(),
            ));
        }
        self.insert_installed_extension_release(
            &mut transaction,
            installed_release_id,
            manifest,
            source,
        )
        .await?;
        let state = if current.state == "enabled" {
            "disabled"
        } else {
            &current.state
        };
        let row = sqlx::query_as::<_, ExtensionInstallation>("UPDATE extension_installations SET installed_release_id = $2, state = $3, configuration = '{}'::jsonb, configuration_version = $4, updated_at = clock_timestamp() WHERE id = $1 RETURNING id, workspace_id, extension_id, installed_release_id, state, configuration, configuration_version, created_at, updated_at")
            .bind(current.id).bind(installed_release_id).bind(state).bind(manifest.configuration.as_ref().map(|item| item.version as i32)).fetch_one(&mut *transaction).await?;
        sqlx::query("DELETE FROM extension_grants WHERE installation_id = $1")
            .bind(current.id)
            .execute(&mut *transaction)
            .await?;
        self.write_extension_lifecycle(
            &mut transaction,
            &row,
            "upgrade",
            Some(&current.state),
            Some(&row.state),
            source,
            json!({"prior_installed_release_id": current.installed_release_id}),
        )
        .await?;
        self.commit_extension_mutation(transaction, "upgrade", &row.extension_id)
            .await?;
        Ok(row)
    }

    pub async fn remove_extension(&self, extension_id: &str) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let current = self.lock_extension(&mut transaction, extension_id).await?;
        self.write_extension_lifecycle(
            &mut transaction,
            &current,
            "remove",
            Some(&current.state),
            None,
            "lifecycle",
            json!({}),
        )
        .await?;
        sqlx::query("DELETE FROM extension_installations WHERE id = $1")
            .bind(current.id)
            .execute(&mut *transaction)
            .await?;
        self.commit_extension_mutation(transaction, "remove", &current.extension_id)
            .await
    }

    pub async fn extension_lifecycle_history(
        &self,
        extension_id: &str,
    ) -> Result<Vec<ExtensionLifecycleRecord>, RepositoryError> {
        Ok(sqlx::query_as("SELECT id, workspace_id, installation_id, extension_id, installed_release_id, operation, prior_state, new_state, outcome, created_at FROM extension_lifecycle_records WHERE workspace_id = $1 AND extension_id = $2 ORDER BY created_at, id")
            .bind(self.extension_workspace()).bind(extension_id).fetch_all(&self.pool).await?)
    }

    async fn transition_extension(
        &self,
        extension_id: &str,
        state: ExtensionState,
        operation: &str,
    ) -> Result<ExtensionInstallation, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let current = self.lock_extension(&mut transaction, extension_id).await?;
        if current.state == state.as_str() {
            return Err(RepositoryError::InvalidExtensionTransition(
                "extension is already in the requested state",
            ));
        }
        // Leaving quarantine is remediation, not an unchecked state flip.
        if current.state == "quarantined" && state == ExtensionState::Disabled {
            let manifest = self
                .installed_release_manifest(&mut transaction, current.installed_release_id)
                .await?;
            self.validate_extension(&manifest)?;
            manifest
                .validate_configuration(&current.configuration)
                .map_err(|error| RepositoryError::InvalidExtension(error.to_string()))?;
        }
        let row = self
            .update_extension_state(&mut transaction, &current, state.clone())
            .await?;
        self.write_extension_lifecycle(
            &mut transaction,
            &row,
            operation,
            Some(&current.state),
            Some(state.as_str()),
            "lifecycle",
            json!({}),
        )
        .await?;
        self.commit_extension_mutation(transaction, operation, &row.extension_id)
            .await?;
        Ok(row)
    }

    async fn commit_extension_mutation(
        &self,
        mut transaction: Transaction<'_, Postgres>,
        operation: &str,
        extension_id: &str,
    ) -> Result<(), RepositoryError> {
        if self.audit_context.is_some() {
            self.write_audit_event(&mut transaction).await?;
        } else {
            let mut system_repository = self.clone();
            system_repository.audit_context = Some(AuditContext {
                actor_user_id: None,
                actor_token_id: None,
                request_id: Uuid::new_v4(),
                correlation_id: Uuid::new_v4(),
                action: format!("extension.{operation}"),
                authorization_scope: json!({"type": "workspace"}),
                target: json!({"type": "extension", "id": extension_id}),
                metadata: json!({"operation": operation}),
                agent: None,
            });
            system_repository
                .write_audit_event(&mut transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    fn validate_extension(&self, manifest: &Manifest) -> Result<(), RepositoryError> {
        manifest
            .validate(SUPPORTED_HOST_API)
            .map_err(|error| RepositoryError::InvalidExtension(error.to_string()))
    }
    fn validate_extension_source(&self, source: &str) -> Result<(), RepositoryError> {
        if source.trim().is_empty() || source.len() > 512 {
            return Err(RepositoryError::InvalidExtension(
                "source must be non-empty and at most 512 bytes".into(),
            ));
        }
        Ok(())
    }
    fn extension_workspace(&self) -> Uuid {
        self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID)
    }

    async fn lock_extension(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        extension_id: &str,
    ) -> Result<ExtensionInstallation, RepositoryError> {
        sqlx::query_as("SELECT id, workspace_id, extension_id, installed_release_id, state, configuration, configuration_version, created_at, updated_at FROM extension_installations WHERE workspace_id = $1 AND extension_id = $2 FOR UPDATE")
            .bind(self.extension_workspace()).bind(extension_id).fetch_optional(&mut **transaction).await?.ok_or(RepositoryError::NotFound("extension installation"))
    }
    async fn installed_release_manifest(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        installed_release_id: Uuid,
    ) -> Result<Manifest, RepositoryError> {
        let value: Value =
            sqlx::query_scalar("SELECT manifest FROM installed_extension_releases WHERE id = $1")
                .bind(installed_release_id)
                .fetch_one(&mut **transaction)
                .await?;
        serde_json::from_value(value)
            .map_err(|_| RepositoryError::InvalidExtension("stored manifest is invalid".into()))
    }
    async fn insert_installed_extension_release(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        installed_release_id: Uuid,
        manifest: &Manifest,
        source: &str,
    ) -> Result<(), RepositoryError> {
        let serialized = serde_json::to_value(manifest)
            .map_err(|error| RepositoryError::InvalidExtension(error.to_string()))?;
        let bytes = serde_json::to_vec(&serialized)
            .map_err(|error| RepositoryError::InvalidExtension(error.to_string()))?;
        let digest = format!("{:x}", Sha256::digest(bytes));
        sqlx::query("INSERT INTO installed_extension_releases (id, workspace_id, extension_id, version, manifest, manifest_sha256, source) VALUES ($1, $2, $3, $4, $5, $6, $7)")
            .bind(installed_release_id).bind(self.extension_workspace()).bind(&manifest.catalog.id).bind(&manifest.version).bind(serialized).bind(digest).bind(source).execute(&mut **transaction).await?;
        Ok(())
    }
    async fn update_extension_state(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        current: &ExtensionInstallation,
        state: ExtensionState,
    ) -> Result<ExtensionInstallation, RepositoryError> {
        sqlx::query_as("UPDATE extension_installations SET state = $2, updated_at = clock_timestamp() WHERE id = $1 RETURNING id, workspace_id, extension_id, installed_release_id, state, configuration, configuration_version, created_at, updated_at")
            .bind(current.id).bind(state.as_str()).fetch_one(&mut **transaction).await.map_err(Into::into)
    }
    async fn write_extension_lifecycle(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        installation: &ExtensionInstallation,
        operation: &str,
        prior_state: Option<&str>,
        new_state: Option<&str>,
        source: &str,
        diagnostics: Value,
    ) -> Result<(), RepositoryError> {
        let audit = self.audit_context.as_ref();
        sqlx::query("INSERT INTO extension_lifecycle_records (id, workspace_id, installation_id, extension_id, installed_release_id, operation, prior_state, new_state, outcome, actor_user_id, actor_token_id, request_id, correlation_id, source, diagnostics) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'success', $9, $10, $11, $12, $13, $14)")
            .bind(Uuid::new_v4()).bind(installation.workspace_id).bind(installation.id).bind(&installation.extension_id).bind(installation.installed_release_id).bind(operation).bind(prior_state).bind(new_state).bind(audit.and_then(|item| item.actor_user_id)).bind(audit.and_then(|item| item.actor_token_id)).bind(audit.map(|item| item.request_id).unwrap_or_else(Uuid::new_v4)).bind(audit.map(|item| item.correlation_id).unwrap_or_else(Uuid::new_v4)).bind(source).bind(diagnostics).execute(&mut **transaction).await?;
        Ok(())
    }
    async fn required_grants_present(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        installation: &ExtensionInstallation,
    ) -> Result<(), RepositoryError> {
        let manifest = self
            .installed_release_manifest(transaction, installation.installed_release_id)
            .await?;
        let grants: Vec<(String, String)> = sqlx::query_as(
            "SELECT grant_kind, grant_id FROM extension_grants WHERE installation_id = $1",
        )
        .bind(installation.id)
        .fetch_all(&mut **transaction)
        .await?;
        for id in &manifest.permissions {
            if !grants
                .iter()
                .any(|(kind, granted)| kind == "capability" && granted == id)
            {
                return Err(RepositoryError::InvalidExtension(format!(
                    "required capability '{id}' has not been granted"
                )));
            }
        }
        for permission in &manifest.host_permissions {
            if !grants
                .iter()
                .any(|(kind, granted)| kind == "host_permission" && granted == &permission.id)
            {
                return Err(RepositoryError::InvalidExtension(format!(
                    "required host permission '{}' has not been granted",
                    permission.id
                )));
            }
        }
        manifest
            .validate_configuration(&installation.configuration)
            .map_err(|error| RepositoryError::InvalidExtension(error.to_string()))
    }
    #[async_recursion::async_recursion]
    async fn dependencies_enabled(
        &self,
        workspace_id: Uuid,
        extension_id: &str,
        visiting: &mut HashSet<String>,
    ) -> Result<(), RepositoryError> {
        if !visiting.insert(extension_id.to_owned()) {
            return Err(RepositoryError::InvalidExtension("dependency cycle".into()));
        }
        let row: Option<(String, Uuid)> = sqlx::query_as("SELECT state, installed_release_id FROM extension_installations WHERE workspace_id = $1 AND extension_id = $2").bind(workspace_id).bind(extension_id).fetch_optional(&self.pool).await?;
        let Some((state, installed_release_id)) = row else {
            return Err(RepositoryError::NotFound("extension installation"));
        };
        let raw: Value =
            sqlx::query_scalar("SELECT manifest FROM installed_extension_releases WHERE id = $1")
                .bind(installed_release_id)
                .fetch_one(&self.pool)
                .await?;
        let manifest: Manifest = serde_json::from_value(raw)
            .map_err(|_| RepositoryError::InvalidExtension("stored manifest is invalid".into()))?;
        for dependency in &manifest.dependencies {
            let dependency_row: Option<(String, String, Uuid)> = sqlx::query_as("SELECT i.state, r.version, i.installed_release_id FROM extension_installations i JOIN installed_extension_releases r ON r.id = i.installed_release_id WHERE i.workspace_id = $1 AND i.extension_id = $2").bind(workspace_id).bind(&dependency.id).fetch_optional(&self.pool).await?;
            let Some((dependency_state, version, _)) = dependency_row else {
                return Err(RepositoryError::InvalidExtension(format!(
                    "dependency '{}' is not installed",
                    dependency.id
                )));
            };
            let range = VersionReq::parse(&dependency.version).map_err(|_| {
                RepositoryError::InvalidExtension("stored dependency range is invalid".into())
            })?;
            let version = Version::parse(&version).map_err(|_| {
                RepositoryError::InvalidExtension("stored dependency version is invalid".into())
            })?;
            if dependency_state != "enabled" || !range.matches(&version) {
                return Err(RepositoryError::InvalidExtension(format!(
                    "dependency '{}' is not enabled at a compatible version",
                    dependency.id
                )));
            }
            self.dependencies_enabled(workspace_id, &dependency.id, visiting)
                .await?;
        }
        visiting.remove(extension_id);
        if state == "quarantined" {
            return Err(RepositoryError::InvalidExtension(format!(
                "dependency '{extension_id}' is quarantined"
            )));
        }
        Ok(())
    }
}
