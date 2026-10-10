use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::ExitCode,
    time::Duration,
};

use clap::{Args, Parser, Subcommand, ValueEnum};
use futures_util::StreamExt;
use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use url::Url;
use uuid::Uuid;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Large artifacts and file uploads are bounded independently from JSON API calls.
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const GENERATOR_TOKEN_LABEL: &str = "catalog-generator";
const GENERATOR_TOKEN_PERMISSIONS: &[&str] = &[
    "blueprints.read",
    "blueprints.write",
    "blueprints.publish",
    "contexts.read",
    "contexts.write",
    "records.read",
    "records.write",
    "records.publish",
];

#[derive(Parser)]
#[command(name = "acli", about = "JSON-first client for the Catalog API")]
struct Cli {
    /// API base URL, including its `/api` path.
    #[arg(long, env = "CATALOG_SERVER")]
    server: Option<Url>,
    /// Personal API token. Prefer CATALOG_TOKEN or --token-stdin to avoid exposing it in process arguments.
    #[arg(
        long,
        env = "CATALOG_TOKEN",
        hide_env_values = true,
        conflicts_with = "token_stdin"
    )]
    token: Option<String>,
    /// Read the personal API token from standard input.
    #[arg(long, conflicts_with = "token")]
    token_stdin: bool,
    /// Persist browser session and CSRF cookies for the auth commands.
    #[arg(long, env = "CATALOG_SESSION_FILE", value_name = "PATH")]
    session_file: Option<PathBuf>,
    /// Do not load environment variables from a .env file in the current directory.
    #[arg(long, global = true)]
    no_env: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Health,
    /// Browser-session authentication. Use --session-file to reuse a login across commands.
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    Event {
        #[command(subcommand)]
        command: EventCommand,
    },
    Blueprint {
        #[command(subcommand)]
        command: BlueprintCommand,
    },
    Context {
        #[command(subcommand)]
        command: ContextCommand,
    },
    Record {
        #[command(subcommand)]
        command: RecordCommand,
    },
    /// List workspace users and teams that assignment attributes can reference.
    Directory,
    /// Read and manage your own notification inbox in the workspace.
    Notification {
        #[command(subcommand)]
        command: NotificationCommand,
    },
    /// Manage workspace teams (requires `members.manage`).
    Team {
        #[command(subcommand)]
        command: TeamCommand,
    },
    /// Manage workspace translations for `{{…}}` references in catalog labels.
    Lexicon {
        #[command(subcommand)]
        command: LexiconCommand,
    },
    /// Create and share Explorer saved views.
    SavedView {
        #[command(subcommand)]
        command: SavedViewCommand,
    },
    Value {
        #[command(subcommand)]
        command: ValueCommand,
    },
    Audit {
        #[command(subcommand)]
        command: AuditCommand,
    },
    DataHealth {
        #[command(subcommand)]
        command: DataHealthCommand,
    },
    Workflow {
        #[command(subcommand)]
        command: WorkflowCommand,
    },
    Rule {
        #[command(subcommand)]
        command: RuleCommand,
    },
    ExtensionRegistry {
        #[command(subcommand)]
        command: ExtensionRegistryCommand,
    },
    Extension {
        #[command(subcommand)]
        command: ExtensionCommand,
    },
    ExtensionOperation {
        #[command(subcommand)]
        command: ExtensionOperationCommand,
    },
    /// Your own interactive extension runs, started from extension actions.
    ExtensionRun {
        #[command(subcommand)]
        command: ExtensionRunCommand,
    },
    ExtensionSchedule {
        #[command(subcommand)]
        command: ExtensionScheduleCommand,
    },
    ConnectorJob {
        #[command(subcommand)]
        command: ConnectorJobCommand,
    },
    SolutionPack {
        #[command(subcommand)]
        command: SolutionPackCommand,
    },
    /// Discover and download immutable private presentation assets.
    PresentationAsset {
        #[command(subcommand)]
        command: PresentationAssetCommand,
    },
    File {
        #[command(subcommand)]
        command: FileCommand,
    },
    /// Download Prometheus metrics to a file; metrics are intentionally not JSON.
    Metrics {
        #[command(subcommand)]
        command: MetricsCommand,
    },
    /// Administration for the workspace selected by the bearer credential.
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCommand,
    },
    /// Personal API tokens for the authenticated user.
    Token {
        #[command(subcommand)]
        command: TokenCommand,
    },
}

#[derive(Args)]
struct LexiconIdentity {
    /// English source text, as written inside `{{…}}`.
    #[arg(long)]
    key: String,
    /// Disambiguation context, as written after `|`.
    #[arg(long)]
    context: Option<String>,
    /// BCP 47 language tag, such as `pl`.
    #[arg(long)]
    language: String,
    /// CLDR plural category: zero, one, two, few, many, or other.
    #[arg(long, default_value = "other")]
    plural_category: String,
}

#[derive(Subcommand)]
enum NotificationCommand {
    /// List notifications, newest first, with the unread count.
    List {
        /// Only list unread notifications.
        #[arg(long)]
        unread: bool,
        /// Continue after a page: the created_at of its last item.
        #[arg(long, requires = "before_id")]
        before_time: Option<String>,
        /// Continue after a page: the id of its last item.
        #[arg(long, requires = "before_time")]
        before_id: Option<Uuid>,
    },
    /// Print the number of unread notifications.
    Count,
    /// Show one notification.
    Get { id: Uuid },
    /// Mark a notification as read.
    Read { id: Uuid },
    /// Mark a notification as unread.
    Unread { id: Uuid },
    /// Mark every unread notification as read.
    ReadAll,
    /// Permanently delete a notification.
    Delete { id: Uuid },
}

#[derive(Subcommand)]
enum TeamCommand {
    /// List teams and their member user IDs.
    List,
    /// Create a team.
    Create {
        #[arg(long)]
        code: String,
        #[arg(long)]
        name: String,
        /// Member user ID; repeat for several.
        #[arg(long = "member")]
        members: Vec<Uuid>,
    },
    /// Rename a team and/or replace its members.
    Update {
        id: Uuid,
        #[arg(long)]
        name: Option<String>,
        /// Member user ID; repeat for several. Replaces every member.
        #[arg(long = "member")]
        members: Vec<Uuid>,
        /// Remove every member.
        #[arg(long, conflicts_with = "members")]
        clear_members: bool,
    },
    /// Delete a team. Existing assignments keep it and show it as deleted.
    Delete { id: Uuid },
}

#[derive(Subcommand)]
enum LexiconCommand {
    /// List entries, optionally for one language.
    List {
        #[arg(long)]
        language: Option<String>,
    },
    /// Create or replace one entry.
    Set {
        #[command(flatten)]
        identity: LexiconIdentity,
        #[arg(long)]
        text: String,
    },
    /// Delete one entry.
    Delete {
        #[command(flatten)]
        identity: LexiconIdentity,
    },
    /// Print one language as an importable lexicon file.
    Export {
        #[arg(long)]
        language: String,
    },
    /// Import a JSON or TOML lexicon file for one language.
    Import {
        #[arg(long)]
        file: PathBuf,
        /// Delete entries of the file's language that the file does not contain.
        #[arg(long)]
        replace: bool,
    },
    /// Report untranslated references, missing plural forms, and orphaned entries.
    Report {
        /// Language to report; repeat for several. Defaults to `en` and every
        /// language with entries.
        #[arg(long)]
        language: Vec<String>,
    },
}

#[derive(Subcommand)]
enum SavedViewCommand {
    List {
        /// Only list views whose name or description contains this text.
        #[arg(long)]
        query: Option<String>,
    },
    Get {
        id: Uuid,
    },
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        state: String,
        #[arg(long, default_value = "private")]
        visibility: String,
        #[arg(long, default_value = "")]
        description: String,
    },
    Update {
        id: Uuid,
        #[arg(long)]
        name: String,
        #[arg(long)]
        state: String,
        #[arg(long, default_value = "private")]
        visibility: String,
        #[arg(long, default_value = "")]
        description: String,
    },
    Delete {
        id: Uuid,
    },
    /// Create an unnamed short URL snapshot of an Explorer search.
    Link {
        #[arg(long)]
        state: String,
    },
}

#[derive(Subcommand)]
enum AuthCommand {
    Discover {
        /// Workspace login identifier, not a user ID, workspace UUID, or slug.
        #[arg(value_name = "WORKSPACE_LOGIN_IDENTIFIER")]
        login_identifier: String,
    },
    Login {
        /// Workspace login identifier, not a user ID, workspace UUID, or slug.
        #[arg(value_name = "WORKSPACE_LOGIN_IDENTIFIER")]
        login_identifier: String,
        #[arg(long)]
        email: String,
        #[arg(long)]
        password_stdin: bool,
    },
    PasswordReset {
        #[arg(long)]
        email: String,
    },
    PasswordResetConfirm {
        #[arg(long)]
        token_stdin: bool,
        #[arg(long)]
        password_stdin: bool,
    },
    Session,
    /// Update the authenticated user's display preferences.
    Preferences {
        /// IANA time zone used to render timestamps, for example `Europe/Warsaw` or `UTC`.
        #[arg(
            long,
            value_name = "IANA_ZONE",
            required_unless_present = "clear_time_zone"
        )]
        time_zone: Option<String>,
        /// Follow each client's own time zone instead of a stored preference.
        #[arg(long, conflicts_with = "time_zone")]
        clear_time_zone: bool,
    },
    /// Change the authenticated user's display name.
    DisplayName {
        /// 2–64 letters, digits, and spaces; no leading or trailing space.
        name: String,
    },
    Logout,
    Renew,
}

#[derive(Subcommand)]
enum EventCommand {
    DeadLetters,
    Replay { consumer_id: Uuid, event_id: Uuid },
}

#[derive(Subcommand)]
enum BlueprintCommand {
    List {
        #[arg(long)]
        include_drafts: bool,
    },
    Create(SourceInput),
    Revision {
        blueprint_id: Uuid,
        #[command(flatten)]
        source: SourceInput,
    },
    Publish {
        blueprint_id: Uuid,
        version: i64,
    },
    Get {
        blueprint_id: Uuid,
    },
    GetVersion {
        blueprint_id: Uuid,
        version: i64,
    },
    Catalogue,
    RevisionList {
        blueprint_id: Uuid,
    },
    PublishRecords {
        blueprint_id: Uuid,
        version: i64,
        #[arg(long)]
        context_id: Uuid,
    },
    PublishRecordsAll {
        blueprint_id: Uuid,
        version: i64,
    },
    SafeMigrationBatch {
        blueprint_id: Uuid,
        version: i64,
    },
    Resolve {
        code: String,
        #[arg(long)]
        version: Option<i64>,
        #[arg(long)]
        include_drafts: bool,
    },
}

#[derive(Args)]
struct SourceInput {
    #[arg(long, conflicts_with = "stdin")]
    file: Option<PathBuf>,
    #[arg(long)]
    stdin: bool,
}

#[derive(Subcommand)]
enum ContextCommand {
    List,
    Create {
        #[arg(long, conflicts_with_all = ["code", "data", "parent_id"])]
        file: Option<PathBuf>,
        #[arg(long, requires = "data")]
        code: Option<String>,
        #[arg(long, requires = "code")]
        data: Option<String>,
        #[arg(long)]
        parent_id: Option<Uuid>,
    },
    Get {
        code: String,
    },
    Update {
        context_id: Uuid,
        #[arg(long)]
        parent_id: Uuid,
        #[arg(long)]
        data: String,
    },
    Delete {
        context_id: Uuid,
    },
}

