//! Host-managed credential connections of extensions (#424).
//!
//! A connection is one destination account (a Shopify shop, an Amazon
//! marketplace region, an Allegro seller) of a manifest-declared connection
//! kind. Client secrets, refresh tokens, access tokens and device codes are
//! stored only as AES-256-GCM ciphertext bound to their row and field; they
//! are decrypted only inside the host to call a token endpoint or to inject
//! an access token into a mediated request, and never reach a component, the
//! management API, logs, traces or audit records.

use std::sync::OnceLock;

use chrono::{DateTime, Utc};
use ring::{
    aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey},
    rand::{SecureRandom, SystemRandom},
};
use serde::Serialize;
use serde_json::Value;
use sqlx::{FromRow, Postgres, Transaction};
use uuid::Uuid;

use super::{AttricatRepository, RepositoryError};

static CREDENTIAL_KEY: OnceLock<[u8; 32]> = OnceLock::new();

/// Installs the process-wide key that encrypts connection credentials
/// (`EXTENSION_CREDENTIAL_KEY`, 32 bytes). Later calls are ignored.
pub fn use_extension_credential_key(key: [u8; 32]) {
    let _ = CREDENTIAL_KEY.set(key);
}

fn sealing_key() -> Result<LessSafeKey, RepositoryError> {
    let key = CREDENTIAL_KEY.get().ok_or_else(|| {
        RepositoryError::InvalidExtension(
            "extension connections need EXTENSION_CREDENTIAL_KEY to be configured".into(),
        )
    })?;
    Ok(LessSafeKey::new(
        UnboundKey::new(&AES_256_GCM, key).expect("a 32-byte AES-256-GCM key"),
    ))
}

/// The additional data that binds a ciphertext to its connection and field.
fn associated_data(connection_id: Uuid, field: &str) -> String {
    format!("attricat.extension_connection:{connection_id}:{field}")
}

fn seal(connection_id: Uuid, field: &str, plaintext: &str) -> Result<Vec<u8>, RepositoryError> {
    let key = sealing_key()?;
    let mut nonce = [0u8; NONCE_LEN];
    SystemRandom::new()
        .fill(&mut nonce)
        .map_err(|_| RepositoryError::InvalidExtension("random source unavailable".into()))?;
    let mut sealed = plaintext.as_bytes().to_vec();
    key.seal_in_place_append_tag(
        Nonce::assume_unique_for_key(nonce),
        Aad::from(associated_data(connection_id, field).as_bytes()),
        &mut sealed,
    )
    .map_err(|_| RepositoryError::InvalidExtension("credential encryption failed".into()))?;
    let mut ciphertext = nonce.to_vec();
    ciphertext.extend_from_slice(&sealed);
    Ok(ciphertext)
}

fn open(connection_id: Uuid, field: &str, ciphertext: &[u8]) -> Result<String, RepositoryError> {
    let unreadable =
        || RepositoryError::InvalidExtension("stored credential cannot be decrypted".into());
    if ciphertext.len() < NONCE_LEN {
        return Err(unreadable());
    }
    let key = sealing_key()?;
    let (nonce, sealed) = ciphertext.split_at(NONCE_LEN);
    let mut sealed = sealed.to_vec();
    let plaintext = key
        .open_in_place(
            Nonce::try_assume_unique_for_key(nonce).map_err(|_| unreadable())?,
            Aad::from(associated_data(connection_id, field).as_bytes()),
            &mut sealed,
        )
        .map_err(|_| unreadable())?;
    String::from_utf8(plaintext.to_vec()).map_err(|_| unreadable())
}

fn open_optional(
    connection_id: Uuid,
    field: &str,
    ciphertext: Option<Vec<u8>>,
) -> Result<Option<String>, RepositoryError> {
    ciphertext
        .map(|ciphertext| open(connection_id, field, &ciphertext))
        .transpose()
}

