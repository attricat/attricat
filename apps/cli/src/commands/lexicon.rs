use std::{fs, path::PathBuf};

use clap::{Args, Subcommand};
use reqwest::Method;
use serde_json::{Value, json};

use crate::error::CliError;
use crate::transport::{Api, with_query};

#[derive(Args)]
pub(crate) struct LexiconIdentity {
    /// English source text, as written inside `{{…}}`.
    #[arg(long)]
    pub(crate) key: String,
    /// Disambiguation context, as written after `|`.
    #[arg(long)]
    pub(crate) context: Option<String>,
    /// BCP 47 language tag, such as `pl`.
    #[arg(long)]
    pub(crate) language: String,
    /// CLDR plural category: zero, one, two, few, many, or other.
    #[arg(long, default_value = "other")]
    pub(crate) plural_category: String,
}

#[derive(Subcommand)]
pub(crate) enum LexiconCommand {
    /// List entries, optionally for one language.
    List {
        #[arg(long)]
        language: Option<String>,
    },
    /// Create or replace one entry.
    Set {
        #[command(flatten)]
        identity: LexiconIdentity,
        #[arg(long)]
        text: String,
    },
    /// Delete one entry.
    Delete {
        #[command(flatten)]
        identity: LexiconIdentity,
    },
    /// Print one language as an importable lexicon file.
    Export {
        #[arg(long)]
        language: String,
    },
    /// Import a JSON or TOML lexicon file for one language.
    Import {
        #[arg(long)]
        file: PathBuf,
        /// Delete entries of the file's language that the file does not contain.
        #[arg(long)]
        replace: bool,
    },
    /// Report untranslated references, missing plural forms, and orphaned entries.
    Report {
        /// Language to report; repeat for several. Defaults to `en` and every
        /// language with entries.
        #[arg(long)]
        language: Vec<String>,
    },
}

pub(crate) fn lexicon_request(
    command: LexiconCommand,
) -> Result<(Method, String, Option<Value>), CliError> {
    Ok(match command {
        LexiconCommand::List { language } => (
            Method::GET,
            with_query(ENTRIES_PATH, [("language", language)]),
            None,
        ),
        LexiconCommand::Set { identity, text } => (
            Method::PUT,
            ENTRIES_PATH.to_owned(),
            Some(json!({
                "key": identity.key,
                "context": identity.context,
                "language": identity.language,
                "plural_category": identity.plural_category,
                "text": text,
            })),
        ),
        LexiconCommand::Delete { identity } => (
            Method::DELETE,
            with_query(
                ENTRIES_PATH,
                [
                    ("key", Some(identity.key)),
                    ("context", identity.context),
                    ("language", Some(identity.language)),
                    ("plural_category", Some(identity.plural_category)),
                ],
            ),
            None,
        ),
        LexiconCommand::Export { language } => (
            Method::GET,
            with_query("/lexicon/export", [("language", Some(language))]),
            None,
        ),
        LexiconCommand::Import { file, replace } => {
            let mode = if replace {
                ImportMode::Replace
            } else {
                ImportMode::Merge
            };
            (
                Method::POST,
                with_query(
                    "/lexicon/import",
                    [("mode", Some(mode.as_str().to_owned()))],
                ),
                Some(lexicon_file(&file)?),
            )
        }
        LexiconCommand::Report { language } => (
            Method::GET,
            with_query(
                "/lexicon/report",
                [(
                    "languages",
                    (!language.is_empty()).then(|| language.join(",")),
                )],
            ),
            None,
        ),
    })
}

const ENTRIES_PATH: &str = "/lexicon/entries";

/// How an imported lexicon file combines with existing entries.
enum ImportMode {
    /// Keep entries the file does not contain.
    Merge,
    /// Delete entries of the file's language that it does not contain.
    Replace,
}

impl ImportMode {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Merge => "merge",
            Self::Replace => "replace",
        }
    }
}

/// Reads a lexicon file as JSON, or as TOML when the extension is `.toml`.
pub(crate) fn lexicon_file(path: &PathBuf) -> Result<Value, CliError> {
    let source = fs::read_to_string(path).map_err(|error| CliError::Input(error.to_string()))?;
    if path
        .extension()
        .is_some_and(|extension| extension == "toml")
    {
        let value: toml::Value = toml::from_str(&source)
            .map_err(|error| CliError::Input(format!("invalid lexicon TOML: {error}")))?;
        serde_json::to_value(value).map_err(|error| CliError::Input(error.to_string()))
    } else {
        serde_json::from_str(&source)
            .map_err(|error| CliError::Input(format!("invalid lexicon JSON: {error}")))
    }
}

pub(crate) async fn lexicon_command(
    api: &Api,
    command: LexiconCommand,
) -> Result<String, CliError> {
    let (method, path, body) = lexicon_request(command)?;
    api.send(method, &path, body).await
}