#[derive(Subcommand)]
enum RecordCommand {
    Create {
        #[arg(long)]
        blueprint: String,
        #[arg(long)]
        version: Option<i64>,
        #[arg(long)]
        values: PathBuf,
        #[arg(long)]
        context_id: Option<Uuid>,
        /// JSON array of agent/operator-owned tags.
        #[arg(long)]
        system_tags: Option<String>,
        /// JSON object of agent/operator-owned metadata.
        #[arg(long)]
        system_metadata: Option<String>,
    },
    Get {
        record_id: Uuid,
    },
    Delete {
        record_id: Uuid,
    },
    /// Apply create, update and delete operations to several records
    /// atomically: all succeed or none do.
    Batch {
        /// JSON array of batch operations, or a file containing it.
        #[arg(long)]
        operations: String,
    },
    List {
        #[arg(long)]
        blueprint: String,
        #[arg(long)]
        related_from: Uuid,
        #[arg(long)]
        relationship: String,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        cursor: Option<Uuid>,
    },
    Preview {
        record_id: Uuid,
        #[arg(long)]
        relationship_depth: Option<u8>,
        #[arg(long)]
        relationship_limit: Option<u32>,
    },
    ResolvedPreview {
        record_id: Uuid,
        #[arg(long)]
        context_id: Uuid,
    },
    Search {
        #[arg(long)]
        blueprint: String,
        #[arg(long)]
        version: Option<i64>,
        #[arg(long, default_value = "")]
        query: String,
        #[arg(long, default_value_t = 25)]
        size: u32,
        #[arg(long)]
        cursor: Option<String>,
        /// JSON array; return records containing every requested system tag.
        #[arg(long)]
        system_tags: Option<String>,
        /// JSON array of structured search filters, or a file containing it.
        #[arg(long)]
        filters: Option<String>,
        #[arg(long)]
        outdated: bool,
        #[arg(long)]
        include_total: bool,
        /// JSON array of relationship-tree facet requests, or a file containing it.
        #[arg(long)]
        relationship_tree_facets: Option<String>,
        #[arg(long)]
        sort_field: Option<String>,
        #[arg(long, requires = "sort_field", value_parser = ["asc", "desc"])]
        sort_direction: Option<String>,
    },
    Form {
        record_id: Uuid,
    },
    Update {
        record_id: Uuid,
        #[arg(long)]
        values: Option<PathBuf>,
        #[arg(long)]
        relationships: Option<PathBuf>,
        #[arg(long)]
        remove_values: Option<PathBuf>,
        #[arg(long)]
        context_id: Option<Uuid>,
        /// JSON array of agent/operator-owned tags. Omit to retain existing tags.
        #[arg(long)]
        system_tags: Option<String>,
        /// JSON object of agent/operator-owned metadata. Omit to retain existing metadata.
        #[arg(long)]
        system_metadata: Option<String>,
    },
    Migrate {
        record_id: Uuid,
        /// JSON array or file of scalar migration values.
        #[arg(long)]
        values: Option<String>,
        /// JSON array or file of relationship migration sets.
        #[arg(long)]
        relationships: Option<String>,
        /// JSON array or file of attribute codes to discard.
        #[arg(long)]
        discard_attributes: Option<String>,
    },
    Hierarchy {
        record_id: Uuid,
        #[arg(long)]
        context_id: Uuid,
        #[arg(long)]
        field: String,
    },
    IncomingRelationships {
        record_id: Uuid,
        #[arg(long)]
        relationships: String,
        #[arg(long)]
        size: Option<u32>,
        #[arg(long)]
        cursor: Option<String>,
    },
    FacetChildren {
        #[arg(long)]
        blueprint: String,
        #[arg(long)]
        version: Option<i64>,
        #[arg(long)]
        query: Option<String>,
        #[arg(long)]
        source_relationship_field: String,
        #[arg(long)]
        hierarchy_field: Option<String>,
        #[arg(long)]
        context_id: Uuid,
        #[arg(long)]
        parent_id: Option<Uuid>,
        #[arg(long)]
        cursor: Option<Uuid>,
        #[arg(long)]
        selected_target_ids: Option<String>,
    },
    Publication {
        #[command(subcommand)]
        command: RecordPublicationCommand,
    },
    Changes {
        record_id: Uuid,
    },
    ValueHistory {
        record_id: Uuid,
    },
    RestoreValue {
        record_id: Uuid,
        history_id: Uuid,
    },
    MigrateBulk {
        #[arg(long)]
        blueprint: String,
        #[arg(long)]
        from_version: i64,
        #[arg(long, default_value_t = 100)]
        size: u32,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
enum WorkspaceCommand {
    Navigation {
        #[command(subcommand)]
        command: NavigationCommand,
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
enum NavigationCommand {
    Get,
    Set {
        #[arg(long)]
        entries: String,
    },
    Sidebar,
}
#[derive(Subcommand)]
enum TokenPermissionCommand {
    List,
}
#[derive(Subcommand)]
enum GrantTargetCommand {
    List { scope_type: String },
}
#[derive(Subcommand)]
enum PublicationChannelCommand {
    List,
    Set {
        context_id: Uuid,
        #[arg(long, action = clap::ArgAction::Set)]
        enabled: bool,
    },
}
#[derive(Subcommand)]
enum RecordPublicationCommand {
    List {
        record_id: Uuid,
    },
    Publish {
        record_id: Uuid,
        #[arg(long)]
        context_id: Uuid,
    },
    Unpublish {
        record_id: Uuid,
        #[arg(long)]
        context_id: Uuid,
    },
    PublishAll {
        record_id: Uuid,
    },
}

#[derive(Subcommand)]
enum MemberCommand {
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
enum RoleCommand {
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
enum PermissionCommand {
    List,
}
#[derive(Subcommand)]
enum AssignableRoleCommand {
    List,
}

#[derive(Subcommand)]
enum InvitationCommand {
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
enum UserCommand {
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

#[derive(Subcommand)]
enum TokenCommand {
    List,
    Create {
        /// Create a least-privilege token for the demo catalog generator.
        #[arg(long, conflicts_with_all = ["label", "permissions"])]
        generator: bool,
        #[arg(
            long,
            required_unless_present = "generator",
            conflicts_with = "generator"
        )]
        label: Option<String>,
        #[arg(
            long,
            required_unless_present = "generator",
            conflicts_with = "generator"
        )]
        permissions: Option<String>,
        #[arg(long)]
        expires_at: Option<String>,
    },
    Revoke {
        token_id: Uuid,
    },
}

#[derive(Subcommand)]
enum AuditCommand {
    List {
        #[arg(long)]
        limit: Option<i64>,
        #[arg(long)]
        offset: Option<i64>,
        #[arg(long)]
        occurred_after: Option<String>,
        #[arg(long)]
        occurred_before: Option<String>,
        #[arg(long)]
        actor_user_id: Option<Uuid>,
        #[arg(long)]
        action_category: Option<String>,
        #[arg(long)]
        target_type: Option<String>,
        #[arg(long, value_parser = ["human", "agent"])]
        executor_type: Option<String>,
        #[arg(long)]
        agent_run_id: Option<Uuid>,
        #[arg(long)]
        agent_tool_call_id: Option<Uuid>,
    },
}

#[derive(Subcommand)]
enum DataHealthCommand {
    Summary {
        #[arg(long)]
        stale_after_days: Option<u16>,
    },
    Blueprints {
        #[arg(long)]
        stale_after_days: Option<u16>,
    },
    Freshness,
    Completeness,
    Contexts,
    Relationships,
    Storage,
    BackgroundProcessing,
    Refresh,
}

#[derive(Subcommand)]
enum WorkflowCommand {
    List,
    Validate(SourceInput),
    Create(SourceInput),
    Get {
        workflow_id: Uuid,
    },
    RevisionList {
        workflow_id: Uuid,
    },
    VersionGet {
        workflow_id: Uuid,
        version: i64,
    },
    Revision {
        workflow_id: Uuid,
        #[command(flatten)]
        source: SourceInput,
    },
    Publish {
        workflow_id: Uuid,
        version: i64,
    },
    Enable {
        workflow_id: Uuid,
        version: i64,
    },
    Disable {
        workflow_id: Uuid,
    },
    RunNow {
        workflow_id: Uuid,
        #[arg(long)]
        record_id: Uuid,
        #[arg(long)]
        idempotency_key: String,
    },
    RunList,
    /// Per-target outcomes of `referencing_records_update` actions in a run.
    RunTargets {
        run_id: Uuid,
    },
    RunReplay {
        run_id: Uuid,
    },
}

#[derive(Subcommand)]
enum RuleCommand {
    List {
        #[arg(long)]
        blueprint_id: Option<Uuid>,
    },
    Get {
        rule_id: Uuid,
    },
    Validate {
        #[arg(long)]
        blueprint_id: Uuid,
        #[arg(long)]
        blueprint_version: i64,
        #[arg(long)]
        context_id: Option<Uuid>,
        #[command(flatten)]
        source: SourceInput,
    },
    Create {
        #[arg(long)]
        blueprint_id: Uuid,
        #[arg(long)]
        blueprint_version: i64,
        #[arg(long)]
        context_id: Option<Uuid>,
        #[command(flatten)]
        source: SourceInput,
    },
    Revision {
        rule_id: Uuid,
        #[arg(long)]
        blueprint_id: Uuid,
        #[arg(long)]
        blueprint_version: i64,
        #[arg(long)]
        context_id: Option<Uuid>,
        #[command(flatten)]
        source: SourceInput,
    },
    Publish {
        rule_id: Uuid,
        version: i64,
    },
    Enable {
        rule_id: Uuid,
        version: i64,
        /// Enable an enforcing rule although its completed dry run found
        /// existing violations; those records cannot be saved until fixed.
        #[arg(long)]
        accept_existing_violations: bool,
    },
    Disable {
        rule_id: Uuid,
    },
    RunNow {
        rule_id: Uuid,
        #[arg(long)]
        idempotency_key: String,
        #[arg(long)]
        record_id: Option<Uuid>,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    RunList,
    RunReplay {
        run_id: Uuid,
    },
    Findings {
        #[arg(long)]
        record_id: Option<Uuid>,
    },
    Acknowledge {
        finding_id: Uuid,
    },
}

#[derive(Subcommand)]
enum ExtensionRegistryCommand {
    List,
    Add {
        #[arg(long)]
        source: String,
    },
    Remove {
        registry_id: Uuid,
    },
    Discover,
    Details {
        owner: String,
        repository: String,
    },
}

#[derive(Subcommand)]
enum ExtensionCommand {
    List,
    Detail {
        extension_id: String,
    },
    Remove {
        extension_id: String,
    },
    Runtime,
    WorkspaceMode {
        #[arg(long, action = clap::ArgAction::Set)]
        enabled: bool,
    },
    Install {
        #[arg(long)]
        owner: String,
        #[arg(long)]
        repository: String,
        #[arg(long)]
        release_id: u64,
    },
    Sideload {
        #[arg(long)]
        file: PathBuf,
    },
    Upgrade {
        extension_id: String,
        #[arg(long)]
        owner: String,
        #[arg(long)]
        repository: String,
        #[arg(long)]
        release_id: u64,
    },
    Configure {
        extension_id: String,
        #[arg(long)]
        configuration: String,
    },
    Grant {
        extension_id: String,
        #[arg(long)]
        grant_kind: String,
        #[arg(long)]
        grant_id: String,
    },
    Revoke {
        extension_id: String,
        grant_kind: String,
        grant_id: String,
    },
    Enable {
        extension_id: String,
    },
    Disable {
        extension_id: String,
    },
    Quarantine {
        extension_id: String,
        #[arg(long)]
        diagnostic_code: String,
    },
    Artifact {
        extension_id: String,
        contribution_id: String,
        #[arg(long)]
        output: PathBuf,
    },
    Storage {
        extension_id: String,
        contribution_id: String,
        release_id: Uuid,
        #[arg(long)]
        body: String,
    },
    /// Inventory an extension's annotation namespace, or adopt existing
    /// annotations under its name with --adopt.
    AnnotationNamespace {
        extension_id: String,
        #[arg(long)]
        adopt: bool,
    },
    /// Operator repair or cleanup of one extension's annotations on a record.
    RepairAnnotations {
        extension_id: String,
        record_id: Uuid,
        /// Annotation patch JSON object or path to a JSON file.
        #[arg(long)]
        patch: String,
    },
    Command {
        extension_id: String,
        contribution_id: String,
        #[arg(long)]
        release_id: Uuid,
        #[arg(long)]
        command_id: String,
        #[arg(long)]
        payload: String,
    },
}

#[derive(Subcommand)]
enum ExtensionRunCommand {
    /// List your 50 most recent interactive runs.
    List {
        #[arg(long)]
        extension_id: Option<String>,
    },
    /// Show status, outcome and downloadable outputs of one run.
    Show { run_id: Uuid },
    /// Request cancellation of a run that has not finished.
    Cancel { run_id: Uuid },
    /// Download an output of a completed run.
    Download {
        run_id: Uuid,
        artifact_id: Uuid,
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(Subcommand)]
enum ExtensionOperationCommand {
    Start {
        extension_id: String,
        #[arg(long)]
        operation_id: String,
        /// JSON object or path to a JSON file.
        #[arg(long)]
        input: String,
        #[arg(long)]
        idempotency_key: String,
        #[arg(long)]
        input_file_id: Option<Uuid>,
    },
    List,
    Show {
        run_id: Uuid,
    },
    Artifacts {
        run_id: Uuid,
    },
    Deliveries {
        run_id: Uuid,
    },
    Download {
        run_id: Uuid,
        artifact_id: Uuid,
        #[arg(long)]
        output: PathBuf,
    },
    Cancel {
        run_id: Uuid,
    },
    Replay {
        run_id: Uuid,
    },
}

#[derive(Subcommand)]
enum ExtensionScheduleCommand {
    List,
    Create {
        extension_id: String,
        #[arg(long)]
        operation_id: String,
        /// JSON object or path to a JSON file.
        #[arg(long)]
        input: String,
        #[arg(long)]
        input_file_id: Option<Uuid>,
        #[arg(long, value_parser = clap::value_parser!(i32).range(60..=2592000))]
        interval_seconds: i32,
    },
    Update {
        schedule_id: Uuid,
        #[arg(long, action = clap::ArgAction::Set)]
        enabled: bool,
        #[arg(long, value_parser = clap::value_parser!(i32).range(60..=2592000))]
        interval_seconds: i32,
    },
}

#[derive(Subcommand)]
enum ConnectorJobCommand {
    List {
        blueprint_id: Uuid,
    },
    Run {
        job_id: Uuid,
        #[arg(long)]
        idempotency_key: String,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum BlueprintPublicationArgument {
    Draft,
    Publish,
}

impl BlueprintPublicationArgument {
    fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Publish => "publish",
        }
    }
}

#[derive(Subcommand)]
enum SolutionPackPlanCommand {
    /// Show one immutable workspace-scoped plan.
    Show { plan_id: Uuid },
}

#[derive(Subcommand)]
enum SolutionPackApplicationsCommand {
    /// List bounded application history for the active workspace.
    List {
        #[arg(long, default_value_t = 25)]
        limit: u32,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    /// Show one workspace-scoped application and its ordered steps.
    Show { application_id: Uuid },
    /// Permanently abandon a resumable sample-selected application.
    Abandon { application_id: Uuid },
}

#[derive(Subcommand)]
enum SolutionPackChecksCommand {
    /// Evaluate the application's informational checks against current state.
    Rerun { application_id: Uuid },
    /// List bounded immutable check-run history.
    List {
        application_id: Uuid,
        #[arg(long, default_value_t = 25)]
        limit: u32,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    /// Show one immutable check run and its ordered results.
    Show { application_id: Uuid, run_id: Uuid },
}

#[derive(Subcommand)]
enum SolutionPackCommand {
    /// Validate and summarize a local archive on the authoritative server.
    Inspect {
        #[arg(long)]
        file: PathBuf,
    },
    /// Create a local-archive plan, or show an existing plan.
    Plan {
        #[command(subcommand)]
        command: Option<SolutionPackPlanCommand>,
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        prefix: Option<String>,
        #[arg(long)]
        blueprint_publication: Option<BlueprintPublicationArgument>,
        /// Explicitly reuse a published blueprint (`logical_key=existing_code`).
        #[arg(
            long = "map",
            value_name = "LOGICAL_KEY=EXISTING_CODE",
            conflicts_with = "from_application"
        )]
        blueprint_maps: Vec<String>,
        /// Explicitly reuse an immutable presentation asset (`logical_key=asset_uuid`).
        #[arg(
            long = "map-asset",
            value_name = "LOGICAL_KEY=ASSET_UUID",
            conflicts_with = "from_application"
        )]
        asset_maps: Vec<String>,
        /// Use an existing context for a pack context instead of creating one
        /// (`logical_key=existing_code`).
        #[arg(
            long = "map-context",
            value_name = "LOGICAL_KEY=EXISTING_CODE",
            conflicts_with = "from_application"
        )]
        context_maps: Vec<String>,
        /// Reuse unchanged resources from one completed application.
        #[arg(long, conflicts_with_all = ["blueprint_maps", "asset_maps", "context_maps"])]
        from_application: Option<Uuid>,
        /// Explicitly select the pack's optional synthetic sample records.
        #[arg(long)]
        include_sample_data: bool,
    },
    /// Apply exactly the persisted immutable plan; no choices are recomputed.
    Apply { plan_id: Uuid },
    /// Read durable application history.
    Applications {
        #[command(subcommand)]
        command: SolutionPackApplicationsCommand,
    },
    /// Evaluate and read informational setup checks.
    Checks {
        #[command(subcommand)]
        command: SolutionPackChecksCommand,
    },
}

#[derive(Subcommand)]
enum PresentationAssetCommand {
    List {
        #[arg(long, default_value_t = 25, value_parser = clap::value_parser!(u16).range(1..=100))]
        limit: u16,
        #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u16).range(0..=10000))]
        offset: u16,
    },
    Show {
        asset_id: Uuid,
    },
    Download {
        asset_id: Uuid,
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(Subcommand)]
enum FileCommand {
    Upload {
        record_id: Uuid,
        attribute_code: String,
        #[arg(long = "file", required = true)]
        files: Vec<PathBuf>,
        #[arg(long)]
        context_id: Option<Uuid>,
    },
    Metadata {
        file_id: Uuid,
    },
    DownloadOriginal {
        file_id: Uuid,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        range: Option<String>,
    },
    DownloadVariant {
        file_id: Uuid,
        kind: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        range: Option<String>,
    },
}

#[derive(Subcommand)]
enum MetricsCommand {
    Get {
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(Subcommand)]
enum ValueCommand {
    Append {
        record_id: Uuid,
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        context_id: Option<Uuid>,
    },
    Current {
        record_id: Uuid,
    },
    Replace {
        record_id: Uuid,
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        context_id: Option<Uuid>,
    },
    Remove {
        record_id: Uuid,
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        context_id: Option<Uuid>,
    },
}

#[derive(Debug, Error)]
enum CliError {
    #[error("{0}")]
    Input(String),
    #[error("{0}")]
    Transport(String),
    #[error("{message}")]
    Api {
        status: u16,
        code: String,
        message: String,
        /// The API's optional machine-readable `error.details`.
        details: Option<Value>,
    },
    #[error("server returned invalid JSON")]
    InvalidResponse,
    #[error("server response exceeded {MAX_RESPONSE_BYTES} bytes")]
    ResponseTooLarge,
}

impl CliError {
    fn exit_code(&self) -> u8 {
        match self {
            Self::Input(_) => 2,
            Self::Transport(_) => 3,
            Self::Api { .. } => 4,
            Self::InvalidResponse | Self::ResponseTooLarge => 5,
        }
    }

    fn json(&self) -> Value {
        match self {
            Self::Input(message) => {
                json!({ "error": { "code": "invalid_input", "message": message, "status": null } })
            }
            Self::Transport(message) => {
                json!({ "error": { "code": "request_failed", "message": message, "status": null } })
            }
            Self::Api {
                status,
                code,
                message,
                details,
            } => {
                let mut error = json!({ "code": code, "message": message, "status": status });
                if let Some(details) = details {
                    error["details"] = details.clone();
                }
                json!({ "error": error })
            }
            Self::InvalidResponse => {
                json!({ "error": { "code": "invalid_response", "message": self.to_string(), "status": null } })
            }
            Self::ResponseTooLarge => {
                json!({ "error": { "code": "response_too_large", "message": self.to_string(), "status": null } })
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextFile {
    code: String,
    data: toml::Value,
    parent_id: Option<Uuid>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ValueFile {
    values: Vec<ValueInput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ValueInput {
    kind: String,
    attribute_id: Option<Uuid>,
    attribute_code: Option<String>,
    context_id: Option<Uuid>,
    value: Option<toml::Value>,
    target_record_id: Option<Uuid>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RelationshipFile {
    relationships: Vec<RelationshipInput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoveValuesFile {
    remove_values: Vec<RemoveValueInput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoveValueInput {
    attribute_code: String,
    context_id: Option<Uuid>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RelationshipInput {
    attribute_id: Option<Uuid>,
    attribute_code: Option<String>,
    context_id: Option<Uuid>,
    target_record_ids: Vec<Uuid>,
}

#[tokio::main]
async fn main() -> ExitCode {
    if let Err(error) = load_dotenv() {
        eprintln!("{}", error.json());
        return ExitCode::from(error.exit_code());
    }
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) if error.use_stderr() => {
            let error = CliError::Input(error.to_string());
            eprintln!("{}", error.json());
            return ExitCode::from(error.exit_code());
        }
        Err(error) => {
            print!("{error}");
            return ExitCode::SUCCESS;
        }
    };
    match run(cli).await {
        Ok(body) => {
            println!("{body}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{}", error.json());
            ExitCode::from(error.exit_code())
        }
    }
}

fn load_dotenv() -> Result<(), CliError> {
    if std::env::args_os().any(|argument| argument == "--no-env") {
        return Ok(());
    }
    match dotenvy::from_filename(".env") {
        Ok(_) => Ok(()),
        Err(dotenvy::Error::Io(error)) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CliError::Input(format!("cannot load .env: {error}"))),
    }
}

async fn run(cli: Cli) -> Result<String, CliError> {
    let server = match cli.server {
        Some(server) => server,
        None => match std::env::var("CATALOG_API_URL") {
            Ok(value) => Url::parse(&value).map_err(|error| {
                CliError::Input(format!("CATALOG_API_URL is not a valid URL: {error}"))
            })?,
            Err(_) => Url::parse("http://127.0.0.1:3000/api").expect("valid default URL"),
        },
    };
    let session_file = cli.session_file;
    // A personal token is the explicit automation credential. Do not even
    // read a saved browser session when one is supplied: it takes precedence
    // and avoids accidentally combining two identities on a request.
    let token = if cli.token_stdin {
        Some(read_secret_stdin("personal API token")?)
    } else {
        cli.token
    };
    let session = if token.is_some() {
        None
    } else {
        SessionFile::load(session_file.as_ref())?
    };
    if (token.is_some() || session.is_some()) && !is_secure_credential_transport(&server) {
        return Err(CliError::Input(
            "refusing to send credentials over insecure HTTP to a non-loopback host".to_owned(),
        ));
    }
    let mut client = Client::builder().connect_timeout(CONNECT_TIMEOUT);
    let mut headers = reqwest::header::HeaderMap::new();
    if let Some(token) = token {
        let value =
            reqwest::header::HeaderValue::from_str(&format!("Bearer {token}")).map_err(|_| {
                CliError::Input("CATALOG_TOKEN contains invalid header characters".to_owned())
            })?;
        headers.insert(reqwest::header::AUTHORIZATION, value);
    }
    if let Some(session) = &session {
        headers.insert(
            reqwest::header::COOKIE,
            reqwest::header::HeaderValue::from_str(&format!(
                "catalog_session={}; catalog_csrf={}",
                session.session, session.csrf
            ))
            .map_err(|_| {
                CliError::Input("session file contains invalid cookie characters".to_owned())
            })?,
        );
        headers.insert(
            reqwest::header::HeaderName::from_static("x-catalog-csrf"),
            reqwest::header::HeaderValue::from_str(&session.csrf).map_err(|_| {
                CliError::Input("session file contains invalid CSRF characters".to_owned())
            })?,
        );
    }
    if !headers.is_empty() {
        client = client.default_headers(headers);
    }
    let client = client
        .build()
        .map_err(|error| CliError::Transport(error.to_string()))?;

    match cli.command {
        Command::Health => request(&client, &server, Method::GET, "/health", None).await,
        Command::Directory => request(&client, &server, Method::GET, "/directory", None).await,
        Command::Notification { command } => {
            let (method, path, payload) = notification_request(command);
            request(&client, &server, method, &path, payload).await
        }
        Command::Team { command } => {
            let (method, path, payload) = team_request(command);
            request(&client, &server, method, &path, payload).await
        }
        Command::Lexicon { command } => {
            let (method, path, payload) = lexicon_request(command)?;
            request(&client, &server, method, &path, payload).await
        }
        Command::SavedView { command } => {
            let (method, path, payload, url_key) = match command {
                SavedViewCommand::List { query } => {
                    let path = match query {
                        Some(query) => {
                            let mut params = url::form_urlencoded::Serializer::new(String::new());
                            params.append_pair("q", &query);
                            format!("/saved-views?{}", params.finish())
                        }
                        None => "/saved-views".to_owned(),
                    };
                    (Method::GET, path, None, None)
                }
                SavedViewCommand::Get { id } => {
                    (Method::GET, format!("/saved-views/{id}"), None, None)
                }
                SavedViewCommand::Create {
                    name,
                    state,
                    visibility,
                    description,
                } => (
                    Method::POST,
                    "/saved-views".to_owned(),
                    Some(saved_view_payload(&name, &description, &visibility, &state)?),
                    Some("savedView"),
                ),
                SavedViewCommand::Update {
                    id,
                    name,
                    state,
                    visibility,
                    description,
                } => (
                    Method::PUT,
                    format!("/saved-views/{id}"),
                    Some(saved_view_payload(&name, &description, &visibility, &state)?),
                    Some("savedView"),
                ),
                SavedViewCommand::Delete { id } => {
                    (Method::DELETE, format!("/saved-views/{id}"), None, None)
                }
                SavedViewCommand::Link { state } => (
                    Method::POST,
                    "/view-state-links".to_owned(),
                    Some(json!({"kind": "explorer_search", "state": json_object_argument(&state)?})),
                    Some("viewState"),
                ),
            };
            let body = request(&client, &server, method, &path, payload).await?;
            if let Some(key) = url_key {
                let mut output: Value =
                    serde_json::from_str(&body).map_err(|_| CliError::InvalidResponse)?;
                if let Some(id) = output.get("id").and_then(Value::as_str).map(str::to_owned) {
                    let web = std::env::var("CATALOG_WEB_URL").ok().or_else(|| {
                        std::env::var("WEB_PORT")
                            .ok()
                            .map(|port| format!("http://127.0.0.1:{port}/"))
                    });
                    if let Some(web) = web {
                        let mut url = Url::parse(&web).map_err(|error| {
                            CliError::Input(format!("invalid CATALOG_WEB_URL: {error}"))
                        })?;
                        url.set_path("/");
                        url.set_query(Some(&format!("{key}={id}")));
                        output["url"] = json!(url.to_string());
                    }
                }
                Ok(output.to_string())
            } else {
                Ok(body)
            }
        },
        Command::Auth { command } => auth_command(&client, &server, command, session_file.as_ref()).await,
        Command::Event { command } => match command {
            EventCommand::DeadLetters => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    "/event-deliveries/dead-letters",
                    None,
                )
                .await
            }
            EventCommand::Replay {
                consumer_id,
                event_id,
            } => {
                request(
                    &client,
                    &server,
                    Method::POST,
                    &format!(
                        "/event-deliveries/{}/{}/replay",
                        segment(consumer_id),
                        segment(event_id)
                    ),
                    None,
                )
                .await
            }
        },
        Command::Blueprint { command } => match command {
            BlueprintCommand::List { include_drafts } => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    if include_drafts {
                        "/blueprints?include_drafts=true"
                    } else {
                        "/blueprints"
                    },
                    None,
                )
                .await
            }
            BlueprintCommand::Create(source) => {
                let definition = read_source(source)?;
                request(
                    &client,
                    &server,
                    Method::POST,
                    "/blueprints",
                    Some(json!({ "definition": definition })),
                )
                .await
            }
            BlueprintCommand::Revision {
                blueprint_id,
                source,
            } => {
                let definition = read_source(source)?;
                request(
                    &client,
                    &server,
                    Method::POST,
                    &format!("/blueprints/{}/versions", segment(blueprint_id)),
                    Some(json!({ "definition": definition })),
                )
                .await
            }
            BlueprintCommand::Publish {
                blueprint_id,
                version,
            } => {
                request(
                    &client,
                    &server,
                    Method::POST,
                    &format!(
                        "/blueprints/{}/versions/{version}/publish",
                        segment(blueprint_id)
                    ),
                    None,
                )
                .await
            }
            BlueprintCommand::Get { blueprint_id } => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    &format!("/blueprints/{}", segment(blueprint_id)),
                    None,
                )
                .await
            }
            BlueprintCommand::GetVersion {
                blueprint_id,
                version,
            } => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    &format!("/blueprints/{}/versions/{version}", segment(blueprint_id)),
                    None,
                )
                .await
            }
            BlueprintCommand::Catalogue => request(&client, &server, Method::GET, "/blueprints/catalogue", None).await,
            BlueprintCommand::RevisionList { blueprint_id } => request(&client, &server, Method::GET, &format!("/blueprints/{}/versions", segment(blueprint_id)), None).await,
            BlueprintCommand::PublishRecords { blueprint_id, version, context_id } => request(&client, &server, Method::POST, &format!("/blueprints/{}/versions/{version}/record-publications", segment(blueprint_id)), Some(json!({"context_id": context_id}))).await,
            BlueprintCommand::PublishRecordsAll { blueprint_id, version } => request(&client, &server, Method::POST, &format!("/blueprints/{}/versions/{version}/record-publications/publish-all", segment(blueprint_id)), None).await,
            BlueprintCommand::SafeMigrationBatch { blueprint_id, version } => request(&client, &server, Method::POST, &format!("/blueprints/{}/versions/{version}/safe-migration-batches", segment(blueprint_id)), None).await,
            BlueprintCommand::Resolve {
                code,
                version,
                include_drafts,
            } => {
                let path = match version {
                    Some(version) => {
                        format!("/blueprints/by-code/{}/versions/{version}", segment(&code))
                    }
                    None => format!(
                        "/blueprints/by-code/{}{}",
                        segment(&code),
                        if include_drafts {
                            "?include_drafts=true"
                        } else {
                            ""
                        }
                    ),
                };
                request(&client, &server, Method::GET, &path, None).await
            }
        },
        Command::Context { command } => match command {
            ContextCommand::List => request(&client, &server, Method::GET, "/contexts", None).await,
            ContextCommand::Create {
                file,
                code,
                data,
                parent_id,
            } => {
                let body = match (file, code, data, parent_id) {
                    (Some(file), None, None, None) => context_body_from_file(&file)?,
                    (None, Some(code), Some(data), parent_id) => json!({
                        "code": code,
                        "data": serde_json::from_str::<Value>(&data)
                            .map_err(|error| CliError::Input(format!("invalid --data JSON: {error}")))?,
                        "parent_id": parent_id,
                    }),
                    _ => {
                        return Err(CliError::Input(
                            "provide --file or both --code and --data".to_owned(),
                        ));
                    }
                };
                request(&client, &server, Method::POST, "/contexts", Some(body)).await
            }
            ContextCommand::Get { code } => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    &format!("/contexts/{}", segment(&code)),
                    None,
                )
                .await
            }
            ContextCommand::Update {
                context_id,
                parent_id,
                data,
            } => request(
                &client,
                &server,
                Method::PUT,
                &format!("/contexts/id/{}", segment(context_id)),
                Some(json!({
                    "parent_id": parent_id,
                    "data": serde_json::from_str::<Value>(&data)
                        .map_err(|error| CliError::Input(format!("invalid --data JSON: {error}")))?,
                })),
            )
            .await,
            ContextCommand::Delete { context_id } => {
                request(
                    &client,
                    &server,
                    Method::DELETE,
                    &format!("/contexts/id/{}", segment(context_id)),
                    None,
                )
                .await
            }
        },
        Command::Record { command } => match command {
            RecordCommand::Create {
                blueprint,
                version,
                values,
                context_id,
                system_tags,
                system_metadata,
            } => {
                let mut body = json!({
                    "blueprint": { "code": blueprint, "version": version },
                    "values": values_body_from_file(&values, context_id)?["values"].clone(),
                });
                if let Some(tags) = system_tags {
                    body["system_tags"] = json_tags_argument(&tags)?;
                }
                if let Some(metadata) = system_metadata {
                    body["system_metadata"] = json_object_argument(&metadata)?;
                }
                request(&client, &server, Method::POST, "/v1/records", Some(body)).await
            }
            RecordCommand::Get { record_id } => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    &format!("/records/{}", segment(record_id)),
                    None,
                )
                .await
            }
            RecordCommand::Delete { record_id } => {
                request(
                    &client,
                    &server,
                    Method::DELETE,
                    &format!("/records/{}", segment(record_id)),
                    None,
                )
                .await
            }
            RecordCommand::Batch { operations } => {
                request(
                    &client,
                    &server,
                    Method::POST,
                    "/v1/records/batch",
                    Some(json!({ "operations": json_array_input(&operations, "--operations")? })),
                )
                .await
            }
            RecordCommand::List {
                blueprint,
                related_from,
                relationship,
                limit,
                cursor,
            } => {
                let mut query = url::form_urlencoded::Serializer::new(String::new());
                query.append_pair("blueprint", &blueprint);
                query.append_pair("related_from", &related_from.to_string());
                query.append_pair("relationship", &relationship);
                if let Some(limit) = limit {
                    query.append_pair("limit", &limit.to_string());
                }
                if let Some(cursor) = cursor {
                    query.append_pair("cursor", &cursor.to_string());
                }
                request(
                    &client,
                    &server,
                    Method::GET,
                    &format!("/records?{}", query.finish()),
                    None,
                )
                .await
            }
            RecordCommand::Preview {
                record_id,
                relationship_depth,
                relationship_limit,
            } => {
                let mut query = url::form_urlencoded::Serializer::new(String::new());
                if let Some(depth) = relationship_depth {
                    query.append_pair("relationship_depth", &depth.to_string());
                }
                if let Some(limit) = relationship_limit {
                    query.append_pair("relationship_limit", &limit.to_string());
                }
                let query = query.finish();
                let path = format!(
                    "/records/{}/preview{}",
                    segment(record_id),
                    if query.is_empty() {
                        String::new()
                    } else {
                        format!("?{query}")
                    }
                );
                request(&client, &server, Method::GET, &path, None).await
            }
            RecordCommand::ResolvedPreview {
                record_id,
                context_id,
            } => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    &format!(
                        "/records/{}/resolved-preview?context_id={}",
                        segment(record_id),
                        segment(context_id)
                    ),
                    None,
                )
                .await
            }
            RecordCommand::Search {
                blueprint,
                version,
                query,
                size,
                cursor,
                system_tags,
                filters,
                outdated,
                include_total,
                relationship_tree_facets,
                sort_field,
                sort_direction,
            } => {
                let mut body = json!({
                    "blueprint": { "code": blueprint, "version": version },
                    "query": query,
                    "filters": json_array_argument(filters.as_deref(), "--filters")?,
                    "outdated": outdated,
                    "include_total": include_total,
                    "relationship_tree_facets": json_array_argument(relationship_tree_facets.as_deref(), "--relationship-tree-facets")?,
                    "page": { "size": size, "cursor": cursor },
                });
                if let Some(field) = sort_field {
                    body["sort"] = json!({ "field": field, "direction": sort_direction.unwrap_or_else(|| "asc".to_owned()) });
                }
                if let Some(tags) = system_tags {
                    body["system_tags"] = json_tags_argument(&tags)?;
                }
                request(
                    &client,
                    &server,
                    Method::POST,
                    "/v1/records/search",
                    Some(body),
                )
                .await
            }
            RecordCommand::Form { record_id } => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    &format!("/v1/records/{}", segment(record_id)),
                    None,
                )
                .await
            }
            RecordCommand::Update {
                record_id,
                values,
                relationships,
                remove_values,
                context_id,
                system_tags,
                system_metadata,
            } => {
                let mut body = form_update_body(
                    values.as_ref(),
                    relationships.as_ref(),
                    remove_values.as_ref(),
                    context_id,
                )?;
                if let Some(tags) = system_tags {
                    body["system_tags"] = json_tags_argument(&tags)?;
                }
                if let Some(metadata) = system_metadata {
                    body["system_metadata"] = json_object_argument(&metadata)?;
                }
                request(
                    &client,
                    &server,
                    Method::PUT,
                    &format!("/v1/records/{}", segment(record_id)),
                    Some(body),
                )
                .await
            }
            RecordCommand::Hierarchy { record_id, context_id, field } => request(&client, &server, Method::GET, &format!("/records/{}/hierarchy?context_id={}&field={}", segment(record_id), segment(context_id), segment(&field)), None).await,
            RecordCommand::IncomingRelationships { record_id, relationships, size, cursor } => request(&client, &server, Method::POST, &format!("/v1/records/{}/incoming-relationships", segment(record_id)), Some(json!({"relationships": json_array_input(&relationships, "--relationships")?, "page": {"size": size, "cursor": cursor}}))).await,
            RecordCommand::FacetChildren { blueprint, version, query, source_relationship_field, hierarchy_field, context_id, parent_id, cursor, selected_target_ids } => request(&client, &server, Method::POST, "/v1/records/facets/relationship-tree/children", Some(json!({"blueprint":{"code": blueprint, "version":version}, "query":query, "source_relationship_field":source_relationship_field, "hierarchy_field":hierarchy_field, "context_id":context_id, "parent_id":parent_id, "cursor":cursor, "selected_target_ids":json_array_argument(selected_target_ids.as_deref(), "--selected-target-ids")?}))).await,
            RecordCommand::Publication { command } => record_publication_command(&client, &server, command).await,
            RecordCommand::Changes { record_id } => request(&client, &server, Method::GET, &format!("/records/{}/changes", segment(record_id)), None).await,
            RecordCommand::ValueHistory { record_id } => request(&client, &server, Method::GET, &format!("/records/{}/values/history", segment(record_id)), None).await,
            RecordCommand::RestoreValue { record_id, history_id } => request(&client, &server, Method::POST, &format!("/records/{}/values/history/{}/restore", segment(record_id), segment(history_id)), None).await,
            RecordCommand::Migrate { record_id, values, relationships, discard_attributes } => {
                let result = migrate_record(&client, &server, record_id, false, values.as_deref(), relationships.as_deref(), discard_attributes.as_deref()).await?;
                serde_json::to_string(&result).map_err(|_| CliError::InvalidResponse)
            }
            RecordCommand::MigrateBulk {
                blueprint,
                from_version,
                size,
                dry_run,
            } => migrate_records(&client, &server, &blueprint, from_version, size, dry_run).await,
        },
        Command::Workspace { command } => workspace_command(&client, &server, command).await,
        Command::Token { command } => token_command(&client, &server, command).await,
        Command::Audit { command } => audit_command(&client, &server, command).await,
        Command::DataHealth { command } => data_health_command(&client, &server, command).await,
        Command::Workflow { command } => workflow_command(&client, &server, command).await,
        Command::Rule { command } => rule_command(&client, &server, command).await,
        Command::ExtensionRegistry { command } => extension_registry_command(&client, &server, command).await,
        Command::Extension { command } => extension_command(&client, &server, command).await,
        Command::ExtensionOperation { command } => extension_operation_command(&client, &server, command).await,
        Command::ExtensionRun { command } => extension_run_command(&client, &server, command).await,
        Command::ExtensionSchedule { command } => extension_schedule_command(&client, &server, command).await,
        Command::ConnectorJob { command } => connector_job_command(&client, &server, command).await,
        Command::SolutionPack { command } => {
            solution_pack_command(&client, &server, command).await
        }
        Command::PresentationAsset { command } => {
            presentation_asset_command(&client, &server, command).await
        }
        Command::File { command } => file_command(&client, &server, command).await,
        Command::Metrics { command: MetricsCommand::Get { output } } => raw_download(&client, &server, "/metrics", &output, None).await,
        Command::Value { command } => match command {
            ValueCommand::Append {
                record_id,
                file,
                context_id,
            } => {
                request(
                    &client,
                    &server,
                    Method::POST,
                    &format!("/records/{}/values", segment(record_id)),
                    Some(values_body_from_file(&file, context_id)?),
                )
                .await
            }
            ValueCommand::Current { record_id } => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    &format!("/records/{}/values/current", segment(record_id)),
                    None,
                )
                .await
            }
            ValueCommand::Replace {
                record_id,
                file,
                context_id,
            } => {
                request(
                    &client,
                    &server,
                    Method::POST,
                    &format!("/records/{}/relationships/replace", segment(record_id)),
                    Some(relationships_body_from_file(&file, context_id)?),
                )
                .await
            }
            ValueCommand::Remove {
                record_id,
                file,
                context_id,
            } => {
                request(
                    &client,
                    &server,
                    Method::POST,
                    &format!("/records/{}/relationships/remove", segment(record_id)),
                    Some(relationships_body_from_file(&file, context_id)?),
                )
                .await
            }
        },
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SessionFile {
    session: String,
    csrf: String,
}

