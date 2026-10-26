use std::{fs, path::PathBuf};

use clap::{Args, Subcommand};
use reqwest::Method;
use serde_json::{Value, json};

use crate::error::CliError;

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
    let identity_query = |identity: &LexiconIdentity| {
        let mut query = url::form_urlencoded::Serializer::new(String::new());
        query.append_pair("key", &identity.key);
        if let Some(context) = &identity.context {
            query.append_pair("context", context);
        }
        query.append_pair("language", &identity.language);
        query.append_pair("plural_category", &identity.plural_category);
        query.finish()
    };
    let language_query = |name: &str, language: &str| {
        let mut query = url::form_urlencoded::Serializer::new(String::new());
        query.append_pair(name, language);
        query.finish()
    };
    Ok(match command {
        LexiconCommand::List { language } => (
            Method::GET,
            match language {
                Some(language) => {
                    format!("/lexicon/entries?{}", language_query("language", &language))
                }
                None => "/lexicon/entries".to_owned(),
            },
            None,
        ),
        LexiconCommand::Set { identity, text } => (
            Method::PUT,
            "/lexicon/entries".to_owned(),
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
            format!("/lexicon/entries?{}", identity_query(&identity)),
            None,
        ),
        LexiconCommand::Export { language } => (
            Method::GET,
            format!("/lexicon/export?{}", language_query("language", &language)),
            None,
        ),
        LexiconCommand::Import { file, replace } => (
            Method::POST,
            format!(
                "/lexicon/import?mode={}",
                if replace { "replace" } else { "merge" }
            ),
            Some(lexicon_file(&file)?),
        ),
        LexiconCommand::Report { language } => (
            Method::GET,
            if language.is_empty() {
                "/lexicon/report".to_owned()
            } else {
                format!(
                    "/lexicon/report?{}",
                    language_query("languages", &language.join(","))
                )
            },
            None,
        ),
    })
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
