use std::path::PathBuf;

use clap::Subcommand;
use uuid::Uuid;

use crate::cli::PageArgs;
use crate::error::CliError;
use crate::transport::{Api, segment, with_query};

#[derive(Subcommand)]
pub(crate) enum PresentationAssetCommand {
    List {
        #[command(flatten)]
        page: PageArgs,
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

pub(crate) async fn presentation_asset_command(
    api: &Api,
    command: PresentationAssetCommand,
) -> Result<String, CliError> {
    match command {
        PresentationAssetCommand::List { page } => {
            api.get(&with_query("/presentation-assets", page.query()))
                .await
        }
        PresentationAssetCommand::Show { asset_id } => {
            api.get(&format!("/presentation-assets/{}", segment(asset_id)))
                .await
        }
        PresentationAssetCommand::Download { asset_id, output } => {
            api.download(
                &format!("/presentation-assets/{}/content", segment(asset_id)),
                &output,
                None,
            )
            .await
        }
    }
}
