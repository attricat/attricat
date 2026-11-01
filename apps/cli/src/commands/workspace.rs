use clap::{Subcommand, ValueEnum};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::commands::identity_provider::{IdentityProviderCommand, identity_provider_command};
use crate::error::CliError;
use crate::input::{
    json_array_input, json_string_array_input, read_secret_stdin, read_stdin_lines,
};
use crate::transport::{Api, segment};

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
    /// The workspace's single sign-on provider (requires `workspace.manage`).
    IdentityProvider {
        #[command(subcommand)]
        command: IdentityProviderCommand,
    },
    /// Read or change the colors of record system tags (changes require
    /// `tag_colors.manage`).
    TagColor {
        #[command(subcommand)]
        command: TagColorCommand,
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
    #[command(group(
        clap::ArgGroup::new("change")
            .required(true)
            .multiple(true)
            .args(["bulk_edits", "bulk_edit_max_records"])
    ))]
    Set {
        /// per-proposal: one approval covers a whole bulk edit; per-record:
        /// the agent is not offered bulk edits and each record is approved.
        #[arg(long, value_enum)]
        bulk_edits: Option<BulkEditPolicyArgument>,
        /// The most records one bulk edit may change (1-5000).
        #[arg(
            long,
            value_parser = clap::value_parser!(u32).range(1..=i64::from(MAX_BULK_EDIT_RECORDS))
        )]
        bulk_edit_max_records: Option<u32>,
    },
}

#[derive(Subcommand)]
pub(crate) enum TagColorCommand {
    /// The Core defaults and the workspace's own colors.
    List,
    /// Give a tag a color: gray, blue, teal, green, yellow, orange, red,
    /// purple or pink.
    Set { tag: String, color: String },
    /// Remove a tag's color, returning a Core tag to its default.
    Reset { tag: String },
}

/// The patch an `acli workspace tag-color` change sends.
pub(crate) fn tag_color_patch(command: TagColorCommand) -> Option<Value> {
    match command {
        TagColorCommand::List => None,
        TagColorCommand::Set { tag, color } => Some(json!({ "set": { tag: color } })),
        TagColorCommand::Reset { tag } => Some(json!({ "reset": [tag] })),
    }
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

/// What a role grant or invitation applies to.
#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum GrantScope {
    Workspace,
    BlueprintFamily,
    Record,
    ContextSubtree,
}

impl GrantScope {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::BlueprintFamily => "blueprint_family",
            Self::Record => "record",
            Self::ContextSubtree => "context_subtree",
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum MemberState {
    Active,
    Inactive,
}

impl MemberState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
        }
    }
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
    List {
        #[arg(value_enum)]
        scope_type: GrantScope,
    },
}

#[derive(Subcommand)]
pub(crate) enum PublicationChannelCommand {
    List,
    /// Turn a context's publication channel on or off and change what a
    /// record needs before it can be published there. Settings not given are
    /// kept.
    Set {
        context_id: Uuid,
        #[arg(long, action = clap::ArgAction::Set)]
        enabled: bool,
        /// Rule code a record must pass to publish; repeat for several.
        /// Replaces the channel's required rules.
        #[arg(long = "required-rule-code")]
        required_rule_codes: Vec<String>,
        /// Require no rules to publish.
        #[arg(long, conflicts_with = "required_rule_codes")]
        clear_required_rule_codes: bool,
        /// Whether a record must pass validation to publish.
        #[arg(long, action = clap::ArgAction::Set)]
        require_valid_record: Option<bool>,
    },
}

