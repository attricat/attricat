//! Each workspace's OpenID Connect provider. Configuration reaches here from a
//! workspace administrator or the deployment operator, and sign-in reads it
//! only through [`AttricatRepository::identity_provider_for_sign_in`].

use std::sync::OnceLock;

use attricat_domain::states::PasswordSignIn;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    AttricatRepository, RepositoryError,
    sealing::{SEALING_KEY_BYTES, open, seal},
};

const UPDATED_ACTION: &str = "workspace.identity_provider.update";
const DELETED_ACTION: &str = "workspace.identity_provider.delete";

static CREDENTIAL_KEY: OnceLock<[u8; SEALING_KEY_BYTES]> = OnceLock::new();

/// Installs the process-wide key that encrypts provider client secrets
/// (`identity_provider_credential_key`). Later calls are ignored.
pub fn use_identity_provider_credential_key(key: [u8; SEALING_KEY_BYTES]) {
    let _ = CREDENTIAL_KEY.set(key);
}

fn secret_context(workspace_id: Uuid) -> String {
    format!("attricat.identity_provider:{workspace_id}:client_secret")
}

/// A provider as administrators see it: never its secret.
#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct IdentityProvider {
    pub issuer: String,
    pub client_id: String,
    pub display_name: String,
    pub verified_at: Option<DateTime<Utc>>,
    pub enabled: bool,
    #[sqlx(try_from = "String")]
    pub password_sign_in: PasswordSignIn,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// What a sign-in needs, including the decrypted client secret. It is not
/// serializable and its `Debug` leaves the secret out.
pub struct IdentityProviderSignIn {
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
    pub enabled: bool,
    pub verified: bool,
}

impl std::fmt::Debug for IdentityProviderSignIn {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IdentityProviderSignIn")
            .field("issuer", &self.issuer)
            .field("client_id", &self.client_id)
            .field("enabled", &self.enabled)
            .field("verified", &self.verified)
            .finish_non_exhaustive()
    }
}

#[derive(sqlx::FromRow)]
struct SignInRow {
    issuer: String,
    client_id: String,
    client_secret_ciphertext: Vec<u8>,
    enabled: bool,
    verified_at: Option<DateTime<Utc>>,
}

/// What the unauthenticated sign-in page may learn about a workspace.
#[derive(Clone, Debug)]
pub struct SignInOptions {
    /// The enabled provider's display name.
    pub provider: Option<String>,
    pub password_sign_in: PasswordSignIn,
}

pub struct IdentityProviderInput<'a> {
    pub issuer: &'a str,
    pub client_id: &'a str,
    /// Required for a new provider, issuer or client; otherwise `None` keeps
    /// the stored secret.
    pub client_secret: Option<&'a str>,
    pub display_name: &'a str,
}

fn audit_target(workspace_id: Uuid) -> Value {
    json!({ "type": "workspace", "id": workspace_id })
}

impl AttricatRepository {
    /// Returns a copy that audits `action` as the request's member, or as the
    /// deployment operator, whose requests carry no member context.
    fn audited_as(&self, action: &str) -> Self {
        let metadata = if self.audit_context.is_some() {
            json!({})
        } else {
            json!({ "actor": super::operator_workspaces::OPERATOR_ACTOR })
        };
        self.with_workspace_audit_action(action, audit_target(self.workspace_id()), metadata)
    }

    pub async fn identity_provider(&self) -> Result<Option<IdentityProvider>, RepositoryError> {
        Ok(sqlx::query_as("SELECT issuer, client_id, display_name, verified_at, enabled, password_sign_in, created_at, updated_at FROM workspace_identity_providers WHERE workspace_id = $1")
            .bind(self.workspace_id())
            .fetch_optional(&self.pool)
            .await?)
    }

