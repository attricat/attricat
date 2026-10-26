use clap::{Subcommand, ValueEnum};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::error::CliError;

#[derive(Subcommand)]
pub(crate) enum SavedViewCommand {
    List {
        /// Only list views whose name or description contains this text.
        #[arg(long)]
        query: Option<String>,
    },
    Get {
        id: Uuid,
    },
    Create {
        #[arg(long)]
        name: String,
        /// Explorer state JSON object, or a file containing it.
        #[arg(long)]
        state: String,
        #[arg(long, value_enum, default_value_t = SavedViewVisibility::Private)]
        visibility: SavedViewVisibility,
        #[arg(long, default_value = "")]
        description: String,
    },
    /// Change the given fields and keep the others.
    #[command(group(clap::ArgGroup::new("change").required(true).multiple(true).args(["name", "state", "visibility", "description"])))]
    Update {
        id: Uuid,
        #[arg(long)]
        name: Option<String>,
        /// Explorer state JSON object, or a file containing it.
        #[arg(long)]
        state: Option<String>,
        #[arg(long, value_enum)]
        visibility: Option<SavedViewVisibility>,
        #[arg(long)]
        description: Option<String>,
    },
    Delete {
        id: Uuid,
    },
    /// Create an unnamed short URL snapshot of an Explorer search.
    Link {
        #[arg(long)]
        state: String,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum SavedViewVisibility {
    Private,
    Workspace,
}

impl SavedViewVisibility {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Workspace => "workspace",
        }
    }
}

/// The only saved-view kind the API stores.
pub(crate) const SAVED_VIEW_KIND: &str = "explorer_search";

/// The saved view `acli saved-view update` sends: the current view with the
/// given changes, since the API replaces every field.
pub(crate) fn changed_saved_view(
    current: &Value,
    name: Option<String>,
    description: Option<String>,
    visibility: Option<SavedViewVisibility>,
    state: Option<Value>,
) -> Result<Value, CliError> {
    let field = |key: &str| current.get(key).cloned().ok_or(CliError::InvalidResponse);
    Ok(json!({
        "kind": field("kind")?,
        "name": name.map_or_else(|| field("name"), |name| Ok(json!(name)))?,
        "description": description.map_or_else(|| field("description"), |text| Ok(json!(text)))?,
        "visibility": visibility.map_or_else(|| field("visibility"), |value| Ok(json!(value.as_str())))?,
        "state": state.map_or_else(|| field("state"), Ok)?,
    }))
}