impl SessionFile {
    fn load(path: Option<&PathBuf>) -> Result<Option<Self>, CliError> {
        path.map(|path| match fs::read_to_string(path) {
            Ok(source) => serde_json::from_str(&source).map_err(|error| {
                CliError::Input(format!("invalid session file {}: {error}", path.display()))
            }),
            // Login creates this opt-in file after receiving the cookies. Let a
            // first login name a not-yet-existing path rather than requiring a
            // caller to create an empty file first.
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(CliError::Input(format!(
                "cannot read session file {}: {error}",
                path.display()
            ))),
        })
        .transpose()
        .map(|session| session.flatten())
    }

    fn save(path: &Path, session: String, csrf: String) -> Result<(), CliError> {
        let contents =
            serde_json::to_vec(&Self { session, csrf }).map_err(|_| CliError::InvalidResponse)?;
        let mut temporary = temporary_file(path, "session file")?;
        std::io::Write::write_all(temporary.as_file_mut(), &contents).map_err(|error| {
            CliError::Input(format!(
                "cannot write session file {}: {error}",
                path.display()
            ))
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(temporary.path(), fs::Permissions::from_mode(0o600)).map_err(
                |error| {
                    CliError::Input(format!(
                        "cannot secure session file {}: {error}",
                        path.display()
                    ))
                },
            )?;
        }
        temporary.as_file_mut().sync_all().map_err(|error| {
            CliError::Input(format!(
                "cannot sync session file {}: {error}",
                path.display()
            ))
        })?;
        persist_temporary_file(temporary, path, "session file")
    }
}

async fn auth_command(
    client: &Client,
    server: &Url,
    command: AuthCommand,
    session_file: Option<&PathBuf>,
) -> Result<String, CliError> {
    match command {
        AuthCommand::Discover { login_identifier } => {
            request(
                client,
                server,
                Method::POST,
                "/auth/discover",
                Some(json!({ "login_identifier": login_identifier })),
            )
            .await
        }
        AuthCommand::Login {
            login_identifier,
            email,
            password_stdin,
        } => {
            if !password_stdin {
                return Err(CliError::Input(
                    "login requires --password-stdin".to_owned(),
                ));
            }
            let response = client
                .post(endpoint(server, "/auth/login")?)
                .timeout(REQUEST_TIMEOUT)
                .json(&json!({
                    "login_identifier": login_identifier,
                    "email": email,
                    "password": read_secret_stdin("password")?,
                }))
                .send()
                .await
                .map_err(|error| CliError::Transport(error.to_string()))?;
            save_session_response(response, session_file).await
        }
        AuthCommand::PasswordReset { email } => {
            request(
                client,
                server,
                Method::POST,
                "/auth/password-reset",
                Some(json!({ "email": email })),
            )
            .await
        }
        AuthCommand::PasswordResetConfirm {
            token_stdin,
            password_stdin,
        } => {
            if !token_stdin || !password_stdin {
                return Err(CliError::Input(
                    "password reset confirmation requires --token-stdin and --password-stdin"
                        .to_owned(),
                ));
            }
            let (token, password) = read_two_secrets_stdin("reset token", "password")?;
            request(
                client,
                server,
                Method::POST,
                "/auth/password-reset/confirm",
                Some(json!({
                    "token": token, "password": password,
                })),
            )
            .await
        }
        AuthCommand::Session => request(client, server, Method::GET, "/auth/session", None).await,
        AuthCommand::Preferences { time_zone, .. } => {
            request(
                client,
                server,
                Method::PATCH,
                "/auth/preferences",
                Some(json!({ "time_zone": time_zone })),
            )
            .await
        }
        AuthCommand::DisplayName { name } => {
            request(
                client,
                server,
                Method::PUT,
                "/auth/display-name",
                Some(json!({ "display_name": name })),
            )
            .await
        }
        AuthCommand::Logout => {
            let response = client
                .post(endpoint(server, "/auth/logout")?)
                .timeout(REQUEST_TIMEOUT)
                .send()
                .await
                .map_err(|error| CliError::Transport(error.to_string()))?;
            let output = raw_response(response).await?;
            if let Some(path) = session_file {
                match fs::remove_file(path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(CliError::Input(format!(
                            "cannot remove session file {}: {error}",
                            path.display()
                        )));
                    }
                }
            }
            Ok(output)
        }
        AuthCommand::Renew => {
            let response = client
                .post(endpoint(server, "/auth/renew")?)
                .timeout(REQUEST_TIMEOUT)
                .send()
                .await
                .map_err(|error| CliError::Transport(error.to_string()))?;
            save_session_response(response, session_file).await
        }
    }
}

async fn save_session_response(
    response: reqwest::Response,
    session_file: Option<&PathBuf>,
) -> Result<String, CliError> {
    let cookies = response
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|header| header.to_str().ok())
        .filter_map(|header| header.split_once(';').map(|(cookie, _)| cookie))
        .filter_map(|cookie| cookie.split_once('='))
        .fold((None, None), |(session, csrf), (name, value)| match name {
            "catalog_session" => (Some(value.to_owned()), csrf),
            "catalog_csrf" => (session, Some(value.to_owned())),
            _ => (session, csrf),
        });
    let output = raw_response(response).await?;
    if let Some(path) = session_file {
        let (Some(session), Some(csrf)) = cookies else {
            return Err(CliError::InvalidResponse);
        };
        SessionFile::save(path, session, csrf)?;
    }
    Ok(output)
}

async fn workspace_command(
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
            RoleCommand::Create { code, permissions } => request(client, server, Method::POST, "/workspace/roles", Some(json!({ "code": code, "permissions": permissions_input(&permissions)? }))).await,
            RoleCommand::Update { role_id, code, permissions } => request(client, server, Method::PUT, &format!("/workspace/roles/{}", segment(role_id)), Some(json!({ "code": code, "permissions": permissions_input(&permissions)? }))).await,
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

async fn token_command(
    client: &Client,
    server: &Url,
    command: TokenCommand,
) -> Result<String, CliError> {
    match command {
        TokenCommand::List => {
            request(client, server, Method::GET, "/personal-access-tokens", None).await
        }
        TokenCommand::Create {
            generator,
            label,
            permissions,
            expires_at,
        } => {
            let (label, permissions) = if generator {
                (
                    GENERATOR_TOKEN_LABEL.to_owned(),
                    json!(GENERATOR_TOKEN_PERMISSIONS),
                )
            } else {
                let label = label.ok_or_else(|| {
                    CliError::Input("--label is required unless --generator is used".to_owned())
                })?;
                let permissions = permissions.ok_or_else(|| {
                    CliError::Input(
                        "--permissions is required unless --generator is used".to_owned(),
                    )
                })?;
                (label, permissions_input(&permissions)?)
            };
            request(
                client,
                server,
                Method::POST,
                "/personal-access-tokens",
                Some(
                    json!({ "label": label, "permissions": permissions, "expires_at": expires_at }),
                ),
            )
            .await
        }
        TokenCommand::Revoke { token_id } => {
            request(
                client,
                server,
                Method::DELETE,
                &format!("/personal-access-tokens/{}", segment(token_id)),
                None,
            )
            .await
        }
    }
}

async fn audit_command(
    client: &Client,
    server: &Url,
    command: AuditCommand,
) -> Result<String, CliError> {
    match command {
        AuditCommand::List {
            limit,
            offset,
            occurred_after,
            occurred_before,
            actor_user_id,
            action_category,
            target_type,
            executor_type,
            agent_run_id,
            agent_tool_call_id,
        } => {
            let mut query = url::form_urlencoded::Serializer::new(String::new());
            for (key, value) in [
                ("limit", limit.map(|v| v.to_string())),
                ("offset", offset.map(|v| v.to_string())),
                ("occurred_after", occurred_after),
                ("occurred_before", occurred_before),
                ("actor_user_id", actor_user_id.map(|v| v.to_string())),
                ("action_category", action_category),
                ("target_type", target_type),
                ("executor_type", executor_type),
                ("agent_run_id", agent_run_id.map(|v| v.to_string())),
                (
                    "agent_tool_call_id",
                    agent_tool_call_id.map(|v| v.to_string()),
                ),
            ] {
                if let Some(value) = value {
                    query.append_pair(key, &value);
                }
            }
            let query = query.finish();
            request(
                client,
                server,
                Method::GET,
                &format!(
                    "/audit-events{}",
                    if query.is_empty() {
                        String::new()
                    } else {
                        format!("?{query}")
                    }
                ),
                None,
            )
            .await
        }
    }
}

async fn data_health_command(
    client: &Client,
    server: &Url,
    command: DataHealthCommand,
) -> Result<String, CliError> {
    let path = match command {
        DataHealthCommand::Summary { stale_after_days } => {
            data_health_path("summary", stale_after_days)
        }
        DataHealthCommand::Blueprints { stale_after_days } => {
            data_health_path("blueprints", stale_after_days)
        }
        DataHealthCommand::Freshness => "/data-health/freshness".to_owned(),
        DataHealthCommand::Completeness => "/data-health/completeness".to_owned(),
        DataHealthCommand::Contexts => "/data-health/contexts".to_owned(),
        DataHealthCommand::Relationships => "/data-health/relationships".to_owned(),
        DataHealthCommand::Storage => "/data-health/storage".to_owned(),
        DataHealthCommand::BackgroundProcessing => "/data-health/background-processing".to_owned(),
        DataHealthCommand::Refresh => {
            return request(client, server, Method::POST, "/data-health/refresh", None).await;
        }
    };
    request(client, server, Method::GET, &path, None).await
}
fn data_health_path(name: &str, days: Option<u16>) -> String {
    match days {
        Some(days) => format!("/data-health/{name}?stale_after_days={days}"),
        None => format!("/data-health/{name}"),
    }
}

async fn workflow_command(
    client: &Client,
    server: &Url,
    command: WorkflowCommand,
) -> Result<String, CliError> {
    match command {
        WorkflowCommand::List => request(client, server, Method::GET, "/workflows", None).await,
        WorkflowCommand::Validate(source) => {
            request(
                client,
                server,
                Method::POST,
                "/workflows/validate",
                Some(json!({"definition":read_source(source)?})),
            )
            .await
        }
        WorkflowCommand::Create(source) => {
            request(
                client,
                server,
                Method::POST,
                "/workflows",
                Some(json!({"definition":read_source(source)?})),
            )
            .await
        }
        WorkflowCommand::Get { workflow_id } => {
            request(
                client,
                server,
                Method::GET,
                &format!("/workflows/{}", segment(workflow_id)),
                None,
            )
            .await
        }
        WorkflowCommand::RevisionList { workflow_id } => {
            request(
                client,
                server,
                Method::GET,
                &format!("/workflows/{}/versions", segment(workflow_id)),
                None,
            )
            .await
        }
        WorkflowCommand::VersionGet {
            workflow_id,
            version,
        } => {
            request(
                client,
                server,
                Method::GET,
                &format!("/workflows/{}/versions/{version}", segment(workflow_id)),
                None,
            )
            .await
        }
        WorkflowCommand::Revision {
            workflow_id,
            source,
        } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/workflows/{}/versions", segment(workflow_id)),
                Some(json!({"definition":read_source(source)?})),
            )
            .await
        }
        WorkflowCommand::Publish {
            workflow_id,
            version,
        } => {
            request(
                client,
                server,
                Method::POST,
                &format!(
                    "/workflows/{}/versions/{version}/publish",
                    segment(workflow_id)
                ),
                None,
            )
            .await
        }
        WorkflowCommand::Enable {
            workflow_id,
            version,
        } => {
            request(
                client,
                server,
                Method::POST,
                &format!(
                    "/workflows/{}/versions/{version}/enable",
                    segment(workflow_id)
                ),
                None,
            )
            .await
        }
        WorkflowCommand::Disable { workflow_id } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/workflows/{}/disable", segment(workflow_id)),
                None,
            )
            .await
        }
        WorkflowCommand::RunNow {
            workflow_id,
            record_id,
            idempotency_key,
        } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/workflows/{}/run-now", segment(workflow_id)),
                Some(json!({"record_id":record_id,"idempotency_key":idempotency_key})),
            )
            .await
        }
        WorkflowCommand::RunList => {
            request(client, server, Method::GET, "/workflow-runs", None).await
        }
        WorkflowCommand::RunTargets { run_id } => {
            request(
                client,
                server,
                Method::GET,
                &format!("/workflow-runs/{}/targets", segment(run_id)),
                None,
            )
            .await
        }
        WorkflowCommand::RunReplay { run_id } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/workflow-runs/{}/replay", segment(run_id)),
                None,
            )
            .await
        }
    }
}

