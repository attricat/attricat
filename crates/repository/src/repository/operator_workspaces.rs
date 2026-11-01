//! Workspace provisioning for the deployment operator, who stands outside
//! every workspace. Each step can be repeated, so a retry after a failure at
//! any point finishes the job.

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    AttricatRepository, RepositoryError, roles::OWNER_ROLE_ID, system_users::is_reserved_email,
};

const LOGIN_IDENTIFIER_BYTES: std::ops::RangeInclusive<usize> = 3..=253;
const DOMAIN_LABEL_MAX_BYTES: usize = 63;
const TOP_LEVEL_LABEL_MIN_BYTES: usize = 2;
const WORKSPACE_GRANT_SCOPE: &str = "workspace";
const OPERATOR_ACTOR: &str = "operator";
const WORKSPACE_CREATED_ACTION: &str = "workspace.create";
const OWNER_INVITED_ACTION: &str = "workspace.owner_invitation.create";

const OPERATOR_WORKSPACE_COLUMNS: &str = "w.id, w.name, w.login_identifier, w.bootstrap_owner_email AS owner_email, \
     EXISTS (SELECT 1 FROM role_grants g JOIN workspace_memberships m ON m.id = g.membership_id \
             WHERE g.workspace_id = w.id AND g.role_id = $1 AND m.state = 'active') AS has_active_owner, \
     (SELECT max(i.expires_at) FROM workspace_invitations i \
      WHERE i.workspace_id = w.id AND i.role_id = $1 AND i.accepted_at IS NULL \
        AND i.revoked_at IS NULL AND i.expires_at > clock_timestamp()) AS owner_invitation_expires_at, \
     w.created_at, w.updated_at";

/// A workspace as the operator sees it.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct OperatorWorkspace {
    pub id: Uuid,
    pub name: String,
    pub login_identifier: String,
    pub owner_email: Option<String>,
    pub has_active_owner: bool,
    pub owner_invitation_expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct NewOperatorWorkspace<'a> {
    pub id: Uuid,
    pub name: &'a str,
    pub login_identifier: &'a str,
    pub owner_email: &'a str,
}

pub struct OwnerInvitation<'a> {
    pub id: Uuid,
    pub email: &'a str,
    pub display_name: Option<&'a str>,
    pub invitation_digest: Vec<u8>,
    pub action_digest: Vec<u8>,
    pub expires_at: DateTime<Utc>,
}

/// What an owner invitation does about an unexpired earlier one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerInvitationPolicy {
    /// Leaves it in place and issues nothing, so a repeated create request
    /// does not send a second email.
    KeepPending,
    /// Revokes it, and refuses when the workspace already has an owner.
    ReplacePending,
}