#[derive(Subcommand)]
pub(crate) enum MemberCommand {
    List,
    SetState {
        member_id: Uuid,
        #[arg(long, value_enum)]
        state: MemberState,
    },
    /// Remove a member's link to the workspace's identity provider and end the
    /// sessions it gave them here (requires `workspace.manage`).
    UnlinkIdentity {
        member_id: Uuid,
    },
    Grant {
        member_id: Uuid,
        #[arg(long)]
        role_id: Uuid,
        #[arg(long, value_enum)]
        scope_type: GrantScope,
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
        #[arg(long, value_enum)]
        scope_type: GrantScope,
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
        #[arg(long, value_enum, requires_all = ["invite_role_id", "scope_target_id", "expires_at"])]
        scope_type: Option<GrantScope>,
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
    api: &Api,
    command: WorkspaceCommand,
) -> Result<String, CliError> {
    match command {
        WorkspaceCommand::Navigation { command } => navigation_command(api, command).await,
        WorkspaceCommand::IdentityProvider { command } => {
            identity_provider_command(api, IDENTITY_PROVIDER_PATH, command).await
        }
        WorkspaceCommand::Agent { command } => match command {
            AgentSettingsCommand::Get => api.get(AGENT_SETTINGS_PATH).await,
            AgentSettingsCommand::Set {
                bulk_edits,
                bulk_edit_max_records,
            } => {
                let current = api.get_value(AGENT_SETTINGS_PATH).await?;
                let changed = changed_agent_settings(current, bulk_edits, bulk_edit_max_records)?;
                api.put_json(AGENT_SETTINGS_PATH, changed).await
            }
        },
        WorkspaceCommand::TagColor { command } => match tag_color_patch(command) {
            None => api.get(TAG_COLORS_PATH).await,
            Some(patch) => api.patch_json(TAG_COLORS_PATH, patch).await,
        },
        WorkspaceCommand::TokenPermission {
            command: TokenPermissionCommand::List,
        } => api.get("/workspace/token-permissions").await,
        WorkspaceCommand::GrantTarget {
            command: GrantTargetCommand::List { scope_type },
        } => {
            api.get(&format!("/workspace/grant-targets/{}", scope_type.as_str()))
                .await
        }
        WorkspaceCommand::PublicationChannel { command } => match command {
            PublicationChannelCommand::List => api.get("/publication-channels").await,
            PublicationChannelCommand::Set {
                context_id,
                enabled,
                required_rule_codes,
                clear_required_rule_codes,
                require_valid_record,
            } => {
                let required_rule_codes = (clear_required_rule_codes
                    || !required_rule_codes.is_empty())
                .then_some(required_rule_codes);
                api.put_json(
                    &format!("/publication-channels/{}", segment(context_id)),
                    json!({
                        "enabled": enabled,
                        "required_rule_codes": required_rule_codes,
                        "require_valid_record": require_valid_record,
                    }),
                )
                .await
            }
        },
        WorkspaceCommand::Member { command } => member_command(api, command).await,
        WorkspaceCommand::Role { command } => role_command(api, command).await,
        WorkspaceCommand::User { command } => match command {
            UserCommand::Create {
                email,
                display_name,
                invite_role_id,
                scope_type,
                scope_target_id,
                expires_at,
            } => {
                let body = json!({
                    "email": email,
                    "display_name": display_name,
                    "role_id": invite_role_id,
                    "scope_type": scope_type.map(GrantScope::as_str),
                    "scope_target_id": scope_target_id,
                    "expires_at": expires_at,
                });
                api.post_json("/workspace/users", body).await
            }
            UserCommand::SetPassword {
                onboarding_secret_stdin,
                invitation_secret_stdin,
                password_stdin,
            } => {
                if !onboarding_secret_stdin || !invitation_secret_stdin || !password_stdin {
                    return Err(CliError::Input(
                        "password setup requires --onboarding-secret-stdin, \
                         --invitation-secret-stdin, and --password-stdin"
                            .to_owned(),
                    ));
                }
                let [onboarding_secret, invitation_secret, password] =
                    read_stdin_lines(["onboarding secret", "invitation secret", "password"])?;
                let body = json!({
                    "onboarding_secret": onboarding_secret,
                    "invitation_secret": invitation_secret,
                    "password": password,
                });
                api.post_json("/onboarding/complete", body).await
            }
        },
        WorkspaceCommand::Invitation { command } => match command {
            InvitationCommand::List => api.get("/workspace/invitations").await,
            InvitationCommand::Create {
                email,
                role_id,
                scope_type,
                scope_target_id,
                expires_at,
            } => {
                let body = json!({
                    "email": email,
                    "role_id": role_id,
                    "scope_type": scope_type.as_str(),
                    "scope_target_id": scope_target_id,
                    "expires_at": expires_at,
                });
                api.post_json("/workspace/invitations", body).await
            }
            InvitationCommand::Revoke { invitation_id } => {
                api.delete(&format!(
                    "/workspace/invitations/{}",
                    segment(invitation_id)
                ))
                .await
            }
            InvitationCommand::Accept { secret_stdin } => {
                if !secret_stdin {
                    return Err(CliError::Input(
                        "invitation acceptance requires --secret-stdin".to_owned(),
                    ));
                }
                let secret = read_secret_stdin("invitation secret")?;
                api.post_json("/workspace/invitations/accept", json!({ "secret": secret }))
                    .await
            }
        },
    }
}

const AGENT_SETTINGS_PATH: &str = "/workspace/agent-settings";
const IDENTITY_PROVIDER_PATH: &str = "/workspace/identity-provider";
const TAG_COLORS_PATH: &str = "/workspace/tag-colors";

async fn navigation_command(api: &Api, command: NavigationCommand) -> Result<String, CliError> {
    match command {
        NavigationCommand::Get => api.get("/workspace/navigation").await,
        NavigationCommand::Set { entries } => {
            let entries = json_array_input(&entries, "--entries")?;
            api.put_json(
                "/workspace/navigation",
                json!({ "explore_navigation": entries }),
            )
            .await
        }
        NavigationCommand::Sidebar => api.get("/workspace/navigation/sidebar").await,
    }
}

async fn member_command(api: &Api, command: MemberCommand) -> Result<String, CliError> {
    match command {
        MemberCommand::List => api.get("/workspace/members").await,
        MemberCommand::UnlinkIdentity { member_id } => {
            api.delete(&format!(
                "/workspace/members/{}/identity",
                segment(member_id)
            ))
            .await
        }
        MemberCommand::SetState { member_id, state } => {
            api.put_json(
                &format!("/workspace/members/{}", segment(member_id)),
                json!({ "state": state.as_str() }),
            )
            .await
        }
        MemberCommand::Grant {
            member_id,
            role_id,
            scope_type,
            scope_target_id,
        } => {
            let body = json!({
                "role_id": role_id,
                "scope_type": scope_type.as_str(),
                "scope_target_id": scope_target_id,
            });
            api.post_json(
                &format!("/workspace/members/{}/grants", segment(member_id)),
                body,
            )
            .await
        }
        MemberCommand::RevokeGrant {
            member_id,
            grant_id,
        } => {
            api.delete(&format!(
                "/workspace/members/{}/grants/{}",
                segment(member_id),
                segment(grant_id)
            ))
            .await
        }
        MemberCommand::TransferOwnership { member_id } => {
            api.post(&format!(
                "/workspace/members/{}/transfer-ownership",
                segment(member_id)
            ))
            .await
        }
    }
}

async fn role_command(api: &Api, command: RoleCommand) -> Result<String, CliError> {
    match command {
        RoleCommand::List => api.get("/workspace/roles").await,
        RoleCommand::Permission {
            command: PermissionCommand::List,
        } => api.get("/workspace/permissions").await,
        RoleCommand::AssignableRole {
            command: AssignableRoleCommand::List,
        } => api.get("/workspace/assignable-roles").await,
        RoleCommand::Create { code, permissions } => {
            let permissions = json_string_array_input(&permissions, "--permissions")?;
            api.post_json(
                "/workspace/roles",
                json!({ "code": code, "permissions": permissions }),
            )
            .await
        }
        RoleCommand::Update {
            role_id,
            code,
            permissions,
        } => {
            let permissions = json_string_array_input(&permissions, "--permissions")?;
            api.put_json(
                &format!("/workspace/roles/{}", segment(role_id)),
                json!({ "code": code, "permissions": permissions }),
            )
            .await
        }
        RoleCommand::Duplicate { role_id, code } => {
            api.post_json(
                &format!("/workspace/roles/{}/duplicate", segment(role_id)),
                json!({ "code": code }),
            )
            .await
        }
        RoleCommand::Retire {
            role_id,
            replacement_role_id,
        } => {
            api.post_json(
                &format!("/workspace/roles/{}/retire", segment(role_id)),
                json!({ "replacement_role_id": replacement_role_id }),
            )
            .await
        }
    }
}
