use clap::{Subcommand, ValueEnum};
use reqwest::{Client, Method};
use serde_json::{Value, json};
use url::Url;
use uuid::Uuid;

use crate::error::CliError;
use crate::input::{
    json_array_input, json_string_array_input, read_secret_stdin, read_three_secrets_stdin,
};
use crate::transport::{request, request_value, segment};

pub(crate) const MAX_BULK_EDIT_RECORDS: u32 = 5000;

#[derive(Subcommand)]
pub(crate) enum WorkspaceCommand {
    Navigation {
        #[command(subcommand)]
        command: NavigationCommand,
    },
    /// Read or change how agent bulk value edits are approved (requires
    /// `agents.manage`).
    Agent {
        #[command(subcommand)]
        command: AgentSettingsCommand,
    },
    TokenPermission {
        #[command(subcommand)]
        command: TokenPermissionCommand,
    },
    GrantTarget {
        #[command(subcommand)]
        command: GrantTargetCommand,
    },
    PublicationChannel {
        #[command(subcommand)]
        command: PublicationChannelCommand,
    },
    Member {
        #[command(subcommand)]
        command: MemberCommand,
    },
    Role {
        #[command(subcommand)]
        command: RoleCommand,
    },
    Invitation {
        #[command(subcommand)]
        command: InvitationCommand,
    },
    User {
        #[command(subcommand)]
        command: UserCommand,
    },
}

#[derive(Subcommand)]
pub(crate) enum AgentSettingsCommand {
    Get,
    /// Change the given settings and keep the others.
    #[command(group(clap::ArgGroup::new("change").required(true).multiple(true).args(["bulk_edits", "bulk_edit_max_records"])))]
    Set {
        /// per-proposal: one approval covers a whole bulk edit; per-record:
        /// the agent is not offered bulk edits and each record is approved.
        #[arg(long, value_enum)]
        bulk_edits: Option<BulkEditPolicyArgument>,
        /// The most records one bulk edit may change (1-5000).
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..=i64::from(MAX_BULK_EDIT_RECORDS)))]
        bulk_edit_max_records: Option<u32>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum BulkEditPolicyArgument {
    PerProposal,
    PerRecord,
}

impl BulkEditPolicyArgument {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::PerProposal => "per_proposal",
            Self::PerRecord => "per_record",
        }
    }
}

/// The settings `acli workspace agent set` sends: the current settings with
/// the given changes.
pub(crate) fn changed_agent_settings(
    mut current: Value,
    bulk_edits: Option<BulkEditPolicyArgument>,
    bulk_edit_max_records: Option<u32>,
) -> Result<Value, CliError> {
    let settings = current.as_object_mut().ok_or(CliError::InvalidResponse)?;
    if let Some(policy) = bulk_edits {
        settings.insert("bulk_edits".into(), json!(policy.as_str()));
    }
    if let Some(limit) = bulk_edit_max_records {
        settings.insert("bulk_edit_max_records".into(), json!(limit));
    }
    Ok(current)
}

#[derive(Subcommand)]
pub(crate) enum NavigationCommand {
    Get,
    Set {
        #[arg(long)]
        entries: String,
    },
    Sidebar,
}

#[derive(Subcommand)]
pub(crate) enum TokenPermissionCommand {
    List,
}

#[derive(Subcommand)]
pub(crate) enum GrantTargetCommand {
    List { scope_type: String },
}