/// A connection as operators and the component see it: never a secret.
#[derive(Clone, Debug, FromRow, Serialize)]
pub struct ExtensionConnection {
    pub id: Uuid,
    pub extension_id: String,
    pub connection: String,
    pub kind: String,
    pub display_name: String,
    pub grant_type: String,
    pub host_permission_id: String,
    pub token_url: String,
    pub device_authorization_url: Option<String>,
    pub client_id: String,
    pub scope: Option<String>,
    pub parameters: Value,
    pub status: String,
    pub last_error: Option<String>,
    pub last_refreshed_at: Option<DateTime<Utc>>,
    pub access_token_expires_at: Option<DateTime<Utc>>,
    pub device_user_code: Option<String>,
    pub device_verification_uri: Option<String>,
    pub device_expires_at: Option<DateTime<Utc>>,
    pub has_client_secret: bool,
    pub has_refresh_token: bool,
    pub has_access_token: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const CONNECTION_FIELDS: &str = "id,extension_id,connection,kind,display_name,grant_type,host_permission_id,token_url,device_authorization_url,client_id,scope,parameters,status,last_error,last_refreshed_at,access_token_expires_at,device_user_code,device_verification_uri,device_expires_at,client_secret_ciphertext IS NOT NULL AS has_client_secret,refresh_token_ciphertext IS NOT NULL AS has_refresh_token,access_token_ciphertext IS NOT NULL AS has_access_token,created_at,updated_at";

/// A connection to create. Secret fields are encrypted before they are stored.
#[derive(Clone)]
pub struct NewExtensionConnection {
    pub extension_id: String,
    pub connection: String,
    pub kind: String,
    pub display_name: String,
    pub grant_type: String,
    pub host_permission_id: String,
    pub token_url: String,
    pub device_authorization_url: Option<String>,
    pub client_id: String,
    pub scope: Option<String>,
    pub parameters: Value,
    pub client_secret: Option<String>,
    pub refresh_token: Option<String>,
}

/// A change to a connection; absent fields are kept.
#[derive(Clone, Default)]
pub struct ExtensionConnectionPatch {
    pub display_name: Option<String>,
    pub token_url: Option<String>,
    pub device_authorization_url: Option<Option<String>>,
    pub scope: Option<Option<String>>,
    pub parameters: Option<Value>,
    pub client_secret: Option<String>,
    pub refresh_token: Option<String>,
}

/// Decrypted connection material. It exists only inside the host and is
/// never serialized; its `Debug` form omits every secret.
#[derive(Clone)]
pub struct ConnectionCredentials {
    pub id: Uuid,
    pub connection: String,
    pub kind: String,
    pub grant_type: String,
    pub host_permission_id: String,
    pub token_url: String,
    pub device_authorization_url: Option<String>,
    pub client_id: String,
    pub scope: Option<String>,
    pub status: String,
    pub client_secret: Option<String>,
    pub refresh_token: Option<String>,
    pub access_token: Option<String>,
    pub access_token_expires_at: Option<DateTime<Utc>>,
    pub device_code: Option<String>,
    pub device_expires_at: Option<DateTime<Utc>>,
    pub device_interval_seconds: Option<i32>,
    pub device_polled_at: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for ConnectionCredentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ConnectionCredentials")
            .field("id", &self.id)
            .field("connection", &self.connection)
            .field("grant_type", &self.grant_type)
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

#[derive(FromRow)]
struct CredentialRow {
    id: Uuid,
    connection: String,
    kind: String,
    grant_type: String,
    host_permission_id: String,
    token_url: String,
    device_authorization_url: Option<String>,
    client_id: String,
    scope: Option<String>,
    status: String,
    client_secret_ciphertext: Option<Vec<u8>>,
    refresh_token_ciphertext: Option<Vec<u8>>,
    access_token_ciphertext: Option<Vec<u8>>,
    access_token_expires_at: Option<DateTime<Utc>>,
    device_code_ciphertext: Option<Vec<u8>>,
    device_expires_at: Option<DateTime<Utc>>,
    device_interval_seconds: Option<i32>,
    device_polled_at: Option<DateTime<Utc>>,
}

const CREDENTIAL_FIELDS: &str = "id,connection,kind,grant_type,host_permission_id,token_url,device_authorization_url,client_id,scope,status,client_secret_ciphertext,refresh_token_ciphertext,access_token_ciphertext,access_token_expires_at,device_code_ciphertext,device_expires_at,device_interval_seconds,device_polled_at";

impl CredentialRow {
    fn decrypt(self) -> Result<ConnectionCredentials, RepositoryError> {
        let id = self.id;
        Ok(ConnectionCredentials {
            id,
            connection: self.connection,
            kind: self.kind,
            grant_type: self.grant_type,
            host_permission_id: self.host_permission_id,
            token_url: self.token_url,
            device_authorization_url: self.device_authorization_url,
            client_id: self.client_id,
            scope: self.scope,
            status: self.status,
            client_secret: open_optional(id, "client_secret", self.client_secret_ciphertext)?,
            refresh_token: open_optional(id, "refresh_token", self.refresh_token_ciphertext)?,
            access_token: open_optional(id, "access_token", self.access_token_ciphertext)?,
            access_token_expires_at: self.access_token_expires_at,
            device_code: open_optional(id, "device_code", self.device_code_ciphertext)?,
            device_expires_at: self.device_expires_at,
            device_interval_seconds: self.device_interval_seconds,
            device_polled_at: self.device_polled_at,
        })
    }
}

/// A connection's cached access token, read without its other secrets.
pub struct ConnectionAccessToken {
    pub kind: String,
    pub status: String,
    pub access_token: Option<String>,
    pub access_token_expires_at: Option<DateTime<Utc>>,
}

/// Tokens a token endpoint issued.
#[derive(Clone)]
pub struct IssuedTokens {
    pub access_token: String,
    pub expires_at: Option<DateTime<Utc>>,
    /// A rotated refresh token replaces the stored one; `None` keeps it.
    pub refresh_token: Option<String>,
}

/// A device authorization an operator still has to approve.
#[derive(Clone)]
pub struct DeviceAuthorization {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_at: DateTime<Utc>,
    pub interval_seconds: i32,
}

/// A connection locked for a token refresh. While it is held no other worker
/// can refresh the same connection, so a rotating refresh token is never
/// used twice. Dropping it without completing rolls the lock back.
pub struct ConnectionRefresh {
    transaction: Transaction<'static, Postgres>,
    workspace_id: Uuid,
    pub credentials: ConnectionCredentials,
}

impl ConnectionRefresh {
    /// Stores freshly issued tokens and releases the lock.
    pub async fn complete(mut self, tokens: IssuedTokens) -> Result<(), RepositoryError> {
        let id = self.credentials.id;
        let access = seal(id, "access_token", &tokens.access_token)?;
        let refresh = tokens
            .refresh_token
            .as_deref()
            .map(|token| seal(id, "refresh_token", token))
            .transpose()?;
        sqlx::query("UPDATE extension_connections SET access_token_ciphertext=$3,access_token_expires_at=$4,refresh_token_ciphertext=COALESCE($5,refresh_token_ciphertext),device_code_ciphertext=NULL,device_user_code=NULL,device_verification_uri=NULL,device_expires_at=NULL,device_interval_seconds=NULL,status='active',last_error=NULL,last_refreshed_at=clock_timestamp(),updated_at=clock_timestamp() WHERE workspace_id=$1 AND id=$2")
            .bind(self.workspace_id).bind(id).bind(access).bind(tokens.expires_at).bind(refresh)
            .execute(&mut *self.transaction).await?;
        self.transaction.commit().await?;
        Ok(())
    }

