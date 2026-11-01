//! Persistence of sign-ins through an identity provider. A verified
//! `(issuer, subject)` pair only ever signs in to a workspace whose provider is
//! that issuer, and an identity is linked to an account only by an invitation
//! or by the signed-in account itself, never by its email alone.

use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use attricat_domain::permissions::Permission;

use super::{AttricatRepository, AuditContext, RepositoryError};
use crate::account::SessionDigest;

/// Outstanding requests a workspace may have. Without a client address there
/// is no per-caller limit, so this only bounds what anonymous callers can store.
const MAX_PENDING_REQUESTS_PER_WORKSPACE: i64 = 10_000;

const SIGNED_IN_ACTION: &str = "auth.sign_in";
const SIGNED_OUT_ACTION: &str = "auth.sign_out";
const IDENTITY_LINKED_ACTION: &str = "auth.identity.link";
const IDENTITY_UNLINKED_ACTION: &str = "auth.identity.unlink";

/// What a sign-in at the provider is meant to accomplish.
pub enum OidcIntent {
    /// Sign in with an identity that is already linked.
    SignIn,
    /// Accept an invitation, linking the identity to the invited account.
    Invitation { invitation_digest: Vec<u8> },
    /// Link the identity to the account signed in on this browser session,
    /// which also proves the provider works.
    Link { session_digest: SessionDigest },
}

impl OidcIntent {
    fn code(&self) -> &'static str {
        match self {
            Self::SignIn => "sign_in",
            Self::Invitation { .. } => "invitation",
            Self::Link { .. } => "link",
        }
    }
}

pub struct NewOidcLoginRequest<'a> {
    pub state_digest: &'a [u8],
    pub binding_digest: &'a [u8],
    pub workspace_id: Uuid,
    pub issuer: &'a str,
    pub intent: OidcIntent,
    pub expires_at: DateTime<Utc>,
}

pub struct OidcLoginRequest {
    pub workspace_id: Uuid,
    pub issuer: String,
    pub intent: OidcIntent,
}

fn user_audit_context(action: &str, user_id: Uuid, metadata: Value) -> AuditContext {
    AuditContext {
        actor_user_id: Some(user_id),
        actor_token_id: None,
        request_id: Uuid::new_v4(),
        correlation_id: Uuid::new_v4(),
        action: action.to_owned(),
        authorization_scope: json!({ "type": "workspace" }),
        target: json!({ "type": "user", "id": user_id }),
        metadata,
        agent: None,
    }
}

impl AttricatRepository {
    /// Returns a copy whose next mutation is audited as `user_id` signing in
    /// with `method`. A sign-in has no request context yet.
    pub fn audited_as_sign_in(&self, user_id: Uuid, method: &str) -> Self {
        self.clone().with_audit_context(user_audit_context(
            SIGNED_IN_ACTION,
            user_id,
            json!({ "method": method }),
        ))
    }

    /// Returns a copy whose next mutation is audited as `user_id` signing out.
    pub fn audited_as_sign_out(&self, user_id: Uuid) -> Self {
        self.clone()
            .with_audit_context(user_audit_context(SIGNED_OUT_ACTION, user_id, json!({})))
    }
}

impl<S: super::RepositoryScope> AttricatRepository<S> {
    /// Stores a request, or returns `false` if the workspace has too many.
    pub async fn insert_oidc_login_request(
        &self,
        request: &NewOidcLoginRequest<'_>,
    ) -> Result<bool, RepositoryError> {
        let (invitation_digest, session_digest): (Option<&[u8]>, Option<&[u8]>) =
            match &request.intent {
                OidcIntent::SignIn => (None, None),
                OidcIntent::Invitation { invitation_digest } => {
                    (Some(invitation_digest.as_slice()), None)
                }
                OidcIntent::Link { session_digest } => (None, Some(session_digest.as_ref())),
            };
        let inserted = sqlx::query("INSERT INTO oidc_login_requests (state_digest, binding_digest, workspace_id, issuer, intent, invitation_digest, link_session_digest, expires_at) SELECT $1, $2, $3, $4, $5, $6, $7, $8 WHERE (SELECT count(*) FROM oidc_login_requests WHERE workspace_id = $3 AND expires_at > clock_timestamp()) < $9")
            .bind(request.state_digest)
            .bind(request.binding_digest)
            .bind(request.workspace_id)
            .bind(request.issuer)
            .bind(request.intent.code())
            .bind(invitation_digest)
            .bind(session_digest)
            .bind(request.expires_at)
            .bind(MAX_PENDING_REQUESTS_PER_WORKSPACE)
            .execute(&self.pool)
            .await?
            .rows_affected();
        Ok(inserted == 1)
    }

