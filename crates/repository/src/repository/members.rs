use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use super::{CatalogRepository, RepositoryError};

const OWNER_ROLE_ID: Uuid = Uuid::from_u128(0x00000000000040008000000000000101);
type WorkspaceUserInvitation = (Uuid, Uuid, String, Uuid, Vec<u8>, DateTime<Utc>);

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct WorkspaceMember {
    pub id: Uuid,
    pub user_id: Uuid,
    pub email: String,
    pub display_name: Option<String>,
    /// The member's ready avatar in this workspace.
    pub avatar_file_id: Option<Uuid>,
    pub state: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub grants: Value,
}
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct WorkspaceInvitation {
    pub id: Uuid,
    pub invitee_email: String,
    pub inviter_user_id: Uuid,
    pub inviter_email: String,
    pub role_id: Uuid,
    pub role_code: String,
    pub scope_type: String,
    pub scope_target_id: Uuid,
    pub expires_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub accepted_by_user_id: Option<Uuid>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub struct CreatedWorkspaceUser {
    pub user_id: Uuid,
    pub invitation_id: Option<Uuid>,
    pub needs_password_setup: bool,
}

pub struct CompletedWorkspaceOnboarding {
    pub membership_id: Uuid,
    pub user_id: Uuid,
    pub workspace_id: Uuid,
}

impl<S: super::RepositoryScope> CatalogRepository<S> {
    pub async fn create_workspace_user(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
        email: &str,
        display_name: Option<&str>,
        invitation: Option<WorkspaceUserInvitation>,
        action_digest: Option<Vec<u8>>,
    ) -> Result<CreatedWorkspaceUser, RepositoryError> {
        self.require_member_permission(actor_id, workspace_id, "members.manage")
            .await?;
        if email != email.trim().to_lowercase() || !email.contains('@') {
            return Err(RepositoryError::InvitationInvalid);
        }
        if invitation.is_none() && action_digest.is_some() {
            return Err(RepositoryError::InvitationInvalid);
        }
        let mut tx = self.pool.begin().await?;
        let user = sqlx::query("INSERT INTO users (id, email, display_name, email_verified_at) VALUES ($1, $2, NULLIF(btrim($3), ''), clock_timestamp()) ON CONFLICT (email) DO UPDATE SET display_name = COALESCE(users.display_name, EXCLUDED.display_name) RETURNING id, state, email_verified_at, security_version")
            .bind(Uuid::new_v4()).bind(email).bind(display_name).fetch_one(&mut *tx).await?;
        let user_id: Uuid = user.try_get("id")?;
        let active = user.try_get::<String, _>("state")? == "active";
        let verified = user
            .try_get::<Option<DateTime<Utc>>, _>("email_verified_at")?
            .is_some();
        let security_version: i32 = user.try_get("security_version")?;
        let credential_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM local_password_credentials WHERE user_id = $1)",
        )
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;
        let Some((
            invitation_id,
            role_id,
            scope_type,
            scope_target_id,
            invitation_digest,
            expires_at,
        )) = invitation
        else {
            self.commit_mutation(tx).await?;
            return Ok(CreatedWorkspaceUser {
                user_id,
                invitation_id: None,
                needs_password_setup: !credential_exists,
            });
        };
        self.ensure_token_can_delegate_role_on(&mut tx, actor_id, workspace_id, role_id)
            .await?;
        if invitation_digest.len() != 32
            || expires_at <= Utc::now()
            || !Self::scope_is_valid_on(
                &mut tx,
                workspace_id,
                role_id,
                &scope_type,
                scope_target_id,
            )
            .await?
            || !Self::roles_delegable_on(&mut tx, actor_id, workspace_id, role_id).await?
            || (role_id == OWNER_ROLE_ID
                && !Self::active_owner_on(&mut tx, actor_id, workspace_id).await?)
        {
            return Err(RepositoryError::InvitationInvalid);
        }
        sqlx::query("INSERT INTO workspace_invitations (id, workspace_id, invitee_email, inviter_user_id, role_id, scope_type, scope_target_id, token_digest, expires_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)")
            .bind(invitation_id).bind(workspace_id).bind(email).bind(actor_id).bind(role_id).bind(&scope_type).bind(scope_target_id).bind(invitation_digest).bind(expires_at).execute(&mut *tx).await?;
        if !credential_exists {
            let action_digest = action_digest
                .filter(|digest| digest.len() == 32)
                .ok_or(RepositoryError::InvitationInvalid)?;
            if !active || !verified {
                return Err(RepositoryError::InvitationInvalid);
            }
            // Replace standalone setup links and this workspace's earlier
            // onboarding link. Another workspace's pending onboarding stays
            // usable until a password is set, which invalidates it through
            // issued_credential_version; that invitation can still be accepted.
            sqlx::query("UPDATE user_lifecycle_action_tokens a SET revoked_at = clock_timestamp() WHERE a.user_id = $1 AND a.purpose = 'password_setup' AND a.consumed_at IS NULL AND a.revoked_at IS NULL AND NOT EXISTS (SELECT 1 FROM workspace_invitation_onboarding o WHERE o.user_id = a.user_id AND o.action_token_digest = a.token_digest AND o.workspace_id <> $2)")
                .bind(user_id).bind(workspace_id).execute(&mut *tx).await?;
            sqlx::query("INSERT INTO user_lifecycle_action_tokens (id, user_id, purpose, token_digest, issued_security_version, issued_credential_version, expires_at) VALUES ($1,$2,'password_setup',$3,$4,0,$5)")
                .bind(Uuid::new_v4()).bind(user_id).bind(&action_digest).bind(security_version).bind(expires_at).execute(&mut *tx).await?;
            sqlx::query("INSERT INTO workspace_invitation_onboarding (invitation_id, workspace_id, user_id, action_token_digest) VALUES ($1,$2,$3,$4)")
                .bind(invitation_id).bind(workspace_id).bind(user_id).bind(action_digest).execute(&mut *tx).await?;
        }
        self.commit_mutation(tx).await?;
        Ok(CreatedWorkspaceUser {
            user_id,
            invitation_id: Some(invitation_id),
            needs_password_setup: !credential_exists,
        })
    }

    pub async fn complete_workspace_onboarding(
        &self,
        invitation_digest: &[u8],
        action_digest: &[u8],
        password_hash: &str,
    ) -> Result<CompletedWorkspaceOnboarding, RepositoryError> {
        if invitation_digest.len() != 32 || action_digest.len() != 32 {
            return Err(RepositoryError::InvitationInvalid);
        }
        let mut tx: Transaction<'_, Postgres> = self.pool.begin().await?;
        let row = sqlx::query("SELECT i.id invitation_id, i.workspace_id, i.role_id, i.scope_type, i.scope_target_id, o.user_id, u.security_version FROM workspace_invitations i JOIN workspace_invitation_onboarding o ON o.invitation_id = i.id AND o.workspace_id = i.workspace_id JOIN user_lifecycle_action_tokens a ON a.token_digest = o.action_token_digest AND a.user_id = o.user_id AND a.purpose = 'password_setup' JOIN users u ON u.id = o.user_id LEFT JOIN local_password_credentials c ON c.user_id = o.user_id WHERE i.token_digest = $1 AND o.action_token_digest = $2 AND i.accepted_at IS NULL AND i.revoked_at IS NULL AND i.expires_at > clock_timestamp() AND a.consumed_at IS NULL AND a.revoked_at IS NULL AND a.expires_at > clock_timestamp() AND a.issued_security_version = u.security_version AND a.issued_credential_version = coalesce(c.credential_version, 0) AND u.state = 'active' AND u.email_verified_at IS NOT NULL FOR UPDATE OF i, o, a, u")
            .bind(invitation_digest).bind(action_digest).fetch_optional(&mut *tx).await?.ok_or(RepositoryError::InvitationInvalid)?;
        let user_id: Uuid = row.try_get("user_id")?;
        let workspace_id: Uuid = row.try_get("workspace_id")?;
        let action_consumed = sqlx::query("UPDATE user_lifecycle_action_tokens SET consumed_at = clock_timestamp() WHERE token_digest = $1 AND consumed_at IS NULL AND revoked_at IS NULL")
            .bind(action_digest).execute(&mut *tx).await?.rows_affected() == 1;
        if !action_consumed {
            return Err(RepositoryError::InvitationInvalid);
        }
        // A credential set concurrently (another workspace's onboarding) makes
        // this link invalid rather than a primary-key failure.
        let credential_created = sqlx::query("INSERT INTO local_password_credentials (user_id, password_hash, credential_version) VALUES ($1,$2,1) ON CONFLICT (user_id) DO NOTHING")
            .bind(user_id).bind(password_hash).execute(&mut *tx).await?.rows_affected() == 1;
        if !credential_created {
            return Err(RepositoryError::InvitationInvalid);
        }
        let membership_id: Uuid = sqlx::query_scalar("INSERT INTO workspace_memberships (id, workspace_id, user_id, state) VALUES ($1,$2,$3,'active') ON CONFLICT (workspace_id,user_id) DO UPDATE SET state = 'active', updated_at = clock_timestamp() RETURNING id")
            .bind(Uuid::new_v4()).bind(workspace_id).bind(user_id).fetch_one(&mut *tx).await?;
        sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT (workspace_id,membership_id,role_id,scope_type,scope_target_id) DO NOTHING")
            .bind(Uuid::new_v4()).bind(workspace_id).bind(membership_id).bind(row.try_get::<Uuid,_>("role_id")?).bind(row.try_get::<String,_>("scope_type")?).bind(row.try_get::<Uuid,_>("scope_target_id")?).execute(&mut *tx).await?;
        sqlx::query("UPDATE workspace_invitations SET accepted_at = clock_timestamp(), accepted_by_user_id = $1 WHERE id = $2 AND accepted_at IS NULL")
            .bind(user_id).bind(row.try_get::<Uuid,_>("invitation_id")?).execute(&mut *tx).await?;
        self.commit_mutation(tx).await?;
        Ok(CompletedWorkspaceOnboarding {
            membership_id,
            user_id,
            workspace_id,
        })
    }

    async fn require_member_permission(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
        permission: &str,
    ) -> Result<(), RepositoryError> {
        if self
            .is_authorized(actor_id, workspace_id, permission, None, None)
            .await?
        {
            Ok(())
        } else {
            Err(RepositoryError::NotFound("permission"))
        }
    }
    async fn active_owner(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        let mut connection = self.pool.acquire().await?;
        Self::active_owner_on(&mut connection, actor_id, workspace_id).await
    }
    pub(super) async fn active_owner_on(
        connection: &mut sqlx::PgConnection,
        actor_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        Ok(sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM workspace_memberships m JOIN role_grants g ON g.membership_id = m.id AND g.workspace_id = m.workspace_id WHERE m.user_id = $1 AND m.workspace_id = $2 AND m.state = 'active' AND g.role_id = $3 AND g.scope_type = 'workspace' AND g.scope_target_id = $2)").bind(actor_id).bind(workspace_id).bind(OWNER_ROLE_ID).fetch_one(connection).await?)
    }
    async fn scope_is_valid(
        &self,
        workspace_id: Uuid,
        role_id: Uuid,
        scope_type: &str,
        target: Uuid,
    ) -> Result<bool, RepositoryError> {
        let mut connection = self.pool.acquire().await?;
        Self::scope_is_valid_on(&mut connection, workspace_id, role_id, scope_type, target).await
    }
    async fn scope_is_valid_on(
        connection: &mut sqlx::PgConnection,
        workspace_id: Uuid,
        role_id: Uuid,
        scope_type: &str,
        target: Uuid,
    ) -> Result<bool, RepositoryError> {
        let role_ok: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM roles WHERE id = $1 AND (is_system OR workspace_id = $2))").bind(role_id).bind(workspace_id).fetch_one(&mut *connection).await?;
        if !role_ok {
            return Ok(false);
        }
        let target_ok: bool = match scope_type {
            "workspace" => target == workspace_id,
            "blueprint_family" => sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM blueprints WHERE id = $1 AND workspace_id = $2)").bind(target).bind(workspace_id).fetch_one(&mut *connection).await?,
            "entity" => sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM entities WHERE id = $1 AND workspace_id = $2)").bind(target).bind(workspace_id).fetch_one(&mut *connection).await?,
            "context_subtree" => sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM attribute_contexts WHERE id = $1 AND workspace_id = $2)").bind(target).bind(workspace_id).fetch_one(&mut *connection).await?,
            _ => false,
        };
        Ok(target_ok
            && (role_id != OWNER_ROLE_ID || scope_type == "workspace" && target == workspace_id))
    }
    async fn role_is_delegable(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
        role_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        self.roles_delegable(actor_id, workspace_id, role_id).await
    }
    async fn revoke_user_access(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE browser_sessions SET revoked_at = clock_timestamp() WHERE user_id = $1 AND workspace_id = $2 AND revoked_at IS NULL").bind(user_id).bind(workspace_id).execute(&mut **tx).await?;
        // Lifecycle tokens are account-wide; only this workspace's pending
        // onboarding link belongs to the access being revoked. Password reset,
        // email verification and other workspaces' onboarding are untouched.
        sqlx::query("UPDATE user_lifecycle_action_tokens a SET revoked_at = clock_timestamp() FROM workspace_invitation_onboarding o WHERE a.user_id = $1 AND a.purpose = 'password_setup' AND a.consumed_at IS NULL AND a.revoked_at IS NULL AND o.workspace_id = $2 AND o.user_id = a.user_id AND o.action_token_digest = a.token_digest").bind(user_id).bind(workspace_id).execute(&mut **tx).await?;
        Ok(())
    }

    pub async fn list_workspace_members(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<WorkspaceMember>, RepositoryError> {
        self.require_member_permission(actor_id, workspace_id, "members.manage")
            .await?;
        Ok(sqlx::query_as("SELECT m.id, m.user_id, u.email, u.display_name, avatar.id AS avatar_file_id, m.state, m.created_at, m.updated_at, coalesce(jsonb_agg(jsonb_build_object('id', g.id, 'role_id', g.role_id, 'role_code', r.code, 'scope_type', g.scope_type, 'scope_target_id', g.scope_target_id) ORDER BY g.created_at) FILTER (WHERE g.id IS NOT NULL), '[]'::jsonb) AS grants FROM workspace_memberships m JOIN users u ON u.id = m.user_id LEFT JOIN files avatar ON m.state = 'active' AND avatar.workspace_id = m.workspace_id AND avatar.id = m.avatar_file_id AND avatar.purpose = 'avatar' AND avatar.status = 'ready' AND avatar.deleted_at IS NULL LEFT JOIN role_grants g ON g.membership_id = m.id AND g.workspace_id = m.workspace_id LEFT JOIN roles r ON r.id = g.role_id WHERE m.workspace_id = $1 GROUP BY m.id, u.id, avatar.id ORDER BY u.email").bind(workspace_id).fetch_all(&self.pool).await?)
    }
    pub async fn set_workspace_membership_state(
        &self,
        id: Uuid,
        actor_id: Uuid,
        workspace_id: Uuid,
        state: &str,
    ) -> Result<bool, RepositoryError> {
        if !matches!(state, "active" | "inactive") {
            return Err(RepositoryError::NotFound("membership state"));
        }
        self.require_member_permission(actor_id, workspace_id, "members.manage")
            .await?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT id FROM workspaces WHERE id = $1 FOR UPDATE")
            .bind(workspace_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "SELECT id FROM workspace_memberships WHERE id = $1 AND workspace_id = $2 FOR UPDATE",
        )
        .bind(id)
        .bind(workspace_id)
        .execute(&mut *tx)
        .await?;
        if state == "inactive" {
            let is_owner: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM role_grants WHERE workspace_id = $1 AND membership_id = $2 AND role_id = $3 AND scope_type = 'workspace' AND scope_target_id = $1)",
            )
            .bind(workspace_id)
            .bind(id)
            .bind(OWNER_ROLE_ID)
            .fetch_one(&mut *tx)
            .await?;
            if is_owner {
                let active_owners: i64 = sqlx::query_scalar(
                    "SELECT count(*) FROM role_grants g JOIN workspace_memberships m ON m.id = g.membership_id WHERE g.workspace_id = $1 AND g.role_id = $2 AND g.scope_type = 'workspace' AND g.scope_target_id = $1 AND m.state = 'active'",
                )
                .bind(workspace_id)
                .bind(OWNER_ROLE_ID)
                .fetch_one(&mut *tx)
                .await?;
                if active_owners <= 1 {
                    return Err(RepositoryError::NotFound("last workspace owner"));
                }
            }
        }
        let user = sqlx::query("UPDATE workspace_memberships SET state = $1, updated_at = clock_timestamp() WHERE id = $2 AND workspace_id = $3 RETURNING user_id").bind(state).bind(id).bind(workspace_id).fetch_optional(&mut *tx).await?;
        if let Some(user) = user {
            if state == "inactive" {
                self.revoke_user_access(&mut tx, user.try_get("user_id")?, workspace_id)
                    .await?;
            }
            self.commit_mutation(tx).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    pub async fn grant_workspace_member_role(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
        membership_id: Uuid,
        role_id: Uuid,
        scope_type: &str,
        scope_target_id: Uuid,
    ) -> Result<Uuid, RepositoryError> {
        self.require_member_permission(actor_id, workspace_id, "roles.grant")
            .await?;
        let member_ok: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM workspace_memberships WHERE id = $1 AND workspace_id = $2 AND state = 'active')").bind(membership_id).bind(workspace_id).fetch_one(&self.pool).await?;
        if !member_ok
            || !self
                .scope_is_valid(workspace_id, role_id, scope_type, scope_target_id)
                .await?
            || !self
                .role_is_delegable(actor_id, workspace_id, role_id)
                .await?
            || role_id == OWNER_ROLE_ID && !self.active_owner(actor_id, workspace_id).await?
        {
            return Err(RepositoryError::NotFound("role grant"));
        }
        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        self.ensure_token_can_delegate_role_on(&mut tx, actor_id, workspace_id, role_id)
            .await?;
        sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, $4, $5, $6)").bind(id).bind(workspace_id).bind(membership_id).bind(role_id).bind(scope_type).bind(scope_target_id).execute(&mut *tx).await?;
        self.commit_mutation(tx).await?;
        Ok(id)
    }
    pub async fn revoke_workspace_member_role(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
        membership_id: Uuid,
        grant_id: Uuid,
    ) -> Result<(), RepositoryError> {
        let grant = sqlx::query(
            "SELECT role_id FROM role_grants WHERE id = $1 AND workspace_id = $2 AND membership_id = $3",
        )
        .bind(grant_id)
        .bind(workspace_id)
        .bind(membership_id)
        .fetch_optional(&self.pool)
                .await?
                .ok_or(RepositoryError::NotFound("role grant"))?;
        self.require_member_permission(actor_id, workspace_id, "roles.grant")
            .await?;
        let role_id: Uuid = grant.try_get("role_id")?;
        if role_id == OWNER_ROLE_ID && !self.active_owner(actor_id, workspace_id).await? {
            return Err(RepositoryError::NotFound("owner authority"));
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT id FROM workspaces WHERE id = $1 FOR UPDATE")
            .bind(workspace_id)
            .execute(&mut *tx)
            .await?;
        let grant = sqlx::query("SELECT membership_id, role_id FROM role_grants WHERE id = $1 AND workspace_id = $2 AND membership_id = $3 FOR UPDATE")
            .bind(grant_id)
            .bind(workspace_id)
            .bind(membership_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(RepositoryError::NotFound("role grant"))?;
        let membership_id: Uuid = grant.try_get("membership_id")?;
        // Retirement can replace a grant's role while this request waits.
        let role_id: Uuid = grant.try_get("role_id")?;
        if role_id == OWNER_ROLE_ID
            && !Self::active_owner_on(&mut tx, actor_id, workspace_id).await?
        {
            return Err(RepositoryError::NotFound("owner authority"));
        }
        sqlx::query(
            "SELECT id FROM workspace_memberships WHERE id = $1 AND workspace_id = $2 FOR UPDATE",
        )
        .bind(membership_id)
        .bind(workspace_id)
        .execute(&mut *tx)
        .await?;
        if role_id == OWNER_ROLE_ID {
            let owners: i64 = sqlx::query_scalar("SELECT count(*) FROM role_grants g JOIN workspace_memberships m ON m.id = g.membership_id WHERE g.workspace_id = $1 AND g.role_id = $2 AND g.scope_type = 'workspace' AND g.scope_target_id = $1 AND m.state = 'active'").bind(workspace_id).bind(OWNER_ROLE_ID).fetch_one(&mut *tx).await?;
            if owners <= 1 {
                return Err(RepositoryError::NotFound("last workspace owner"));
            }
        }
        sqlx::query("DELETE FROM role_grants WHERE id = $1 AND workspace_id = $2")
            .bind(grant_id)
            .bind(workspace_id)
            .execute(&mut *tx)
            .await?;
        let user_id: Uuid = sqlx::query_scalar(
            "SELECT user_id FROM workspace_memberships WHERE id = $1 AND workspace_id = $2",
        )
        .bind(membership_id)
        .bind(workspace_id)
        .fetch_one(&mut *tx)
        .await?;
        self.revoke_user_access(&mut tx, user_id, workspace_id)
            .await?;
        self.commit_mutation(tx).await?;
        Ok(())
    }
    pub async fn transfer_workspace_ownership(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
        target_membership_id: Uuid,
    ) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT id FROM workspaces WHERE id = $1 FOR UPDATE")
            .bind(workspace_id)
            .execute(&mut *tx)
            .await?;
        // Ownership may have changed while waiting for the workspace lock.
        if !Self::active_owner_on(&mut tx, actor_id, workspace_id).await? {
            return Err(RepositoryError::NotFound("owner authority"));
        }
        self.ensure_token_can_delegate_role_on(&mut tx, actor_id, workspace_id, OWNER_ROLE_ID)
            .await?;
        let target_user: Uuid = sqlx::query_scalar("SELECT user_id FROM workspace_memberships WHERE id = $1 AND workspace_id = $2 AND state = 'active'").bind(target_membership_id).bind(workspace_id).fetch_optional(&mut *tx).await?.ok_or(RepositoryError::NotFound("ownership target"))?;
        if target_user != actor_id {
            sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, $4, 'workspace', $2) ON CONFLICT (workspace_id, membership_id, role_id, scope_type, scope_target_id) DO NOTHING").bind(Uuid::new_v4()).bind(workspace_id).bind(target_membership_id).bind(OWNER_ROLE_ID).execute(&mut *tx).await?;
            sqlx::query("DELETE FROM role_grants WHERE workspace_id = $1 AND membership_id IN (SELECT id FROM workspace_memberships WHERE workspace_id = $1 AND user_id = $2) AND role_id = $3 AND scope_type = 'workspace' AND scope_target_id = $1").bind(workspace_id).bind(actor_id).bind(OWNER_ROLE_ID).execute(&mut *tx).await?;
        }
        self.commit_mutation(tx).await?;
        Ok(())
    }
    // The durable invitation record maps directly to the authorization inputs.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_workspace_invitation(
        &self,
        id: Uuid,
        actor_id: Uuid,
        workspace_id: Uuid,
        email: &str,
        role_id: Uuid,
        scope_type: &str,
        scope_target_id: Uuid,
        digest: &[u8],
        expires_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        self.require_member_permission(actor_id, workspace_id, "members.manage")
            .await?;
        if email != email.trim().to_lowercase()
            || !email.contains('@')
            || digest.len() != 32
            || expires_at <= Utc::now()
            || !self
                .scope_is_valid(workspace_id, role_id, scope_type, scope_target_id)
                .await?
            || !self
                .role_is_delegable(actor_id, workspace_id, role_id)
                .await?
            || role_id == OWNER_ROLE_ID && !self.active_owner(actor_id, workspace_id).await?
        {
            return Err(RepositoryError::InvitationInvalid);
        }
        let mut tx = self.pool.begin().await?;
        self.ensure_token_can_delegate_role_on(&mut tx, actor_id, workspace_id, role_id)
            .await?;
        sqlx::query("INSERT INTO workspace_invitations (id, workspace_id, invitee_email, inviter_user_id, role_id, scope_type, scope_target_id, token_digest, expires_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)").bind(id).bind(workspace_id).bind(email).bind(actor_id).bind(role_id).bind(scope_type).bind(scope_target_id).bind(digest).bind(expires_at).execute(&mut *tx).await?;
        self.commit_mutation(tx).await?;
        Ok(())
    }
    pub async fn list_workspace_invitations(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<WorkspaceInvitation>, RepositoryError> {
        self.require_member_permission(actor_id, workspace_id, "members.manage")
            .await?;
        Ok(sqlx::query_as("SELECT i.id, i.invitee_email, i.inviter_user_id, u.email AS inviter_email, i.role_id, r.code AS role_code, i.scope_type, i.scope_target_id, i.expires_at, i.accepted_at, i.accepted_by_user_id, i.revoked_at, i.created_at FROM workspace_invitations i JOIN users u ON u.id = i.inviter_user_id JOIN roles r ON r.id = i.role_id WHERE i.workspace_id = $1 ORDER BY i.created_at DESC").bind(workspace_id).fetch_all(&self.pool).await?)
    }
    pub async fn workspace_invitation(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
        id: Uuid,
    ) -> Result<WorkspaceInvitation, RepositoryError> {
        self.require_member_permission(actor_id, workspace_id, "members.manage")
            .await?;
        sqlx::query_as("SELECT i.id, i.invitee_email, i.inviter_user_id, u.email AS inviter_email, i.role_id, r.code AS role_code, i.scope_type, i.scope_target_id, i.expires_at, i.accepted_at, i.accepted_by_user_id, i.revoked_at, i.created_at FROM workspace_invitations i JOIN users u ON u.id = i.inviter_user_id JOIN roles r ON r.id = i.role_id WHERE i.workspace_id = $1 AND i.id = $2")
            .bind(workspace_id)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(RepositoryError::NotFound("workspace invitation"))
    }
    pub async fn revoke_workspace_invitation(
        &self,
        id: Uuid,
        actor_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        self.require_member_permission(actor_id, workspace_id, "members.manage")
            .await?;
        let mut tx = self.pool.begin().await?;
        let revoked = sqlx::query("UPDATE workspace_invitations SET revoked_at = clock_timestamp() WHERE id = $1 AND workspace_id = $2 AND accepted_at IS NULL AND revoked_at IS NULL").bind(id).bind(workspace_id).execute(&mut *tx).await?.rows_affected() == 1;
        if revoked {
            sqlx::query("UPDATE user_lifecycle_action_tokens action SET revoked_at = clock_timestamp() FROM workspace_invitation_onboarding onboarding WHERE onboarding.invitation_id = $1 AND action.token_digest = onboarding.action_token_digest AND action.consumed_at IS NULL AND action.revoked_at IS NULL")
                .bind(id).execute(&mut *tx).await?;
            self.commit_mutation(tx).await?;
        }
        Ok(revoked)
    }
    pub async fn accept_workspace_invitation(
        &self,
        digest: &[u8],
        user_id: Uuid,
    ) -> Result<Uuid, RepositoryError> {
        if digest.len() != 32 {
            return Err(RepositoryError::InvitationInvalid);
        }
        let mut tx = self.pool.begin().await?;
        let invitation = sqlx::query("SELECT id, workspace_id, invitee_email, role_id, scope_type, scope_target_id FROM workspace_invitations WHERE token_digest = $1 AND accepted_at IS NULL AND revoked_at IS NULL AND expires_at > clock_timestamp() FOR UPDATE").bind(digest).fetch_optional(&mut *tx).await?.ok_or(RepositoryError::InvitationInvalid)?;
        let email: String = invitation.try_get("invitee_email")?;
        let user_ok: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM users WHERE id = $1 AND state = 'active' AND email = $2 AND email_verified_at IS NOT NULL)").bind(user_id).bind(&email).fetch_one(&mut *tx).await?;
        if !user_ok {
            return Err(RepositoryError::InvitationInvalid);
        }
        let workspace_id: Uuid = invitation.try_get("workspace_id")?;
        let membership_id: Uuid = sqlx::query_scalar("INSERT INTO workspace_memberships (id, workspace_id, user_id, state) VALUES ($1,$2,$3,'active') ON CONFLICT (workspace_id,user_id) DO UPDATE SET state = 'active', updated_at = clock_timestamp() RETURNING id").bind(Uuid::new_v4()).bind(workspace_id).bind(user_id).fetch_one(&mut *tx).await?;
        sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT (workspace_id,membership_id,role_id,scope_type,scope_target_id) DO NOTHING").bind(Uuid::new_v4()).bind(workspace_id).bind(membership_id).bind(invitation.try_get::<Uuid,_>("role_id")?).bind(invitation.try_get::<String,_>("scope_type")?).bind(invitation.try_get::<Uuid,_>("scope_target_id")?).execute(&mut *tx).await?;
        sqlx::query("UPDATE workspace_invitations SET accepted_at = clock_timestamp(), accepted_by_user_id = $1 WHERE id = $2").bind(user_id).bind(invitation.try_get::<Uuid,_>("id")?).execute(&mut *tx).await?;
        // Invitation acceptance is scoped by the invitation, not the caller's
        // current workspace selection. Audit it in the destination tenant.
        self.for_workspace(workspace_id)
            .await?
            .commit_mutation(tx)
            .await?;
        Ok(membership_id)
    }
}