    /// Records a device-code poll the provider answered with "pending" and
    /// releases the lock. `slow_down` adds 5 seconds to the poll interval,
    /// as RFC 8628 requires.
    pub async fn pending(mut self, slow_down: bool) -> Result<(), RepositoryError> {
        sqlx::query("UPDATE extension_connections SET device_polled_at=clock_timestamp(),device_interval_seconds=CASE WHEN $3 THEN LEAST(COALESCE(device_interval_seconds,5)+5,3600) ELSE device_interval_seconds END WHERE workspace_id=$1 AND id=$2")
            .bind(self.workspace_id).bind(self.credentials.id).bind(slow_down)
            .execute(&mut *self.transaction).await?;
        self.transaction.commit().await?;
        Ok(())
    }

    /// Records a failed refresh and releases the lock. A rejected grant
    /// (`revoked`) needs an operator to reconnect; other failures keep the
    /// connection usable for a later attempt.
    pub async fn fail(mut self, message: &str, revoked: bool) -> Result<(), RepositoryError> {
        let message: String = message.chars().take(1024).collect();
        sqlx::query("UPDATE extension_connections SET status=CASE WHEN $4 THEN 'failed' ELSE status END,last_error=$3,access_token_ciphertext=CASE WHEN $4 THEN NULL ELSE access_token_ciphertext END,updated_at=clock_timestamp() WHERE workspace_id=$1 AND id=$2")
            .bind(self.workspace_id).bind(self.credentials.id).bind(message).bind(revoked)
            .execute(&mut *self.transaction).await?;
        self.transaction.commit().await?;
        Ok(())
    }
}

fn check_connection_name(connection: &str) -> Result<(), RepositoryError> {
    if super::valid_connection_key(connection) {
        Ok(())
    } else {
        Err(RepositoryError::InvalidExtension(
            "connection names are 1-128 letters, digits, '.', '_' or '-'".into(),
        ))
    }
}

impl AttricatRepository {
    pub async fn list_extension_connections(
        &self,
        extension_id: &str,
    ) -> Result<Vec<ExtensionConnection>, RepositoryError> {
        Ok(sqlx::query_as(&format!("SELECT {CONNECTION_FIELDS} FROM extension_connections WHERE workspace_id=$1 AND extension_id=$2 ORDER BY connection"))
            .bind(self.extension_workspace()).bind(extension_id)
            .fetch_all(&self.pool).await?)
    }

