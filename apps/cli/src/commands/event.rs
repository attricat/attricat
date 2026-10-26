use clap::Subcommand;
use uuid::Uuid;

#[derive(Subcommand)]
pub(crate) enum EventCommand {
    DeadLetters,
    Replay { consumer_id: Uuid, event_id: Uuid },
}