async fn rule_command(
    client: &Client,
    server: &Url,
    command: RuleCommand,
) -> Result<String, CliError> {
    match command {
        RuleCommand::List { blueprint_id } => {
            let path = match blueprint_id {
                Some(id) => format!("/rules?blueprint_id={id}"),
                None => "/rules".to_owned(),
            };
            request(client, server, Method::GET, &path, None).await
        }
        RuleCommand::Get { rule_id } => {
            request(
                client,
                server,
                Method::GET,
                &format!("/rules/{rule_id}"),
                None,
            )
            .await
        }
        RuleCommand::Validate {
            blueprint_id,
            blueprint_version,
            context_id,
            source,
        } => {
            request(
                client,
                server,
                Method::POST,
                "/rules/validate",
                Some(rule_body(
                    blueprint_id,
                    blueprint_version,
                    context_id,
                    source,
                )?),
            )
            .await
        }
        RuleCommand::Create {
            blueprint_id,
            blueprint_version,
            context_id,
            source,
        } => {
            request(
                client,
                server,
                Method::POST,
                "/rules",
                Some(rule_body(
                    blueprint_id,
                    blueprint_version,
                    context_id,
                    source,
                )?),
            )
            .await
        }
        RuleCommand::Revision {
            rule_id,
            blueprint_id,
            blueprint_version,
            context_id,
            source,
        } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/rules/{rule_id}/versions"),
                Some(rule_body(
                    blueprint_id,
                    blueprint_version,
                    context_id,
                    source,
                )?),
            )
            .await
        }
        RuleCommand::Publish { rule_id, version } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/rules/{rule_id}/versions/{version}/publish"),
                None,
            )
            .await
        }
        RuleCommand::Enable {
            rule_id,
            version,
            accept_existing_violations,
        } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/rules/{rule_id}/versions/{version}/enable"),
                Some(json!({ "accept_existing_violations": accept_existing_violations })),
            )
            .await
        }
        RuleCommand::Disable { rule_id } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/rules/{rule_id}/disable"),
                None,
            )
            .await
        }
        RuleCommand::RunNow {
            rule_id,
            idempotency_key,
            record_id,
            dry_run,
        } => request(
            client,
            server,
            Method::POST,
            &format!("/rules/{rule_id}/run-now"),
            Some(
                json!({"idempotency_key":idempotency_key,"record_id":record_id,"dry_run":dry_run}),
            ),
        )
        .await,
        RuleCommand::RunList => request(client, server, Method::GET, "/rule-runs", None).await,
        RuleCommand::RunReplay { run_id } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/rule-runs/{run_id}/replay"),
                None,
            )
            .await
        }
        RuleCommand::Findings { record_id } => {
            let path = match record_id {
                Some(id) => format!("/rule-findings?record_id={id}"),
                None => "/rule-findings".to_owned(),
            };
            request(client, server, Method::GET, &path, None).await
        }
        RuleCommand::Acknowledge { finding_id } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/rule-findings/{finding_id}/acknowledge"),
                None,
            )
            .await
        }
    }
}

fn rule_body(
    blueprint_id: Uuid,
    blueprint_version: i64,
    context_id: Option<Uuid>,
    source: SourceInput,
) -> Result<Value, CliError> {
    Ok(
        json!({"blueprint_id":blueprint_id,"blueprint_version":blueprint_version,"context_id":context_id,"definition":read_source(source)?}),
    )
}

async fn extension_registry_command(
    client: &Client,
    server: &Url,
    command: ExtensionRegistryCommand,
) -> Result<String, CliError> {
    match command {
        ExtensionRegistryCommand::List => {
            request(client, server, Method::GET, "/extension-registries", None).await
        }
        ExtensionRegistryCommand::Add { source } => {
            request(
                client,
                server,
                Method::POST,
                "/extension-registries",
                Some(json!({"source":source})),
            )
            .await
        }
        ExtensionRegistryCommand::Remove { registry_id } => {
            request(
                client,
                server,
                Method::DELETE,
                &format!("/extension-registries/{}", segment(registry_id)),
                None,
            )
            .await
        }
        ExtensionRegistryCommand::Discover => {
            request(
                client,
                server,
                Method::GET,
                "/extension-registries/discover",
                None,
            )
            .await
        }
        ExtensionRegistryCommand::Details { owner, repository } => {
            request(
                client,
                server,
                Method::GET,
                &format!(
                    "/extension-registries/extensions/{}/{}",
                    segment(&owner),
                    segment(&repository)
                ),
                None,
            )
            .await
        }
    }
}

async fn extension_command(
    client: &Client,
    server: &Url,
    command: ExtensionCommand,
) -> Result<String, CliError> {
    match command {
    ExtensionCommand::List => request(client,server,Method::GET,"/extensions",None).await,
    ExtensionCommand::Detail { extension_id } => request(client,server,Method::GET,&format!("/extensions/{}",segment(&extension_id)),None).await,
    ExtensionCommand::Remove { extension_id } => request(client,server,Method::DELETE,&format!("/extensions/{}",segment(&extension_id)),None).await,
    ExtensionCommand::Runtime => request(client,server,Method::GET,"/extensions/runtime",None).await,
    ExtensionCommand::WorkspaceMode { enabled } => request(client,server,Method::PUT,"/workspace/extensions-mode",Some(json!({"enabled":enabled}))).await,
    ExtensionCommand::Install { owner, repository, release_id } => request(client,server,Method::POST,"/extensions",Some(json!({"owner":owner,"repository":repository,"release_id":release_id}))).await,
    ExtensionCommand::Sideload { file } => raw_upload(client,server,"/extensions/sideload",&file,"application/zstd").await,
    ExtensionCommand::Upgrade { extension_id, owner, repository, release_id } => request(client,server,Method::POST,&format!("/extensions/{}/upgrade",segment(&extension_id)),Some(json!({"owner":owner,"repository":repository,"release_id":release_id}))).await,
    ExtensionCommand::Configure { extension_id, configuration } => request(client,server,Method::PUT,&format!("/extensions/{}/configure",segment(&extension_id)),Some(json!({"configuration":json_input(&configuration,"--configuration")?}))).await,
    ExtensionCommand::Grant { extension_id, grant_kind, grant_id } => request(client,server,Method::POST,&format!("/extensions/{}/grants",segment(&extension_id)),Some(json!({"grant_kind":grant_kind,"grant_id":grant_id}))).await,
    ExtensionCommand::Revoke { extension_id, grant_kind, grant_id } => request(client,server,Method::DELETE,&format!("/extensions/{}/grants/{}/{}",segment(&extension_id),segment(&grant_kind),segment(&grant_id)),None).await,
    ExtensionCommand::Enable { extension_id } => request(client,server,Method::POST,&format!("/extensions/{}/enable",segment(&extension_id)),None).await,
    ExtensionCommand::Disable { extension_id } => request(client,server,Method::POST,&format!("/extensions/{}/disable",segment(&extension_id)),None).await,
    ExtensionCommand::Quarantine { extension_id, diagnostic_code } => request(client,server,Method::POST,&format!("/extensions/{}/quarantine",segment(&extension_id)),Some(json!({"diagnostic_code":diagnostic_code}))).await,
    ExtensionCommand::Artifact { extension_id, contribution_id, output } => raw_download(client,server,&format!("/extensions/{}/{}/artifact",segment(&extension_id),segment(&contribution_id)),&output,None).await,
    ExtensionCommand::Storage { extension_id, contribution_id, release_id, body } => request(client,server,Method::POST,&format!("/extensions/{}/{}/storage/{}",segment(&extension_id),segment(&contribution_id),segment(release_id)),Some(json_input(&body,"--body")?)).await,
    ExtensionCommand::AnnotationNamespace { extension_id, adopt } => request(client,server,if adopt { Method::POST } else { Method::GET },&format!("/extensions/{}/annotation-namespace",segment(&extension_id)),None).await,
    ExtensionCommand::RepairAnnotations { extension_id, record_id, patch } => request(client,server,Method::POST,&format!("/extensions/{}/annotation-namespace/records/{record_id}",segment(&extension_id)),Some(json_object_argument(&patch)?)).await,
    ExtensionCommand::Command { extension_id, contribution_id, release_id, command_id, payload } => request(client,server,Method::POST,&format!("/extensions/{}/{}/command",segment(&extension_id),segment(&contribution_id)),Some(json!({"release_id":release_id,"command_id":command_id,"payload":json_input(&payload,"--payload")?}))).await,
}
}

async fn extension_run_command(
    client: &Client,
    server: &Url,
    command: ExtensionRunCommand,
) -> Result<String, CliError> {
    match command {
        ExtensionRunCommand::List { extension_id } => {
            let path = match extension_id {
                Some(id) => format!("/extension-runs?extension_id={}", segment(id)),
                None => "/extension-runs".to_owned(),
            };
            request(client, server, Method::GET, &path, None).await
        }
        ExtensionRunCommand::Show { run_id } => {
            request(
                client,
                server,
                Method::GET,
                &format!("/extension-runs/{run_id}"),
                None,
            )
            .await
        }
        ExtensionRunCommand::Cancel { run_id } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/extension-runs/{run_id}/cancel"),
                None,
            )
            .await
        }
        ExtensionRunCommand::Download {
            run_id,
            artifact_id,
            output,
        } => {
            raw_download(
                client,
                server,
                &format!("/extension-runs/{run_id}/artifacts/{artifact_id}/download"),
                &output,
                None,
            )
            .await
        }
    }
}

async fn extension_operation_command(
    client: &Client,
    server: &Url,
    command: ExtensionOperationCommand,
) -> Result<String, CliError> {
    match command {
        ExtensionOperationCommand::Start { extension_id, operation_id, input, idempotency_key, input_file_id } => {
            request(client, server, Method::POST,
                &format!("/extensions/{}/operations", segment(extension_id)),
                Some(json!({"operation_id":operation_id,"input":json_object_argument(&input)?,
                    "idempotency_key":idempotency_key,"source_reference":operation_source(input_file_id),
                    "destination_reference":{}}))).await
        }
        ExtensionOperationCommand::List => request(client, server, Method::GET, "/extension-operation-runs", None).await,
        ExtensionOperationCommand::Show { run_id } => request(client, server, Method::GET, &format!("/extension-operation-runs/{run_id}"), None).await,
        ExtensionOperationCommand::Artifacts { run_id } => request(client, server, Method::GET, &format!("/extension-operation-runs/{run_id}/artifacts"), None).await,
        ExtensionOperationCommand::Deliveries { run_id } => request(client, server, Method::GET, &format!("/extension-operation-runs/{run_id}/deliveries"), None).await,
        ExtensionOperationCommand::Download { run_id, artifact_id, output } => raw_download(client, server, &format!("/extension-operation-runs/{run_id}/artifacts/{artifact_id}/download"), &output, None).await,
        ExtensionOperationCommand::Cancel { run_id } => request(client, server, Method::POST, &format!("/extension-operation-runs/{run_id}/cancel"), None).await,
        ExtensionOperationCommand::Replay { run_id } => request(client, server, Method::POST, &format!("/extension-operation-runs/{run_id}/replay"), None).await,
    }
}

fn operation_source(file_id: Option<Uuid>) -> Value {
    match file_id {
        Some(id) => json!({"input_file_id":id}),
        None => json!({}),
    }
}

async fn extension_schedule_command(
    client: &Client,
    server: &Url,
    command: ExtensionScheduleCommand,
) -> Result<String, CliError> {
    match command {
        ExtensionScheduleCommand::List => {
            request(
                client,
                server,
                Method::GET,
                "/extension-operation-schedules",
                None,
            )
            .await
        }
        ExtensionScheduleCommand::Create {
            extension_id,
            operation_id,
            input,
            input_file_id,
            interval_seconds,
        } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/extensions/{}/operation-schedules", segment(extension_id)),
                Some(
                    json!({"operation_id":operation_id,"input":json_object_argument(&input)?,
                    "source_reference":operation_source(input_file_id),"destination_reference":{},
                    "interval_seconds":interval_seconds}),
                ),
            )
            .await
        }
        ExtensionScheduleCommand::Update {
            schedule_id,
            enabled,
            interval_seconds,
        } => {
            request(
                client,
                server,
                Method::PATCH,
                &format!("/extension-operation-schedules/{schedule_id}"),
                Some(json!({"enabled":enabled,"interval_seconds":interval_seconds})),
            )
            .await
        }
    }
}

async fn connector_job_command(
    client: &Client,
    server: &Url,
    command: ConnectorJobCommand,
) -> Result<String, CliError> {
    match command {
        ConnectorJobCommand::List { blueprint_id } => {
            request(
                client,
                server,
                Method::GET,
                &format!("/blueprints/{blueprint_id}/connector-jobs"),
                None,
            )
            .await
        }
        ConnectorJobCommand::Run {
            job_id,
            idempotency_key,
        } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/blueprint-connector-jobs/{job_id}/run"),
                Some(json!({"idempotency_key":idempotency_key})),
            )
            .await
        }
    }
}

async fn solution_pack_command(
    client: &Client,
    server: &Url,
    command: SolutionPackCommand,
) -> Result<String, CliError> {
    match command {
        SolutionPackCommand::Inspect { file } => {
            raw_upload(
                client,
                server,
                "/solution-packs/inspect",
                &file,
                "application/zstd",
            )
            .await
        }
        SolutionPackCommand::Plan {
            command: Some(SolutionPackPlanCommand::Show { plan_id }),
            file: None,
            prefix: None,
            blueprint_publication: None,
            blueprint_maps,
            asset_maps,
            context_maps,
            from_application: None,
            include_sample_data: false,
        } if blueprint_maps.is_empty() && asset_maps.is_empty() && context_maps.is_empty() => {
            request(
                client,
                server,
                Method::GET,
                &format!("/solution-packs/plans/{}", segment(plan_id)),
                None,
            )
            .await
        }
        SolutionPackCommand::Plan {
            command: None,
            file: Some(file),
            prefix: Some(prefix),
            blueprint_publication: Some(publication),
            blueprint_maps,
            asset_maps,
            context_maps,
            from_application,
            include_sample_data,
        } => {
            let mut path = format!(
                "/solution-packs/plans?prefix={}&blueprint_publication={}",
                segment(prefix),
                publication.as_str()
            );
            if let Some(application_id) = from_application {
                path.push_str("&from_application=");
                path.push_str(&segment(application_id));
            }
            if include_sample_data {
                path.push_str("&include_sample_data=true");
            }
            if blueprint_maps.is_empty() && asset_maps.is_empty() && context_maps.is_empty() {
                raw_upload(client, server, &path, &file, "application/zstd").await
            } else {
                solution_pack_plan_upload(
                    client,
                    server,
                    &path,
                    &file,
                    &blueprint_maps,
                    &asset_maps,
                    &context_maps,
                )
                .await
            }
        }
        SolutionPackCommand::Plan { .. } => Err(CliError::Input(
            "solution-pack plan requires --file, --prefix, and --blueprint-publication; plan show accepts only a plan ID".to_owned(),
        )),
        SolutionPackCommand::Apply { plan_id } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/solution-packs/plans/{}/apply", segment(plan_id)),
                None,
            )
            .await
        }
        SolutionPackCommand::Applications {
            command: SolutionPackApplicationsCommand::List { limit, offset },
        } => {
            request(
                client,
                server,
                Method::GET,
                &format!("/solution-packs/applications?limit={limit}&offset={offset}"),
                None,
            )
            .await
        }
        SolutionPackCommand::Applications {
            command: SolutionPackApplicationsCommand::Show { application_id },
        } => {
            request(
                client,
                server,
                Method::GET,
                &format!(
                    "/solution-packs/applications/{}",
                    segment(application_id)
                ),
                None,
            )
            .await
        }
        SolutionPackCommand::Applications {
            command: SolutionPackApplicationsCommand::Abandon { application_id },
        } => {
            request(
                client,
                server,
                Method::POST,
                &format!(
                    "/solution-packs/applications/{}/abandon",
                    segment(application_id)
                ),
                None,
            )
            .await
        }
        SolutionPackCommand::Checks { command: SolutionPackChecksCommand::Rerun { application_id } } => {
            request(client, server, Method::POST, &format!("/solution-packs/applications/{}/checks", segment(application_id)), None).await
        }
        SolutionPackCommand::Checks { command: SolutionPackChecksCommand::List { application_id, limit, offset } } => {
            request(client, server, Method::GET, &format!("/solution-packs/applications/{}/checks?limit={limit}&offset={offset}", segment(application_id)), None).await
        }
        SolutionPackCommand::Checks { command: SolutionPackChecksCommand::Show { application_id, run_id } } => {
            request(client, server, Method::GET, &format!("/solution-packs/applications/{}/checks/{}", segment(application_id), segment(run_id)), None).await
        }
    }
}

async fn record_publication_command(
    client: &Client,
    server: &Url,
    command: RecordPublicationCommand,
) -> Result<String, CliError> {
    match command {
        RecordPublicationCommand::List { record_id } => {
            request(
                client,
                server,
                Method::GET,
                &format!("/v1/records/{}/publications", segment(record_id)),
                None,
            )
            .await
        }
        RecordPublicationCommand::Publish {
            record_id,
            context_id,
        } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/v1/records/{}/publications", segment(record_id)),
                Some(json!({"context_id":context_id})),
            )
            .await
        }
        RecordPublicationCommand::Unpublish {
            record_id,
            context_id,
        } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/v1/records/{}/publications/unpublish", segment(record_id)),
                Some(json!({"context_id":context_id})),
            )
            .await
        }
        RecordPublicationCommand::PublishAll { record_id } => {
            request(
                client,
                server,
                Method::POST,
                &format!(
                    "/v1/records/{}/publications/publish-all",
                    segment(record_id)
                ),
                None,
            )
            .await
        }
    }
}