    pub async fn extension_connection(
        &self,
        extension_id: &str,
        connection: &str,
    ) -> Result<ExtensionConnection, RepositoryError> {
        sqlx::query_as(&format!("SELECT {CONNECTION_FIELDS} FROM extension_connections WHERE workspace_id=$1 AND extension_id=$2 AND connection=$3"))
            .bind(self.extension_workspace()).bind(extension_id).bind(connection)
            .fetch_optional(&self.pool).await?
            .ok_or(RepositoryError::NotFound("extension connection"))
    }

    /// Creates a connection. Grants that need an operator's authorization
    /// start as `pending_authorization`.
    pub async fn create_extension_connection(
        &self,
        input: NewExtensionConnection,
    ) -> Result<ExtensionConnection, RepositoryError> {
        check_connection_name(&input.connection)?;
        let id = Uuid::new_v4();
        let client_secret = input
            .client_secret
            .as_deref()
            .map(|secret| seal(id, "client_secret", secret))
            .transpose()?;
        let refresh_token = input
            .refresh_token
            .as_deref()
            .map(|token| seal(id, "refresh_token", token))
            .transpose()?;
        let status = match input.grant_type.as_str() {
            "client_credentials" => "active",
            "refresh_token" if refresh_token.is_some() => "active",
            _ => "pending_authorization",
        };
        let mut transaction = self.pool.begin().await?;
        let created = sqlx::query_as(&format!("INSERT INTO extension_connections(id,workspace_id,extension_id,connection,kind,display_name,grant_type,host_permission_id,token_url,device_authorization_url,client_id,scope,parameters,client_secret_ciphertext,refresh_token_ciphertext,status) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16) RETURNING {CONNECTION_FIELDS}"))
            .bind(id).bind(self.extension_workspace()).bind(&input.extension_id).bind(&input.connection)
            .bind(&input.kind).bind(&input.display_name).bind(&input.grant_type).bind(&input.host_permission_id)
            .bind(&input.token_url).bind(&input.device_authorization_url).bind(&input.client_id)
            .bind(&input.scope).bind(&input.parameters).bind(client_secret).bind(refresh_token).bind(status)
            .fetch_one(&mut *transaction).await
            .map_err(|error| match error {
                sqlx::Error::Database(database) if database.is_unique_violation() => {
                    RepositoryError::ExportMappingConflict("a connection with this name already exists".into())
                }
                error => error.into(),
            })?;
        self.write_audit_event(&mut transaction).await?;
        transaction.commit().await?;
        Ok(created)
    }

