//! The text of PDF, Word and Excel files as the agent and smart fill read it.
//!
//! The file worker stores each document's extracted text. A file it has not
//! reached yet, or extracted with an older extractor, is extracted here on
//! demand and not stored; the worker's job stores it later.

use std::{collections::HashMap, sync::Arc};

use attricat_document_text::{Document, EXTRACTOR_VERSION, Section, SectionKind};
use attricat_domain::states::TextExtractionFailure;
use attricat_workers::document_text::extract_text;
use uuid::Uuid;

use crate::{
    repository::{AttricatRepository, FileObject, FileTextOutcome, RepositoryError},
    storage::ObjectStore,
};

/// Labels listed in a document outline before the rest are only counted.
const MAX_OUTLINE_SECTIONS: usize = 50;

#[derive(Debug, thiserror::Error)]
pub(crate) enum DocumentTextError {
    #[error("the document has no readable text ({})", .0.as_str())]
    Unavailable(TextExtractionFailure),
    #[error("the document could not be read from storage")]
    Storage,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

/// The text of a document file whose original is `file`.
pub(crate) async fn load(
    repository: &AttricatRepository,
    object_store: &dyn ObjectStore,
    file_id: Uuid,
    file: &FileObject,
) -> Result<Document, DocumentTextError> {
    match repository.file_text_extraction(file_id).await? {
        Some(extraction) if extraction.extractor_version >= EXTRACTOR_VERSION => {
            match extraction.outcome {
                FileTextOutcome::Ready { object_key } => {
                    let object = object_store
                        .get(&object_key)
                        .await
                        .map_err(|_| DocumentTextError::Storage)?;
                    // Stored text that no longer parses is extracted again.
                    if let Ok(document) = serde_json::from_slice::<Document>(&object.bytes) {
                        return Ok(document);
                    }
                }
                FileTextOutcome::Failed(failure) => {
                    return Err(DocumentTextError::Unavailable(failure));
                }
            }
        }
        _ => {}
    }
    let original = object_store
        .get(&file.object_key)
        .await
        .map_err(|_| DocumentTextError::Storage)?;
    extract_text(file.mime_type.clone(), original.bytes)
        .await
        .map_err(DocumentTextError::Unavailable)
}

/// Documents a run has already loaded, by file ID. Conversation history is
/// converted once per run, and each `read_file` window of a document would
/// otherwise load, or extract, the whole document again.
#[derive(Default)]
pub(crate) struct DocumentCache(HashMap<Uuid, Arc<Document>>);

impl DocumentCache {
    /// The text of a document file, loaded at most once per cache. Failures
    /// are not cached.
    pub(crate) async fn load(
        &mut self,
        repository: &AttricatRepository,
        object_store: &dyn ObjectStore,
        file_id: Uuid,
        file: &FileObject,
    ) -> Result<Arc<Document>, DocumentTextError> {
        if let Some(document) = self.0.get(&file_id) {
            return Ok(document.clone());
        }
        let document = Arc::new(load(repository, object_store, file_id, file).await?);
        self.0.insert(file_id, document.clone());
        Ok(document)
    }
}

/// The sections `selector` names: a page number, a page range such as `3-5`,
/// or a sheet name. Returns the available labels when nothing matches.
pub(crate) fn select<'a>(
    document: &'a Document,
    selector: &str,
) -> Result<Vec<&'a Section>, String> {
    let selector = selector.trim();
    let pages = selector
        .split_once('-')
        .map_or(Some((selector, selector)), Some)
        .and_then(|(first, last)| {
            Some((
                first.trim().parse::<u32>().ok()?,
                last.trim().parse::<u32>().ok()?,
            ))
        });
    let selected = document
        .sections
        .iter()
        .filter(|section| match (section.kind, pages) {
            (SectionKind::Page, Some((first, last))) => section
                .label
                .parse::<u32>()
                .is_ok_and(|page| (first..=last).contains(&page)),
            _ => section.label.eq_ignore_ascii_case(selector),
        })
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(format!(
            "section '{selector}' does not exist; available sections: {}",
            outline(document)
        ));
    }
    Ok(selected)
}

