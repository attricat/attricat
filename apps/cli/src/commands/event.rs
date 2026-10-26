use crate::error::CliError;
use crate::transport::{Api, segment};
use clap::Subcommand;
use uuid::Uuid;

#[derive(Subcommand)]
pub(crate) enum EventCommand {
    DeadLetters,
    Replay { consumer_id: Uuid, event_id: Uuid },
}

pub(crate) async fn event_command(api: &Api, command: EventCommand) -> Result<String, CliError> {
    match command {
        EventCommand::DeadLetters => api.get("/event-deliveries/dead-letters").await,
        EventCommand::Replay {
            consumer_id,
            event_id,
        } => {
            api.post(&format!(
                "/event-deliveries/{}/{}/replay",
                segment(consumer_id),
                segment(event_id)
            ))
            .await
        }
    }
}