    /// Takes a live request, once, for the browser that started it. A request
    /// presented by another browser stays usable by its own.
    pub async fn take_oidc_login_request(
        &self,
        state_digest: &[u8],
        binding_digest: &[u8],
    ) -> Result<Option<OidcLoginRequest>, RepositoryError> {
        let row = sqlx::query("DELETE FROM oidc_login_requests WHERE state_digest = $1 AND binding_digest = $2 AND expires_at > clock_timestamp() RETURNING workspace_id, issuer, intent, invitation_digest, link_session_digest")
            .bind(state_digest)
            .bind(binding_digest)
            .fetch_optional(&self.pool)
            .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let intent = match row.try_get::<String, _>("intent")?.as_str() {
            "invitation" => OidcIntent::Invitation {
                invitation_digest: row.try_get("invitation_digest")?,
            },
            "link" => OidcIntent::Link {
                session_digest: SessionDigest::from_slice(
                    &row.try_get::<Vec<u8>, _>("link_session_digest")?,
                )
                .map_err(|_| sqlx::Error::Protocol("stored session digest is invalid".into()))?,
            },
            _ => OidcIntent::SignIn,
        };
        Ok(Some(OidcLoginRequest {
            workspace_id: row.try_get("workspace_id")?,
            issuer: row.try_get("issuer")?,
            intent,
        }))
    }

    /// Links an identity in the caller's transaction. Repeating a link is
    /// harmless; an identity linked to someone else is refused, never moved.
    async fn link_identity_on(
        tx: &mut Transaction<'_, Postgres>,
        user_id: Uuid,
        issuer: &str,
        subject: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query("INSERT INTO external_identities (issuer, subject, user_id) VALUES ($1, $2, $3) ON CONFLICT (issuer, subject) DO NOTHING")
            .bind(issuer)
            .bind(subject)
            .bind(user_id)
            .execute(&mut **tx)
            .await?;
        let owner: Uuid = sqlx::query_scalar(
            "SELECT user_id FROM external_identities WHERE issuer = $1 AND subject = $2",
        )
        .bind(issuer)
        .bind(subject)
        .fetch_one(&mut **tx)
        .await?;
        if owner == user_id {
            Ok(())
        } else {
            Err(RepositoryError::IdentityLinkedElsewhere)
        }
    }

