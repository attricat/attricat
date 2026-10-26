use clap::{Subcommand, ValueEnum};
use serde_json::{Value, json};
use url::Url;
use uuid::Uuid;

use crate::error::CliError;
use crate::input::json_object_input;
use crate::transport::{Api, segment, with_query};

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
    #[command(group(
        clap::ArgGroup::new("change")
            .required(true)
            .multiple(true)
            .args(["name", "state", "visibility", "description"])
    ))]
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
        "visibility": visibility
            .map_or_else(|| field("visibility"), |value| Ok(json!(value.as_str())))?,
        "state": state.map_or_else(|| field("state"), Ok)?,
    }))
}

pub(crate) async fn saved_view_command(
    api: &Api,
    command: SavedViewCommand,
) -> Result<String, CliError> {
    match command {
        SavedViewCommand::List { query } => {
            api.get(&with_query("/saved-views", [("q", query)])).await
        }
        SavedViewCommand::Get { id } => api.get(&format!("/saved-views/{}", segment(id))).await,
        SavedViewCommand::Create {
            name,
            state,
            visibility,
            description,
        } => {
            let body = json!({
                "kind": SAVED_VIEW_KIND,
                "name": name,
                "description": description,
                "visibility": visibility.as_str(),
                "state": json_object_input(&state, "--state")?,
            });
            with_web_url(api.post_json("/saved-views", body).await?, "savedView")
        }
        SavedViewCommand::Update {
            id,
            name,
            state,
            visibility,
            description,
        } => {
            let path = format!("/saved-views/{}", segment(id));
            let current = api.get_value(&path).await?;
            let state = state
                .map(|state| json_object_input(&state, "--state"))
                .transpose()?;
            let body = changed_saved_view(&current, name, description, visibility, state)?;
            with_web_url(api.put_json(&path, body).await?, "savedView")
        }
        SavedViewCommand::Delete { id } => {
            api.delete(&format!("/saved-views/{}", segment(id))).await
        }
        SavedViewCommand::Link { state } => {
            let body =
                json!({ "kind": SAVED_VIEW_KIND, "state": json_object_input(&state, "--state")? });
            with_web_url(api.post_json("/view-state-links", body).await?, "viewState")
        }
    }
}

/// Adds the Explorer `url` that opens a saved view or link when
/// `ATTRICAT_WEB_URL` is set; `key` is the web app's query parameter.
fn with_web_url(body: String, key: &str) -> Result<String, CliError> {
    let Ok(web) = std::env::var("ATTRICAT_WEB_URL") else {
        return Ok(body);
    };
    let mut output: Value = serde_json::from_str(&body).map_err(|_| CliError::InvalidResponse)?;
    let Some(id) = output.get("id").and_then(Value::as_str).map(str::to_owned) else {
        return Ok(body);
    };
    let mut url = Url::parse(&web)
        .map_err(|error| CliError::Input(format!("invalid ATTRICAT_WEB_URL: {error}")))?;
    url.set_path("/");
    url.set_query(Some(&format!("{key}={id}")));
    output["url"] = json!(url.to_string());
    Ok(output.to_string())
}
