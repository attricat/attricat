//! Inbound webhooks of extensions (#279).
//!
//! Operators create named endpoints of a release's manifest-declared
//! webhooks. An endpoint's public URL carries a 256-bit token of which only
//! the SHA-256 is stored. The public route resolves the endpoint across
//! workspaces by that digest, lets the component answer the request and,
//! when the component keeps a payload, stores it here and enqueues a
//! `webhook.received.v1` event addressed to that extension in the same
//! transaction, before the sender gets its response.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use ring::rand::{SecureRandom, SystemRandom};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use uuid::Uuid;

use super::{AttricatRepository, CodedResource, RepositoryError};
use crate::domain_events::{EventSource, EventSourceKind, NewDomainEvent, WEBHOOK_RECEIVED_V1};

/// How long kept webhook payloads stay readable before the retention sweep
/// removes them.
pub const WEBHOOK_RECEIPT_RETENTION_DAYS: i64 = 7;
/// The most endpoints one installation may have.
pub const MAX_WEBHOOK_ENDPOINTS: i64 = 64;
/// The longest dedupe key a component may give a kept payload.
pub const MAX_WEBHOOK_DEDUPE_KEY_BYTES: usize = 256;
/// The aggregate kind of `webhook.received.v1`: the receipt it announces.
pub const WEBHOOK_RECEIPT_AGGREGATE: &str = "webhook_receipt";
/// The event source of `webhook.received.v1`.
const WEBHOOK_EVENT_SOURCE: &str = "webhook";

/// The SHA-256 under which an endpoint's URL token is stored.
pub fn webhook_token_digest(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

/// A new 256-bit URL token, base64url-encoded without padding (43
/// characters), and its digest.
fn new_webhook_token() -> Result<(String, [u8; 32]), RepositoryError> {
    let mut bytes = [0u8; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| RepositoryError::InvalidExtension("random source unavailable".into()))?;
    let token = URL_SAFE_NO_PAD.encode(bytes);
    let digest = webhook_token_digest(&token);
    Ok((token, digest))
}

/// An endpoint as operators see it: never its token.
#[derive(Clone, Debug, FromRow, Serialize)]
pub struct ExtensionWebhookEndpoint {
    pub id: Uuid,
    pub extension_id: String,
    pub webhook: String,
    pub endpoint: String,
    pub created_at: DateTime<Utc>,
    pub token_rotated_at: Option<DateTime<Utc>>,
    pub last_request_at: Option<DateTime<Utc>>,
    pub last_reply_status: Option<i16>,
}

/// An endpoint with the URL token that was just issued for it. The token is
/// never stored and cannot be shown again.
#[derive(Clone, Debug)]
pub struct IssuedWebhookEndpoint {
    pub endpoint: ExtensionWebhookEndpoint,
    pub token: String,
}

/// The endpoint a public request's token names.
#[derive(Clone, Debug, FromRow)]
pub struct ResolvedWebhookEndpoint {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub extension_id: String,
    pub webhook: String,
    pub endpoint: String,
}

/// What became of a payload the component kept.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeptWebhookPayload {
    /// Stored as this receipt, and its event enqueued.
    Stored(Uuid),
    /// A payload with the same dedupe key is already stored.
    Duplicate,
}

/// A kept payload, as the extension's handler reads it back.
#[derive(Clone, Debug, FromRow, Serialize)]
pub struct ExtensionWebhookReceipt {
    pub id: Uuid,
    pub webhook: String,
    pub endpoint: String,
    pub dedupe_key: Option<String>,
    pub payload: String,
    pub received_at: DateTime<Utc>,
}

const ENDPOINT_FIELDS: &str = "id,extension_id,webhook,endpoint,created_at,token_rotated_at,last_request_at,last_reply_status";

/// Endpoint names share connection keys' alphabet, so they are safe in URLs
/// and logs.
fn check_endpoint_name(endpoint: &str) -> Result<(), RepositoryError> {
    if super::valid_connection_key(endpoint) {
        Ok(())
    } else {
        Err(RepositoryError::InvalidExtension(
            "endpoint names are 1-128 letters, digits, '.', '_' or '-'".into(),
        ))
    }
}