    /// Creates or replaces the provider. Pointing it at a different issuer or
    /// client makes it unproven, so it goes back to unverified and off, and
    /// password sign-in returns for everyone rather than locking people out.
    pub async fn put_identity_provider(
        &self,
        input: &IdentityProviderInput<'_>,
    ) -> Result<IdentityProvider, RepositoryError> {
        let workspace_id = self.workspace_id();
        let sealed = input
            .client_secret
            .map(|secret| {
                let key = CREDENTIAL_KEY
                    .get()
                    .ok_or(RepositoryError::IdentityProviderKeyMissing)?;
                seal(key, &secret_context(workspace_id), secret)
                    .ok_or(RepositoryError::IdentityProviderKeyMissing)
            })
            .transpose()?;
        let mut tx = self.pool.begin().await?;
        let current: Option<(String, String)> = sqlx::query_as(
            "SELECT issuer, client_id FROM workspace_identity_providers WHERE workspace_id = $1 FOR UPDATE",
        )
        .bind(workspace_id)
        .fetch_optional(&mut *tx)
        .await?;
        // The token request sends the secret to the issuer, so keeping it for a
        // different issuer or client would hand it to whoever runs that one.
        let same_client = current.as_ref().is_some_and(|(issuer, client_id)| {
            issuer == input.issuer && client_id == input.client_id
        });
        if !same_client && sealed.is_none() {
            return Err(RepositoryError::IdentityProviderSecretRequired);
        }
        let provider: IdentityProvider = sqlx::query_as(
            "INSERT INTO workspace_identity_providers AS p (workspace_id, issuer, client_id, client_secret_ciphertext, display_name) \
             VALUES ($1, $2, $3, COALESCE($4, ''::bytea), $5) \
             ON CONFLICT (workspace_id) DO UPDATE SET \
               issuer = EXCLUDED.issuer, client_id = EXCLUDED.client_id, \
               client_secret_ciphertext = COALESCE($4, p.client_secret_ciphertext), \
               display_name = EXCLUDED.display_name, \
               verified_at = CASE WHEN p.issuer = EXCLUDED.issuer AND p.client_id = EXCLUDED.client_id THEN p.verified_at END, \
               enabled = p.enabled AND p.issuer = EXCLUDED.issuer AND p.client_id = EXCLUDED.client_id, \
               password_sign_in = CASE WHEN p.issuer = EXCLUDED.issuer AND p.client_id = EXCLUDED.client_id THEN p.password_sign_in ELSE $6 END, \
               updated_at = clock_timestamp() \
             RETURNING issuer, client_id, display_name, verified_at, enabled, password_sign_in, created_at, updated_at",
        )
        .bind(workspace_id)
        .bind(input.issuer)
        .bind(input.client_id)
        .bind(sealed)
        .bind(input.display_name)
        .bind(PasswordSignIn::All.as_str())
        .fetch_one(&mut *tx)
        .await?;
        self.audited_as(UPDATED_ACTION).commit_mutation(tx).await?;
        Ok(provider)
    }

