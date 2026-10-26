use std::path::PathBuf;

use clap::Subcommand;
use uuid::Uuid;

#[derive(Subcommand)]
pub(crate) enum ValueCommand {
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
