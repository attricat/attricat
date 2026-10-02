use std::collections::HashSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    plural::{DEFAULT_PLURAL_CATEGORY, PLURAL_CATEGORIES, canonical_language, plural_categories},
    reference::normalize,
};

pub const MAX_KEY_CHARS: usize = 200;
pub const MAX_CONTEXT_CHARS: usize = 100;
pub const MAX_TEXT_CHARS: usize = 1_000;
/// Upper bound for one import or one solution-pack lexicon file.
pub const MAX_FILE_ENTRIES: usize = 10_000;
pub const LEXICON_FILE_FORMAT_VERSION: u32 = 1;

/// One translation. Identity is `(key, context, language, plural_category)`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Entry {
    pub key: String,
    pub context: Option<String>,
    pub language: String,
    pub plural_category: String,
    pub text: String,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EntryError {
    #[error("lexicon key {0}")]
    InvalidKey(&'static str),
    #[error("lexicon context {0}")]
    InvalidContext(&'static str),
    #[error("unsupported lexicon language '{0}'")]
    UnsupportedLanguage(String),
    #[error("plural category '{category}' is not used by language '{language}'")]
    InvalidPluralCategory { category: String, language: String },
    #[error("lexicon text {0}")]
    InvalidText(&'static str),
    #[error("unsupported lexicon file format version {0}")]
    UnsupportedFormatVersion(u32),
    #[error("lexicon file has more than {MAX_FILE_ENTRIES} entries")]
    TooManyEntries,
    #[error("duplicate lexicon entry for key '{key}' and plural category '{plural_category}'")]
    Duplicate {
        key: String,
        plural_category: String,
    },
}

fn validate_term(value: &str, max_chars: usize) -> Result<String, &'static str> {
    let value = normalize(value);
    if value.is_empty() {
        return Err("must not be empty");
    }
    if value.chars().count() > max_chars {
        return Err("is too long");
    }
    if value.contains(['{', '}', '|']) || value.chars().any(char::is_control) {
        return Err("must not contain braces, '|', or control characters");
    }
    Ok(value)
}

/// A normalized entry identity: `(key, context, language, plural_category)`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct EntryIdentity {
    pub key: String,
    pub context: Option<String>,
    pub language: String,
    pub plural_category: String,
}

impl EntryIdentity {
    /// Normalizes and validates an identity. A blank context means none.
    pub fn validated(
        key: &str,
        context: Option<&str>,
        language: &str,
        plural_category: &str,
    ) -> Result<Self, EntryError> {
        let key = validate_term(key, MAX_KEY_CHARS).map_err(EntryError::InvalidKey)?;
        let context = context
            .filter(|context| !context.trim().is_empty())
            .map(|context| validate_term(context, MAX_CONTEXT_CHARS))
            .transpose()
            .map_err(EntryError::InvalidContext)?;
        let language = canonical_language(language)
            .ok_or_else(|| EntryError::UnsupportedLanguage(language.to_owned()))?;
        if !plural_categories(&language)
            .unwrap_or_default()
            .contains(&plural_category)
        {
            return Err(EntryError::InvalidPluralCategory {
                category: plural_category.to_owned(),
                language,
            });
        }
        Ok(Self {
            key,
            context,
            language,
            plural_category: plural_category.to_owned(),
        })
    }
}

impl Entry {
    /// Normalizes and validates every field. A blank context means none.
    pub fn validated(self) -> Result<Self, EntryError> {
        let EntryIdentity {
            key,
            context,
            language,
            plural_category,
        } = EntryIdentity::validated(
            &self.key,
            self.context.as_deref(),
            &self.language,
            &self.plural_category,
        )?;
        if self.text.trim().is_empty() {
            return Err(EntryError::InvalidText("must not be empty"));
        }
        if self.text.chars().count() > MAX_TEXT_CHARS {
            return Err(EntryError::InvalidText("is too long"));
        }
        if self
            .text
            .chars()
            .any(|character| character.is_control() && character != '\n')
        {
            return Err(EntryError::InvalidText(
                "must not contain control characters",
            ));
        }
        Ok(Self {
            key,
            context,
            language,
            plural_category,
            text: self.text,
        })
    }
}

/// The exchange format for one language, used by lexicon import and export
/// (as JSON or TOML). Solution-pack lexicon files reuse its entry shape.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[schemars(title = "Attricat lexicon v1")]
pub struct LexiconFile {
    /// Lexicon file format version. Only `1` is supported.
    #[schemars(extend("const" = 1))]
    pub format_version: u32,
    /// BCP 47 language tag of every entry, such as `pl` or `pt-BR`.
    pub language: String,
    /// Translations for the language.
    #[schemars(length(max = MAX_FILE_ENTRIES))]
    pub entries: Vec<LexiconFileEntry>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LexiconFileEntry {
    /// English source text referenced as `{{key}}`.
    #[schemars(length(min = 1, max = MAX_KEY_CHARS))]
    pub key: String,
    /// Disambiguation context referenced as `{{key|context}}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = MAX_CONTEXT_CHARS))]
    pub context: Option<String>,
    /// CLDR plural category. Defaults to `other`, used for text without a count.
    #[serde(
        default = "default_plural_category",
        skip_serializing_if = "is_default_plural_category"
    )]
    #[schemars(extend("enum" = PLURAL_CATEGORIES))]
    pub plural_category: String,
    /// Displayed text. It is rendered literally.
    #[schemars(length(min = 1, max = MAX_TEXT_CHARS))]
    pub text: String,
}

