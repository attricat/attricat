use clap::Subcommand;
use reqwest::{Client, Method};
use url::Url;

use crate::error::CliError;
use crate::transport::request;

#[derive(Subcommand)]
pub(crate) enum DataHealthCommand {
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

pub(crate) async fn data_health_command(
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

pub(crate) fn data_health_path(name: &str, days: Option<u16>) -> String {
    match days {
        Some(days) => format!("/data-health/{name}?stale_after_days={days}"),
        None => format!("/data-health/{name}"),
    }
}
