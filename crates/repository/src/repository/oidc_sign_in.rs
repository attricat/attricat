//! Persistence of sign-ins through an identity provider. A verified
//! `(issuer, subject)` pair only ever signs in to a workspace whose provider is
//! that issuer, and an identity is linked to an account only by an invitation
//! or by the signed-in account itself, never by its email alone.

use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use attricat_domain::{permissions::Permission, states::SessionAuthMethod};

use super::{AttricatRepository, AuditContext, RepositoryError};
use crate::account::SessionDigest;

/// Outstanding requests a workspace keeps. Anonymous callers can start sign-ins
/// and the app sees no client address to limit them by, so past this the oldest
/// are dropped. Refusing new ones instead would let anyone block sign-in.
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
    pub pkce_verifier_digest: &'a [u8],
    pub nonce_digest: &'a [u8],
    pub workspace_id: Uuid,
    pub issuer: &'a str,
    pub intent: OidcIntent,
    pub expires_at: DateTime<Utc>,
}

/// A new session's secrets and lifetime.
pub struct NewOidcSession<'a> {
    pub id: Uuid,
    pub session: &'a SessionDigest,
    pub csrf: &'a SessionDigest,
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

/// Whether an address a provider proved is the one invited. A non-ASCII
/// character that lowercases to ASCII, like the Kelvin sign, is a different
/// address at the provider that would otherwise compare equal here.
fn email_matches_invitation(email: &str, invitee_email: &str) -> bool {
    let email = email.trim();
    email.chars().all(|character| {
        character.is_ascii() || !character.to_lowercase().all(|lower| lower.is_ascii())
    }) && email.to_lowercase() == invitee_email
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
    /// Stores a request, dropping the workspace's oldest beyond the limit.
    pub async fn insert_oidc_login_request(
        &self,
        request: &NewOidcLoginRequest<'_>,
    ) -> Result<(), RepositoryError> {
        let (invitation_digest, session_digest): (Option<&[u8]>, Option<&[u8]>) =
            match &request.intent {
                OidcIntent::SignIn => (None, None),
                OidcIntent::Invitation { invitation_digest } => {
                    (Some(invitation_digest.as_slice()), None)
                }
                OidcIntent::Link { session_digest } => (None, Some(session_digest.as_ref())),
            };
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO oidc_login_requests (state_digest, binding_digest, pkce_verifier_digest, nonce_digest, workspace_id, issuer, intent, invitation_digest, link_session_digest, expires_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)")
            .bind(request.state_digest)
            .bind(request.binding_digest)
            .bind(request.pkce_verifier_digest)
            .bind(request.nonce_digest)
            .bind(request.workspace_id)
            .bind(request.issuer)
            .bind(request.intent.code())
            .bind(invitation_digest)
            .bind(session_digest)
            .bind(request.expires_at)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM oidc_login_requests WHERE state_digest IN (SELECT state_digest FROM oidc_login_requests WHERE workspace_id = $1 ORDER BY expires_at DESC OFFSET $2)")
            .bind(request.workspace_id)
            .bind(MAX_PENDING_REQUESTS_PER_WORKSPACE)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Takes a live request of this workspace, once, for the browser that
    /// started it. Every secret in the browser's cookie must be the one stored
    /// with the request, so values taken from another sign-in cannot be swapped
    /// in. A request presented wrongly stays usable by its own browser.
    pub async fn take_oidc_login_request(
        &self,
        workspace_id: Uuid,
        state_digest: &[u8],
        binding_digest: &[u8],
        pkce_verifier_digest: &[u8],
        nonce_digest: &[u8],
    ) -> Result<Option<OidcLoginRequest>, RepositoryError> {
        let row = sqlx::query("DELETE FROM oidc_login_requests WHERE state_digest = $1 AND binding_digest = $2 AND pkce_verifier_digest = $3 AND nonce_digest = $4 AND workspace_id = $5 AND expires_at > clock_timestamp() RETURNING workspace_id, issuer, intent, invitation_digest, link_session_digest")
            .bind(state_digest)
            .bind(binding_digest)
            .bind(pkce_verifier_digest)
            .bind(nonce_digest)
            .bind(workspace_id)
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
        workspace_id: Uuid,
        user_id: Uuid,
        issuer: &str,
        subject: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query("INSERT INTO external_identities (workspace_id, issuer, subject, user_id) VALUES ($1, $2, $3, $4) ON CONFLICT (workspace_id, issuer, subject) DO NOTHING")
            .bind(workspace_id)
            .bind(issuer)
            .bind(subject)
            .bind(user_id)
            .execute(&mut **tx)
            .await?;
        let owner: Uuid = sqlx::query_scalar(
            "SELECT user_id FROM external_identities WHERE workspace_id = $1 AND issuer = $2 AND subject = $3",
        )
        .bind(workspace_id)
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
        Self::link_identity_on(&mut tx, workspace_id, user_id, issuer, subject).await?;
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
    /// used by a different person. The acceptance and the session it gives
    /// succeed or fail together. Returns the invited user.
    pub async fn accept_invitation_with_identity(
        &self,
        workspace_id: Uuid,
        invitation_digest: &[u8],
        issuer: &str,
        subject: &str,
        email: &str,
        session: NewOidcSession<'_>,
    ) -> Result<Uuid, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        Self::lock_invitation_acceptance_on(&mut tx, invitation_digest).await?;
        // Checked here so a provider changed during the token exchange cannot
        // spend the invitation and leave the invitee with no way in.
        let provider_current: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM workspace_identity_providers WHERE workspace_id = $1 AND issuer = $2 AND enabled FOR SHARE)",
        )
        .bind(workspace_id)
        .bind(issuer)
        .fetch_one(&mut *tx)
        .await?;
        if !provider_current {
            return Err(RepositoryError::NotFound("identity provider"));
        }
        let invitation: Option<(Uuid, String)> = sqlx::query_as(
            "SELECT id, invitee_email FROM workspace_invitations WHERE token_digest = $1 AND workspace_id = $2 AND accepted_at IS NULL AND revoked_at IS NULL AND expires_at > clock_timestamp()",
        )
        .bind(invitation_digest)
        .bind(workspace_id)
        .fetch_optional(&mut *tx)
        .await?;
        let (invitation_id, invitee_email) =
            invitation.ok_or(RepositoryError::InvitationInvalid)?;
        if !email_matches_invitation(email, &invitee_email) {
            return Err(RepositoryError::InvitationEmailMismatch);
        }
        let user_id = Self::upsert_invited_user_on(&mut tx, &invitee_email, None)
            .await?
            .user_id;
        Self::link_identity_on(&mut tx, workspace_id, user_id, issuer, subject).await?;
        // The onboarding link would set a password this person does not need.
        sqlx::query("UPDATE user_lifecycle_action_tokens a SET revoked_at = clock_timestamp() FROM workspace_invitation_onboarding o WHERE o.invitation_id = $1 AND a.token_digest = o.action_token_digest AND a.consumed_at IS NULL AND a.revoked_at IS NULL")
            .bind(invitation_id)
            .execute(&mut *tx)
            .await?;
        self.accept_invitation_on(&mut tx, invitation_digest, user_id)
            .await?;
        if !Self::insert_oidc_session_on(&mut tx, user_id, workspace_id, &session).await? {
            return Err(RepositoryError::InvitationInvalid);
        }
        self.for_workspace(workspace_id)
            .with_audit_context(user_audit_context(
                SIGNED_IN_ACTION,
                user_id,
                json!({ "method": SessionAuthMethod::Oidc.as_str() }),
            ))
            .write_audit_event(&mut tx)
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

    /// Issues a session for a verified identity in one transaction under the
    /// workspace lock, which membership revocation and unlinking also take, so
    /// neither can slip between the checks and the session. The identity must
    /// be linked here and its issuer must still be this workspace's enabled
    /// provider. Returns the user, or `None` if any of that fails.
    pub async fn issue_session_for_identity(
        &self,
        workspace_id: Uuid,
        issuer: &str,
        subject: &str,
        session: NewOidcSession<'_>,
    ) -> Result<Option<Uuid>, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "SELECT id FROM workspaces WHERE id = $1 AND deleted_at IS NULL FOR NO KEY UPDATE",
        )
        .bind(workspace_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(RepositoryError::NotFound("workspace"))?;
        let user_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT i.user_id FROM external_identities i JOIN workspace_identity_providers p ON p.workspace_id = i.workspace_id AND p.issuer = i.issuer AND p.enabled WHERE i.workspace_id = $1 AND i.issuer = $2 AND i.subject = $3 FOR SHARE OF p",
        )
        .bind(workspace_id)
        .bind(issuer)
        .bind(subject)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(user_id) = user_id else {
            return Ok(None);
        };
        if !Self::insert_oidc_session_on(&mut tx, user_id, workspace_id, &session).await? {
            return Ok(None);
        }
        self.for_workspace(workspace_id)
            .with_audit_context(user_audit_context(
                SIGNED_IN_ACTION,
                user_id,
                json!({ "method": SessionAuthMethod::Oidc.as_str() }),
            ))
            .commit_mutation(tx)
            .await?;
        Ok(Some(user_id))
    }

    /// Whether an account has a link to `issuer` in this workspace.
    pub async fn has_external_identity(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        issuer: &str,
    ) -> Result<bool, RepositoryError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM external_identities WHERE workspace_id = $1 AND user_id = $2 AND issuer = $3)",
        )
        .bind(workspace_id)
        .bind(user_id)
        .bind(issuer)
        .fetch_one(&self.pool)
        .await?)
    }
}

impl AttricatRepository {
    /// Removes a member's links made in this workspace and ends the sessions
    /// they gave them here. Links and sessions in other workspaces are separate
    /// and stay. Returns `false` if there was no link.
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
        let removed =
            sqlx::query("DELETE FROM external_identities WHERE workspace_id = $1 AND user_id = $2")
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
