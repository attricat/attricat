//! Host-managed credential connections of extensions (#424).
//!
//! A connection is one destination account (a Shopify shop, an Amazon
//! marketplace region, an Allegro seller) of a manifest-declared connection
//! kind. Client secrets, refresh tokens, access tokens and device codes are
//! stored only as AES-256-GCM ciphertext bound to their row and field; they
//! are decrypted only inside the host to call a token endpoint or to inject
//! an access token into a mediated request, and never reach a component, the
//! management API, logs, traces or audit records.

use attricat_domain::states::ConnectionStatus;
use std::sync::OnceLock;

use chrono::{DateTime, Utc};
use ring::{
    aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey},
    rand::{SecureRandom, SystemRandom},
};
use serde::Serialize;
use serde_json::Value;
use sqlx::{FromRow, Postgres};
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
    #[sqlx(try_from = "String")]
    pub status: ConnectionStatus,
    pub last_error: Option<String>,
    pub last_refreshed_at: Option<DateTime<Utc>>,
    pub access_token_expires_at: Option<DateTime<Utc>>,
    pub device_user_code: Option<String>,
    pub device_verification_uri: Option<String>,
    pub device_expires_at: Option<DateTime<Utc>>,
    /// The provider's authorization endpoint, for browser authorization of
    /// an authorization-code connection.
    pub authorization_url: Option<String>,
    /// When the pending browser authorization expires, if one was started.
    pub authorization_expires_at: Option<DateTime<Utc>>,
    /// The redirect URI to register with the provider, filled in by the API
    /// for authorization-code connections when browser authorization is
    /// configured.
    #[sqlx(skip)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_uri: Option<String>,
    pub has_client_secret: bool,
    pub has_refresh_token: bool,
    pub has_access_token: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const CONNECTION_FIELDS: &str = "id,extension_id,connection,kind,display_name,grant_type,host_permission_id,token_url,device_authorization_url,client_id,scope,parameters,status,last_error,last_refreshed_at,access_token_expires_at,device_user_code,device_verification_uri,device_expires_at,authorization_url,authorization_expires_at,client_secret_ciphertext IS NOT NULL AS has_client_secret,refresh_token_ciphertext IS NOT NULL AS has_refresh_token,access_token_ciphertext IS NOT NULL AS has_access_token,created_at,updated_at";

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
    pub authorization_url: Option<String>,
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
    pub authorization_url: Option<Option<String>>,
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
    pub status: ConnectionStatus,
    pub client_secret: Option<String>,
    pub refresh_token: Option<String>,
    pub access_token: Option<String>,
    pub access_token_expires_at: Option<DateTime<Utc>>,
    pub device_code: Option<String>,
    pub device_expires_at: Option<DateTime<Utc>>,
    pub device_interval_seconds: Option<i32>,
    pub device_polled_at: Option<DateTime<Utc>>,
    pub last_refreshed_at: Option<DateTime<Utc>>,
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
    #[sqlx(try_from = "String")]
    status: ConnectionStatus,
    client_secret_ciphertext: Option<Vec<u8>>,
    refresh_token_ciphertext: Option<Vec<u8>>,
    access_token_ciphertext: Option<Vec<u8>>,
    access_token_expires_at: Option<DateTime<Utc>>,
    device_code_ciphertext: Option<Vec<u8>>,
    device_expires_at: Option<DateTime<Utc>>,
    device_interval_seconds: Option<i32>,
    device_polled_at: Option<DateTime<Utc>>,
    last_refreshed_at: Option<DateTime<Utc>>,
}

const CREDENTIAL_FIELDS: &str = "id,connection,kind,grant_type,host_permission_id,token_url,device_authorization_url,client_id,scope,status,client_secret_ciphertext,refresh_token_ciphertext,access_token_ciphertext,access_token_expires_at,device_code_ciphertext,device_expires_at,device_interval_seconds,device_polled_at,last_refreshed_at";

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
            last_refreshed_at: self.last_refreshed_at,
        })
    }
}

