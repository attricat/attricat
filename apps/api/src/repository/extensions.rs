use std::collections::HashSet;

use chrono::{DateTime, Utc};
use semver::{Version, VersionReq};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    extension_installer::installed_artifact_key,
    extensions::{Manifest, SUPPORTED_HOST_API, UiContributionKind, UiOutlet},
};

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
    pub actor_user_id: Option<Uuid>,
    pub actor_token_id: Option<Uuid>,
    pub source: Option<String>,
    pub diagnostics: Value,
    pub created_at: DateTime<Utc>,
}

/// Management-safe view of an installed release. The immutable manifest is
/// included so callers can show requested permissions before changing grants.
#[derive(Clone, Debug, FromRow)]
pub struct InstalledExtension {
    pub id: Uuid,
    pub extension_id: String,
    pub installed_release_id: Uuid,
    pub state: String,
    pub configuration: Value,
    pub configuration_version: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: String,
    pub manifest: Value,
    pub manifest_sha256: String,
    pub source: String,
}

#[derive(Clone, Debug, FromRow)]
pub struct ExtensionGrant {
    pub grant_kind: String,
    pub grant_id: String,
    pub granted_at: DateTime<Utc>,
}

/// Immutable snapshot plus invocation-time state used by the WASM runtime.
/// It deliberately contains grants, never secret values.
#[derive(Clone, Debug)]
pub struct ExtensionRuntimeInstallation {
    pub extension_id: String,
    pub installed_release_id: Uuid,
    pub configuration: Value,
    pub manifest: Manifest,
    pub capability_grants: HashSet<String>,
    pub host_permission_grants: HashSet<String>,
}

/// Client-safe descriptor for an enabled contribution. It deliberately omits
/// source identity, grants, server components, and secrets.
#[derive(Clone, Debug)]
pub struct ClientExtensionContribution {
    pub extension_id: String,
    pub installed_release_id: Uuid,
    pub configuration: Value,
    pub capabilities: Vec<String>,
    pub id: String,
    pub version: u32,
    pub kind: UiContributionKind,
    pub outlet: Option<UiOutlet>,
    pub title: Option<String>,
    pub element: String,
    pub artifact_key: String,
}

impl CatalogRepository {
    /// Lists installations with their immutable release metadata for management UI.
    pub async fn installed_extensions(&self) -> Result<Vec<InstalledExtension>, RepositoryError> {
        Ok(sqlx::query_as("SELECT i.id, i.extension_id, i.installed_release_id, i.state, i.configuration, i.configuration_version, i.created_at, i.updated_at, r.version, r.manifest, r.manifest_sha256, r.source FROM extension_installations i JOIN installed_extension_releases r ON r.id = i.installed_release_id WHERE i.workspace_id = $1 ORDER BY i.extension_id")
            .bind(self.extension_workspace()).fetch_all(&self.pool).await?)
    }

    pub async fn installed_extension(
        &self,
        extension_id: &str,
    ) -> Result<InstalledExtension, RepositoryError> {
        sqlx::query_as("SELECT i.id, i.extension_id, i.installed_release_id, i.state, i.configuration, i.configuration_version, i.created_at, i.updated_at, r.version, r.manifest, r.manifest_sha256, r.source FROM extension_installations i JOIN installed_extension_releases r ON r.id = i.installed_release_id WHERE i.workspace_id = $1 AND i.extension_id = $2")
            .bind(self.extension_workspace()).bind(extension_id).fetch_optional(&self.pool).await?
            .ok_or(RepositoryError::NotFound("extension installation"))
    }

    pub async fn extension_grants(
        &self,
        extension_id: &str,
    ) -> Result<Vec<ExtensionGrant>, RepositoryError> {
        let installation = self.installed_extension(extension_id).await?;
        Ok(sqlx::query_as("SELECT grant_kind, grant_id, granted_at FROM extension_grants WHERE installation_id = $1 ORDER BY grant_kind, grant_id")
            .bind(installation.id).fetch_all(&self.pool).await?)
    }

    /// Lists only enabled client contributions for the active workspace.
    /// Deserializing the immutable manifest here keeps runtime selection tied to
    /// the installed release rather than caller-controlled identifiers.
    pub async fn client_extension_contributions(
        &self,
    ) -> Result<Vec<ClientExtensionContribution>, RepositoryError> {
        let rows: Vec<(Uuid, String, Uuid, Value, Value)> = sqlx::query_as(
            "SELECT i.id, i.extension_id, i.installed_release_id, i.configuration, r.manifest FROM extension_installations i JOIN installed_extension_releases r ON r.id = i.installed_release_id WHERE i.workspace_id = $1 AND i.state = 'enabled' ORDER BY i.extension_id",
        )
        .bind(self.extension_workspace())
        .fetch_all(&self.pool)
        .await?;
        let mut contributions = Vec::new();
        for (installation_id, extension_id, installed_release_id, configuration, stored_manifest) in
            rows
        {
            let capabilities: Vec<String> = sqlx::query_scalar(
                "SELECT grant_id FROM extension_grants WHERE installation_id = $1 AND grant_kind = 'capability' ORDER BY grant_id",
            )
            .bind(installation_id)
            .fetch_all(&self.pool)
            .await?;
            let client_configuration = capabilities
                .iter()
                .any(|capability| capability == "configuration.read")
                .then_some(configuration.clone())
                .unwrap_or_else(|| json!({}));
            let manifest: Manifest = serde_json::from_value(stored_manifest).map_err(|_| {
                RepositoryError::InvalidExtension("stored extension manifest is invalid".into())
            })?;
            for contribution in manifest.ui {
                contributions.push(ClientExtensionContribution {
                    artifact_key: installed_artifact_key(
                        installed_release_id,
                        &contribution.artifact,
                    ),
                    extension_id: extension_id.clone(),
                    installed_release_id,
                    configuration: client_configuration.clone(),
                    capabilities: capabilities.clone(),
                    id: contribution.id,
                    version: contribution.version,
                    kind: contribution.kind,
                    outlet: contribution.outlet,
                    title: contribution.title,
                    element: contribution.element,
                });
            }
        }
        Ok(contributions)
    }

