use std::path::PathBuf;

use clap::Subcommand;
use uuid::Uuid;

#[derive(Subcommand)]
pub(crate) enum ContextCommand {
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