/// A connection's cached access token, read without its other secrets.
pub struct ConnectionAccessToken {
    pub kind: String,
    pub status: ConnectionStatus,
    pub access_token: Option<String>,
    pub access_token_expires_at: Option<DateTime<Utc>>,
    /// When the token was issued, so a short-lived token is not refreshed on
    /// every request.
    pub last_refreshed_at: Option<DateTime<Utc>>,
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

/// How long a refresh claim lasts if its worker never settles it: longer than
/// a DNS lookup and a token request may take at their longest timeouts.
const REFRESH_CLAIM: chrono::Duration = chrono::Duration::seconds(300);

/// A connection claimed for a token refresh or device-code poll. While the
/// claim is live no other worker refreshes the same connection, so a
/// rotating refresh token is never used twice, yet no row lock or pooled
/// connection is held across the token request. Dropping it unsettled
/// releases the claim in the background.
pub struct ConnectionRefresh {
    pool: sqlx::PgPool,
    workspace_id: Uuid,
    claim: Uuid,
    settled: bool,
    /// The stored grant the refresh started from, so tokens it obtained
    /// are not stored over credentials an operator changed meanwhile.
    basis: RefreshBasis,
    pub credentials: ConnectionCredentials,
}

/// The credential columns a refresh's tokens were obtained with.
#[derive(Default)]
struct RefreshBasis {
    client_secret: Option<Vec<u8>>,
    refresh_token: Option<Vec<u8>>,
    token_url: String,
    scope: Option<String>,
}

impl ConnectionRefresh {
    /// Runs one claim-settling update; it applies only while the claim is
    /// still this worker's.
    async fn settle(
        &mut self,
        query: sqlx::query::Query<'_, Postgres, sqlx::postgres::PgArguments>,
    ) -> Result<(), RepositoryError> {
        let settled = query.execute(&self.pool).await?.rows_affected() == 1;
        self.settled = true;
        if settled {
            Ok(())
        } else {
            // The guarded change matched nothing (the connection changed or
            // the claim lapsed); the claim is still ended, so others are not
            // held off until it expires.
            let _ = release_claim(self.workspace_id, self.credentials.id, self.claim)
                .execute(&self.pool)
                .await;
            Err(RepositoryError::InvalidExtension(
                "the connection refresh claim expired".into(),
            ))
        }
    }

    /// Stores freshly issued tokens and releases the claim. The tokens are
    /// stored even when the claim has lapsed: the provider may already have
    /// rotated the refresh token, and losing the new one would break the
    /// connection. They are discarded when an operator replaced the refresh
    /// token they were obtained with, so the new credentials win, and when
    /// the token endpoint moved to another origin, whose tokens they are
    /// not. When only the client secret, the token URL's path or the scope
    /// changed, the rotated refresh token is still kept but the access token
    /// is not, so the next call refreshes under the new settings.
    pub async fn complete(self, tokens: IssuedTokens) -> Result<(), RepositoryError> {
        self.store_tokens(tokens, false).await
    }

    /// Stores the tokens a device authorization's poll obtained, ending that
    /// authorization; see [`Self::complete`].
    pub async fn complete_device_authorization(
        self,
        tokens: IssuedTokens,
    ) -> Result<(), RepositoryError> {
        self.store_tokens(tokens, true).await
    }