    /// Links an identity to the account signed in on this browser. When that
    /// account can manage the workspace, this proves the provider works and
    /// marks it verified, which is what allows enabling it.
    pub async fn link_identity_to_session_user(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        issuer: &str,
        subject: &str,
    ) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;
        // The provider row serializes this with a configuration change, so a
        // proof of the old issuer cannot verify a new one.
        let current: Option<String> = sqlx::query_scalar(
            "SELECT issuer FROM workspace_identity_providers WHERE workspace_id = $1 FOR UPDATE",
        )
        .bind(workspace_id)
        .fetch_optional(&mut *tx)
        .await?;
        if current.as_deref() != Some(issuer) {
            return Err(RepositoryError::NotFound("identity provider"));
        }
        Self::link_identity_on(&mut tx, user_id, issuer, subject).await?;
        if Self::is_authorized_on(
            &mut tx,
            user_id,
            workspace_id,
            Permission::WorkspaceManage,
            None,
            None,
        )
        .await?
        {
            sqlx::query("UPDATE workspace_identity_providers SET verified_at = COALESCE(verified_at, clock_timestamp()), updated_at = clock_timestamp() WHERE workspace_id = $1")
                .bind(workspace_id)
                .execute(&mut *tx)
                .await?;
        }
        self.for_workspace(workspace_id)
            .with_audit_context(user_audit_context(
                IDENTITY_LINKED_ACTION,
                user_id,
                json!({ "issuer": issuer }),
            ))
            .commit_mutation(tx)
            .await
    }

    /// Accepts an invitation with a verified identity, linking it to the
    /// invited account. Holding the invitation is what authorizes the link;
    /// the matching address only guards against a forwarded invitation being
    /// used by a different person. Returns the invited user.
    pub async fn accept_invitation_with_identity(
        &self,
        workspace_id: Uuid,
        invitation_digest: &[u8],
        issuer: &str,
        subject: &str,
        email: &str,
    ) -> Result<Uuid, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        Self::lock_invitation_acceptance_on(&mut tx, invitation_digest).await?;
        let invitation: Option<(Uuid, String)> = sqlx::query_as(
            "SELECT id, invitee_email FROM workspace_invitations WHERE token_digest = $1 AND workspace_id = $2 AND accepted_at IS NULL AND revoked_at IS NULL AND expires_at > clock_timestamp()",
        )
        .bind(invitation_digest)
        .bind(workspace_id)
        .fetch_optional(&mut *tx)
        .await?;
        let (invitation_id, invitee_email) =
            invitation.ok_or(RepositoryError::InvitationInvalid)?;
        if email.trim().to_lowercase() != invitee_email {
            return Err(RepositoryError::InvitationEmailMismatch);
        }
        let user_id = Self::upsert_invited_user_on(&mut tx, &invitee_email, None)
            .await?
            .user_id;
        Self::link_identity_on(&mut tx, user_id, issuer, subject).await?;
        // The onboarding link would set a password this person does not need.
        sqlx::query("UPDATE user_lifecycle_action_tokens a SET revoked_at = clock_timestamp() FROM workspace_invitation_onboarding o WHERE o.invitation_id = $1 AND a.token_digest = o.action_token_digest AND a.consumed_at IS NULL AND a.revoked_at IS NULL")
            .bind(invitation_id)
            .execute(&mut *tx)
            .await?;
        self.accept_invitation_on(&mut tx, invitation_digest, user_id)
            .await?;
        self.for_workspace(workspace_id)
            .with_audit_context(user_audit_context(
                IDENTITY_LINKED_ACTION,
                user_id,
                json!({ "issuer": issuer, "invitation_id": invitation_id }),
            ))
            .commit_mutation(tx)
            .await?;
        Ok(user_id)
    }

    /// The identifier of the workspace a pending invitation belongs to, for
    /// someone holding its secret and not yet signed in.
    pub async fn invitation_login_identifier(
        &self,
        invitation_digest: &[u8],
    ) -> Result<Option<(Uuid, String)>, RepositoryError> {
        Ok(sqlx::query_as(
            "SELECT w.id, w.login_identifier FROM workspace_invitations i JOIN workspaces w ON w.id = i.workspace_id WHERE i.token_digest = $1 AND i.accepted_at IS NULL AND i.revoked_at IS NULL AND i.expires_at > clock_timestamp() AND w.deleted_at IS NULL",
        )
        .bind(invitation_digest)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// Whether an account has an identity at `issuer`.
    pub async fn has_external_identity(
        &self,
        user_id: Uuid,
        issuer: &str,
    ) -> Result<bool, RepositoryError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM external_identities WHERE user_id = $1 AND issuer = $2)",
        )
        .bind(user_id)
        .bind(issuer)
        .fetch_one(&self.pool)
        .await?)
    }
}

impl AttricatRepository {
    /// Removes a member's link to this workspace's provider and ends the
    /// sessions it gave them here. Their sessions in other workspaces are
    /// separate and stay. Returns `false` if there was no link.
    pub async fn unlink_member_identity(
        &self,
        membership_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        let workspace_id = self.workspace_id();
        let mut tx = self.pool.begin().await?;
        Self::lock_invitation_changes_on(&mut tx, workspace_id).await?;
        let user_id: Uuid = sqlx::query_scalar(
            "SELECT user_id FROM workspace_memberships WHERE id = $1 AND workspace_id = $2",
        )
        .bind(membership_id)
        .bind(workspace_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(RepositoryError::NotFound("workspace member"))?;
        let removed = sqlx::query("DELETE FROM external_identities e USING workspace_identity_providers p WHERE p.workspace_id = $1 AND e.issuer = p.issuer AND e.user_id = $2")
            .bind(workspace_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        if removed == 0 {
            return Ok(false);
        }
        sqlx::query("UPDATE browser_sessions SET revoked_at = clock_timestamp() WHERE user_id = $1 AND workspace_id = $2 AND auth_method = 'oidc' AND revoked_at IS NULL")
            .bind(user_id)
            .bind(workspace_id)
            .execute(&mut *tx)
            .await?;
        self.with_workspace_audit_action(
            IDENTITY_UNLINKED_ACTION,
            json!({ "type": "user", "id": user_id }),
            json!({}),
        )
        .commit_mutation(tx)
        .await?;
        Ok(true)
    }
}