/// Sections as one text, each headed by its page number or sheet name.
pub(crate) fn render<'a>(sections: impl IntoIterator<Item = &'a Section>) -> String {
    sections
        .into_iter()
        .map(|section| match section.kind {
            SectionKind::Page => format!("--- Page {} ---\n{}", section.label, section.text),
            SectionKind::Sheet => format!("## Sheet: {}\n{}", section.label, section.text),
            SectionKind::Body => section.text.clone(),
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// The sections of a document with their sizes, for a reader choosing what
/// to read.
pub(crate) fn outline(document: &Document) -> String {
    let mut entries = document
        .sections
        .iter()
        .take(MAX_OUTLINE_SECTIONS)
        .map(|section| {
            let kind = match section.kind {
                SectionKind::Page => "page",
                SectionKind::Sheet => "sheet",
                SectionKind::Body => "body",
            };
            format!(
                "{kind} {} ({} characters)",
                section.label,
                section.text.chars().count()
            )
        })
        .collect::<Vec<_>>();
    if document.sections.len() > MAX_OUTLINE_SECTIONS {
        entries.push(format!(
            "and {} more",
            document.sections.len() - MAX_OUTLINE_SECTIONS
        ));
    }
    let mut outline = entries.join(", ");
    if document.truncated {
        outline.push_str("; the extracted text was cut at a size limit");
    }
    outline
}

/// A character range of a text.
pub(crate) struct Window {
    pub text: String,
    /// Character offset just past the window.
    pub end: usize,
    pub total: usize,
}

/// At most `max_chars` characters and `max_bytes` bytes of `text`, starting
/// at character `offset`.
pub(crate) fn window(text: &str, offset: usize, max_chars: usize, max_bytes: usize) -> Window {
    let total = text.chars().count();
    let mut bytes = 0;
    let selected = text
        .chars()
        .skip(offset)
        .take(max_chars)
        .take_while(|character| {
            bytes += character.len_utf8();
            bytes <= max_bytes
        })
        .collect::<String>();
    let end = offset.min(total) + selected.chars().count();
    Window {
        text: selected,
        end,
        total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document() -> Document {
        let page = |label: &str, text: &str| Section {
            kind: SectionKind::Page,
            label: label.to_owned(),
            text: text.to_owned(),
        };
        Document {
            extractor_version: EXTRACTOR_VERSION,
            sections: vec![page("1", "one"), page("2", "two"), page("3", "three")],
            truncated: false,
        }
    }

    #[test]
    fn selectors_name_pages_ranges_and_sheets() {
        let document = document();
        let labels = |selector| {
            select(&document, selector)
                .unwrap()
                .iter()
                .map(|section| section.label.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(labels("2"), ["2"]);
        assert_eq!(labels("2-3"), ["2", "3"]);
        assert!(
            select(&document, "9")
                .unwrap_err()
                .contains("page 1 (3 characters)")
        );

        let sheets = Document {
            sections: vec![Section {
                kind: SectionKind::Sheet,
                label: "Prices".to_owned(),
                text: "SKU".to_owned(),
            }],
            ..document
        };
        assert_eq!(select(&sheets, "prices").unwrap().len(), 1);
    }

    #[test]
    fn rendering_heads_each_section() {
        assert_eq!(
            render(&document().sections[..2]),
            "--- Page 1 ---\none\n\n--- Page 2 ---\ntwo"
        );
    }

    #[test]
    fn windows_respect_characters_and_bytes() {
        let window_of = |offset, chars, bytes| {
            let window = window("ab€cd", offset, chars, bytes);
            (window.text, window.end, window.total)
        };
        assert_eq!(window_of(0, 2, 100), ("ab".to_owned(), 2, 5));
        assert_eq!(window_of(1, 10, 3), ("b".to_owned(), 2, 5));
        assert_eq!(window_of(9, 10, 100), (String::new(), 5, 5));
    }
}