    /// Changes a connection. A new secret or refresh token replaces the
    /// stored one and discards the cached access token.
    pub async fn update_extension_connection(
        &self,
        extension_id: &str,
        connection: &str,
        patch: ExtensionConnectionPatch,
    ) -> Result<ExtensionConnection, RepositoryError> {
        let current = self.extension_connection(extension_id, connection).await?;
        let id = current.id;
        let client_secret = patch
            .client_secret
            .as_deref()
            .map(|secret| seal(id, "client_secret", secret))
            .transpose()?;
        let refresh_token = patch
            .refresh_token
            .as_deref()
            .map(|token| seal(id, "refresh_token", token))
            .transpose()?;
        let rotated = client_secret.is_some() || refresh_token.is_some();
        let mut transaction = self.pool.begin().await?;
        let updated = sqlx::query_as(&format!(
            "UPDATE extension_connections SET display_name=COALESCE($3,display_name),token_url=COALESCE($4,token_url),\
             device_authorization_url=CASE WHEN $5 THEN $6 ELSE device_authorization_url END,\
             scope=CASE WHEN $7 THEN $8 ELSE scope END,parameters=COALESCE($9,parameters),\
             client_secret_ciphertext=COALESCE($10,client_secret_ciphertext),refresh_token_ciphertext=COALESCE($11,refresh_token_ciphertext),\
             access_token_ciphertext=CASE WHEN $12 THEN NULL ELSE access_token_ciphertext END,\
             access_token_expires_at=CASE WHEN $12 THEN NULL ELSE access_token_expires_at END,\
             status=CASE WHEN $11::bytea IS NOT NULL OR (grant_type='client_credentials' AND $10::bytea IS NOT NULL) THEN 'active' ELSE status END,\
             last_error=CASE WHEN $12 THEN NULL ELSE last_error END,updated_at=clock_timestamp() \
             WHERE workspace_id=$1 AND id=$2 RETURNING {CONNECTION_FIELDS}"
        ))
        .bind(self.extension_workspace())
        .bind(id)
        .bind(&patch.display_name)
        .bind(&patch.token_url)
        .bind(patch.device_authorization_url.is_some())
        .bind(patch.device_authorization_url.clone().flatten())
        .bind(patch.scope.is_some())
        .bind(patch.scope.clone().flatten())
        .bind(&patch.parameters)
        .bind(client_secret)
        .bind(refresh_token)
        .bind(rotated)
        .fetch_one(&mut *transaction)
        .await?;
        self.write_audit_event(&mut transaction).await?;
        transaction.commit().await?;
        Ok(updated)
    }

    /// Deletes a connection and its credentials. Its export mappings stay, so
    /// re-creating the connection under the same name keeps the destination's
    /// external IDs.
    pub async fn delete_extension_connection(
        &self,
        extension_id: &str,
        connection: &str,
    ) -> Result<bool, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let deleted = sqlx::query("DELETE FROM extension_connections WHERE workspace_id=$1 AND extension_id=$2 AND connection=$3")
            .bind(self.extension_workspace()).bind(extension_id).bind(connection)
            .execute(&mut *transaction).await?.rows_affected() == 1;
        if deleted {
            self.write_audit_event(&mut transaction).await?;
        }
        transaction.commit().await?;
        Ok(deleted)
    }

    /// Decrypted credentials, without a lock. Use them to inject a still
    /// valid access token; refresh through [`Self::lock_extension_connection`].
    pub async fn extension_connection_credentials(
        &self,
        extension_id: &str,
        connection: &str,
    ) -> Result<ConnectionCredentials, RepositoryError> {
        let row: CredentialRow = sqlx::query_as(&format!("SELECT {CREDENTIAL_FIELDS} FROM extension_connections WHERE workspace_id=$1 AND extension_id=$2 AND connection=$3"))
            .bind(self.extension_workspace()).bind(extension_id).bind(connection)
            .fetch_optional(&self.pool).await?
            .ok_or(RepositoryError::NotFound("extension connection"))?;
        row.decrypt()
    }

    /// The cached access token of a connection with its kind and status. Only
    /// the access token is decrypted, so a mediated request that reuses a
    /// valid token does not open the connection's other secrets.
    pub async fn extension_connection_access_token(
        &self,
        extension_id: &str,
        connection: &str,
    ) -> Result<ConnectionAccessToken, RepositoryError> {
        let (id, kind, status, ciphertext, expires_at): (
            Uuid,
            String,
            String,
            Option<Vec<u8>>,
            Option<DateTime<Utc>>,
        ) = sqlx::query_as("SELECT id,kind,status,access_token_ciphertext,access_token_expires_at FROM extension_connections WHERE workspace_id=$1 AND extension_id=$2 AND connection=$3")
            .bind(self.extension_workspace()).bind(extension_id).bind(connection)
            .fetch_optional(&self.pool).await?
            .ok_or(RepositoryError::NotFound("extension connection"))?;
        Ok(ConnectionAccessToken {
            kind,
            status,
            access_token: open_optional(id, "access_token", ciphertext)?,
            access_token_expires_at: expires_at,
        })
    }