#[derive(Subcommand)]
pub(crate) enum PublicationChannelCommand {
    List,
    Set {
        context_id: Uuid,
        #[arg(long, action = clap::ArgAction::Set)]
        enabled: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum MemberCommand {
    List,
    SetState {
        member_id: Uuid,
        #[arg(long, value_parser = ["active", "inactive"])]
        state: String,
    },
    Grant {
        member_id: Uuid,
        #[arg(long)]
        role_id: Uuid,
        #[arg(long)]
        scope_type: String,
        #[arg(long)]
        scope_target_id: Uuid,
    },
    RevokeGrant {
        member_id: Uuid,
        grant_id: Uuid,
    },
    TransferOwnership {
        member_id: Uuid,
    },
}

#[derive(Subcommand)]
pub(crate) enum RoleCommand {
    List,
    Permission {
        #[command(subcommand)]
        command: PermissionCommand,
    },
    AssignableRole {
        #[command(subcommand)]
        command: AssignableRoleCommand,
    },
    Create {
        #[arg(long)]
        code: String,
        #[arg(long)]
        permissions: String,
    },
    Update {
        role_id: Uuid,
        #[arg(long)]
        code: String,
        #[arg(long)]
        permissions: String,
    },
    Duplicate {
        role_id: Uuid,
        #[arg(long)]
        code: String,
    },
    Retire {
        role_id: Uuid,
        #[arg(long)]
        replacement_role_id: Option<Uuid>,
    },
}

#[derive(Subcommand)]
pub(crate) enum PermissionCommand {
    List,
}

#[derive(Subcommand)]
pub(crate) enum AssignableRoleCommand {
    List,
}

#[derive(Subcommand)]
pub(crate) enum InvitationCommand {
    List,
    Create {
        #[arg(long)]
        email: String,
        #[arg(long)]
        role_id: Uuid,
        #[arg(long)]
        scope_type: String,
        #[arg(long)]
        scope_target_id: Uuid,
        #[arg(long)]
        expires_at: String,
    },
    Revoke {
        invitation_id: Uuid,
    },
    Accept {
        #[arg(long)]
        secret_stdin: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum UserCommand {
    Create {
        #[arg(long)]
        email: String,
        #[arg(long)]
        display_name: Option<String>,
        #[arg(long, requires_all = ["scope_type", "scope_target_id", "expires_at"])]
        invite_role_id: Option<Uuid>,
        #[arg(long, requires_all = ["invite_role_id", "scope_target_id", "expires_at"])]
        scope_type: Option<String>,
        #[arg(long, requires_all = ["invite_role_id", "scope_type", "expires_at"])]
        scope_target_id: Option<Uuid>,
        #[arg(long, requires_all = ["invite_role_id", "scope_type", "scope_target_id"])]
        expires_at: Option<String>,
    },
    SetPassword {
        #[arg(long)]
        onboarding_secret_stdin: bool,
        #[arg(long)]
        invitation_secret_stdin: bool,
        #[arg(long)]
        password_stdin: bool,
    },
}

pub(crate) async fn workspace_command(
    client: &Client,
    server: &Url,
    command: WorkspaceCommand,
) -> Result<String, CliError> {
    match command {
        WorkspaceCommand::Navigation { command } => match command {
            NavigationCommand::Get => request(client, server, Method::GET, "/workspace/navigation", None).await,
            NavigationCommand::Set { entries } => request(client, server, Method::PUT, "/workspace/navigation", Some(json!({"explore_navigation": json_array_input(&entries, "--entries")?}))).await,
            NavigationCommand::Sidebar => request(client, server, Method::GET, "/workspace/navigation/sidebar", None).await,
        },
        WorkspaceCommand::Agent { command } => match command {
            AgentSettingsCommand::Get => request(client, server, Method::GET, "/workspace/agent-settings", None).await,
            AgentSettingsCommand::Set { bulk_edits, bulk_edit_max_records } => {
                let current = request_value(client, server, Method::GET, "/workspace/agent-settings", None).await?;
                let changed = changed_agent_settings(current, bulk_edits, bulk_edit_max_records)?;
                request(client, server, Method::PUT, "/workspace/agent-settings", Some(changed)).await
            }
        },
        WorkspaceCommand::TokenPermission { command: TokenPermissionCommand::List } => request(client, server, Method::GET, "/workspace/token-permissions", None).await,
        WorkspaceCommand::GrantTarget { command: GrantTargetCommand::List { scope_type } } => request(client, server, Method::GET, &format!("/workspace/grant-targets/{}", segment(&scope_type)), None).await,
        WorkspaceCommand::PublicationChannel { command } => match command {
            PublicationChannelCommand::List => request(client, server, Method::GET, "/publication-channels", None).await,
            PublicationChannelCommand::Set { context_id, enabled } => request(client, server, Method::PUT, &format!("/publication-channels/{}", segment(context_id)), Some(json!({"enabled":enabled}))).await,
        },
        WorkspaceCommand::Member { command } => match command {
            MemberCommand::List => request(client, server, Method::GET, "/workspace/members", None).await,
            MemberCommand::SetState { member_id, state } => request(client, server, Method::PUT, &format!("/workspace/members/{}", segment(member_id)), Some(json!({ "state": state }))).await,
            MemberCommand::Grant { member_id, role_id, scope_type, scope_target_id } => request(client, server, Method::POST, &format!("/workspace/members/{}/grants", segment(member_id)), Some(json!({ "role_id": role_id, "scope_type": scope_type, "scope_target_id": scope_target_id }))).await,
            MemberCommand::RevokeGrant { member_id, grant_id } => request(client, server, Method::DELETE, &format!("/workspace/members/{}/grants/{}", segment(member_id), segment(grant_id)), None).await,
            MemberCommand::TransferOwnership { member_id } => request(client, server, Method::POST, &format!("/workspace/members/{}/transfer-ownership", segment(member_id)), None).await,
        },
        WorkspaceCommand::Role { command } => match command {
            RoleCommand::List => request(client, server, Method::GET, "/workspace/roles", None).await,
            RoleCommand::Permission { command: PermissionCommand::List } => request(client, server, Method::GET, "/workspace/permissions", None).await,
            RoleCommand::AssignableRole { command: AssignableRoleCommand::List } => request(client, server, Method::GET, "/workspace/assignable-roles", None).await,
            RoleCommand::Create { code, permissions } => request(client, server, Method::POST, "/workspace/roles", Some(json!({ "code": code, "permissions": json_string_array_input(&permissions, "--permissions")? }))).await,
            RoleCommand::Update { role_id, code, permissions } => request(client, server, Method::PUT, &format!("/workspace/roles/{}", segment(role_id)), Some(json!({ "code": code, "permissions": json_string_array_input(&permissions, "--permissions")? }))).await,
            RoleCommand::Duplicate { role_id, code } => request(client, server, Method::POST, &format!("/workspace/roles/{}/duplicate", segment(role_id)), Some(json!({ "code": code }))).await,
            RoleCommand::Retire { role_id, replacement_role_id } => request(client, server, Method::POST, &format!("/workspace/roles/{}/retire", segment(role_id)), Some(json!({ "replacement_role_id": replacement_role_id }))).await,
        },
        WorkspaceCommand::User { command } => match command {
            UserCommand::Create { email, display_name, invite_role_id, scope_type, scope_target_id, expires_at } => request(client, server, Method::POST, "/workspace/users", Some(json!({ "email": email, "display_name": display_name, "role_id": invite_role_id, "scope_type": scope_type, "scope_target_id": scope_target_id, "expires_at": expires_at }))).await,
            UserCommand::SetPassword { onboarding_secret_stdin, invitation_secret_stdin, password_stdin } => {
                if !onboarding_secret_stdin || !invitation_secret_stdin || !password_stdin { return Err(CliError::Input("password setup requires --onboarding-secret-stdin, --invitation-secret-stdin, and --password-stdin".to_owned())); }
                let values = read_three_secrets_stdin()?;
                request(client, server, Method::POST, "/onboarding/complete", Some(json!({ "onboarding_secret": values.0, "invitation_secret": values.1, "password": values.2 }))).await
            }
        },
        WorkspaceCommand::Invitation { command } => match command {
            InvitationCommand::List => request(client, server, Method::GET, "/workspace/invitations", None).await,
            InvitationCommand::Create { email, role_id, scope_type, scope_target_id, expires_at } => request(client, server, Method::POST, "/workspace/invitations", Some(json!({ "email": email, "role_id": role_id, "scope_type": scope_type, "scope_target_id": scope_target_id, "expires_at": expires_at }))).await,
            InvitationCommand::Revoke { invitation_id } => request(client, server, Method::DELETE, &format!("/workspace/invitations/{}", segment(invitation_id)), None).await,
            InvitationCommand::Accept { secret_stdin } => {
                if !secret_stdin { return Err(CliError::Input("invitation acceptance requires --secret-stdin".to_owned())); }
                request(client, server, Method::POST, "/workspace/invitations/accept", Some(json!({ "secret": read_secret_stdin("invitation secret")? }))).await
            }
        },
    }
}
