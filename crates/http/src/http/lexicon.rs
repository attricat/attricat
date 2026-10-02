use axum::{Json, http::StatusCode};
use catalog_lexicon::{
    DEFAULT_PLURAL_CATEGORY, Entry, EntryIdentity, LexiconFile, Report, canonical_language,
};
use serde::Deserialize;

use super::{
    auth::ScopedRepository,
    error::ApiError,
    extractors::{ApiJson, ApiQuery},
};
use crate::repository::{LexiconEntry, LexiconImportMode, LexiconImportSummary};

/// Enough for a full-size import file of long entries.
pub(super) const MAX_IMPORT_BYTES: usize = 16 * 1024 * 1024;
/// Upper bound for `languages` in a report request.
const MAX_REPORT_LANGUAGES: usize = 32;

#[derive(Deserialize)]
pub(super) struct LanguageQuery {
    language: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct ExportQuery {
    language: String,
}

#[derive(Deserialize)]
pub(super) struct ReportQuery {
    /// Comma-separated BCP 47 tags. Defaults to `en` plus every language with
    /// entries.
    #[serde(default)]
    languages: String,
}

#[derive(Deserialize)]
pub(super) struct ImportQuery {
    #[serde(default = "default_import_mode")]
    mode: LexiconImportMode,
}

fn default_import_mode() -> LexiconImportMode {
    LexiconImportMode::Merge
}

fn default_plural_category() -> String {
    DEFAULT_PLURAL_CATEGORY.to_owned()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EntryInput {
    key: String,
    context: Option<String>,
    language: String,
    #[serde(default = "default_plural_category")]
    plural_category: String,
    text: String,
}

#[derive(Deserialize)]
pub(super) struct IdentityQuery {
    key: String,
    context: Option<String>,
    language: String,
    #[serde(default = "default_plural_category")]
    plural_category: String,
}

fn language(language: &str) -> Result<String, ApiError> {
    canonical_language(language).ok_or_else(|| {
        ApiError::invalid_input(format!("unsupported lexicon language '{language}'"))
    })
}

pub(super) async fn list(
    ScopedRepository(repository): ScopedRepository,
    ApiQuery(query): ApiQuery<LanguageQuery>,
) -> Result<Json<Vec<LexiconEntry>>, ApiError> {
    let language = query.language.as_deref().map(language).transpose()?;
    Ok(Json(
        repository.list_lexicon_entries(language.as_deref()).await?,
    ))
}

pub(super) async fn upsert(
    ScopedRepository(repository): ScopedRepository,
    ApiJson(input): ApiJson<EntryInput>,
) -> Result<Json<LexiconEntry>, ApiError> {
    Ok(Json(
        repository
            .upsert_lexicon_entry(Entry {
                key: input.key,
                context: input.context,
                language: input.language,
                plural_category: input.plural_category,
                text: input.text,
            })
            .await?,
    ))
}

pub(super) async fn delete(
    ScopedRepository(repository): ScopedRepository,
    ApiQuery(query): ApiQuery<IdentityQuery>,
) -> Result<StatusCode, ApiError> {
    let identity = EntryIdentity::validated(
        &query.key,
        query.context.as_deref(),
        &query.language,
        &query.plural_category,
    )
    .map_err(|error| ApiError::invalid_input(error.to_string()))?;
    if !repository.delete_lexicon_entry(&identity).await? {
        return Err(ApiError::not_found("lexicon entry"));
    }
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn export(
    ScopedRepository(repository): ScopedRepository,
    ApiQuery(query): ApiQuery<ExportQuery>,
) -> Result<Json<LexiconFile>, ApiError> {
    let language = language(&query.language)?;
    let entries = repository.list_lexicon_entries(Some(&language)).await?;
    Ok(Json(LexiconFile::from_entries(
        language,
        entries.into_iter().map(Entry::from),
    )))
}

pub(super) async fn import(
    ScopedRepository(repository): ScopedRepository,
    ApiQuery(query): ApiQuery<ImportQuery>,
    ApiJson(file): ApiJson<LexiconFile>,
) -> Result<Json<LexiconImportSummary>, ApiError> {
    let language = language(&file.language)?;
    let entries = file
        .into_entries()
        .map_err(|error| ApiError::invalid_input(error.to_string()))?;
    Ok(Json(
        repository
            .import_lexicon_entries(&language, &entries, query.mode)
            .await?,
    ))
}

pub(super) async fn report(
    ScopedRepository(repository): ScopedRepository,
    ApiQuery(query): ApiQuery<ReportQuery>,
) -> Result<Json<Report>, ApiError> {
    let languages = query
        .languages
        .split(',')
        .map(str::trim)
        .filter(|language| !language.is_empty())
        .map(language)
        .collect::<Result<Vec<_>, _>>()?;
    if languages.len() > MAX_REPORT_LANGUAGES {
        return Err(ApiError::invalid_input(format!(
            "a report can include at most {MAX_REPORT_LANGUAGES} languages"
        )));
    }
    Ok(Json(repository.lexicon_report(&languages).await?))
}