/// Mirrors the `workspaces_login_identifier_format_check` constraint, so a bad
/// value gets its own error instead of a constraint failure.
fn is_login_identifier(value: &str) -> bool {
    if !LOGIN_IDENTIFIER_BYTES.contains(&value.len()) {
        return false;
    }
    let mut labels = value.rsplit('.');
    let top_level = labels.next().unwrap_or_default();
    let mut domain = labels.peekable();
    (TOP_LEVEL_LABEL_MIN_BYTES..=DOMAIN_LABEL_MAX_BYTES).contains(&top_level.len())
        && top_level.bytes().all(|byte| byte.is_ascii_lowercase())
        && domain.peek().is_some()
        && domain.all(|label| {
            (1..=DOMAIN_LABEL_MAX_BYTES).contains(&label.len())
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

/// The workspace slug, which people see as the namespace of its reusable
/// attributes. An identifier has no underscores, so distinct identifiers keep
/// distinct slugs.
fn workspace_slug(login_identifier: &str) -> String {
    login_identifier.replace('.', "_")
}

fn operator_metadata(fields: Value) -> Value {
    let mut metadata = json!({ "actor": OPERATOR_ACTOR });
    if let (Some(metadata), Value::Object(fields)) = (metadata.as_object_mut(), fields) {
        metadata.extend(fields);
    }
    metadata
}

impl<S: super::RepositoryScope> AttricatRepository<S> {
    /// Inserts a workspace and returns whether this call created it. The same
    /// ID with the same values is a repeat of an earlier request.
    pub async fn create_operator_workspace(
        &self,
        workspace: &NewOperatorWorkspace<'_>,
    ) -> Result<bool, RepositoryError> {
        if !is_login_identifier(workspace.login_identifier) {
            return Err(RepositoryError::InvalidLoginIdentifier);
        }
        if is_reserved_email(workspace.owner_email) {
            return Err(RepositoryError::InvitationInvalid);
        }
        let mut tx = self.pool.begin().await?;
        let inserted = sqlx::query("INSERT INTO workspaces (id, slug, name, login_identifier, bootstrap_owner_email) VALUES ($1, $2, $3, $4, $5) ON CONFLICT DO NOTHING")
            .bind(workspace.id)
            .bind(workspace_slug(workspace.login_identifier))
            .bind(workspace.name)
            .bind(workspace.login_identifier)
            .bind(workspace.owner_email)
            .execute(&mut *tx)
            .await?
            .rows_affected()
            == 1;
        if inserted {
            self.for_workspace(workspace.id)
                .with_workspace_audit_action(
                    WORKSPACE_CREATED_ACTION,
                    json!({ "type": "workspace", "id": workspace.id }),
                    operator_metadata(json!({ "login_identifier": workspace.login_identifier })),
                )
                .commit_mutation(tx)
                .await?;
            return Ok(true);
        }
        let existing: Option<(String, String, Option<String>)> = sqlx::query_as(
            "SELECT name, login_identifier, bootstrap_owner_email FROM workspaces WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(workspace.id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((name, login_identifier, owner_email)) = existing else {
            return Err(RepositoryError::LoginIdentifierTaken);
        };
        if name == workspace.name
            && login_identifier == workspace.login_identifier
            && owner_email.as_deref() == Some(workspace.owner_email)
        {
            Ok(false)
        } else {
            Err(RepositoryError::WorkspaceConflict)
        }
    }

    pub async fn operator_workspace(
        &self,
        workspace_id: Uuid,
    ) -> Result<Option<OperatorWorkspace>, RepositoryError> {
        Ok(sqlx::query_as(&format!(
            "SELECT {OPERATOR_WORKSPACE_COLUMNS} FROM workspaces w WHERE w.id = $2 AND w.deleted_at IS NULL"
        ))
        .bind(OWNER_ROLE_ID)
        .bind(workspace_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// One page of workspaces, oldest first, and whether more follow.
    pub async fn list_operator_workspaces(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<OperatorWorkspace>, bool), RepositoryError> {
        let mut workspaces: Vec<OperatorWorkspace> = sqlx::query_as(&format!(
            "SELECT {OPERATOR_WORKSPACE_COLUMNS} FROM workspaces w WHERE w.deleted_at IS NULL ORDER BY w.created_at, w.id LIMIT $2 OFFSET $3"
        ))
        .bind(OWNER_ROLE_ID)
        .bind(limit + 1)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let has_more = workspaces.len() as i64 > limit;
        workspaces.truncate(limit as usize);
        Ok((workspaces, has_more))
    }

    /// Issues the invitation that makes someone the workspace's first owner,
    /// authored by its system user because no person is acting. Returns
    /// whether the invitee must set a password, or `None` when the policy
    /// left things as they are.
    pub async fn issue_owner_invitation(
        &self,
        workspace_id: Uuid,
        invitation: OwnerInvitation<'_>,
        policy: OwnerInvitationPolicy,
    ) -> Result<Option<bool>, RepositoryError> {
        if is_reserved_email(invitation.email) {
            return Err(RepositoryError::InvitationInvalid);
        }
        let mut tx = self.pool.begin().await?;
        Self::lock_invitation_changes_on(&mut tx, workspace_id).await?;
        let (has_owner, has_pending): (bool, bool) = sqlx::query_as(
            "SELECT EXISTS (SELECT 1 FROM role_grants g JOIN workspace_memberships m ON m.id = g.membership_id WHERE g.workspace_id = $1 AND g.role_id = $2 AND m.state = 'active'), EXISTS (SELECT 1 FROM workspace_invitations WHERE workspace_id = $1 AND role_id = $2 AND accepted_at IS NULL AND revoked_at IS NULL AND expires_at > clock_timestamp())",
        )
        .bind(workspace_id)
        .bind(OWNER_ROLE_ID)
        .fetch_one(&mut *tx)
        .await?;
        match policy {
            OwnerInvitationPolicy::KeepPending if has_owner || has_pending => return Ok(None),
            OwnerInvitationPolicy::ReplacePending if has_owner => {
                return Err(RepositoryError::WorkspaceOwnerExists);
            }
            OwnerInvitationPolicy::KeepPending => {}
            OwnerInvitationPolicy::ReplacePending => {
                Self::revoke_pending_owner_invitations_on(&mut tx, workspace_id).await?;
                sqlx::query("UPDATE workspaces SET bootstrap_owner_email = $2, updated_at = clock_timestamp() WHERE id = $1")
                    .bind(workspace_id)
                    .bind(invitation.email)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        let inviter: Option<Uuid> =
            sqlx::query_scalar("SELECT system_user_id FROM workspaces WHERE id = $1")
                .bind(workspace_id)
                .fetch_one(&mut *tx)
                .await?;
        let inviter = inviter.ok_or(RepositoryError::NotFound("system user"))?;
        let user = Self::upsert_invited_user_on(&mut tx, invitation.email, invitation.display_name)
            .await?;
        Self::insert_invitation_on(
            &mut tx,
            &user,
            inviter,
            workspace_id,
            invitation.email,
            (
                invitation.id,
                OWNER_ROLE_ID,
                WORKSPACE_GRANT_SCOPE.to_owned(),
                workspace_id,
                invitation.invitation_digest,
                invitation.expires_at,
            ),
            Some(invitation.action_digest),
        )
        .await?;
        self.for_workspace(workspace_id)
            .with_workspace_audit_action(
                OWNER_INVITED_ACTION,
                json!({ "type": "workspace", "id": workspace_id }),
                operator_metadata(json!({ "invitation_id": invitation.id })),
            )
            .commit_mutation(tx)
            .await?;
        Ok(Some(!user.credential_exists))
    }

    /// Withdraws an owner invitation whose email could not be sent. Only
    /// digests are stored, so a later attempt must issue a new one.
    pub async fn revoke_pending_owner_invitations(
        &self,
        workspace_id: Uuid,
    ) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;
        Self::lock_invitation_changes_on(&mut tx, workspace_id).await?;
        Self::revoke_pending_owner_invitations_on(&mut tx, workspace_id).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn revoke_pending_owner_invitations_on(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        workspace_id: Uuid,
    ) -> Result<(), RepositoryError> {
        sqlx::query("UPDATE user_lifecycle_action_tokens a SET revoked_at = clock_timestamp() FROM workspace_invitation_onboarding o JOIN workspace_invitations i ON i.id = o.invitation_id WHERE a.token_digest = o.action_token_digest AND o.workspace_id = $1 AND i.role_id = $2 AND i.accepted_at IS NULL AND i.revoked_at IS NULL AND a.consumed_at IS NULL AND a.revoked_at IS NULL")
            .bind(workspace_id)
            .bind(OWNER_ROLE_ID)
            .execute(&mut **tx)
            .await?;
        sqlx::query("UPDATE workspace_invitations SET revoked_at = clock_timestamp() WHERE workspace_id = $1 AND role_id = $2 AND accepted_at IS NULL AND revoked_at IS NULL")
            .bind(workspace_id)
            .bind(OWNER_ROLE_ID)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{is_login_identifier, workspace_slug};

    #[test]
    fn login_identifiers_match_the_database_format() {
        for valid in [
            "acme.example",
            "a.co",
            "default.local",
            "a-b.c-d.example",
            "x1.y2.example",
        ] {
            assert!(is_login_identifier(valid), "{valid}");
        }
        let long_label = format!("{}.example", "a".repeat(64));
        let too_long = format!("{}.example", vec!["a".repeat(60); 5].join("."));
        for invalid in [
            "",
            "ab",
            "example",
            "Acme.example",
            "acme..example",
            ".example",
            "acme.",
            "-acme.example",
            "acme-.example",
            "acme.c0m",
            "acme.c",
            "ac me.example",
            "acme_co.example",
            long_label.as_str(),
            too_long.as_str(),
        ] {
            assert!(!is_login_identifier(invalid), "{invalid}");
        }
    }

    #[test]
    fn slugs_stay_distinct_for_distinct_identifiers() {
        assert_eq!(workspace_slug("acme.example"), "acme_example");
        assert_ne!(workspace_slug("a-b.example"), workspace_slug("a.b-example"));
    }
}