async fn presentation_asset_command(
    client: &Client,
    server: &Url,
    command: PresentationAssetCommand,
) -> Result<String, CliError> {
    match command {
        PresentationAssetCommand::List { limit, offset } => {
            request(
                client,
                server,
                Method::GET,
                &format!("/presentation-assets?limit={limit}&offset={offset}"),
                None,
            )
            .await
        }
        PresentationAssetCommand::Show { asset_id } => {
            request(
                client,
                server,
                Method::GET,
                &format!("/presentation-assets/{}", segment(asset_id)),
                None,
            )
            .await
        }
        PresentationAssetCommand::Download { asset_id, output } => {
            raw_download(
                client,
                server,
                &format!("/presentation-assets/{}/content", segment(asset_id)),
                &output,
                None,
            )
            .await
        }
    }
}

async fn file_command(
    client: &Client,
    server: &Url,
    command: FileCommand,
) -> Result<String, CliError> {
    match command {
        FileCommand::Upload {
            record_id,
            attribute_code,
            files,
            context_id,
        } => {
            multipart_upload(
                client,
                server,
                &format!(
                    "/records/{}/file-attributes/{}/uploads",
                    segment(record_id),
                    segment(&attribute_code)
                ),
                files,
                context_id,
            )
            .await
        }
        FileCommand::Metadata { file_id } => {
            request(
                client,
                server,
                Method::GET,
                &format!("/files/{}", segment(file_id)),
                None,
            )
            .await
        }
        FileCommand::DownloadOriginal {
            file_id,
            output,
            range,
        } => {
            raw_download(
                client,
                server,
                &format!("/files/{}/download", segment(file_id)),
                &output,
                range.as_deref(),
            )
            .await
        }
        FileCommand::DownloadVariant {
            file_id,
            kind,
            output,
            range,
        } => {
            raw_download(
                client,
                server,
                &format!(
                    "/files/{}/variants/{}/download",
                    segment(file_id),
                    segment(&kind)
                ),
                &output,
                range.as_deref(),
            )
            .await
        }
    }
}

fn json_input(input: &str, label: &str) -> Result<Value, CliError> {
    let source = match fs::read_to_string(input) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => input.to_owned(),
        Err(error) => {
            return Err(CliError::Input(format!(
                "cannot read {label} file: {error}"
            )));
        }
    };
    serde_json::from_str(&source).map_err(|error| {
        CliError::Input(format!(
            "{label} must be JSON or a readable JSON file: {error}"
        ))
    })
}
fn json_array_input(input: &str, label: &str) -> Result<Value, CliError> {
    let value = json_input(input, label)?;
    if value.is_array() {
        Ok(value)
    } else {
        Err(CliError::Input(format!("{label} must be a JSON array")))
    }
}
fn json_array_argument(input: Option<&str>, label: &str) -> Result<Value, CliError> {
    input
        .map(|value| json_array_input(value, label))
        .transpose()
        .map(|value| value.unwrap_or_else(|| json!([])))
}

async fn raw_upload(
    client: &Client,
    server: &Url,
    path: &str,
    file: &PathBuf,
    content_type: &str,
) -> Result<String, CliError> {
    let stream =
        tokio_util::io::ReaderStream::new(tokio::fs::File::open(file).await.map_err(|error| {
            CliError::Input(format!("cannot read {}: {error}", file.display()))
        })?);
    raw_response(
        client
            .request(Method::POST, endpoint(server, path)?)
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .body(reqwest::Body::wrap_stream(stream))
            .timeout(TRANSFER_TIMEOUT)
            .send()
            .await
            .map_err(|error| CliError::Transport(error.to_string()))?,
    )
    .await
}
async fn solution_pack_plan_upload(
    client: &Client,
    server: &Url,
    path: &str,
    archive: &Path,
    requested_maps: &[String],
    requested_asset_maps: &[String],
    requested_context_maps: &[String],
) -> Result<String, CliError> {
    let blueprint_maps = parse_pack_mappings(
        requested_maps,
        "--map",
        "blueprints/",
        "EXISTING_CODE",
        blueprint_mapping_value,
    )?;
    let asset_maps = parse_pack_mappings(
        requested_asset_maps,
        "--map-asset",
        "assets/",
        "ASSET_UUID",
        |id| Uuid::parse_str(id).ok().map(|id| json!({ "id": id })),
    )?;
    let context_maps = parse_pack_mappings(
        requested_context_maps,
        "--map-context",
        "contexts/",
        "EXISTING_CODE",
        |code| {
            let valid = !code.is_empty()
                && code.len() <= 128
                && code
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'));
            valid.then(|| json!({ "code": code }))
        },
    )?;

    let length = fs::metadata(archive)
        .map_err(|error| CliError::Input(format!("cannot read {}: {error}", archive.display())))?
        .len();
    let stream =
        tokio_util::io::ReaderStream::new(tokio::fs::File::open(archive).await.map_err(
            |error| CliError::Input(format!("cannot read {}: {error}", archive.display())),
        )?);
    let archive_part =
        reqwest::multipart::Part::stream_with_length(reqwest::Body::wrap_stream(stream), length)
            .file_name("solution-pack.tar.zst")
            .mime_str("application/zstd")
            .map_err(|error| CliError::Input(error.to_string()))?;
    let mut form = reqwest::multipart::Form::new().part("archive", archive_part);
    for (field, mappings) in [
        ("blueprint_map", blueprint_maps),
        ("asset_map", asset_maps),
        ("context_map", context_maps),
    ] {
        for mapping in mappings {
            form = form.text(field, mapping);
        }
    }
    raw_response(
        client
            .post(endpoint(server, path)?)
            .multipart(form)
            .timeout(TRANSFER_TIMEOUT)
            .send()
            .await
            .map_err(|error| CliError::Transport(error.to_string()))?,
    )
    .await
}

// Match the solution-pack planner's stable-code contract, including codes it
// generates from hyphenated logical keys. Existing mappings are not prefixes.
fn blueprint_mapping_value(code: &str) -> Option<Value> {
    let valid = !code.is_empty()
        && code.len() <= 128
        && code.as_bytes()[0].is_ascii_lowercase()
        && code.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
        && Uuid::parse_str(code).is_err();
    valid.then(|| json!({ "code": code }))
}

/// Parses `LOGICAL_KEY=VALUE` solution-pack mapping flags into the JSON parts
/// the plan endpoint accepts. Keys must start with `key_prefix` and appear
/// once; `parse_value` returns the part's value fields, or `None` when the
/// value is invalid.
fn parse_pack_mappings(
    requested: &[String],
    flag: &str,
    key_prefix: &str,
    value_name: &str,
    parse_value: impl Fn(&str) -> Option<Value>,
) -> Result<Vec<String>, CliError> {
    let mut seen = std::collections::BTreeSet::new();
    requested
        .iter()
        .map(|requested| {
            let (key, value) = requested.split_once('=').ok_or_else(|| {
                CliError::Input(format!("{flag} must be LOGICAL_KEY={value_name}"))
            })?;
            let invalid = || CliError::Input(format!("invalid {flag} value '{requested}'"));
            if !key.starts_with(key_prefix) || key.len() > 128 {
                return Err(invalid());
            }
            let mut part = parse_value(value).ok_or_else(invalid)?;
            if !seen.insert(key) {
                return Err(CliError::Input(format!("duplicate {flag} key '{key}'")));
            }
            part["key"] = json!(key);
            Ok(part.to_string())
        })
        .collect()
}

async fn multipart_upload(
    client: &Client,
    server: &Url,
    path: &str,
    files: Vec<PathBuf>,
    context_id: Option<Uuid>,
) -> Result<String, CliError> {
    let mut form = reqwest::multipart::Form::new();
    if let Some(context_id) = context_id {
        form = form.text("context_id", context_id.to_string());
    }
    for path in files {
        let length = fs::metadata(&path)
            .map_err(|error| CliError::Input(format!("cannot read {}: {error}", path.display())))?
            .len();
        let filename = path
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or_else(|| CliError::Input("file name must be UTF-8".to_owned()))?
            .to_owned();
        let stream =
            tokio_util::io::ReaderStream::new(tokio::fs::File::open(&path).await.map_err(
                |error| CliError::Input(format!("cannot read {}: {error}", path.display())),
            )?);
        let part = reqwest::multipart::Part::stream_with_length(
            reqwest::Body::wrap_stream(stream),
            length,
        )
        .file_name(filename)
        .mime_str(
            mime_guess::from_path(&path)
                .first_or_octet_stream()
                .as_ref(),
        )
        .map_err(|error| CliError::Input(format!("invalid file MIME type: {error}")))?;
        form = form.part("files", part);
    }
    raw_response(
        client
            .post(endpoint(server, path)?)
            .multipart(form)
            .timeout(TRANSFER_TIMEOUT)
            .send()
            .await
            .map_err(|error| CliError::Transport(error.to_string()))?,
    )
    .await
}

fn temporary_file(path: &Path, label: &str) -> Result<tempfile::NamedTempFile, CliError> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    tempfile::NamedTempFile::new_in(parent).map_err(|error| {
        CliError::Input(format!(
            "cannot create temporary {label} for {}: {error}",
            path.display()
        ))
    })
}

fn persist_temporary_file(
    temporary: tempfile::NamedTempFile,
    path: &Path,
    label: &str,
) -> Result<(), CliError> {
    temporary.persist(path).map_err(|error| {
        CliError::Input(format!(
            "cannot replace {label} {}: {}",
            path.display(),
            error.error
        ))
    })?;
    Ok(())
}

async fn raw_download(
    client: &Client,
    server: &Url,
    path: &str,
    output: &Path,
    range: Option<&str>,
) -> Result<String, CliError> {
    let mut request = client.get(endpoint(server, path)?);
    if let Some(range) = range {
        request = request.header(reqwest::header::RANGE, range);
    }
    let response = request
        .timeout(TRANSFER_TIMEOUT)
        .send()
        .await
        .map_err(|error| CliError::Transport(error.to_string()))?;
    if !response.status().is_success() {
        return raw_response(response).await;
    }
    let mut temporary = temporary_file(output, "download")?;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| CliError::Transport(error.to_string()))?;
        std::io::Write::write_all(temporary.as_file_mut(), &chunk).map_err(|error| {
            CliError::Input(format!("cannot write {}: {error}", output.display()))
        })?;
    }
    temporary
        .as_file_mut()
        .sync_all()
        .map_err(|error| CliError::Input(format!("cannot sync {}: {error}", output.display())))?;
    persist_temporary_file(temporary, output, "download")?;
    Ok("null".to_owned())
}
async fn buffered_response_body(
    response: reqwest::Response,
) -> Result<(reqwest::StatusCode, String), CliError> {
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(CliError::ResponseTooLarge);
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| CliError::Transport(error.to_string()))?;
        if bytes.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(CliError::ResponseTooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok((
        status,
        String::from_utf8(bytes).map_err(|_| CliError::InvalidResponse)?,
    ))
}

async fn raw_response(response: reqwest::Response) -> Result<String, CliError> {
    let (status, body) = buffered_response_body(response).await?;
    if status.is_success() {
        if status == reqwest::StatusCode::NO_CONTENT {
            return Ok("null".to_owned());
        }
        serde_json::from_str::<Value>(&body).map_err(|_| CliError::InvalidResponse)?;
        Ok(body)
    } else {
        api_error(status, &body)
    }
}
fn api_error(status: reqwest::StatusCode, body: &str) -> Result<String, CliError> {
    let error = serde_json::from_str::<Value>(body).ok();
    Err(CliError::Api {
        status: status.as_u16(),
        code: error
            .as_ref()
            .and_then(|body| body["error"]["code"].as_str())
            .unwrap_or("api_error")
            .to_owned(),
        details: error
            .as_ref()
            .map(|body| body["error"]["details"].clone())
            .filter(|details| !details.is_null()),
        message: error
            .as_ref()
            .and_then(|body| body["error"]["message"].as_str())
            .unwrap_or(body)
            .to_owned(),
    })
}

fn permissions_input(input: &str) -> Result<Value, CliError> {
    let source = match fs::read_to_string(input) {
        Ok(source) => source,
        Err(error) if error.kind() == io::ErrorKind::NotFound => input.to_owned(),
        Err(error) => {
            return Err(CliError::Input(format!(
                "cannot read --permissions file: {error}"
            )));
        }
    };
    let permissions = serde_json::from_str::<Vec<String>>(&source).map_err(|error| {
        CliError::Input(format!(
            "--permissions must be a JSON array of strings or a readable JSON file: {error}"
        ))
    })?;
    Ok(json!(permissions))
}

fn read_three_secrets_stdin() -> Result<(String, String, String), CliError> {
    let mut values = String::new();
    io::stdin()
        .read_to_string(&mut values)
        .map_err(|error| CliError::Input(error.to_string()))?;
    let mut lines = values.lines();
    let onboarding = lines
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            CliError::Input("onboarding secret must be the first stdin line".to_owned())
        })?;
    let invitation = lines
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            CliError::Input("invitation secret must be the second stdin line".to_owned())
        })?;
    let password = lines
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CliError::Input("password must be the third stdin line".to_owned()))?;
    if lines.next().is_some() {
        return Err(CliError::Input(
            "stdin must contain exactly three lines".to_owned(),
        ));
    }
    Ok((
        onboarding.to_owned(),
        invitation.to_owned(),
        password.to_owned(),
    ))
}

fn read_two_secrets_stdin(
    first_label: &str,
    second_label: &str,
) -> Result<(String, String), CliError> {
    let mut values = String::new();
    io::stdin()
        .read_to_string(&mut values)
        .map_err(|error| CliError::Input(error.to_string()))?;
    let mut lines = values.lines();
    let first = lines
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CliError::Input(format!("{first_label} must be the first stdin line")))?;
    let second = lines
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CliError::Input(format!("{second_label} must be the second stdin line")))?;
    if lines.next().is_some() {
        return Err(CliError::Input(
            "stdin must contain exactly two lines".to_owned(),
        ));
    }
    Ok((first.to_owned(), second.to_owned()))
}

fn read_secret_stdin(label: &str) -> Result<String, CliError> {
    let mut secret = String::new();
    io::stdin()
        .read_to_string(&mut secret)
        .map_err(|error| CliError::Input(error.to_string()))?;
    let secret = secret.trim_end_matches(['\r', '\n']);
    if secret.is_empty() {
        return Err(CliError::Input(format!("{label} must not be empty")));
    }
    Ok(secret.to_owned())
}

async fn migrate_record(
    client: &Client,
    server: &Url,
    record_id: Uuid,
    dry_run: bool,
    values: Option<&str>,
    relationships: Option<&str>,
    discard_attributes: Option<&str>,
) -> Result<Value, CliError> {
    let preview = request_value(
        client,
        server,
        Method::POST,
        &format!(
            "/v1/records/{}/blueprint-migration/preview",
            segment(record_id)
        ),
        None,
    )
    .await?;
    let status = preview["status"]
        .as_str()
        .ok_or(CliError::InvalidResponse)?;
    // Bulk migration supplies no remediation input, so a needs-input preview is
    // a classification result, not a migration request. Manual migration may
    // submit the caller's explicit remediation arrays.
    if status == "blocked"
        || dry_run
        || (status == "needs_input"
            && values.is_none()
            && relationships.is_none()
            && discard_attributes.is_none())
    {
        return Ok(json!({
            "record_id": record_id,
            "status": status,
            "issues": preview["issues"],
        }));
    }
    let migration_id = preview["migration_id"]
        .as_str()
        .ok_or(CliError::InvalidResponse)?;
    let target_version = preview["target"]["blueprint"]["version"]
        .as_i64()
        .ok_or(CliError::InvalidResponse)?;
    request_value(
        client,
        server,
        Method::POST,
        &format!("/v1/records/{}/blueprint-migration", segment(record_id)),
        Some(json!({
            "migration_id": migration_id,
            "expected_target_version": target_version,
            "values": json_array_argument(values, "--values")?,
            "relationships": json_array_argument(relationships, "--relationships")?,
            "discard_attributes": json_array_argument(discard_attributes, "--discard-attributes")?,
        })),
    )
    .await
}

async fn migrate_records(
    client: &Client,
    server: &Url,
    blueprint: &str,
    from_version: i64,
    size: u32,
    dry_run: bool,
) -> Result<String, CliError> {
    if from_version <= 0 {
        return Err(CliError::Input(
            "--from-version must be positive".to_owned(),
        ));
    }
    if size == 0 {
        return Err(CliError::Input("--size must be positive".to_owned()));
    }
    let mut cursor: Option<String> = None;
    let mut seen_cursors = std::collections::HashSet::new();
    let mut migrated = 0;
    let mut ready = 0;
    let mut needs_input = Vec::new();
    let mut blocked = Vec::new();
    let mut failed = Vec::new();
    loop {
        if let Some(current_cursor) = cursor.as_ref()
            && !seen_cursors.insert(current_cursor.clone())
        {
            return Err(CliError::InvalidResponse);
        }
        let page = request_value(
            client,
            server,
            Method::POST,
            "/v1/records/search",
            Some(json!({
                "blueprint": { "code": blueprint, "version": from_version },
                "query": "",
                "filters": [],
                "page": { "size": size, "cursor": cursor },
            })),
        )
        .await?;
        let items = page["items"].as_array().ok_or(CliError::InvalidResponse)?;
        for item in items {
            let record_id = item["id"]
                .as_str()
                .ok_or(CliError::InvalidResponse)?
                .parse::<Uuid>()
                .map_err(|_| CliError::InvalidResponse)?;
            match migrate_record(client, server, record_id, dry_run, None, None, None).await {
                Ok(result) => match result["status"].as_str() {
                    Some("ready") => ready += 1,
                    Some("needs_input") => needs_input.push(result),
                    Some("blocked") => blocked.push(result),
                    _ => migrated += 1,
                },
                Err(error) => failed.push(json!({
                    "record_id": record_id,
                    "error": error.json()["error"],
                })),
            }
        }
        cursor = match page.get("next_cursor") {
            Some(Value::Null) => None,
            Some(Value::String(cursor)) if !cursor.is_empty() => Some(cursor.clone()),
            _ => return Err(CliError::InvalidResponse),
        };
        if cursor.is_none() {
            break;
        }
    }
    serde_json::to_string(&json!({
        "blueprint": blueprint,
        "from_version": from_version,
        "dry_run": dry_run,
        "migrated": migrated,
        "ready": ready,
        "needs_input": needs_input,
        "blocked": blocked,
        "failed": failed,
    }))
    .map_err(|_| CliError::InvalidResponse)
}

fn read_source(input: SourceInput) -> Result<String, CliError> {
    match (input.file, input.stdin) {
        (Some(file), false) => {
            fs::read_to_string(file).map_err(|error| CliError::Input(error.to_string()))
        }
        (None, true) => {
            let mut source = String::new();
            io::stdin()
                .read_to_string(&mut source)
                .map_err(|error| CliError::Input(error.to_string()))?;
            Ok(source)
        }
        _ => Err(CliError::Input(
            "provide exactly one of --file or --stdin".to_owned(),
        )),
    }
}

fn context_body_from_file(path: &PathBuf) -> Result<Value, CliError> {
    let input: ContextFile = parse_toml_file(path)?;
    Ok(
        json!({ "code": input.code, "data": toml_to_json(input.data)?, "parent_id": input.parent_id }),
    )
}

fn values_body_from_file(path: &PathBuf, context_id: Option<Uuid>) -> Result<Value, CliError> {
    let input: ValueFile = parse_toml_file(path)?;
    values_body(input, context_id)
}