    /// A refresh leaves a device authorization the operator started in
    /// place; only the poll that completes it ends it.
    async fn store_tokens(
        mut self,
        tokens: IssuedTokens,
        ends_device_authorization: bool,
    ) -> Result<(), RepositoryError> {
        let id = self.credentials.id;
        let access = seal(id, "access_token", &tokens.access_token)?;
        let refresh = tokens
            .refresh_token
            .as_deref()
            .map(|token| seal(id, "refresh_token", token))
            .transpose()?;
        let (workspace_id, claim) = (self.workspace_id, self.claim);
        let basis = std::mem::take(&mut self.basis);
        // Tokens from an endpoint that has since moved to another origin
        // belong to the old provider and are dropped. Origins are compared
        // as the connection update compares them.
        let origin = |url: &str| url::Url::parse(url).ok().map(|url| url.origin());
        // The row is locked while it is checked and changed, so an update
        // moving the endpoint cannot slip in between.
        let changed = || {
            RepositoryError::InvalidExtension(
                "the connection changed during the token refresh".into(),
            )
        };
        let mut transaction = self.pool.begin().await?;
        let current: Option<String> = sqlx::query_scalar(
            "SELECT token_url FROM extension_connections WHERE workspace_id=$1 AND id=$2 FOR UPDATE",
        )
        .bind(workspace_id)
        .bind(id)
        .fetch_optional(&mut *transaction)
        .await?;
        let stored = current.as_deref().map(origin) == Some(origin(&basis.token_url))
            && sqlx::query("UPDATE extension_connections SET access_token_ciphertext=CASE WHEN token_url=$9 AND scope IS NOT DISTINCT FROM $10 AND client_secret_ciphertext IS NOT DISTINCT FROM $7 THEN $3 END,access_token_expires_at=CASE WHEN token_url=$9 AND scope IS NOT DISTINCT FROM $10 AND client_secret_ciphertext IS NOT DISTINCT FROM $7 THEN $4 END,refresh_token_ciphertext=COALESCE($5,refresh_token_ciphertext),device_code_ciphertext=CASE WHEN $11 THEN NULL ELSE device_code_ciphertext END,device_user_code=CASE WHEN $11 THEN NULL ELSE device_user_code END,device_verification_uri=CASE WHEN $11 THEN NULL ELSE device_verification_uri END,device_expires_at=CASE WHEN $11 THEN NULL ELSE device_expires_at END,device_interval_seconds=CASE WHEN $11 THEN NULL ELSE device_interval_seconds END,status='active',last_error=NULL,last_refreshed_at=clock_timestamp(),refresh_claim=CASE WHEN refresh_claim=$6 THEN NULL ELSE refresh_claim END,refresh_claimed_until=CASE WHEN refresh_claim=$6 THEN NULL ELSE refresh_claimed_until END,updated_at=clock_timestamp() WHERE workspace_id=$1 AND id=$2 AND refresh_token_ciphertext IS NOT DISTINCT FROM $8")
                .bind(workspace_id).bind(id).bind(access).bind(tokens.expires_at).bind(refresh).bind(claim)
                .bind(basis.client_secret).bind(basis.refresh_token).bind(&basis.token_url).bind(basis.scope).bind(ends_device_authorization)
                .execute(&mut *transaction).await?.rows_affected() == 1;
        if !stored {
            // The tokens are dropped, but the claim still ends.
            release_claim(workspace_id, id, claim)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        self.settled = true;
        if stored { Ok(()) } else { Err(changed()) }
    }

    /// Records a device-code poll the provider answered with "pending" and
    /// releases the claim. `slow_down` adds 5 seconds to the poll interval,
    /// as RFC 8628 requires.
    pub async fn pending(mut self, slow_down: bool) -> Result<(), RepositoryError> {
        let (workspace_id, id, claim) = (self.workspace_id, self.credentials.id, self.claim);
        self.settle(sqlx::query("UPDATE extension_connections SET device_polled_at=clock_timestamp(),device_interval_seconds=CASE WHEN $3 THEN LEAST(COALESCE(device_interval_seconds,5)+5,3600) ELSE device_interval_seconds END,refresh_claim=NULL,refresh_claimed_until=NULL WHERE workspace_id=$1 AND id=$2 AND refresh_claim=$4")
            .bind(workspace_id).bind(id).bind(slow_down).bind(claim))
            .await
    }

    /// Records a failed refresh and releases the claim. A rejected grant
    /// (`revoked`) needs an operator to reconnect; other failures keep the
    /// connection usable for a later attempt.
    pub async fn fail(mut self, message: &str, revoked: bool) -> Result<(), RepositoryError> {
        let message: String = message.chars().take(1024).collect();
        let (workspace_id, id, claim) = (self.workspace_id, self.credentials.id, self.claim);
        self.settle(sqlx::query("UPDATE extension_connections SET status=CASE WHEN $4 THEN 'failed' ELSE status END,last_error=$3,access_token_ciphertext=CASE WHEN $4 THEN NULL ELSE access_token_ciphertext END,refresh_claim=NULL,refresh_claimed_until=NULL,updated_at=clock_timestamp() WHERE workspace_id=$1 AND id=$2 AND refresh_claim=$5")
            .bind(workspace_id).bind(id).bind(message).bind(revoked).bind(claim))
            .await
    }

    /// Ends a device authorization that expired or was refused and releases
    /// the claim. An active connection keeps its tokens and status, and a
    /// revoked one stays revoked; one that was waiting for this
    /// authorization fails.
    pub async fn abandon_device_authorization(
        mut self,
        message: &str,
    ) -> Result<(), RepositoryError> {
        let message: String = message.chars().take(1024).collect();
        let (workspace_id, id, claim) = (self.workspace_id, self.credentials.id, self.claim);
        self.settle(sqlx::query("UPDATE extension_connections SET device_code_ciphertext=NULL,device_user_code=NULL,device_verification_uri=NULL,device_expires_at=NULL,device_interval_seconds=NULL,device_polled_at=NULL,status=CASE WHEN status IN ('active','revoked') THEN status ELSE 'failed' END,last_error=$3,refresh_claim=NULL,refresh_claimed_until=NULL,updated_at=clock_timestamp() WHERE workspace_id=$1 AND id=$2 AND refresh_claim=$4")
            .bind(workspace_id).bind(id).bind(message).bind(claim))
            .await
    }

    /// Releases the claim without changing the connection.
    pub async fn release(mut self) -> Result<(), RepositoryError> {
        let (workspace_id, id, claim) = (self.workspace_id, self.credentials.id, self.claim);
        let _ = self.settle(release_claim(workspace_id, id, claim)).await;
        Ok(())
    }
}

fn release_claim(
    workspace_id: Uuid,
    id: Uuid,
    claim: Uuid,
) -> sqlx::query::Query<'static, Postgres, sqlx::postgres::PgArguments> {
    sqlx::query("UPDATE extension_connections SET refresh_claim=NULL,refresh_claimed_until=NULL WHERE workspace_id=$1 AND id=$2 AND refresh_claim=$3")
        .bind(workspace_id)
        .bind(id)
        .bind(claim)
}

impl Drop for ConnectionRefresh {
    fn drop(&mut self) {
        if self.settled {
            return;
        }
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            let pool = self.pool.clone();
            let query = release_claim(self.workspace_id, self.credentials.id, self.claim);
            runtime.spawn(async move {
                let _ = query.execute(&pool).await;
            });
        }
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

/// The capabilities a public file link is issued and served under. Revoking
/// any of them ends the extension's links.
pub const PUBLIC_FILE_LINK_GRANTS: [&str; 3] = [
    "attricat.read",
    "attricat.files.read",
    "attricat.files.public_links",
];

impl AttricatRepository {
    /// Whether the extension has a connection named `connection`.
    pub(crate) async fn extension_connection_exists<'e>(
        &self,
        executor: impl sqlx::PgExecutor<'e>,
        extension_id: &str,
        connection: &str,
    ) -> Result<bool, RepositoryError> {
        Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM extension_connections WHERE workspace_id=$1 AND extension_id=$2 AND connection=$3)")
            .bind(self.workspace_id()).bind(extension_id).bind(connection)
            .fetch_one(executor).await?)
    }

    pub async fn list_extension_connections(
        &self,
        extension_id: &str,
    ) -> Result<Vec<ExtensionConnection>, RepositoryError> {
        Ok(sqlx::query_as(&format!("SELECT {CONNECTION_FIELDS} FROM extension_connections WHERE workspace_id=$1 AND extension_id=$2 ORDER BY connection"))
            .bind(self.workspace_id()).bind(extension_id)
            .fetch_all(&self.pool).await?)
    }

    pub async fn extension_connection(
        &self,
        extension_id: &str,
        connection: &str,
    ) -> Result<ExtensionConnection, RepositoryError> {
        sqlx::query_as(&format!("SELECT {CONNECTION_FIELDS} FROM extension_connections WHERE workspace_id=$1 AND extension_id=$2 AND connection=$3"))
            .bind(self.workspace_id()).bind(extension_id).bind(connection)
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
            "client_credentials" => ConnectionStatus::Active,
            "refresh_token" if refresh_token.is_some() => ConnectionStatus::Active,
            _ => ConnectionStatus::PendingAuthorization,
        };
        let mut transaction = self.pool.begin().await?;
        let created = sqlx::query_as(&format!("INSERT INTO extension_connections(id,workspace_id,extension_id,connection,kind,display_name,grant_type,host_permission_id,token_url,device_authorization_url,client_id,scope,parameters,client_secret_ciphertext,refresh_token_ciphertext,status,authorization_url) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17) RETURNING {CONNECTION_FIELDS}"))
            .bind(id).bind(self.workspace_id()).bind(&input.extension_id).bind(&input.connection)
            .bind(&input.kind).bind(&input.display_name).bind(&input.grant_type).bind(&input.host_permission_id)
            .bind(&input.token_url).bind(&input.device_authorization_url).bind(&input.client_id)
            .bind(&input.scope).bind(&input.parameters).bind(client_secret).bind(refresh_token).bind(status.as_str())
            .bind(&input.authorization_url)
            .fetch_one(&mut *transaction).await
            .map_err(|error| match error {
                sqlx::Error::Database(database) if database.is_unique_violation() => {
                    RepositoryError::ExtensionConnectionExists
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
        // The row is locked while the change is judged against it, so two
        // concurrent changes cannot both pass checks made on the old row.
        let mut transaction = self.pool.begin().await?;
        let current: ExtensionConnection = sqlx::query_as(&format!("SELECT {CONNECTION_FIELDS} FROM extension_connections WHERE workspace_id=$1 AND extension_id=$2 AND connection=$3 FOR UPDATE"))
            .bind(self.workspace_id()).bind(extension_id).bind(connection)
            .fetch_optional(&mut *transaction).await?
            .ok_or(RepositoryError::NotFound("extension connection"))?;
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
        // New credentials, a new token endpoint or a new scope make the
        // cached access token stale, and give a failed connection another
        // try when it still has something to obtain a token with.
        let changed = client_secret.is_some()
            || refresh_token.is_some()
            || patch
                .token_url
                .as_ref()
                .is_some_and(|url| *url != current.token_url)
            || patch
                .device_authorization_url
                .as_ref()
                .is_some_and(|url| *url != current.device_authorization_url)
            || patch
                .scope
                .as_ref()
                .is_some_and(|scope| *scope != current.scope)
            || patch
                .authorization_url
                .as_ref()
                .is_some_and(|url| *url != current.authorization_url);
        // A token endpoint moved to another origin is another provider: the
        // stored secret must be entered again and the stored tokens are not
        // sent there, so the connection waits for a new authorization unless
        // a refresh token comes with the change.
        let origin = |url: &str| url::Url::parse(url).ok().map(|url| url.origin());
        let token_origin = origin(patch.token_url.as_deref().unwrap_or(&current.token_url));
        // A device endpoint moves when it is set to an origin that neither
        // it nor the token endpoint had; adding one at the token endpoint's
        // origin, or removing it, sends nothing anywhere new.
        let moved = token_origin != origin(&current.token_url)
            || patch
                .device_authorization_url
                .as_ref()
                .and_then(|url| url.as_deref())
                .is_some_and(|url| {
                    let to = origin(url);
                    to != token_origin
                        && current.device_authorization_url.as_deref().map(origin) != Some(to)
                });
        if moved && current.has_client_secret && client_secret.is_none() {
            return Err(RepositoryError::InvalidExtension(
                "enter the client secret again to move the token endpoint to another host".into(),
            ));
        }
        let updated = sqlx::query_as(&format!(
            "UPDATE extension_connections SET display_name=COALESCE($3,display_name),token_url=COALESCE($4,token_url),\
             device_authorization_url=CASE WHEN $5 THEN $6 ELSE device_authorization_url END,\
             scope=CASE WHEN $7 THEN $8 ELSE scope END,parameters=COALESCE($9,parameters),\
             client_secret_ciphertext=COALESCE($10,client_secret_ciphertext),refresh_token_ciphertext=CASE WHEN $13 THEN $11 ELSE COALESCE($11,refresh_token_ciphertext) END,\
             access_token_ciphertext=CASE WHEN $12 THEN NULL ELSE access_token_ciphertext END,\
             device_code_ciphertext=CASE WHEN $13 THEN NULL ELSE device_code_ciphertext END,\
             device_user_code=CASE WHEN $13 THEN NULL ELSE device_user_code END,\
             device_verification_uri=CASE WHEN $13 THEN NULL ELSE device_verification_uri END,\
             device_expires_at=CASE WHEN $13 THEN NULL ELSE device_expires_at END,\
             device_interval_seconds=CASE WHEN $13 THEN NULL ELSE device_interval_seconds END,\
             device_polled_at=CASE WHEN $13 THEN NULL ELSE device_polled_at END,\
             authorization_url=CASE WHEN $14 THEN $15 ELSE authorization_url END,\
             authorization_state_sha256=CASE WHEN $12 THEN NULL ELSE authorization_state_sha256 END,\
             authorization_verifier_ciphertext=CASE WHEN $12 THEN NULL ELSE authorization_verifier_ciphertext END,\
             authorization_expires_at=CASE WHEN $12 THEN NULL ELSE authorization_expires_at END,\
             authorization_session_sha256=CASE WHEN $12 THEN NULL ELSE authorization_session_sha256 END,\
             access_token_expires_at=CASE WHEN $12 THEN NULL ELSE access_token_expires_at END,\
             status=CASE WHEN $11::bytea IS NOT NULL OR (grant_type='client_credentials' AND $10::bytea IS NOT NULL) THEN 'active' \
               WHEN $13 THEN 'pending_authorization' \
               WHEN $12 AND status='failed' AND (refresh_token_ciphertext IS NOT NULL OR (grant_type='client_credentials' AND client_secret_ciphertext IS NOT NULL)) THEN 'active' ELSE status END,\
             last_error=CASE WHEN $12 THEN NULL ELSE last_error END,\
             updated_at=clock_timestamp() \
             WHERE workspace_id=$1 AND id=$2 RETURNING {CONNECTION_FIELDS}"
        ))
        .bind(self.workspace_id())
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
        .bind(changed || moved)
        .bind(moved)
        .bind(patch.authorization_url.is_some())
        .bind(patch.authorization_url.clone().flatten())
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
            .bind(self.workspace_id()).bind(extension_id).bind(connection)
            .execute(&mut *transaction).await?.rows_affected() == 1;
        if deleted {
            self.write_audit_event(&mut transaction).await?;
        }
        transaction.commit().await?;
        Ok(deleted)
    }

    /// The cached access token of a connection with its kind and status. Only
    /// the access token is decrypted, so a mediated request that reuses a
    /// valid token does not open the connection's other secrets.
    pub async fn extension_connection_access_token(
        &self,
        extension_id: &str,
        connection: &str,
    ) -> Result<ConnectionAccessToken, RepositoryError> {
        #[derive(FromRow)]
        struct AccessTokenRow {
            id: Uuid,
            kind: String,
            #[sqlx(try_from = "String")]
            status: ConnectionStatus,
            access_token_ciphertext: Option<Vec<u8>>,
            access_token_expires_at: Option<DateTime<Utc>>,
            last_refreshed_at: Option<DateTime<Utc>>,
        }
        let row: AccessTokenRow = sqlx::query_as("SELECT id,kind,status,access_token_ciphertext,access_token_expires_at,last_refreshed_at FROM extension_connections WHERE workspace_id=$1 AND extension_id=$2 AND connection=$3")
            .bind(self.workspace_id()).bind(extension_id).bind(connection)
            .fetch_optional(&self.pool).await?
            .ok_or(RepositoryError::NotFound("extension connection"))?;
        Ok(ConnectionAccessToken {
            kind: row.kind,
            status: row.status,
            access_token: open_optional(row.id, "access_token", row.access_token_ciphertext)?,
            access_token_expires_at: row.access_token_expires_at,
            last_refreshed_at: row.last_refreshed_at,
        })
    }

    /// Claims a connection for a token refresh or device-code poll. Returns
    /// `None` while another worker's claim is live.
    pub async fn claim_extension_connection_refresh(
        &self,
        extension_id: &str,
        connection: &str,
    ) -> Result<Option<ConnectionRefresh>, RepositoryError> {
        let claim = Uuid::new_v4();
        let mut transaction = self.pool.begin().await?;
        let claimed: Option<Uuid> = sqlx::query_scalar("UPDATE extension_connections SET refresh_claim=$4,refresh_claimed_until=clock_timestamp()+make_interval(secs => $5) WHERE workspace_id=$1 AND extension_id=$2 AND connection=$3 AND (refresh_claim IS NULL OR refresh_claimed_until<=clock_timestamp()) RETURNING id")
            .bind(self.workspace_id()).bind(extension_id).bind(connection).bind(claim)
            .bind(REFRESH_CLAIM.num_seconds() as f64)
            .fetch_optional(&mut *transaction).await?;
        if claimed.is_none() {
            // End the transaction before distinguishing a missing connection
            // from a busy one, so a waiter holds one pooled connection.
            transaction.rollback().await?;
            self.extension_connection(extension_id, connection).await?;
            return Ok(None);
        }
        let row: CredentialRow = sqlx::query_as(&format!("SELECT {CREDENTIAL_FIELDS} FROM extension_connections WHERE workspace_id=$1 AND extension_id=$2 AND connection=$3"))
            .bind(self.workspace_id()).bind(extension_id).bind(connection)
            .fetch_one(&mut *transaction).await?;
        let basis = RefreshBasis {
            client_secret: row.client_secret_ciphertext.clone(),
            refresh_token: row.refresh_token_ciphertext.clone(),
            token_url: row.token_url.clone(),
            scope: row.scope.clone(),
        };
        // Credentials that cannot be decrypted roll the claim back.
        let credentials = row.decrypt()?;
        transaction.commit().await?;
        Ok(Some(ConnectionRefresh {
            pool: self.pool.clone(),
            workspace_id: self.workspace_id(),
            claim,
            settled: false,
            basis,
            credentials,
        }))
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
            .bind(self.workspace_id()).bind(extension_id).bind(connection)
            .fetch_optional(&mut *transaction).await?;
        let Some((id, ciphertext)) = row else {
            return Ok(());
        };
        if open_optional(id, "access_token", ciphertext)?.as_deref() == Some(rejected) {
            sqlx::query("UPDATE extension_connections SET access_token_ciphertext=NULL,access_token_expires_at=NULL,updated_at=clock_timestamp() WHERE workspace_id=$1 AND id=$2")
                .bind(self.workspace_id()).bind(id)
                .execute(&mut *transaction).await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    /// Stores a started device authorization for an operator to approve. A
    /// connection that is already active keeps working on its current tokens
    /// until the new authorization is approved.
    pub async fn record_extension_device_authorization(
        &self,
        extension_id: &str,
        connection: &str,
        authorization: &DeviceAuthorization,
    ) -> Result<ExtensionConnection, RepositoryError> {
        let current = self.extension_connection(extension_id, connection).await?;
        let device_code = seal(current.id, "device_code", &authorization.device_code)?;
        let mut transaction = self.pool.begin().await?;
        let updated = sqlx::query_as(&format!("UPDATE extension_connections SET device_code_ciphertext=$3,device_user_code=$4,device_verification_uri=$5,device_expires_at=$6,device_interval_seconds=$7,device_polled_at=NULL,status=CASE WHEN status='active' AND (access_token_ciphertext IS NOT NULL OR refresh_token_ciphertext IS NOT NULL) THEN 'active' ELSE 'pending_authorization' END,last_error=NULL,updated_at=clock_timestamp() WHERE workspace_id=$1 AND id=$2 RETURNING {CONNECTION_FIELDS}"))
            .bind(self.workspace_id()).bind(current.id).bind(device_code)
            .bind(&authorization.user_code).bind(&authorization.verification_uri)
            .bind(authorization.expires_at).bind(authorization.interval_seconds.clamp(1, 3600))
            .fetch_one(&mut *transaction).await?;
        self.write_audit_event(&mut transaction).await?;
        transaction.commit().await?;
        Ok(updated)
    }

    /// Stores a started browser authorization: the SHA-256 of its `state`,
    /// its PKCE code verifier and the binding of the browser session that
    /// started it, which the public OAuth callback takes back once, from that
    /// session. A new request replaces an earlier one. A connection that is
    /// already active keeps working on its current tokens until the new
    /// authorization completes.
    pub async fn record_extension_authorization_request(
        &self,
        extension_id: &str,
        connection: &str,
        state_sha256: &str,
        code_verifier: &str,
        session_sha256: &[u8; 32],
        expires_at: DateTime<Utc>,
    ) -> Result<ExtensionConnection, RepositoryError> {
        let current = self.extension_connection(extension_id, connection).await?;
        let verifier = seal(current.id, "authorization_verifier", code_verifier)?;
        let mut transaction = self.pool.begin().await?;
        let updated = sqlx::query_as(&format!("UPDATE extension_connections SET authorization_state_sha256=$3,authorization_verifier_ciphertext=$4,authorization_expires_at=$5,authorization_session_sha256=$6,status=CASE WHEN status='active' AND (access_token_ciphertext IS NOT NULL OR refresh_token_ciphertext IS NOT NULL) THEN 'active' ELSE 'pending_authorization' END,last_error=NULL,updated_at=clock_timestamp() WHERE workspace_id=$1 AND id=$2 RETURNING {CONNECTION_FIELDS}"))
            .bind(self.workspace_id()).bind(current.id).bind(state_sha256).bind(verifier).bind(expires_at)
            .bind(session_sha256.as_slice())
            .fetch_one(&mut *transaction).await?;
        self.write_audit_event(&mut transaction).await?;
        transaction.commit().await?;
        Ok(updated)
    }

    /// Takes the pending browser authorization whose `state` hashes to
    /// `state_sha256`, so a callback can complete it at most once, and only
    /// from the browser session that started it: a request bound to another
    /// session is left in place, so a leaked `state` cannot use it up.
    /// Returns its PKCE code verifier, or `None` when it is unknown, used,
    /// expired or bound to another session.
    pub async fn take_extension_authorization_request(
        &self,
        extension_id: &str,
        connection: &str,
        state_sha256: &str,
        session_sha256: &[u8; 32],
    ) -> Result<Option<String>, RepositoryError> {
        let taken: Option<(Uuid, Vec<u8>, bool)> = sqlx::query_as("UPDATE extension_connections c SET authorization_state_sha256=NULL,authorization_verifier_ciphertext=NULL,authorization_expires_at=NULL,authorization_session_sha256=NULL FROM (SELECT id,authorization_verifier_ciphertext AS verifier,authorization_expires_at>clock_timestamp() AS live FROM extension_connections WHERE workspace_id=$1 AND extension_id=$2 AND connection=$3 AND authorization_state_sha256=$4 AND authorization_session_sha256=$5 FOR UPDATE) p WHERE c.id=p.id RETURNING c.id,p.verifier,p.live")
            .bind(self.workspace_id()).bind(extension_id).bind(connection).bind(state_sha256)
            .bind(session_sha256.as_slice())
            .fetch_optional(&self.pool).await?;
        match taken {
            Some((id, verifier, true)) => Ok(Some(open(id, "authorization_verifier", &verifier)?)),
            _ => Ok(None),
        }
    }

    /// Records why a browser authorization did not complete, such as the
    /// operator declining at the provider. An active connection keeps its
    /// tokens and status; one that was waiting for this authorization fails.
    pub async fn fail_extension_authorization(
        &self,
        extension_id: &str,
        connection: &str,
        message: &str,
    ) -> Result<(), RepositoryError> {
        let message: String = message.chars().take(1024).collect();
        sqlx::query("UPDATE extension_connections SET status=CASE WHEN status IN ('active','revoked') THEN status ELSE 'failed' END,last_error=$4,updated_at=clock_timestamp() WHERE workspace_id=$1 AND extension_id=$2 AND connection=$3")
            .bind(self.workspace_id()).bind(extension_id).bind(connection).bind(message)
            .execute(&self.pool).await?;
        Ok(())
    }
}

impl AttricatRepository<super::SystemScope> {
    /// Resolves a pending browser authorization by the SHA-256 of its
    /// `state`, across workspaces, for the public OAuth callback. Returns
    /// the workspace, extension and connection; expired requests are not
    /// resolved.
    pub async fn resolve_extension_authorization_state(
        &self,
        state_sha256: &str,
    ) -> Result<Option<(Uuid, String, String)>, RepositoryError> {
        Ok(sqlx::query_as("SELECT workspace_id,extension_id,connection FROM extension_connections WHERE authorization_state_sha256=$1 AND authorization_expires_at>clock_timestamp()")
            .bind(state_sha256)
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
        let mut tampered = sealed;
        *tampered.last_mut().unwrap() ^= 1;
        assert!(open(id, "refresh_token", &tampered).is_err());
    }
}
