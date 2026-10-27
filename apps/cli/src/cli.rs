//! Command-line arguments shared by every command.

use std::path::PathBuf;

use clap::parser::ValueSource;
use clap::{ArgMatches, Args, Parser, Subcommand};
use url::Url;
use uuid::Uuid;

use crate::commands::agent::AgentCommand;
use crate::commands::audit::AuditCommand;
use crate::commands::auth::AuthCommand;
use crate::commands::blueprint::BlueprintCommand;
use crate::commands::context::ContextCommand;
use crate::commands::data_health::DataHealthCommand;
use crate::commands::directory::DirectoryCommand;
use crate::commands::event::EventCommand;
use crate::commands::extension::ExtensionCommand;
use crate::commands::file::FileCommand;
use crate::commands::lexicon::LexiconCommand;
use crate::commands::notification::NotificationCommand;
use crate::commands::presentation_asset::PresentationAssetCommand;
use crate::commands::record::RecordCommand;
use crate::commands::reusable_attribute::ReusableAttributeCommand;
use crate::commands::rule::RuleCommand;
use crate::commands::saved_view::SavedViewCommand;
use crate::commands::solution_pack::SolutionPackCommand;
use crate::commands::team::TeamCommand;
use crate::commands::token::TokenCommand;
use crate::commands::value::ValueCommand;
use crate::commands::workflow::WorkflowCommand;
use crate::commands::workspace::WorkspaceCommand;
use crate::error::CliError;

pub(crate) const DEFAULT_SERVER_URL: &str = "http://127.0.0.1:3000/api";

/// Page size `acli` asks for when a command needs one and none is given.
pub(crate) const DEFAULT_LIST_LIMIT: u32 = 25;

/// The deepest offset any paged API endpoint accepts.
pub(crate) const MAX_LIST_OFFSET: u16 = 10_000;

/// Offset paging. Omitted values use the endpoint's default, and the endpoint
/// enforces its own maximum page size.
#[derive(Args, Clone, Copy, Default)]
pub(crate) struct PageArgs {
    /// Items per page.
    #[arg(long, value_parser = clap::value_parser!(u16).range(1..))]
    pub(crate) limit: Option<u16>,
    /// Items to skip.
    #[arg(long, value_parser = clap::value_parser!(u16).range(0..=i64::from(MAX_LIST_OFFSET)))]
    pub(crate) offset: Option<u16>,
}

/// Keyset paging for newest-first lists: pass the `created_at` and `id` of a
/// page's last item to continue after it.
#[derive(Args, Clone, Default)]
pub(crate) struct BeforeArgs {
    /// Continue after a page: the created_at of its last item.
    #[arg(long, requires = "before_id")]
    pub(crate) before_time: Option<String>,
    /// Continue after a page: the id of its last item.
    #[arg(long, requires = "before_time")]
    pub(crate) before_id: Option<Uuid>,
}

impl BeforeArgs {
    pub(crate) fn query(self) -> [(&'static str, Option<String>); 2] {
        [
            ("before_time", self.before_time),
            ("before_id", self.before_id.map(|id| id.to_string())),
        ]
    }
}

impl PageArgs {
    pub(crate) fn query(self) -> [(&'static str, Option<String>); 2] {
        [
            ("limit", self.limit.map(|limit| limit.to_string())),
            ("offset", self.offset.map(|offset| offset.to_string())),
        ]
    }
}

/// The argument id of `--token-stdin`.
const TOKEN_STDIN: &str = "token_stdin";
/// The id suffix every flag that reads standard input shares.
const STDIN_SUFFIX: &str = "stdin";

/// Rejects `--token-stdin` together with a command flag that also reads
/// standard input: the token would consume all of it, leaving the command an
/// empty value.
pub(crate) fn reject_shared_stdin(matches: &ArgMatches) -> Result<(), CliError> {
    if !matches.get_flag(TOKEN_STDIN) {
        return Ok(());
    }
    let mut current = matches;
    while let Some((_, command)) = current.subcommand() {
        if let Some(id) = command.ids().find(|id| {
            let id = id.as_str();
            id != TOKEN_STDIN
                && id.ends_with(STDIN_SUFFIX)
                && command.value_source(id) == Some(ValueSource::CommandLine)
        }) {
            return Err(CliError::Input(format!(
                "--token-stdin cannot be combined with --{}: both read standard input; \
                 pass the token with ATTRICAT_TOKEN instead",
                id.as_str().replace('_', "-")
            )));
        }
        current = command;
    }
    Ok(())
}

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
    /// Personal API token. Prefer ATTRICAT_TOKEN or --token-stdin to avoid
    /// exposing it in process arguments.
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
    /// Check that the API answers; a subcommand checks something specific.
    Health {
        #[command(subcommand)]
        command: Option<HealthCommand>,
    },
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
    /// Attribute definitions shared across blueprints and added to records.
    ReusableAttribute {
        #[command(subcommand)]
        command: ReusableAttributeCommand,
    },
    Record {
        #[command(subcommand)]
        command: RecordCommand,
    },
    /// Workspace users and teams that assignment attributes can reference.
    Directory {
        #[command(subcommand)]
        command: DirectoryCommand,
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
    /// Converse with the workspace agent and approve its changes.
    Agent {
        #[command(subcommand)]
        command: AgentCommand,
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
    /// Installed extensions, their registries, runs, schedules and connector jobs.
    Extension {
        #[command(subcommand)]
        command: ExtensionCommand,
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

#[derive(Subcommand)]
pub(crate) enum HealthCommand {
    /// Whether the API process is running.
    Live,
    /// Whether the API can serve requests; fails while it cannot.
    Ready,
    /// The running build (requires `data_health.read`).
    System,
}