fn values_body(input: ValueFile, default_context_id: Option<Uuid>) -> Result<Value, CliError> {
    let values = input
        .values
        .into_iter()
        .map(|value| {
            let (attribute_key, attribute_value) = match (value.attribute_id, value.attribute_code)
            {
                (Some(attribute_id), None) => ("attribute_id", json!(attribute_id)),
                (None, Some(attribute_code)) => ("attribute_code", json!(attribute_code)),
                _ => {
                    return Err(CliError::Input(
                        "provide exactly one of attribute_id or attribute_code".to_owned(),
                    ));
                }
            };

            match value.kind.as_str() {
                "scalar" => {
                    if value.target_record_id.is_some() {
                        return Err(CliError::Input(
                            "scalar values must not include target_record_id".to_owned(),
                        ));
                    }
                    let payload = value
                        .value
                        .ok_or_else(|| CliError::Input("scalar values require value".to_owned()))?;
                    let mut output = json!({
                        "kind": "scalar",
                        "context_id": value.context_id.or(default_context_id),
                        "value": toml_to_json(payload)?,
                    });
                    output[attribute_key] = attribute_value;
                    Ok(output)
                }
                "relationship" => {
                    if value.value.is_some() {
                        return Err(CliError::Input(
                            "relationship values must not include value".to_owned(),
                        ));
                    }
                    let target_record_id = value.target_record_id.ok_or_else(|| {
                        CliError::Input("relationship values require target_record_id".to_owned())
                    })?;
                    let mut output = json!({
                        "kind": "relationship",
                        "context_id": value.context_id.or(default_context_id),
                        "target_record_id": target_record_id,
                    });
                    output[attribute_key] = attribute_value;
                    Ok(output)
                }
                _ => Err(CliError::Input(
                    "value kind must be scalar or relationship".to_owned(),
                )),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({ "values": values }))
}

fn relationships_body_from_file(
    path: &PathBuf,
    context_id: Option<Uuid>,
) -> Result<Value, CliError> {
    let input: RelationshipFile = parse_toml_file(path)?;
    relationships_body(input, context_id)
}

fn relationships_body(
    input: RelationshipFile,
    default_context_id: Option<Uuid>,
) -> Result<Value, CliError> {
    let relationships = input
        .relationships
        .into_iter()
        .map(|relationship| {
            let (attribute_key, attribute_value) =
                match (relationship.attribute_id, relationship.attribute_code) {
                    (Some(attribute_id), None) => ("attribute_id", json!(attribute_id)),
                    (None, Some(attribute_code)) => ("attribute_code", json!(attribute_code)),
                    _ => {
                        return Err(CliError::Input(
                            "provide exactly one of attribute_id or attribute_code".to_owned(),
                        ));
                    }
                };
            let mut output = json!({
                "context_id": relationship.context_id.or(default_context_id),
                "target_record_ids": relationship.target_record_ids,
            });
            output[attribute_key] = attribute_value;
            Ok(output)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({ "relationships": relationships }))
}

fn json_tags_argument(input: &str) -> Result<Value, CliError> {
    let value: Value = serde_json::from_str(input)
        .map_err(|error| CliError::Input(format!("invalid --system-tags JSON: {error}")))?;
    if !value.is_array()
        || !value
            .as_array()
            .is_some_and(|tags| tags.iter().all(Value::is_string))
    {
        return Err(CliError::Input(
            "--system-tags must be a JSON array of strings".to_owned(),
        ));
    }
    Ok(value)
}

fn saved_view_payload(
    name: &str,
    description: &str,
    visibility: &str,
    state: &str,
) -> Result<Value, CliError> {
    Ok(json!({
        "kind": "explorer_search",
        "name": name,
        "description": description,
        "visibility": visibility,
        "state": json_object_argument(state)?,
    }))
}

fn notification_request(command: NotificationCommand) -> (Method, String, Option<Value>) {
    match command {
        NotificationCommand::List {
            unread,
            before_time,
            before_id,
        } => {
            let mut params = url::form_urlencoded::Serializer::new(String::new());
            if unread {
                params.append_pair("unread_only", "true");
            }
            if let (Some(time), Some(id)) = (before_time, before_id) {
                params.append_pair("before_time", &time);
                params.append_pair("before_id", &id.to_string());
            }
            let query = params.finish();
            let path = if query.is_empty() {
                "/notifications".to_owned()
            } else {
                format!("/notifications?{query}")
            };
            (Method::GET, path, None)
        }
        NotificationCommand::Count => (Method::GET, "/notifications/unread-count".to_owned(), None),
        NotificationCommand::Get { id } => (Method::GET, format!("/notifications/{id}"), None),
        NotificationCommand::Read { id } => (
            Method::PATCH,
            format!("/notifications/{id}"),
            Some(json!({"read": true})),
        ),
        NotificationCommand::Unread { id } => (
            Method::PATCH,
            format!("/notifications/{id}"),
            Some(json!({"read": false})),
        ),
        NotificationCommand::ReadAll => (
            Method::POST,
            "/notifications/read-all".to_owned(),
            Some(json!({})),
        ),
        NotificationCommand::Delete { id } => {
            (Method::DELETE, format!("/notifications/{id}"), None)
        }
    }
}

fn team_request(command: TeamCommand) -> (Method, String, Option<Value>) {
    match command {
        TeamCommand::List => (Method::GET, "/workspace/teams".to_owned(), None),
        TeamCommand::Create {
            code,
            name,
            members,
        } => (
            Method::POST,
            "/workspace/teams".to_owned(),
            Some(json!({"code": code, "name": name, "member_user_ids": members})),
        ),
        TeamCommand::Update {
            id,
            name,
            members,
            clear_members,
        } => {
            let mut body = serde_json::Map::new();
            if let Some(name) = name {
                body.insert("name".into(), json!(name));
            }
            if clear_members || !members.is_empty() {
                body.insert("member_user_ids".into(), json!(members));
            }
            (
                Method::PATCH,
                format!("/workspace/teams/{id}"),
                Some(Value::Object(body)),
            )
        }
        TeamCommand::Delete { id } => (Method::DELETE, format!("/workspace/teams/{id}"), None),
    }
}

fn lexicon_request(command: LexiconCommand) -> Result<(Method, String, Option<Value>), CliError> {
    let identity_query = |identity: &LexiconIdentity| {
        let mut query = url::form_urlencoded::Serializer::new(String::new());
        query.append_pair("key", &identity.key);
        if let Some(context) = &identity.context {
            query.append_pair("context", context);
        }
        query.append_pair("language", &identity.language);
        query.append_pair("plural_category", &identity.plural_category);
        query.finish()
    };
    let language_query = |name: &str, language: &str| {
        let mut query = url::form_urlencoded::Serializer::new(String::new());
        query.append_pair(name, language);
        query.finish()
    };
    Ok(match command {
        LexiconCommand::List { language } => (
            Method::GET,
            match language {
                Some(language) => {
                    format!("/lexicon/entries?{}", language_query("language", &language))
                }
                None => "/lexicon/entries".to_owned(),
            },
            None,
        ),
        LexiconCommand::Set { identity, text } => (
            Method::PUT,
            "/lexicon/entries".to_owned(),
            Some(json!({
                "key": identity.key,
                "context": identity.context,
                "language": identity.language,
                "plural_category": identity.plural_category,
                "text": text,
            })),
        ),
        LexiconCommand::Delete { identity } => (
            Method::DELETE,
            format!("/lexicon/entries?{}", identity_query(&identity)),
            None,
        ),
        LexiconCommand::Export { language } => (
            Method::GET,
            format!("/lexicon/export?{}", language_query("language", &language)),
            None,
        ),
        LexiconCommand::Import { file, replace } => (
            Method::POST,
            format!(
                "/lexicon/import?mode={}",
                if replace { "replace" } else { "merge" }
            ),
            Some(lexicon_file(&file)?),
        ),
        LexiconCommand::Report { language } => (
            Method::GET,
            if language.is_empty() {
                "/lexicon/report".to_owned()
            } else {
                format!(
                    "/lexicon/report?{}",
                    language_query("languages", &language.join(","))
                )
            },
            None,
        ),
    })
}

/// Reads a lexicon file as JSON, or as TOML when the extension is `.toml`.
fn lexicon_file(path: &PathBuf) -> Result<Value, CliError> {
    let source = fs::read_to_string(path).map_err(|error| CliError::Input(error.to_string()))?;
    if path
        .extension()
        .is_some_and(|extension| extension == "toml")
    {
        let value: toml::Value = toml::from_str(&source)
            .map_err(|error| CliError::Input(format!("invalid lexicon TOML: {error}")))?;
        serde_json::to_value(value).map_err(|error| CliError::Input(error.to_string()))
    } else {
        serde_json::from_str(&source)
            .map_err(|error| CliError::Input(format!("invalid lexicon JSON: {error}")))
    }
}

fn json_object_argument(input: &str) -> Result<Value, CliError> {
    let value: Value = serde_json::from_str(input)
        .map_err(|error| CliError::Input(format!("invalid --system-metadata JSON: {error}")))?;
    if !value.is_object() {
        return Err(CliError::Input(
            "--system-metadata must be a JSON object".to_owned(),
        ));
    }
    Ok(value)
}

fn form_update_body(
    values_path: Option<&PathBuf>,
    relationships_path: Option<&PathBuf>,
    remove_values_path: Option<&PathBuf>,
    context_id: Option<Uuid>,
) -> Result<Value, CliError> {
    let values = match values_path {
        Some(path) => values_body_from_file(path, context_id)?["values"].clone(),
        None => Value::Array(Vec::new()),
    };
    if values
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item["kind"] == "relationship"))
    {
        return Err(CliError::Input(
            "--values accepts scalar values only; use --relationships for relationship sets"
                .to_owned(),
        ));
    }
    let relationships = match relationships_path {
        Some(path) => relationships_body_from_file(path, context_id)?["relationships"].clone(),
        None => Value::Array(Vec::new()),
    };
    let remove_values = match remove_values_path {
        Some(path) => remove_values_body_from_file(path, context_id)?,
        None => Value::Array(Vec::new()),
    };
    Ok(json!({ "values": values, "relationships": relationships, "remove_values": remove_values }))
}

fn remove_values_body_from_file(
    path: &PathBuf,
    default_context_id: Option<Uuid>,
) -> Result<Value, CliError> {
    let input: RemoveValuesFile = parse_toml_file(path)?;
    Ok(Value::Array(
        input
            .remove_values
            .into_iter()
            .map(|value| {
                json!({
                    "attribute_code": value.attribute_code,
                    "context_id": value.context_id.or(default_context_id),
                })
            })
            .collect(),
    ))
}

fn parse_toml_file<T: for<'de> Deserialize<'de>>(path: &PathBuf) -> Result<T, CliError> {
    let source = fs::read_to_string(path).map_err(|error| CliError::Input(error.to_string()))?;
    toml::from_str(&source).map_err(|error| CliError::Input(format!("invalid TOML: {error}")))
}

fn toml_to_json(value: toml::Value) -> Result<Value, CliError> {
    Ok(match value {
        toml::Value::String(value) => Value::String(value),
        toml::Value::Integer(value) => json!(value),
        toml::Value::Float(value) => json!(value),
        toml::Value::Boolean(value) => Value::Bool(value),
        // TOML's serde representation makes temporal literals objects. The API
        // accepts its canonical ISO 8601 text representation instead.
        toml::Value::Datetime(value) => Value::String(value.to_string()),
        toml::Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(toml_to_json)
                .collect::<Result<_, _>>()?,
        ),
        toml::Value::Table(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| Ok((key, toml_to_json(value)?)))
                .collect::<Result<_, CliError>>()?,
        ),
    })
}

