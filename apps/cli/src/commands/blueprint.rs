use clap::Subcommand;
use uuid::Uuid;

use crate::cli::SourceInput;

#[derive(Subcommand)]
pub(crate) enum BlueprintCommand {
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
    /// Run every save check on a definition without storing it. With
    /// `--blueprint-id`, check it as that blueprint's next revision.
    Validate {
        #[arg(long)]
        blueprint_id: Option<Uuid>,
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