impl AttricatRepository {
    pub async fn list_extension_webhook_endpoints(
        &self,
        extension_id: &str,
    ) -> Result<Vec<ExtensionWebhookEndpoint>, RepositoryError> {
        Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {ENDPOINT_FIELDS} FROM extension_webhook_endpoints WHERE workspace_id=$1 AND extension_id=$2 ORDER BY endpoint")))
            .bind(self.workspace_id()).bind(extension_id)
            .fetch_all(&self.pool).await?)
    }

    /// Creates an endpoint of `webhook` named `endpoint` and issues its URL
    /// token. The caller has checked that the installed release declares the
    /// webhook.
    pub async fn create_extension_webhook_endpoint(
        &self,
        extension_id: &str,
        webhook: &str,
        endpoint: &str,
    ) -> Result<IssuedWebhookEndpoint, RepositoryError> {
        check_endpoint_name(endpoint)?;
        let (token, digest) = new_webhook_token()?;
        let mut transaction = self.pool.begin().await?;
        // Serializes concurrent creations of one installation, so the count
        // below cannot be raced past the limit.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1 || ':' || $2, 0))")
            .bind(self.workspace_id().to_string())
            .bind(format!("webhook-endpoints:{extension_id}"))
            .execute(&mut *transaction)
            .await?;
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM extension_webhook_endpoints WHERE workspace_id=$1 AND extension_id=$2")
            .bind(self.workspace_id()).bind(extension_id)
            .fetch_one(&mut *transaction).await?;
        if count >= MAX_WEBHOOK_ENDPOINTS {
            return Err(RepositoryError::InvalidExtension(format!(
                "an extension can have at most {MAX_WEBHOOK_ENDPOINTS} webhook endpoints"
            )));
        }
        let created = sqlx::query_as(sqlx::AssertSqlSafe(format!("INSERT INTO extension_webhook_endpoints(id,workspace_id,extension_id,webhook,endpoint,token_sha256) VALUES($1,$2,$3,$4,$5,$6) RETURNING {ENDPOINT_FIELDS}")))
            .bind(Uuid::new_v4()).bind(self.workspace_id()).bind(extension_id)
            .bind(webhook).bind(endpoint).bind(digest.as_slice())
            .fetch_one(&mut *transaction).await
            .map_err(|error| match error {
                sqlx::Error::Database(database) if database.is_unique_violation() => {
                    RepositoryError::CodeTaken(CodedResource::WebhookEndpoint)
                }
                error => error.into(),
            })?;
        self.write_audit_event(&mut transaction).await?;
        transaction.commit().await?;
        Ok(IssuedWebhookEndpoint {
            endpoint: created,
            token,
        })
    }

    /// Replaces an endpoint's URL token. The previous URL stops working at
    /// once; payloads already kept are still delivered.
    pub async fn rotate_extension_webhook_endpoint(
        &self,
        extension_id: &str,
        endpoint: &str,
    ) -> Result<IssuedWebhookEndpoint, RepositoryError> {
        let (token, digest) = new_webhook_token()?;
        let mut transaction = self.pool.begin().await?;
        let rotated = sqlx::query_as(sqlx::AssertSqlSafe(format!("UPDATE extension_webhook_endpoints SET token_sha256=$4,token_rotated_at=clock_timestamp() WHERE workspace_id=$1 AND extension_id=$2 AND endpoint=$3 RETURNING {ENDPOINT_FIELDS}")))
            .bind(self.workspace_id()).bind(extension_id).bind(endpoint).bind(digest.as_slice())
            .fetch_optional(&mut *transaction).await?
            .ok_or(RepositoryError::NotFound("webhook endpoint"))?;
        self.write_audit_event(&mut transaction).await?;
        transaction.commit().await?;
        Ok(IssuedWebhookEndpoint {
            endpoint: rotated,
            token,
        })
    }

    /// Deletes an endpoint; its URL stops working. Payloads it kept stay until
    /// they are delivered and retained.
    pub async fn delete_extension_webhook_endpoint(
        &self,
        extension_id: &str,
        endpoint: &str,
    ) -> Result<bool, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let deleted = sqlx::query("DELETE FROM extension_webhook_endpoints WHERE workspace_id=$1 AND extension_id=$2 AND endpoint=$3")
            .bind(self.workspace_id()).bind(extension_id).bind(endpoint)
            .execute(&mut *transaction).await?.rows_affected() == 1;
        if deleted {
            self.write_audit_event(&mut transaction).await?;
        }
        transaction.commit().await?;
        Ok(deleted)
    }

    /// Stores a payload the component kept and enqueues its
    /// `webhook.received.v1` event in one transaction. A payload whose dedupe
    /// key the endpoint already stored is not stored again.
    pub async fn keep_extension_webhook_payload(
        &self,
        endpoint: &ResolvedWebhookEndpoint,
        payload: &str,
        dedupe_key: Option<&str>,
    ) -> Result<KeptWebhookPayload, RepositoryError> {
        let id = Uuid::new_v4();
        let mut transaction = self.pool.begin().await?;
        let stored: Option<DateTime<Utc>> = sqlx::query_scalar("INSERT INTO extension_webhook_receipts(id,workspace_id,extension_id,webhook,endpoint,dedupe_key,payload) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT (workspace_id,extension_id,endpoint,dedupe_key) DO NOTHING RETURNING received_at")
            .bind(id).bind(self.workspace_id()).bind(&endpoint.extension_id)
            .bind(&endpoint.webhook).bind(&endpoint.endpoint).bind(dedupe_key).bind(payload)
            .fetch_optional(&mut *transaction).await?;
        let Some(received_at) = stored else {
            return Ok(KeptWebhookPayload::Duplicate);
        };
        let event = NewDomainEvent {
            event_type: WEBHOOK_RECEIVED_V1.to_owned(),
            aggregate_kind: WEBHOOK_RECEIPT_AGGREGATE.to_owned(),
            aggregate_id: id,
            correlation_id: Uuid::new_v4(),
            causation_id: None,
            source: EventSource {
                kind: EventSourceKind::System,
                name: WEBHOOK_EVENT_SOURCE.to_owned(),
            },
            metadata: json!({}),
            payload: json!({
                "extension_id": endpoint.extension_id,
                "webhook": endpoint.webhook,
                "endpoint": endpoint.endpoint,
                "receipt_id": id,
                "dedupe_key": dedupe_key,
                "received_at": received_at,
            }),
        };
        self.enqueue_events(&mut transaction, vec![event]).await?;
        transaction.commit().await?;
        Ok(KeptWebhookPayload::Stored(id))
    }

    /// Notes the time and reply status of an endpoint's latest request.
    pub async fn record_extension_webhook_request(
        &self,
        endpoint_id: Uuid,
        status: u16,
    ) -> Result<(), RepositoryError> {
        sqlx::query("UPDATE extension_webhook_endpoints SET last_request_at=clock_timestamp(),last_reply_status=$3 WHERE workspace_id=$1 AND id=$2")
            .bind(self.workspace_id()).bind(endpoint_id).bind(status as i16)
            .execute(&self.pool).await?;
        Ok(())
    }

    /// A payload the extension kept, until the retention sweep removes it.
    pub async fn extension_webhook_receipt(
        &self,
        extension_id: &str,
        receipt_id: Uuid,
    ) -> Result<Option<ExtensionWebhookReceipt>, RepositoryError> {
        Ok(sqlx::query_as("SELECT id,webhook,endpoint,dedupe_key,payload,received_at FROM extension_webhook_receipts WHERE workspace_id=$1 AND extension_id=$2 AND id=$3")
            .bind(self.workspace_id()).bind(extension_id).bind(receipt_id)
            .fetch_optional(&self.pool).await?)
    }
}

impl AttricatRepository<super::SystemScope> {
    /// Resolves a public webhook request's endpoint by the SHA-256 of its URL
    /// token, across workspaces.
    pub async fn resolve_extension_webhook_endpoint(
        &self,
        token_sha256: &[u8; 32],
    ) -> Result<Option<ResolvedWebhookEndpoint>, RepositoryError> {
        Ok(sqlx::query_as("SELECT id,workspace_id,extension_id,webhook,endpoint FROM extension_webhook_endpoints WHERE token_sha256=$1")
            .bind(token_sha256.as_slice())
            .fetch_optional(&self.pool).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_random_and_stored_only_as_their_digest() {
        let (first, digest) = new_webhook_token().unwrap();
        let (second, _) = new_webhook_token().unwrap();
        assert_eq!(first.len(), 43);
        assert_ne!(first, second);
        assert_eq!(digest, webhook_token_digest(&first));
        assert!(check_endpoint_name("shop-eu.1").is_ok());
        assert!(check_endpoint_name("shop eu").is_err());
    }
}