async fn request(
    client: &Client,
    server: &Url,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> Result<String, CliError> {
    let url = endpoint(server, path)?;
    let mut request = client.request(method, url).timeout(REQUEST_TIMEOUT);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request
        .send()
        .await
        .map_err(|error| CliError::Transport(error.to_string()))?;
    raw_response(response).await
}

async fn request_value(
    client: &Client,
    server: &Url,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> Result<Value, CliError> {
    let response = request(client, server, method, path, body).await?;
    serde_json::from_str(&response).map_err(|_| CliError::InvalidResponse)
}

fn is_secure_credential_transport(server: &Url) -> bool {
    if server.scheme() == "https" {
        return true;
    }
    match server.host() {
        Some(url::Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    }
}

fn endpoint(server: &Url, path: &str) -> Result<Url, CliError> {
    let mut base = server.clone();
    let mut base_path = base.path().to_owned();
    if !base_path.ends_with('/') {
        base_path.push('/');
        base.set_path(&base_path);
    }
    base.join(path.trim_start_matches('/'))
        .map_err(|error| CliError::Input(format!("invalid API path: {error}")))
}

fn segment(value: impl std::fmt::Display) -> String {
    url::form_urlencoded::byte_serialize(value.to_string().as_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blueprint_mapping_accepts_planner_generated_stable_codes() {
        for code in [
            "seed_environmental-declaration",
            "material_specification",
            "a-",
            "a_",
        ] {
            let requested = vec![format!("blueprints/environmental-declaration={code}")];
            let parts = parse_pack_mappings(
                &requested,
                "--map",
                "blueprints/",
                "EXISTING_CODE",
                blueprint_mapping_value,
            )
            .unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(&parts[0]).unwrap(),
                json!({"key":"blueprints/environmental-declaration","code":code})
            );
        }
        assert!(blueprint_mapping_value(&"a".repeat(128)).is_some());
    }

    #[test]
    fn blueprint_mapping_rejects_non_stable_codes_and_duplicate_keys() {
        for code in [
            "",
            "Uppercase",
            "1code",
            "-code",
            "has space",
            "a/b",
            "é",
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
        ] {
            assert!(blueprint_mapping_value(code).is_none(), "{code}");
        }
        assert!(blueprint_mapping_value(&"a".repeat(129)).is_none());
        let requested = vec!["blueprints/item=one".into(), "blueprints/item=two".into()];
        assert!(
            parse_pack_mappings(
                &requested,
                "--map",
                "blueprints/",
                "EXISTING_CODE",
                blueprint_mapping_value
            )
            .is_err()
        );
        assert!(
            parse_pack_mappings(
                &["contexts/item=one".into()],
                "--map",
                "blueprints/",
                "EXISTING_CODE",
                blueprint_mapping_value
            )
            .is_err()
        );
    }

    #[test]
    fn auth_preferences_require_exactly_one_time_zone_choice() {
        assert!(
            Cli::try_parse_from([
                "acli",
                "auth",
                "preferences",
                "--time-zone",
                "Europe/Warsaw"
            ])
            .is_ok()
        );
        assert!(Cli::try_parse_from(["acli", "auth", "preferences", "--clear-time-zone"]).is_ok());
        assert!(Cli::try_parse_from(["acli", "auth", "preferences"]).is_err());
        assert!(
            Cli::try_parse_from([
                "acli",
                "auth",
                "preferences",
                "--time-zone",
                "UTC",
                "--clear-time-zone"
            ])
            .is_err()
        );
    }

    #[test]
    fn notification_commands_build_requests() {
        let parse = |args: &[&str]| match Cli::try_parse_from(args).unwrap().command {
            Command::Notification { command } => notification_request(command),
            _ => unreachable!(),
        };
        let id = "6a1f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f60";
        assert_eq!(
            parse(&["acli", "notification", "list"]),
            (Method::GET, "/notifications".to_owned(), None)
        );
        assert_eq!(
            parse(&[
                "acli",
                "notification",
                "list",
                "--unread",
                "--before-time",
                "2026-10-08T10:00:00Z",
                "--before-id",
                id
            ])
            .1,
            format!(
                "/notifications?unread_only=true&before_time=2026-10-08T10%3A00%3A00Z&before_id={id}"
            )
        );
        assert!(Cli::try_parse_from(["acli", "notification", "list", "--before-id", id]).is_err());
        assert_eq!(
            parse(&["acli", "notification", "read", id]),
            (
                Method::PATCH,
                format!("/notifications/{id}"),
                Some(json!({"read": true}))
            )
        );
        assert_eq!(
            parse(&["acli", "notification", "unread", id]).2,
            Some(json!({"read": false}))
        );
        assert_eq!(
            parse(&["acli", "notification", "read-all"]).1,
            "/notifications/read-all"
        );
        assert_eq!(
            parse(&["acli", "notification", "delete", id]),
            (Method::DELETE, format!("/notifications/{id}"), None)
        );
        assert_eq!(
            parse(&["acli", "notification", "count"]).1,
            "/notifications/unread-count"
        );
    }

    #[test]
    fn team_commands_build_requests() {
        let parse = |args: &[&str]| match Cli::try_parse_from(args).unwrap().command {
            Command::Team { command } => team_request(command),
            _ => unreachable!(),
        };
        let member = "6a1f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f60";
        let team = "8c3f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f62";
        assert_eq!(
            parse(&[
                "acli", "team", "create", "--code", "qa", "--name", "QA", "--member", member
            ]),
            (
                Method::POST,
                "/workspace/teams".to_owned(),
                Some(json!({"code": "qa", "name": "QA", "member_user_ids": [member]}))
            )
        );
        assert_eq!(
            parse(&["acli", "team", "update", team, "--name", "Quality"]),
            (
                Method::PATCH,
                format!("/workspace/teams/{team}"),
                Some(json!({"name": "Quality"}))
            )
        );
        assert_eq!(
            parse(&["acli", "team", "update", team, "--clear-members"]).2,
            Some(json!({"member_user_ids": []}))
        );
        assert!(
            Cli::try_parse_from([
                "acli",
                "team",
                "update",
                team,
                "--clear-members",
                "--member",
                member
            ])
            .is_err()
        );
    }

    #[test]
    fn lexicon_commands_build_requests_and_read_toml_files() {
        let parse = |args: &[&str]| match Cli::try_parse_from(args).unwrap().command {
            Command::Lexicon { command } => lexicon_request(command).unwrap(),
            _ => unreachable!(),
        };
        let (method, path, _) = parse(&[
            "acli",
            "lexicon",
            "delete",
            "--key",
            "Order",
            "--context",
            "sorting & paging",
            "--language",
            "pl",
        ]);
        assert_eq!(method, Method::DELETE);
        assert_eq!(
            path,
            "/lexicon/entries?key=Order&context=sorting+%26+paging&language=pl&plural_category=other"
        );
        let (_, path, _) = parse(&[
            "acli",
            "lexicon",
            "report",
            "--language",
            "pl",
            "--language",
            "de",
        ]);
        assert_eq!(path, "/lexicon/report?languages=pl%2Cde");
        let (_, _, body) = parse(&[
            "acli",
            "lexicon",
            "set",
            "--key",
            "Product",
            "--language",
            "pl",
            "--plural-category",
            "few",
            "--text",
            "Produkty",
        ]);
        assert_eq!(body.unwrap()["plural_category"], "few");

        let file = tempfile::Builder::new().suffix(".toml").tempfile().unwrap();
        fs::write(
            file.path(),
            "format_version = 1\nlanguage = \"pl\"\n[[entries]]\nkey = \"Product\"\ntext = \"Produkt\"\n",
        )
        .unwrap();
        let path = file.path().to_str().unwrap();
        let (_, request_path, body) =
            parse(&["acli", "lexicon", "import", "--file", path, "--replace"]);
        assert_eq!(request_path, "/lexicon/import?mode=replace");
        assert_eq!(
            body.unwrap(),
            json!({"format_version": 1, "language": "pl", "entries": [{"key": "Product", "text": "Produkt"}]})
        );
    }

    #[test]
    fn saved_view_commands_accept_json_state_and_reject_bad_identifiers() {
        assert!(
            Cli::try_parse_from([
                "acli",
                "saved-view",
                "create",
                "--name",
                "Assets",
                "--state",
                "{\"blueprint\":\"asset\"}"
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from(["acli", "saved-view", "link", "--state", "state.json"]).is_ok()
        );
        assert!(Cli::try_parse_from(["acli", "saved-view", "get", "not-a-uuid"]).is_err());
        assert_eq!(
            json_object_argument("{\"blueprint\":\"asset\"}").unwrap(),
            json!({"blueprint":"asset"})
        );
    }

    #[test]
    fn only_allows_plaintext_credential_transport_to_loopback() {
        for url in [
            "https://catalog.example.test",
            "http://localhost:3000",
            "http://127.0.0.1:3000",
            "http://[::1]:3000",
        ] {
            assert!(
                is_secure_credential_transport(&Url::parse(url).unwrap()),
                "{url}"
            );
        }
        assert!(!is_secure_credential_transport(
            &Url::parse("http://catalog.example.test").unwrap()
        ));
    }

    #[test]
    fn converts_value_file_to_api_shape() {
        let source = r#"
[[values]]
kind = "scalar"
attribute_id = "00000000-0000-0000-0000-000000000001"
value = "Blue shirt"
"#;
        let input: ValueFile = toml::from_str(source).unwrap();
        assert_eq!(
            values_body(input, None).unwrap(),
            json!({
                "values": [{
                    "kind": "scalar",
                    "attribute_id": "00000000-0000-0000-0000-000000000001",
                    "context_id": null,
                    "value": "Blue shirt"
                }]
            })
        );
    }

    #[test]
    fn converts_attribute_code_to_api_shape() {
        let source = r#"
[[values]]
kind = "relationship"
attribute_code = "related_products"
target_record_id = "00000000-0000-0000-0000-000000000002"
"#;
        let input: ValueFile = toml::from_str(source).unwrap();
        assert_eq!(
            values_body(input, None).unwrap(),
            json!({
                "values": [{
                    "kind": "relationship",
                    "attribute_code": "related_products",
                    "context_id": null,
                    "target_record_id": "00000000-0000-0000-0000-000000000002"
                }]
            })
        );
    }

    #[test]
    fn converts_toml_temporal_literals_to_iso_strings() {
        let input: ValueFile = toml::from_str(
            r#"
[[values]]
kind = "scalar"
attribute_code = "available_on"
value = 2026-08-12

[[values]]
kind = "scalar"
attribute_code = "released_at"
value = 2026-08-12T14:30:00Z
"#,
        )
        .unwrap();
        assert_eq!(
            values_body(input, None).unwrap()["values"],
            json!([
                { "kind": "scalar", "attribute_code": "available_on", "context_id": null, "value": "2026-08-12" },
                { "kind": "scalar", "attribute_code": "released_at", "context_id": null, "value": "2026-08-12T14:30:00Z" }
            ])
        );
    }

    #[test]
    fn converts_relationship_replacement_file_to_api_shape() {
        let source = r#"
[[relationships]]
attribute_code = "categories"
target_record_ids = [
  "00000000-0000-0000-0000-000000000002",
  "00000000-0000-0000-0000-000000000003",
]
"#;
        let input: RelationshipFile = toml::from_str(source).unwrap();
        assert_eq!(
            relationships_body(input, None).unwrap(),
            json!({
                "relationships": [{
                    "attribute_code": "categories",
                    "context_id": null,
                    "target_record_ids": [
                        "00000000-0000-0000-0000-000000000002",
                        "00000000-0000-0000-0000-000000000003"
                    ]
                }]
            })
        );
    }

    #[test]
    fn combines_record_update_files_into_the_form_contract() {
        let values = tempfile::NamedTempFile::new().unwrap();
        fs::write(
            values.path(),
            "[[values]]\nkind = \"scalar\"\nattribute_code = \"title\"\nvalue = \"Updated shirt\"\n",
        )
        .unwrap();
        let removals = tempfile::NamedTempFile::new().unwrap();
        fs::write(
            removals.path(),
            "[[remove_values]]\nattribute_code = \"subtitle\"\n",
        )
        .unwrap();
        let context_id = Uuid::parse_str("00000000-0000-4000-8000-000000000001").unwrap();

        assert_eq!(
            form_update_body(
                Some(&values.path().to_path_buf()),
                None,
                Some(&removals.path().to_path_buf()),
                Some(context_id),
            )
            .unwrap(),
            json!({
                "values": [{
                    "kind": "scalar",
                    "attribute_code": "title",
                    "context_id": context_id,
                    "value": "Updated shirt"
                }],
                "relationships": [],
                "remove_values": [{ "attribute_code": "subtitle", "context_id": context_id }]
            })
        );
    }

    #[test]
    fn rejects_incompatible_value_fields_and_unknown_toml_keys() {
        let relationship_with_value = r#"
[[values]]
kind = "relationship"
attribute_id = "00000000-0000-0000-0000-000000000001"
target_record_id = "00000000-0000-0000-0000-000000000002"
value = "ignored before this validation"
"#;
        let input: ValueFile = toml::from_str(relationship_with_value).unwrap();
        assert!(matches!(values_body(input, None), Err(CliError::Input(_))));

        let conflicting_selectors = r#"
[[values]]
kind = "scalar"
attribute_id = "00000000-0000-0000-0000-000000000001"
attribute_code = "title"
value = "Blue shirt"
"#;
        let input: ValueFile = toml::from_str(conflicting_selectors).unwrap();
        assert!(matches!(values_body(input, None), Err(CliError::Input(_))));
    }

    #[test]
    fn preserves_base_path_and_encodes_dynamic_segments() {
        let server = Url::parse("https://example.test/catalog-api/").unwrap();
        let url = endpoint(&server, &format!("/contexts/{}", segment("en/GB?#"))).unwrap();
        assert_eq!(
            url.as_str(),
            "https://example.test/catalog-api/contexts/en%2FGB%3F%23"
        );
    }

    #[test]
    fn accepts_only_a_json_string_array_or_file_for_permissions() {
        assert_eq!(
            permissions_input("[\"records.read\"]").unwrap(),
            json!(["records.read"])
        );
        let file = tempfile::NamedTempFile::new().unwrap();
        fs::write(file.path(), "[\"members.manage\"]").unwrap();
        assert_eq!(
            permissions_input(file.path().to_str().unwrap()).unwrap(),
            json!(["members.manage"])
        );
        assert!(permissions_input("not-json").is_err());
        assert!(permissions_input("{\"permission\": \"records.read\"}").is_err());
        assert!(permissions_input("[\"records.read\", 1]").is_err());
    }

    #[test]
    fn generator_token_preset_requires_no_manual_fields_and_uses_least_privilege_permissions() {
        assert!(Cli::try_parse_from(["acli", "token", "create", "--generator"]).is_ok());
        assert!(
            Cli::try_parse_from([
                "acli",
                "token",
                "create",
                "--label",
                "manual",
                "--permissions",
                "[\"records.read\"]"
            ])
            .is_ok()
        );
        assert!(Cli::try_parse_from(["acli", "token", "create", "--label", "manual"]).is_err());
        assert!(
            Cli::try_parse_from([
                "acli",
                "token",
                "create",
                "--permissions",
                "[\"records.read\"]"
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from([
                "acli",
                "token",
                "create",
                "--generator",
                "--label",
                "manual"
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from([
                "acli",
                "token",
                "create",
                "--generator",
                "--permissions",
                "[\"records.read\"]"
            ])
            .is_err()
        );
        assert_eq!(GENERATOR_TOKEN_LABEL, "catalog-generator");
        assert_eq!(
            GENERATOR_TOKEN_PERMISSIONS,
            [
                "blueprints.read",
                "blueprints.write",
                "blueprints.publish",
                "contexts.read",
                "contexts.write",
                "records.read",
                "records.write",
                "records.publish",
            ]
        );
    }

    #[test]
    fn context_creation_defaults_the_parent_and_role_duplication_requires_a_code() {
        assert!(
            Cli::try_parse_from([
                "acli", "context", "create", "--code", "en-GB", "--data", "{}",
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "acli",
                "workspace",
                "role",
                "duplicate",
                "00000000-0000-0000-0000-000000000001",
            ])
            .is_err()
        );
    }

    #[test]
    fn context_file_can_omit_parent_id() {
        let input: ContextFile =
            toml::from_str("code = \"en-GB\"\ndata = { language = \"en-GB\" }").unwrap();
        let body = json!({ "code": input.code, "data": toml_to_json(input.data).unwrap(), "parent_id": input.parent_id });
        assert_eq!(body["parent_id"], Value::Null);
    }

    #[test]
    fn maps_api_errors_to_exit_code_four() {
        let error = CliError::Api {
            status: reqwest::StatusCode::UNPROCESSABLE_ENTITY.as_u16(),
            code: "invalid_input".to_owned(),
            message: "bad value".to_owned(),
            details: None,
        };
        assert_eq!(error.exit_code(), 4);
        assert_eq!(error.json()["error"]["status"], 422);
        assert!(error.json()["error"].get("details").is_none());
        let conflict = CliError::Api {
            status: reqwest::StatusCode::CONFLICT.as_u16(),
            code: "unique_key_conflict".to_owned(),
            message: "taken".to_owned(),
            details: Some(json!({ "conflicting_record_id": "e" })),
        };
        assert_eq!(
            conflict.json()["error"]["details"]["conflicting_record_id"],
            "e"
        );
    }

    #[test]
    fn json_or_file_inputs_and_generated_help_cover_new_commands() {
        assert_eq!(
            json_array_input("[\"one\"]", "--filters").unwrap(),
            json!(["one"])
        );
        assert!(json_array_input("{\"field\":\"title\"}", "--filters").is_err());
        let file = tempfile::NamedTempFile::new().unwrap();
        fs::write(file.path(), "{\"operation\":\"get\",\"key\":\"sync\"}").unwrap();
        assert_eq!(
            json_input(file.path().to_str().unwrap(), "--body").unwrap()["operation"],
            "get"
        );
        let help = Cli::try_parse_from(["acli", "--help"]);
        assert!(matches!(
            help,
            Err(error) if error.kind() == clap::error::ErrorKind::DisplayHelp
        ));
        assert!(Cli::try_parse_from(["acli", "workflow", "validate", "--stdin"]).is_ok());
        assert!(
            Cli::try_parse_from(["acli", "solution-pack", "inspect", "--file", "pack.tar.zst"])
                .is_ok()
        );
        assert!(Cli::try_parse_from(["acli", "solution-pack", "inspect"]).is_err());
        assert!(Cli::try_parse_from(["acli", "--no-env", "health"]).is_ok());
        assert!(
            Cli::try_parse_from([
                "acli",
                "file",
                "download-original",
                "00000000-0000-0000-0000-000000000001",
                "--output",
                "x.bin"
            ])
            .is_ok()
        );
    }

    #[test]
    fn rule_commands_require_definition_metadata_and_sources() {
        let id = "00000000-0000-4000-8000-000000000001";
        assert!(
            Cli::try_parse_from([
                "acli",
                "rule",
                "validate",
                "--blueprint-id",
                id,
                "--blueprint-version",
                "1",
                "--stdin"
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "acli",
                "rule",
                "create",
                "--blueprint-id",
                id,
                "--blueprint-version",
                "1",
                "--file",
                "rule.toml"
            ])
            .is_ok()
        );
        assert!(Cli::try_parse_from(["acli", "rule", "validate", "--stdin"]).is_err());
        assert!(
            read_source(SourceInput {
                file: None,
                stdin: false
            })
            .is_err()
        );
        assert!(
            Cli::try_parse_from([
                "acli",
                "rule",
                "run-now",
                id,
                "--idempotency-key",
                "check",
                "--dry-run"
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "acli",
                "rule",
                "enable",
                id,
                "2",
                "--accept-existing-violations"
            ])
            .is_ok()
        );
        assert!(Cli::try_parse_from(["acli", "data-health", "background-processing"]).is_ok());
    }

    #[tokio::test]
    async fn rule_routes_forward_definition_and_run_options() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = axum::Router::new()
            .route(
                "/rules/validate",
                axum::routing::post(|axum::Json(body): axum::Json<Value>| async move {
                    axum::Json(body)
                }),
            )
            .route(
                "/rules/{rule_id}/run-now",
                axum::routing::post(|axum::Json(body): axum::Json<Value>| async move {
                    axum::Json(body)
                }),
            )
            .route(
                "/rules/{rule_id}/versions/{version}/enable",
                axum::routing::post(|axum::Json(body): axum::Json<Value>| async move {
                    axum::Json(body)
                }),
            )
            .route(
                "/rule-findings",
                axum::routing::get(|uri: axum::http::Uri| async move {
                    axum::Json(json!({"query":uri.query()}))
                }),
            );
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = Client::new();
        let url = Url::parse(&format!("http://{address}")).unwrap();
        let id = Uuid::from_u128(1);
        let file = tempfile::NamedTempFile::new().unwrap();
        fs::write(file.path(), "format_version = 1\n").unwrap();
        let result = rule_command(
            &client,
            &url,
            RuleCommand::Validate {
                blueprint_id: id,
                blueprint_version: 2,
                context_id: None,
                source: SourceInput {
                    file: Some(file.path().to_owned()),
                    stdin: false,
                },
            },
        )
        .await
        .unwrap();
        let body: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(
            body,
            json!({"blueprint_id":id,"blueprint_version":2,"context_id":null,"definition":"format_version = 1\n"})
        );
        let result = rule_command(
            &client,
            &url,
            RuleCommand::RunNow {
                rule_id: id,
                idempotency_key: "check".into(),
                record_id: Some(id),
                dry_run: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&result).unwrap(),
            json!({"idempotency_key":"check","record_id":id,"dry_run":true})
        );
        let result = rule_command(
            &client,
            &url,
            RuleCommand::Enable {
                rule_id: id,
                version: 3,
                accept_existing_violations: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&result).unwrap(),
            json!({"accept_existing_violations":true})
        );
        let result = rule_command(
            &client,
            &url,
            RuleCommand::Findings {
                record_id: Some(id),
            },
        )
        .await
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&result).unwrap()["query"],
            format!("record_id={id}")
        );
        server.abort();
    }

    #[test]
    fn extension_run_and_annotation_commands_validate_arguments() {
        let id = "00000000-0000-4000-8000-000000000001";
        for args in [
            vec![
                "acli",
                "extension-run",
                "list",
                "--extension-id",
                "acme.docs",
            ],
            vec!["acli", "extension-run", "show", id],
            vec!["acli", "extension-run", "cancel", id],
            vec![
                "acli",
                "extension-run",
                "download",
                id,
                id,
                "--output",
                "out.pdf",
            ],
            vec![
                "acli",
                "extension",
                "annotation-namespace",
                "acme.docs",
                "--adopt",
            ],
            vec![
                "acli",
                "extension",
                "repair-annotations",
                "acme.docs",
                id,
                "--patch",
                "{}",
            ],
        ] {
            assert!(Cli::try_parse_from(args.clone()).is_ok(), "{args:?}");
        }
        assert!(Cli::try_parse_from(["acli", "extension-run", "show", "not-a-uuid"]).is_err());
        assert!(
            Cli::try_parse_from(["acli", "extension", "repair-annotations", "acme.docs", id])
                .is_err()
        );
    }

    #[test]
    fn operation_commands_validate_arguments() {
        let id = "00000000-0000-4000-8000-000000000001";
        assert!(
            Cli::try_parse_from([
                "acli",
                "extension-operation",
                "start",
                "example",
                "--operation-id",
                "export",
                "--input",
                "{}",
                "--idempotency-key",
                "once",
                "--input-file-id",
                id
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "acli",
                "extension-operation",
                "download",
                id,
                id,
                "--output",
                "export.csv"
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "acli",
                "extension-operation",
                "start",
                "example",
                "--input",
                "{}"
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from([
                "acli",
                "extension-schedule",
                "create",
                "example",
                "--operation-id",
                "export",
                "--input",
                "{}",
                "--interval-seconds",
                "59"
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from([
                "acli",
                "extension-schedule",
                "update",
                id,
                "--enabled",
                "false",
                "--interval-seconds",
                "60"
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "acli",
                "connector-job",
                "run",
                id,
                "--idempotency-key",
                "once"
            ])
            .is_ok()
        );
        assert!(json_object_argument("[]").is_err());
        assert_eq!(
            operation_source(Some(Uuid::from_u128(1))),
            json!({"input_file_id":Uuid::from_u128(1)})
        );
    }

    #[tokio::test]
    async fn operation_commands_forward_bounded_requests() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = axum::Router::new()
            .route(
                "/extensions/{extension_id}/operations",
                axum::routing::post(
                    |axum::extract::Path(id): axum::extract::Path<String>,
                     axum::Json(body): axum::Json<Value>| async move {
                        axum::Json(json!({"id":id,"body":body}))
                    },
                ),
            )
            .route(
                "/extensions/{extension_id}/operation-schedules",
                axum::routing::post(
                    |axum::extract::Path(id): axum::extract::Path<String>,
                     axum::Json(body): axum::Json<Value>| async move {
                        axum::Json(json!({"id":id,"body":body}))
                    },
                ),
            )
            .route(
                "/extension-operation-schedules/{id}",
                axum::routing::patch(|axum::Json(body): axum::Json<Value>| async move {
                    axum::Json(body)
                }),
            )
            .route(
                "/blueprint-connector-jobs/{id}/run",
                axum::routing::post(|axum::Json(body): axum::Json<Value>| async move {
                    axum::Json(body)
                }),
            );
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = Client::new();
        let url = Url::parse(&format!("http://{address}")).unwrap();
        let id = Uuid::from_u128(1);
        let start: Value = serde_json::from_str(
            &extension_operation_command(
                &client,
                &url,
                ExtensionOperationCommand::Start {
                    extension_id: "example".into(),
                    operation_id: "export".into(),
                    input: "{\"limit\":1}".into(),
                    idempotency_key: "once".into(),
                    input_file_id: Some(id),
                },
            )
            .await
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            start["body"],
            json!({"operation_id":"export","input":{"limit":1},"idempotency_key":"once","source_reference":{"input_file_id":id},"destination_reference":{}})
        );
        let schedule: Value = serde_json::from_str(
            &extension_schedule_command(
                &client,
                &url,
                ExtensionScheduleCommand::Create {
                    extension_id: "example".into(),
                    operation_id: "export".into(),
                    input: "{}".into(),
                    input_file_id: None,
                    interval_seconds: 60,
                },
            )
            .await
            .unwrap(),
        )
        .unwrap();
        assert_eq!(schedule["body"]["interval_seconds"], 60);
        let updated: Value = serde_json::from_str(
            &extension_schedule_command(
                &client,
                &url,
                ExtensionScheduleCommand::Update {
                    schedule_id: id,
                    enabled: false,
                    interval_seconds: 120,
                },
            )
            .await
            .unwrap(),
        )
        .unwrap();
        assert_eq!(updated, json!({"enabled":false,"interval_seconds":120}));
        let run: Value = serde_json::from_str(
            &connector_job_command(
                &client,
                &url,
                ConnectorJobCommand::Run {
                    job_id: id,
                    idempotency_key: "once".into(),
                },
            )
            .await
            .unwrap(),
        )
        .unwrap();
        assert_eq!(run, json!({"idempotency_key":"once"}));
        server.abort();
    }

    #[test]
    fn data_health_and_audit_query_paths_are_encoded() {
        assert_eq!(
            data_health_path("summary", Some(7)),
            "/data-health/summary?stale_after_days=7"
        );
        assert_eq!(data_health_path("storage", None), "/data-health/storage");
        assert_eq!(segment("owner/repository"), "owner%2Frepository");
    }

    #[test]
    fn auth_commands_require_stdin_secrets_and_support_a_session_file() {
        assert!(
            Cli::try_parse_from([
                "acli",
                "--session-file",
                "session.json",
                "auth",
                "login",
                "workspace",
                "--email",
                "user@example.test",
                "--password-stdin",
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "acli",
                "auth",
                "password-reset-confirm",
                "--token-stdin",
                "--password-stdin",
            ])
            .is_ok()
        );

        let file = tempfile::NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();
        SessionFile::save(&path, "session-secret".to_owned(), "csrf-secret".to_owned()).unwrap();
        let loaded = SessionFile::load(Some(&path)).unwrap().unwrap();
        assert_eq!(loaded.session, "session-secret");
        assert_eq!(loaded.csrf, "csrf-secret");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[tokio::test]
    async fn raw_responses_are_bounded_before_buffering() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let app = axum::Router::new().route(
                "/large",
                axum::routing::get(|| async {
                    (
                        axum::http::StatusCode::BAD_REQUEST,
                        "x".repeat(MAX_RESPONSE_BYTES + 1),
                    )
                }),
            );
            axum::serve(listener, app).await.unwrap();
        });
        let response = Client::new()
            .get(format!("http://{address}/large"))
            .send()
            .await
            .unwrap();
        assert!(matches!(
            raw_response(response).await,
            Err(CliError::ResponseTooLarge)
        ));
        server.abort();
    }

    #[tokio::test]
    async fn failed_download_leaves_the_existing_output_unchanged() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let app = axum::Router::new().route(
                "/download",
                axum::routing::get(|| async {
                    axum::response::Response::new(axum::body::Body::from_stream(
                        futures_util::stream::iter(vec![
                            Ok::<_, std::io::Error>(axum::body::Bytes::from_static(b"partial")),
                            Err(std::io::Error::other("interrupted transfer")),
                        ]),
                    ))
                }),
            );
            axum::serve(listener, app).await.unwrap();
        });
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("artifact.bin");
        fs::write(&output, "previous").unwrap();
        let result = raw_download(
            &Client::new(),
            &Url::parse(&format!("http://{address}")).unwrap(),
            "/download",
            &output,
            None,
        )
        .await;
        assert!(matches!(result, Err(CliError::Transport(_))));
        assert_eq!(fs::read_to_string(output).unwrap(), "previous");
        server.abort();
    }

    #[tokio::test]
    async fn migrate_bulk_classifies_needs_input_without_submitting_a_migration() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let migrations = Arc::new(AtomicUsize::new(0));
        let server = tokio::spawn({
            let migrations = migrations.clone();
            async move {
                let app = axum::Router::new()
                    .route("/v1/records/search", axum::routing::post(|| async {
                        axum::Json(json!({
                            "items": [{ "id": "00000000-0000-0000-0000-000000000001" }],
                            "next_cursor": null,
                        }))
                    }))
                    .route("/v1/records/{record_id}/blueprint-migration/preview", axum::routing::post(|| async {
                        axum::Json(json!({ "status": "needs_input", "issues": [{ "code": "required" }] }))
                    }))
                    .route("/v1/records/{record_id}/blueprint-migration", axum::routing::post(move || {
                        let migrations = migrations.clone();
                        async move {
                            migrations.fetch_add(1, Ordering::SeqCst);
                            axum::Json(json!({ "status": "migrated" }))
                        }
                    }));
                axum::serve(listener, app).await.unwrap();
            }
        });
        let client = Client::builder().build().unwrap();
        let body = migrate_records(
            &client,
            &Url::parse(&format!("http://{address}")).unwrap(),
            "product",
            1,
            25,
            false,
        )
        .await
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&body).unwrap()["needs_input"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(migrations.load(Ordering::SeqCst), 0);
        server.abort();
    }

    #[tokio::test]
    async fn migrate_bulk_rejects_a_repeated_next_cursor() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(AtomicUsize::new(0));
        let server = tokio::spawn({
            let requests = requests.clone();
            async move {
                let app = axum::Router::new().route(
                    "/v1/records/search",
                    axum::routing::post(move || {
                        let requests = requests.clone();
                        async move {
                            requests.fetch_add(1, Ordering::SeqCst);
                            axum::Json(json!({ "items": [], "next_cursor": "repeated" }))
                        }
                    }),
                );
                axum::serve(listener, app).await.unwrap();
            }
        });
        let result = migrate_records(
            &Client::new(),
            &Url::parse(&format!("http://{address}")).unwrap(),
            "product",
            1,
            25,
            false,
        )
        .await;
        assert!(matches!(result, Err(CliError::InvalidResponse)));
        assert_eq!(requests.load(Ordering::SeqCst), 2);
        server.abort();
    }

    #[tokio::test]
    async fn migrate_bulk_rejects_a_malformed_next_cursor() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let app = axum::Router::new().route(
                "/v1/records/search",
                axum::routing::post(|| async {
                    axum::Json(json!({ "items": [], "next_cursor": true }))
                }),
            );
            axum::serve(listener, app).await.unwrap();
        });
        let client = Client::new();
        let result = migrate_records(
            &client,
            &Url::parse(&format!("http://{address}")).unwrap(),
            "product",
            1,
            25,
            false,
        )
        .await;
        assert!(matches!(result, Err(CliError::InvalidResponse)));
        server.abort();
    }

    #[tokio::test]
    async fn solution_pack_inspect_streams_the_archive_to_the_server() {
        use std::sync::{Arc, Mutex};

        let received = Arc::new(Mutex::new(None));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn({
            let received = received.clone();
            async move {
                let app = axum::Router::new().route(
                    "/solution-packs/inspect",
                    axum::routing::post(
                        move |headers: axum::http::HeaderMap, body: axum::body::Bytes| {
                            let received = received.clone();
                            async move {
                                *received.lock().unwrap() = Some((
                                    headers
                                        .get(axum::http::header::CONTENT_TYPE)
                                        .unwrap()
                                        .to_str()
                                        .unwrap()
                                        .to_owned(),
                                    body.to_vec(),
                                ));
                                axum::Json(json!({"archive_sha256": "server-validated"}))
                            }
                        },
                    ),
                );
                axum::serve(listener, app).await.unwrap();
            }
        });
        let archive = tempfile::NamedTempFile::new().unwrap();
        fs::write(archive.path(), b"opaque archive bytes").unwrap();

        let body = run(Cli {
            server: Some(Url::parse(&format!("http://{address}")).unwrap()),
            token: None,
            token_stdin: false,
            session_file: None,
            no_env: false,
            command: Command::SolutionPack {
                command: SolutionPackCommand::Inspect {
                    file: archive.path().to_path_buf(),
                },
            },
        })
        .await
        .unwrap();

        assert_eq!(body, r#"{"archive_sha256":"server-validated"}"#);
        assert_eq!(
            received.lock().unwrap().take().unwrap(),
            (
                "application/zstd".to_owned(),
                b"opaque archive bytes".to_vec()
            )
        );
        server.abort();
    }

    #[test]
    fn presentation_asset_parser_supports_bounded_discovery_and_download() {
        assert!(
            Cli::try_parse_from([
                "acli",
                "presentation-asset",
                "list",
                "--limit",
                "100",
                "--offset",
                "10000"
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from(["acli", "presentation-asset", "list", "--limit", "101"]).is_err()
        );
        assert!(
            Cli::try_parse_from([
                "acli",
                "presentation-asset",
                "show",
                "00000000-0000-4000-8000-000000000001",
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "acli",
                "presentation-asset",
                "download",
                "00000000-0000-4000-8000-000000000001",
                "--output",
                "asset.svg",
            ])
            .is_ok()
        );
    }

    #[test]
    fn solution_pack_plan_parser_supports_create_and_show_shapes() {
        let create = Cli::try_parse_from([
            "acli",
            "solution-pack",
            "plan",
            "--file",
            "pack.tar.zst",
            "--prefix",
            "ecom",
            "--blueprint-publication",
            "publish",
            "--include-sample-data",
        ]);
        assert!(create.is_ok());
        assert!(
            Cli::try_parse_from([
                "acli",
                "solution-pack",
                "plan",
                "--file",
                "pack.tar.zst",
                "--prefix",
                "ecom",
                "--blueprint-publication",
                "publish",
                "--from-application",
                "00000000-0000-4000-8000-000000000001",
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "acli",
                "solution-pack",
                "plan",
                "--file",
                "pack.tar.zst",
                "--prefix",
                "ecom",
                "--blueprint-publication",
                "publish",
                "--from-application",
                "00000000-0000-4000-8000-000000000001",
                "--map",
                "blueprints/product=shared_product",
            ])
            .is_err()
        );
        let show = Cli::try_parse_from([
            "acli",
            "solution-pack",
            "plan",
            "show",
            "00000000-0000-4000-8000-000000000001",
        ]);
        assert!(show.is_ok());
        assert!(
            Cli::try_parse_from([
                "acli",
                "solution-pack",
                "apply",
                "00000000-0000-4000-8000-000000000001",
            ])
            .is_ok()
        );
        assert!(Cli::try_parse_from(["acli", "solution-pack", "apply", "not-a-uuid"]).is_err());
        assert!(Cli::try_parse_from(["acli", "solution-pack", "applications", "list"]).is_ok());
        assert!(
            Cli::try_parse_from([
                "acli",
                "solution-pack",
                "applications",
                "show",
                "00000000-0000-4000-8000-000000000002",
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "acli",
                "solution-pack",
                "checks",
                "rerun",
                "00000000-0000-4000-8000-000000000002"
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "acli",
                "solution-pack",
                "checks",
                "list",
                "00000000-0000-4000-8000-000000000002",
                "--limit",
                "100",
                "--offset",
                "10000"
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "acli",
                "solution-pack",
                "checks",
                "show",
                "00000000-0000-4000-8000-000000000002",
                "00000000-0000-4000-8000-000000000003"
            ])
            .is_ok()
        );
    }

    #[tokio::test]
    async fn solution_pack_apply_and_history_use_uuid_only_json_routes() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let application_id = Uuid::from_u128(2);
        let app =
            axum::Router::new()
                .route(
                    "/solution-packs/plans/{plan_id}/apply",
                    axum::routing::post(
                        move |axum::extract::Path(plan_id): axum::extract::Path<Uuid>| async move {
                            axum::Json(json!({"plan_id": plan_id}))
                        },
                    ),
                )
                .route(
                    "/solution-packs/applications",
                    axum::routing::get(|uri: axum::http::Uri| async move {
                        axum::Json(json!({"query": uri.query()}))
                    }),
                )
                .route(
                    "/solution-packs/applications/{application_id}",
                    axum::routing::get(
                        move |axum::extract::Path(id): axum::extract::Path<Uuid>| async move {
                            axum::Json(json!({"id": id}))
                        },
                    ),
                )
                .route(
                    "/solution-packs/applications/{application_id}/abandon",
                    axum::routing::post(
                        |axum::extract::Path(id): axum::extract::Path<Uuid>| async move {
                            axum::Json(json!({"abandoned": id}))
                        },
                    ),
                )
                .route(
                    "/solution-packs/applications/{application_id}/checks",
                    axum::routing::get(
                        |axum::extract::Path(id): axum::extract::Path<Uuid>,
                         uri: axum::http::Uri| async move {
                            axum::Json(json!({"id":id,"query":uri.query()}))
                        },
                    )
                    .post(
                        |axum::extract::Path(id): axum::extract::Path<Uuid>| async move {
                            axum::Json(json!({"rerun":id}))
                        },
                    ),
                )
                .route(
                    "/solution-packs/applications/{application_id}/checks/{run_id}",
                    axum::routing::get(
                        |axum::extract::Path((application_id, run_id)): axum::extract::Path<(
                            Uuid,
                            Uuid,
                        )>| async move {
                            axum::Json(json!({"application_id":application_id,"run_id":run_id}))
                        },
                    ),
                );
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = Client::new();
        let url = Url::parse(&format!("http://{address}")).unwrap();
        let plan_id = Uuid::from_u128(1);
        assert_eq!(
            solution_pack_command(&client, &url, SolutionPackCommand::Apply { plan_id })
                .await
                .unwrap(),
            format!(r#"{{"plan_id":"{plan_id}"}}"#)
        );
        assert_eq!(
            solution_pack_command(
                &client,
                &url,
                SolutionPackCommand::Applications {
                    command: SolutionPackApplicationsCommand::List {
                        limit: 25,
                        offset: 0,
                    },
                },
            )
            .await
            .unwrap(),
            r#"{"query":"limit=25&offset=0"}"#
        );
        assert_eq!(
            solution_pack_command(
                &client,
                &url,
                SolutionPackCommand::Applications {
                    command: SolutionPackApplicationsCommand::List {
                        limit: 100,
                        offset: 10_000,
                    },
                },
            )
            .await
            .unwrap(),
            r#"{"query":"limit=100&offset=10000"}"#
        );
        assert_eq!(
            solution_pack_command(
                &client,
                &url,
                SolutionPackCommand::Applications {
                    command: SolutionPackApplicationsCommand::Show { application_id },
                },
            )
            .await
            .unwrap(),
            format!(r#"{{"id":"{application_id}"}}"#)
        );
        assert_eq!(
            solution_pack_command(
                &client,
                &url,
                SolutionPackCommand::Applications {
                    command: SolutionPackApplicationsCommand::Abandon { application_id },
                },
            )
            .await
            .unwrap(),
            format!(r#"{{"abandoned":"{application_id}"}}"#)
        );
        assert_eq!(
            solution_pack_command(
                &client,
                &url,
                SolutionPackCommand::Checks {
                    command: SolutionPackChecksCommand::Rerun { application_id },
                }
            )
            .await
            .unwrap(),
            format!(r#"{{"rerun":"{application_id}"}}"#)
        );
        assert_eq!(
            solution_pack_command(
                &client,
                &url,
                SolutionPackCommand::Checks {
                    command: SolutionPackChecksCommand::List {
                        application_id,
                        limit: 25,
                        offset: 0
                    },
                }
            )
            .await
            .unwrap(),
            format!(r#"{{"id":"{application_id}","query":"limit=25&offset=0"}}"#)
        );
        let run_id = Uuid::from_u128(3);
        assert_eq!(
            solution_pack_command(
                &client,
                &url,
                SolutionPackCommand::Checks {
                    command: SolutionPackChecksCommand::Show {
                        application_id,
                        run_id
                    },
                }
            )
            .await
            .unwrap(),
            format!(r#"{{"application_id":"{application_id}","run_id":"{run_id}"}}"#)
        );
        server.abort();
    }

    #[tokio::test]
    async fn solution_pack_plan_rejects_incomplete_and_invalid_forms() {
        assert!(
            Cli::try_parse_from([
                "acli",
                "solution-pack",
                "plan",
                "--file",
                "pack.tar.zst",
                "--prefix",
                "ecom",
                "--blueprint-publication",
                "invalid",
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from(["acli", "solution-pack", "plan", "show", "not-a-uuid",]).is_err()
        );

        let api = Url::parse("http://127.0.0.1:1").unwrap();
        let missing_publication = solution_pack_command(
            &Client::new(),
            &api,
            SolutionPackCommand::Plan {
                command: None,
                file: Some(PathBuf::from("pack.tar.zst")),
                prefix: Some("ecom".to_owned()),
                blueprint_publication: None,
                blueprint_maps: Vec::new(),
                asset_maps: Vec::new(),
                context_maps: Vec::new(),
                from_application: None,
                include_sample_data: false,
            },
        )
        .await;
        assert!(matches!(missing_publication, Err(CliError::Input(_))));

        let mixed_show = solution_pack_command(
            &Client::new(),
            &api,
            SolutionPackCommand::Plan {
                command: Some(SolutionPackPlanCommand::Show {
                    plan_id: Uuid::nil(),
                }),
                file: Some(PathBuf::from("pack.tar.zst")),
                prefix: Some("ecom".to_owned()),
                blueprint_publication: Some(BlueprintPublicationArgument::Draft),
                blueprint_maps: Vec::new(),
                asset_maps: Vec::new(),
                context_maps: Vec::new(),
                from_application: None,
                include_sample_data: false,
            },
        )
        .await;
        assert!(matches!(mixed_show, Err(CliError::Input(_))));

        let mut command = <Cli as clap::CommandFactory>::command();
        let help = command
            .find_subcommand_mut("solution-pack")
            .unwrap()
            .find_subcommand_mut("plan")
            .unwrap()
            .render_long_help()
            .to_string();
        assert!(help.contains("--map <LOGICAL_KEY=EXISTING_CODE>"));

        for mapping in [
            "missing_equals",
            "=missing_key",
            "blueprints/product=Unsafe-code",
            "contexts/default=valid_code",
        ] {
            let invalid_mapping = solution_pack_command(
                &Client::new(),
                &api,
                SolutionPackCommand::Plan {
                    command: None,
                    file: Some(PathBuf::from("pack.tar.zst")),
                    prefix: Some("ecom".to_owned()),
                    blueprint_publication: Some(BlueprintPublicationArgument::Draft),
                    blueprint_maps: vec![mapping.to_owned()],
                    asset_maps: Vec::new(),
                    context_maps: Vec::new(),
                    from_application: None,
                    include_sample_data: false,
                },
            )
            .await;
            assert!(matches!(invalid_mapping, Err(CliError::Input(_))));
        }
    }

    #[tokio::test]
    async fn solution_pack_plan_streams_parameters_and_show_fetches_by_id() {
        use std::sync::{Arc, Mutex};

        let received = Arc::new(Mutex::new(None));
        let plan_id = Uuid::from_u128(0x00000000000040008000000000000166);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn({
            let received = received.clone();
            async move {
                let app = axum::Router::new()
                    .route(
                        "/solution-packs/plans",
                        axum::routing::post(
                            move |uri: axum::http::Uri,
                                  headers: axum::http::HeaderMap,
                                  body: axum::body::Bytes| {
                                let received = received.clone();
                                async move {
                                    *received.lock().unwrap() = Some((
                                        uri.query().unwrap().to_owned(),
                                        headers
                                            .get(axum::http::header::CONTENT_TYPE)
                                            .unwrap()
                                            .to_str()
                                            .unwrap()
                                            .to_owned(),
                                        body.to_vec(),
                                    ));
                                    axum::Json(json!({
                                        "id":"created",
                                        "actions":[{
                                            "resource_kind":"workspace_setting",
                                            "logical_key":"workspace/explore-navigation",
                                            "action":"append"
                                        }]
                                    }))
                                }
                            },
                        ),
                    )
                    .route(
                        "/solution-packs/plans/{plan_id}",
                        axum::routing::get(
                            |axum::extract::Path(plan_id): axum::extract::Path<Uuid>| async move {
                                axum::Json(json!({"id": plan_id}))
                            },
                        ),
                    );
                axum::serve(listener, app).await.unwrap();
            }
        });
        let archive = tempfile::NamedTempFile::new().unwrap();
        fs::write(archive.path(), b"opaque archive bytes").unwrap();
        let api = Url::parse(&format!("http://{address}")).unwrap();

        let created = solution_pack_command(
            &Client::new(),
            &api,
            SolutionPackCommand::Plan {
                command: None,
                file: Some(archive.path().to_path_buf()),
                prefix: Some("shop prefix".to_owned()),
                blueprint_publication: Some(BlueprintPublicationArgument::Publish),
                blueprint_maps: Vec::new(),
                asset_maps: Vec::new(),
                context_maps: Vec::new(),
                from_application: None,
                include_sample_data: false,
            },
        )
        .await
        .unwrap();
        assert!(created.contains(r#""resource_kind":"workspace_setting""#));
        assert!(created.contains(r#""logical_key":"workspace/explore-navigation""#));
        assert_eq!(
            received.lock().unwrap().take().unwrap(),
            (
                "prefix=shop+prefix&blueprint_publication=publish".to_owned(),
                "application/zstd".to_owned(),
                b"opaque archive bytes".to_vec()
            )
        );

        let prior_application_id = Uuid::parse_str("00000000-0000-4000-8000-000000000123").unwrap();
        solution_pack_command(
            &Client::new(),
            &api,
            SolutionPackCommand::Plan {
                command: None,
                file: Some(archive.path().to_path_buf()),
                prefix: Some("shop".to_owned()),
                blueprint_publication: Some(BlueprintPublicationArgument::Publish),
                blueprint_maps: Vec::new(),
                asset_maps: Vec::new(),
                context_maps: Vec::new(),
                from_application: Some(prior_application_id),
                include_sample_data: false,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            received.lock().unwrap().take().unwrap(),
            (
                format!(
                    "prefix=shop&blueprint_publication=publish&from_application={prior_application_id}"
                ),
                "application/zstd".to_owned(),
                b"opaque archive bytes".to_vec()
            )
        );

        solution_pack_command(
            &Client::new(),
            &api,
            SolutionPackCommand::Plan {
                command: None,
                file: Some(archive.path().to_path_buf()),
                prefix: Some("shop".to_owned()),
                blueprint_publication: Some(BlueprintPublicationArgument::Publish),
                blueprint_maps: vec![
                    "blueprints/product=shared_product".to_owned(),
                    "blueprints/category=shared_category".to_owned(),
                ],
                asset_maps: vec![
                    "assets/brand-logo=00000000-0000-4000-8000-000000000999".to_owned(),
                ],
                context_maps: vec!["contexts/poland=PL".to_owned()],
                from_application: None,
                include_sample_data: false,
            },
        )
        .await
        .unwrap();
        let (query, content_type, body) = received.lock().unwrap().take().unwrap();
        assert_eq!(query, "prefix=shop&blueprint_publication=publish");
        assert!(content_type.starts_with("multipart/form-data; boundary="));
        let body = String::from_utf8(body).unwrap();
        assert!(body.contains("name=\"archive\""));
        assert!(body.contains("application/zstd"));
        assert!(body.contains("opaque archive bytes"));
        assert_eq!(body.matches("name=\"blueprint_map\"").count(), 2);
        assert_eq!(body.matches("name=\"asset_map\"").count(), 1);
        assert!(body.contains("assets/brand-logo"));
        assert!(body.contains("00000000-0000-4000-8000-000000000999"));
        assert!(body.contains("blueprints/product"));
        assert!(body.contains("shared_product"));
        assert_eq!(body.matches("name=\"context_map\"").count(), 1);
        assert!(body.contains(r#"{"code":"PL","key":"contexts/poland"}"#));

        let duplicate = solution_pack_command(
            &Client::new(),
            &api,
            SolutionPackCommand::Plan {
                command: None,
                file: Some(archive.path().to_path_buf()),
                prefix: Some("shop".to_owned()),
                blueprint_publication: Some(BlueprintPublicationArgument::Publish),
                blueprint_maps: vec![
                    "blueprints/product=shared_product".to_owned(),
                    "blueprints/product=other_product".to_owned(),
                ],
                asset_maps: Vec::new(),
                context_maps: Vec::new(),
                from_application: None,
                include_sample_data: false,
            },
        )
        .await;
        assert!(matches!(duplicate, Err(CliError::Input(_))));

        let shown = solution_pack_command(
            &Client::new(),
            &api,
            SolutionPackCommand::Plan {
                command: Some(SolutionPackPlanCommand::Show { plan_id }),
                file: None,
                prefix: None,
                blueprint_publication: None,
                blueprint_maps: Vec::new(),
                asset_maps: Vec::new(),
                context_maps: Vec::new(),
                from_application: None,
                include_sample_data: false,
            },
        )
        .await
        .unwrap();
        assert!(shown.contains(&plan_id.to_string()));
        server.abort();
    }

    #[tokio::test]
    async fn bearer_token_skips_a_saved_browser_session() {
        let session_file = tempfile::NamedTempFile::new().unwrap();
        fs::write(session_file.path(), "not JSON").unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                axum::Router::new().route(
                    "/health",
                    axum::routing::get(|| async { axum::Json(json!({ "status": "ok" })) }),
                ),
            )
            .await
            .unwrap()
        });

        let body = run(Cli {
            server: Some(Url::parse(&format!("http://{address}")).unwrap()),
            token: Some("personal-token".to_owned()),
            token_stdin: false,
            session_file: Some(session_file.path().to_path_buf()),
            no_env: false,
            command: Command::Health,
        })
        .await
        .unwrap();
        assert_eq!(body, r#"{"status":"ok"}"#);
        server.abort();
    }

    #[tokio::test]
    async fn forwards_http_json_without_reformatting() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                axum::Router::new().route(
                    "/health",
                    axum::routing::get(|| async { axum::Json(json!({ "status": "ok" })) }),
                ),
            )
            .await
            .unwrap()
        });

        let body = run(Cli {
            server: Some(Url::parse(&format!("http://{address}")).unwrap()),
            token: None,
            token_stdin: false,
            session_file: None,
            no_env: false,
            command: Command::Health,
        })
        .await
        .unwrap();
        assert_eq!(body, r#"{"status":"ok"}"#);

        server.abort();
    }
}