    /// Turns the provider on or off and sets who may use a password. Turning
    /// it off restores password sign-in for everyone.
    pub async fn update_identity_provider_state(
        &self,
        enabled: Option<bool>,
        password_sign_in: Option<PasswordSignIn>,
    ) -> Result<IdentityProvider, RepositoryError> {
        let workspace_id = self.workspace_id();
        let mut tx = self.pool.begin().await?;
        let current: Option<(Option<DateTime<Utc>>, bool, String)> = sqlx::query_as(
            "SELECT verified_at, enabled, password_sign_in FROM workspace_identity_providers WHERE workspace_id = $1 FOR UPDATE",
        )
        .bind(workspace_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((verified_at, current_enabled, current_policy)) = current else {
            return Err(RepositoryError::NotFound("identity provider"));
        };
        let enabled = enabled.unwrap_or(current_enabled);
        if enabled && verified_at.is_none() {
            return Err(RepositoryError::IdentityProviderNotVerified);
        }
        let requested = password_sign_in.map(PasswordSignIn::as_str);
        if requested == Some(PasswordSignIn::Owners.as_str()) && !enabled {
            return Err(RepositoryError::IdentityProviderNotEnabled);
        }
        let policy = if enabled {
            requested.unwrap_or(&current_policy)
        } else {
            PasswordSignIn::All.as_str()
        };
        let provider: IdentityProvider = sqlx::query_as(
            "UPDATE workspace_identity_providers SET enabled = $2, password_sign_in = $3, updated_at = clock_timestamp() WHERE workspace_id = $1 RETURNING issuer, client_id, display_name, verified_at, enabled, password_sign_in, created_at, updated_at",
        )
        .bind(workspace_id)
        .bind(enabled)
        .bind(policy)
        .fetch_one(&mut *tx)
        .await?;
        self.audited_as(UPDATED_ACTION).commit_mutation(tx).await?;
        Ok(provider)
    }

    /// Removes the provider. Linked identities stay, and sign-in ignores them
    /// unless this workspace configures that issuer again.
    pub async fn delete_identity_provider(&self) -> Result<bool, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        let deleted =
            sqlx::query("DELETE FROM workspace_identity_providers WHERE workspace_id = $1")
                .bind(self.workspace_id())
                .execute(&mut *tx)
                .await?
                .rows_affected()
                == 1;
        if deleted {
            self.audited_as(DELETED_ACTION).commit_mutation(tx).await?;
        }
        Ok(deleted)
    }
}

impl<S: super::RepositoryScope> AttricatRepository<S> {
    /// Whether only owners may use a password in this workspace.
    pub(super) async fn passwords_restricted_on(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        workspace_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM workspace_identity_providers WHERE workspace_id = $1 AND enabled AND password_sign_in = $2)",
        )
        .bind(workspace_id)
        .bind(PasswordSignIn::Owners.as_str())
        .fetch_one(&mut **tx)
        .await?)
    }

    /// Whether this user is an active owner of the workspace, who keeps a
    /// password as the way back in if the provider fails.
    pub async fn is_active_workspace_owner(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        let mut connection = self.pool.acquire().await?;
        Self::active_owner_on(&mut connection, user_id, workspace_id).await
    }

    /// The workspace's provider with its secret decrypted, for a sign-in.
    pub async fn identity_provider_for_sign_in(
        &self,
        workspace_id: Uuid,
    ) -> Result<Option<IdentityProviderSignIn>, RepositoryError> {
        let row: Option<SignInRow> = sqlx::query_as(
            "SELECT issuer, client_id, client_secret_ciphertext, enabled, verified_at FROM workspace_identity_providers WHERE workspace_id = $1",
        )
        .bind(workspace_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let key = CREDENTIAL_KEY
            .get()
            .ok_or(RepositoryError::IdentityProviderKeyMissing)?;
        let client_secret = open(
            key,
            &secret_context(workspace_id),
            &row.client_secret_ciphertext,
        )
        .ok_or(RepositoryError::IdentityProviderKeyMissing)?;
        Ok(Some(IdentityProviderSignIn {
            issuer: row.issuer,
            client_id: row.client_id,
            client_secret,
            enabled: row.enabled,
            verified: row.verified_at.is_some(),
        }))
    }

    /// What a workspace offers on its sign-in page.
    pub async fn sign_in_options(
        &self,
        workspace_id: Uuid,
    ) -> Result<SignInOptions, RepositoryError> {
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT display_name, password_sign_in FROM workspace_identity_providers WHERE workspace_id = $1 AND enabled",
        )
        .bind(workspace_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(match row {
            Some((display_name, policy)) => SignInOptions {
                provider: Some(display_name),
                password_sign_in: PasswordSignIn::parse(&policy).unwrap_or(PasswordSignIn::All),
            },
            None => SignInOptions {
                provider: None,
                password_sign_in: PasswordSignIn::All,
            },
        })
    }
}
