use std::path::PathBuf;

use clap::Subcommand;
use reqwest::{Client, Method};
use url::Url;
use uuid::Uuid;

use crate::cli::{DEFAULT_LIST_LIMIT, MAX_LIST_LIMIT, MAX_LIST_OFFSET};
use crate::error::CliError;
use crate::transport::{raw_download, request, segment};

#[derive(Subcommand)]
pub(crate) enum PresentationAssetCommand {
    List {
        #[arg(long, default_value_t = DEFAULT_LIST_LIMIT, value_parser = clap::value_parser!(u16).range(1..=i64::from(MAX_LIST_LIMIT)))]
        limit: u16,
        #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u16).range(0..=i64::from(MAX_LIST_OFFSET)))]
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

pub(crate) async fn presentation_asset_command(
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