    /// Resolves an artifact only through an enabled declared client UI
    /// contribution. Callers never receive the storage key directly.
    pub async fn client_extension_contribution(
        &self,
        extension_id: &str,
        contribution_id: &str,
    ) -> Result<ClientExtensionContribution, RepositoryError> {
        self.client_extension_contributions()
            .await?
            .into_iter()
            .find(|contribution| {
                contribution.extension_id == extension_id && contribution.id == contribution_id
            })
            .ok_or(RepositoryError::NotFound("enabled extension contribution"))
    }

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

    /// Lists enabled handler candidates. A candidate is intentionally not an
    /// authorization decision: callers must use
    /// `runtime_extension_installation` directly before invocation.
    pub async fn enabled_extension_handlers(
        &self,
        event_type: &str,
    ) -> Result<Vec<ExtensionRuntimeInstallation>, RepositoryError> {
        let rows: Vec<(String, Uuid)> = sqlx::query_as(
            "SELECT extension_id, installed_release_id FROM extension_installations WHERE workspace_id = $1 AND state = 'enabled'",
        )
        .bind(self.extension_workspace())
        .fetch_all(&self.pool)
        .await?;
        let mut enabled = Vec::new();
        for (extension_id, installed_release_id) in rows {
            if let Some(installation) = self
                .runtime_extension_installation(&extension_id, installed_release_id)
                .await?
            {
                let subscribed = installation.manifest.server.as_ref().is_some_and(|server| {
                    server.event_handlers.iter().any(|handler| {
                        handler
                            .event_types
                            .iter()
                            .any(|registered| registered == event_type)
                    })
                });
                if subscribed {
                    enabled.push(installation);
                }
            }
        }
        Ok(enabled)
    }

    /// Re-fetches authorization data from the current installation immediately
    /// before a component runs. The expected release ID prevents a queued
    /// delivery from invoking a replacement release after an upgrade, disable,
    /// grant revocation, or configuration change.
    pub async fn runtime_extension_installation(
        &self,
        extension_id: &str,
        expected_release_id: Uuid,
    ) -> Result<Option<ExtensionRuntimeInstallation>, RepositoryError> {
        // Lock the installation while loading grants so an upgrade, disable,
        // configuration change, or revocation cannot produce a mixed snapshot.
        // The transaction commits before the untrusted invocation; lifecycle
        // operations therefore never wait on component execution.
        let mut transaction = self.pool.begin().await?;
        let row: Option<(Uuid, Value, Value)> = sqlx::query_as(
            "SELECT i.id, i.configuration, r.manifest FROM extension_installations i JOIN installed_extension_releases r ON r.id = i.installed_release_id WHERE i.workspace_id = $1 AND i.extension_id = $2 AND i.installed_release_id = $3 AND i.state = 'enabled' FOR UPDATE OF i",
        )
        .bind(self.extension_workspace())
        .bind(extension_id)
        .bind(expected_release_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((installation_id, configuration, manifest_value)) = row else {
            transaction.commit().await?;
            return Ok(None);
        };
        let manifest: Manifest = serde_json::from_value(manifest_value).map_err(|_| {
            RepositoryError::InvalidExtension("installed manifest cannot be decoded".into())
        })?;
        manifest
            .validate_configuration(&configuration)
            .map_err(|error| RepositoryError::InvalidExtension(error.to_string()))?;
        let grants: Vec<(String, String)> = sqlx::query_as(
            "SELECT grant_kind, grant_id FROM extension_grants WHERE installation_id = $1",
        )
        .bind(installation_id)
        .fetch_all(&mut *transaction)
        .await?;
        let capability_grants: HashSet<String> = grants
            .iter()
            .filter(|(kind, _)| kind == "capability")
            .map(|(_, id)| id.clone())
            .collect();
        let host_permission_grants: HashSet<String> = grants
            .iter()
            .filter(|(kind, _)| kind == "host_permission")
            .map(|(_, id)| id.clone())
            .collect();
        let authorized = manifest
            .permissions
            .iter()
            .all(|permission| capability_grants.contains(permission))
            && manifest
                .host_permissions
                .iter()
                .all(|permission| host_permission_grants.contains(&permission.id));
        transaction.commit().await?;
        if !authorized {
            return Ok(None);
        }
        Ok(Some(ExtensionRuntimeInstallation {
            extension_id: extension_id.to_owned(),
            installed_release_id: expected_release_id,
            configuration,
            manifest,
            capability_grants,
            host_permission_grants,
        }))
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
        Ok(sqlx::query_as("SELECT id, workspace_id, installation_id, extension_id, installed_release_id, operation, prior_state, new_state, outcome, actor_user_id, actor_token_id, source, diagnostics, created_at FROM extension_lifecycle_records WHERE workspace_id = $1 AND extension_id = $2 ORDER BY created_at, id")
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
    pub(in crate::repository) fn extension_workspace(&self) -> Uuid {
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