    /// Locks a connection for a token refresh or device-code poll.
    pub async fn lock_extension_connection(
        &self,
        extension_id: &str,
        connection: &str,
    ) -> Result<ConnectionRefresh, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let row: CredentialRow = sqlx::query_as(&format!("SELECT {CREDENTIAL_FIELDS} FROM extension_connections WHERE workspace_id=$1 AND extension_id=$2 AND connection=$3 FOR UPDATE"))
            .bind(self.extension_workspace()).bind(extension_id).bind(connection)
            .fetch_optional(&mut *transaction).await?
            .ok_or(RepositoryError::NotFound("extension connection"))?;
        Ok(ConnectionRefresh {
            transaction,
            workspace_id: self.extension_workspace(),
            credentials: row.decrypt()?,
        })
    }

    /// Discards a cached access token the destination rejected, so the next
    /// request refreshes it. Only `rejected`, the token that was sent, is
    /// discarded: a token another worker refreshed meanwhile is kept.
    pub async fn invalidate_extension_connection_token(
        &self,
        extension_id: &str,
        connection: &str,
        rejected: &str,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let row: Option<(Uuid, Option<Vec<u8>>)> = sqlx::query_as("SELECT id,access_token_ciphertext FROM extension_connections WHERE workspace_id=$1 AND extension_id=$2 AND connection=$3 FOR UPDATE")
            .bind(self.extension_workspace()).bind(extension_id).bind(connection)
            .fetch_optional(&mut *transaction).await?;
        let Some((id, ciphertext)) = row else {
            return Ok(());
        };
        if open_optional(id, "access_token", ciphertext)?.as_deref() == Some(rejected) {
            sqlx::query("UPDATE extension_connections SET access_token_ciphertext=NULL,access_token_expires_at=NULL,updated_at=clock_timestamp() WHERE workspace_id=$1 AND id=$2")
                .bind(self.extension_workspace()).bind(id)
                .execute(&mut *transaction).await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    /// Stores a started device authorization for an operator to approve.
    pub async fn record_extension_device_authorization(
        &self,
        extension_id: &str,
        connection: &str,
        authorization: &DeviceAuthorization,
    ) -> Result<ExtensionConnection, RepositoryError> {
        let current = self.extension_connection(extension_id, connection).await?;
        let device_code = seal(current.id, "device_code", &authorization.device_code)?;
        let mut transaction = self.pool.begin().await?;
        let updated = sqlx::query_as(&format!("UPDATE extension_connections SET device_code_ciphertext=$3,device_user_code=$4,device_verification_uri=$5,device_expires_at=$6,device_interval_seconds=$7,device_polled_at=NULL,status='pending_authorization',last_error=NULL,updated_at=clock_timestamp() WHERE workspace_id=$1 AND id=$2 RETURNING {CONNECTION_FIELDS}"))
            .bind(self.extension_workspace()).bind(current.id).bind(device_code)
            .bind(&authorization.user_code).bind(&authorization.verification_uri)
            .bind(authorization.expires_at).bind(authorization.interval_seconds.clamp(1, 3600))
            .fetch_one(&mut *transaction).await?;
        self.write_audit_event(&mut transaction).await?;
        transaction.commit().await?;
        Ok(updated)
    }

    /// The operator-lowered budget of a host-permission rule, if any.
    pub async fn extension_host_permission_budget(
        &self,
        extension_id: &str,
        host_permission_id: &str,
    ) -> Result<Option<(i32, i32)>, RepositoryError> {
        Ok(sqlx::query_as("SELECT requests,per_seconds FROM extension_host_permission_budgets WHERE workspace_id=$1 AND extension_id=$2 AND host_permission_id=$3")
            .bind(self.extension_workspace()).bind(extension_id).bind(host_permission_id)
            .fetch_optional(&self.pool).await?)
    }

    /// Every operator-lowered budget of an extension.
    pub async fn extension_host_permission_budgets(
        &self,
        extension_id: &str,
    ) -> Result<Vec<(String, i32, i32)>, RepositoryError> {
        Ok(sqlx::query_as("SELECT host_permission_id,requests,per_seconds FROM extension_host_permission_budgets WHERE workspace_id=$1 AND extension_id=$2 ORDER BY host_permission_id")
            .bind(self.extension_workspace()).bind(extension_id)
            .fetch_all(&self.pool).await?)
    }

    /// Sets (or with `None` clears) an operator-lowered budget.
    pub async fn set_extension_host_permission_budget(
        &self,
        extension_id: &str,
        host_permission_id: &str,
        budget: Option<(i32, i32)>,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        match budget {
            Some((requests, per_seconds)) => {
                sqlx::query("INSERT INTO extension_host_permission_budgets(workspace_id,extension_id,host_permission_id,requests,per_seconds) VALUES($1,$2,$3,$4,$5) ON CONFLICT(workspace_id,extension_id,host_permission_id) DO UPDATE SET requests=$4,per_seconds=$5,updated_at=clock_timestamp()")
                    .bind(self.extension_workspace()).bind(extension_id).bind(host_permission_id)
                    .bind(requests).bind(per_seconds)
                    .execute(&mut *transaction).await?;
            }
            None => {
                sqlx::query("DELETE FROM extension_host_permission_budgets WHERE workspace_id=$1 AND extension_id=$2 AND host_permission_id=$3")
                    .bind(self.extension_workspace()).bind(extension_id).bind(host_permission_id)
                    .execute(&mut *transaction).await?;
            }
        }
        self.write_audit_event(&mut transaction).await?;
        transaction.commit().await?;
        Ok(())
    }

    // ---- public file links ------------------------------------------------

    /// Issues a public link to a file for `lifetime`. Only the token's
    /// SHA-256 is stored; the token itself is returned once.
    pub async fn create_extension_file_link(
        &self,
        extension_id: &str,
        file_id: Uuid,
        record_id: Uuid,
        token_sha256: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        sqlx::query("INSERT INTO extension_file_links(id,workspace_id,extension_id,file_id,record_id,token_sha256,expires_at) VALUES($1,$2,$3,$4,$5,$6,$7)")
            .bind(Uuid::new_v4()).bind(self.extension_workspace()).bind(extension_id)
            .bind(file_id).bind(record_id).bind(token_sha256).bind(expires_at)
            .execute(&self.pool).await?;
        Ok(())
    }

    /// Drops this workspace's links that expired more than a day ago.
    pub async fn sweep_extension_file_links(&self) -> Result<u64, RepositoryError> {
        Ok(sqlx::query("DELETE FROM extension_file_links WHERE workspace_id=$1 AND expires_at < clock_timestamp() - interval '1 day'")
            .bind(self.extension_workspace())
            .execute(&self.pool).await?.rows_affected())
    }

    /// Revokes every link an extension issued, for example after disabling it.
    pub async fn revoke_extension_file_links(
        &self,
        extension_id: &str,
    ) -> Result<u64, RepositoryError> {
        Ok(sqlx::query("UPDATE extension_file_links SET revoked_at=clock_timestamp() WHERE workspace_id=$1 AND extension_id=$2 AND revoked_at IS NULL AND expires_at>clock_timestamp()")
            .bind(self.extension_workspace()).bind(extension_id)
            .execute(&self.pool).await?.rows_affected())
    }
}

impl AttricatRepository<super::SystemScope> {
    /// Resolves a public file link token (by its SHA-256) across workspaces.
    /// Returns the workspace, file, issuing extension and record of a live
    /// link; the caller checks the grant and that the record still references
    /// the file.
    pub async fn resolve_extension_file_link(
        &self,
        token_sha256: &str,
    ) -> Result<Option<(Uuid, Uuid, String, Uuid)>, RepositoryError> {
        Ok(sqlx::query_as("SELECT l.workspace_id,l.file_id,l.extension_id,l.record_id FROM extension_file_links l JOIN files f ON f.id=l.file_id AND f.workspace_id=l.workspace_id WHERE l.token_sha256=$1 AND l.revoked_at IS NULL AND l.expires_at>clock_timestamp() AND f.deleted_at IS NULL")
            .bind(token_sha256)
            .fetch_optional(&self.pool).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ciphertexts_are_bound_to_their_connection_and_field() {
        use_extension_credential_key([7u8; 32]);
        let id = Uuid::new_v4();
        let sealed = seal(id, "refresh_token", "secret-value").unwrap();
        assert!(!sealed.windows(12).any(|window| window == b"secret-value"));
        assert_eq!(open(id, "refresh_token", &sealed).unwrap(), "secret-value");
        assert!(open(id, "access_token", &sealed).is_err());
        assert!(open(Uuid::new_v4(), "refresh_token", &sealed).is_err());
        let mut tampered = sealed.clone();
        *tampered.last_mut().unwrap() ^= 1;
        assert!(open(id, "refresh_token", &tampered).is_err());
    }
}
