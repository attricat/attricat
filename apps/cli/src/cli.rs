//! Command-line arguments shared by every command.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use url::Url;
use uuid::Uuid;

use crate::commands::audit::AuditCommand;
use crate::commands::auth::AuthCommand;
use crate::commands::blueprint::BlueprintCommand;
use crate::commands::context::ContextCommand;
use crate::commands::data_health::DataHealthCommand;
use crate::commands::event::EventCommand;
use crate::commands::extension::{
    ConnectorJobCommand, ExtensionCommand, ExtensionOperationCommand, ExtensionRegistryCommand,
    ExtensionRunCommand, ExtensionScheduleCommand,
};
use crate::commands::file::FileCommand;
use crate::commands::lexicon::LexiconCommand;
use crate::commands::notification::NotificationCommand;
use crate::commands::presentation_asset::PresentationAssetCommand;
use crate::commands::record::RecordCommand;
use crate::commands::rule::RuleCommand;
use crate::commands::saved_view::SavedViewCommand;
use crate::commands::solution_pack::SolutionPackCommand;
use crate::commands::team::TeamCommand;
use crate::commands::token::TokenCommand;
use crate::commands::value::ValueCommand;
use crate::commands::workflow::WorkflowCommand;
use crate::commands::workspace::WorkspaceCommand;

pub(crate) const DEFAULT_SERVER_URL: &str = "http://127.0.0.1:3000/api";

pub(crate) const DEFAULT_LIST_LIMIT: u16 = 25;

pub(crate) const MAX_LIST_LIMIT: u16 = 100;

pub(crate) const MAX_LIST_OFFSET: u16 = 10_000;

#[derive(Parser)]
#[command(
    name = "acli",
    version,
    about = "JSON-first client for the Attricat API"
)]
pub(crate) struct Cli {
    /// API base URL, including its `/api` path.
    #[arg(long, global = true, env = "ATTRICAT_API_URL", default_value = DEFAULT_SERVER_URL)]
    pub(crate) server: Url,
    /// Personal API token. Prefer ATTRICAT_TOKEN or --token-stdin to avoid exposing it in process arguments.
    #[arg(
        long,
        global = true,
        env = "ATTRICAT_TOKEN",
        hide_env_values = true,
        conflicts_with = "token_stdin"
    )]
    pub(crate) token: Option<String>,
    /// Read the personal API token from standard input.
    #[arg(long, global = true, conflicts_with = "token")]
    pub(crate) token_stdin: bool,
    /// Persist browser session and CSRF cookies for the auth commands.
    #[arg(
        long,
        global = true,
        env = "ATTRICAT_SESSION_FILE",
        value_name = "PATH"
    )]
    pub(crate) session_file: Option<PathBuf>,
    /// Do not load environment variables from a .env file in the current directory.
    #[arg(long, global = true)]
    pub(crate) no_env: bool,
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand)]
pub(crate) enum Command {
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
    Directory {
        /// Show one member's profile, with their teams, instead.
        #[arg(long)]
        user: Option<Uuid>,
    },
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
pub(crate) struct SourceInput {
    #[arg(long, conflicts_with = "stdin")]
    pub(crate) file: Option<PathBuf>,
    #[arg(long)]
    pub(crate) stdin: bool,
}

#[derive(Subcommand)]
pub(crate) enum MetricsCommand {
    Get {
        #[arg(long)]
        output: PathBuf,
    },
}