fn default_plural_category() -> String {
    DEFAULT_PLURAL_CATEGORY.to_owned()
}

fn is_default_plural_category(category: &String) -> bool {
    category == DEFAULT_PLURAL_CATEGORY
}

/// The JSON Schema for lexicon import/export files, published as
/// `contracts/lexicon-v1.schema.json`.
pub fn lexicon_file_json_schema() -> serde_json::Value {
    schemars::generate::SchemaSettings::draft2020_12()
        .into_generator()
        .into_root_schema_for::<LexiconFile>()
        .to_value()
}

impl LexiconFile {
    /// Validates the file and returns its normalized entries. Entries whose
    /// identities collide after normalization are rejected.
    pub fn into_entries(self) -> Result<Vec<Entry>, EntryError> {
        if self.format_version != LEXICON_FILE_FORMAT_VERSION {
            return Err(EntryError::UnsupportedFormatVersion(self.format_version));
        }
        if self.entries.len() > MAX_FILE_ENTRIES {
            return Err(EntryError::TooManyEntries);
        }
        let mut seen = HashSet::new();
        self.entries
            .into_iter()
            .map(|entry| {
                let entry = Entry {
                    key: entry.key,
                    context: entry.context,
                    language: self.language.clone(),
                    plural_category: entry.plural_category,
                    text: entry.text,
                }
                .validated()?;
                if !seen.insert((
                    entry.key.clone(),
                    entry.context.clone(),
                    entry.plural_category.clone(),
                )) {
                    return Err(EntryError::Duplicate {
                        key: entry.key,
                        plural_category: entry.plural_category,
                    });
                }
                Ok(entry)
            })
            .collect()
    }

    /// Builds an export file from entries of one language.
    pub fn from_entries(language: String, entries: impl IntoIterator<Item = Entry>) -> Self {
        Self {
            format_version: LEXICON_FILE_FORMAT_VERSION,
            language,
            entries: entries
                .into_iter()
                .map(|entry| LexiconFileEntry {
                    key: entry.key,
                    context: entry.context,
                    plural_category: entry.plural_category,
                    text: entry.text,
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, language: &str, plural_category: &str) -> Entry {
        Entry {
            key: key.to_owned(),
            context: Some("  ".to_owned()),
            language: language.to_owned(),
            plural_category: plural_category.to_owned(),
            text: "Produkt".to_owned(),
        }
    }

    #[test]
    fn validates_and_normalizes_entries() {
        let valid = entry("  Product \n family ", "PL", "few")
            .validated()
            .unwrap();
        assert_eq!(valid.key, "Product family");
        assert_eq!(valid.context, None);
        assert_eq!(valid.language, "pl");
        assert!(matches!(
            entry("Product", "en", "few").validated(),
            Err(EntryError::InvalidPluralCategory { .. })
        ));
        assert!(matches!(
            entry("Product", "xx", "other").validated(),
            Err(EntryError::UnsupportedLanguage(_))
        ));
        assert!(matches!(
            entry("{{Product}}", "en", "other").validated(),
            Err(EntryError::InvalidKey(_))
        ));
        assert!(matches!(
            entry("Order|purchase", "en", "other").validated(),
            Err(EntryError::InvalidKey(_))
        ));
    }

    #[test]
    fn files_reject_duplicate_normalized_identities() {
        let file: LexiconFile = serde_json::from_value(serde_json::json!({
            "format_version": 1,
            "language": "pl",
            "entries": [
                {"key": "Product", "text": "Produkt"},
                {"key": " Product ", "text": "Towar"},
            ],
        }))
        .unwrap();
        assert!(matches!(
            file.into_entries(),
            Err(EntryError::Duplicate { .. })
        ));
    }
}
