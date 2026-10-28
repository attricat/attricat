//! Plain-text extraction from uploaded PDF, Word and Excel documents.
//!
//! Extraction is pure and synchronous: it works on complete in-memory bytes and
//! performs no I/O, so callers decide where it runs (a blocking thread with a
//! deadline) and whether the result is cached or stored. Every resource an
//! untrusted document can make the parser spend is bounded by [`Limits`].

mod archive;
mod docx;
mod pdf;
mod xlsx;

use attricat_validation::files::{DOCX_MIME, PDF_MIME, XLSX_MIME};
use serde::{Deserialize, Serialize};

/// Increases whenever extraction output changes, so stored text produced by an
/// older extractor can be recognised and produced again.
pub const EXTRACTOR_VERSION: u32 = 1;

/// Upper bounds on the work one document may cause.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Size of the original document.
    pub max_source_bytes: usize,
    /// Entries in an Office Open XML archive.
    pub max_archive_entries: usize,
    /// Uncompressed size of one archive entry.
    pub max_archive_entry_bytes: u64,
    /// Uncompressed size of all archive entries together.
    pub max_archive_total_bytes: u64,
    /// Characters of extracted text across all sections.
    pub max_output_chars: usize,
    /// Rows read from each worksheet.
    pub max_sheet_rows: usize,
    /// Columns read from each worksheet row.
    pub max_sheet_columns: usize,
    /// Pages read from a PDF.
    pub max_pdf_pages: usize,
    /// Decompressed content-stream bytes of one PDF page.
    pub max_pdf_page_stream_bytes: usize,
}

impl Limits {
    pub const DEFAULT: Self = Self {
        max_source_bytes: 25 * 1024 * 1024,
        max_archive_entries: 10_000,
        max_archive_entry_bytes: 64 * 1024 * 1024,
        max_archive_total_bytes: 256 * 1024 * 1024,
        max_output_chars: 2_000_000,
        max_sheet_rows: 10_000,
        max_sheet_columns: 200,
        max_pdf_pages: 2_000,
        max_pdf_page_stream_bytes: 64 * 1024 * 1024,
    };
}

impl Default for Limits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// A document format text can be extracted from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentFormat {
    Pdf,
    Docx,
    Xlsx,
}

impl DocumentFormat {
    /// The format of a detected upload MIME type, if text can be extracted from it.
    pub fn from_mime(mime: &str) -> Option<Self> {
        match mime {
            PDF_MIME => Some(Self::Pdf),
            DOCX_MIME => Some(Self::Docx),
            XLSX_MIME => Some(Self::Xlsx),
            _ => None,
        }
    }
}

/// The text of a document, split into the parts a reader navigates by.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Document {
    pub extractor_version: u32,
    pub sections: Vec<Section>,
    /// Whether a limit cut the text short.
    pub truncated: bool,
}

impl Document {
    /// Characters across every section.
    pub fn char_count(&self) -> usize {
        self.sections
            .iter()
            .map(|section| section.text.chars().count())
            .sum()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Section {
    pub kind: SectionKind,
    /// The page number, sheet name, or document title a reader uses to ask
    /// for this section.
    pub label: String,
    pub text: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionKind {
    Page,
    Sheet,
    Body,
}

/// Why a document has no text.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ExtractError {
    #[error("the document is password protected")]
    Encrypted,
    #[error("the document has no text layer")]
    NoTextLayer,
    #[error("the document exceeds a processing limit")]
    TooLarge,
    #[error("the document is damaged or not in its declared format")]
    Malformed,
    #[error("text cannot be extracted from this file type")]
    Unsupported,
}

/// Extracts the text of a document of a detected upload MIME type.
pub fn extract(mime: &str, bytes: &[u8], limits: &Limits) -> Result<Document, ExtractError> {
    let format = DocumentFormat::from_mime(mime).ok_or(ExtractError::Unsupported)?;
    if bytes.len() > limits.max_source_bytes {
        return Err(ExtractError::TooLarge);
    }
    let mut output = Output::new(limits.max_output_chars);
    match format {
        DocumentFormat::Pdf => pdf::extract(bytes, limits, &mut output)?,
        DocumentFormat::Docx => docx::extract(bytes, limits, &mut output)?,
        DocumentFormat::Xlsx => xlsx::extract(bytes, limits, &mut output)?,
    }
    // A scanned PDF parses but has no text; an empty Office document is still
    // a valid, empty document.
    if format == DocumentFormat::Pdf
        && output
            .sections
            .iter()
            .all(|section| section.text.trim().is_empty())
    {
        return Err(ExtractError::NoTextLayer);
    }
    Ok(Document {
        extractor_version: EXTRACTOR_VERSION,
        sections: output.sections,
        truncated: output.truncated,
    })
}

/// Sections collected under the shared output-character budget.
pub(crate) struct Output {
    sections: Vec<Section>,
    remaining_chars: usize,
    truncated: bool,
}

impl Output {
    fn new(max_chars: usize) -> Self {
        Self {
            sections: Vec::new(),
            remaining_chars: max_chars,
            truncated: false,
        }
    }

    /// Whether the budget is spent; extractors stop reading once it is.
    pub(crate) fn is_full(&self) -> bool {
        self.remaining_chars == 0
    }

    /// Characters still available to sections.
    pub(crate) fn remaining_chars(&self) -> usize {
        self.remaining_chars
    }

    /// Records that a per-format limit dropped part of the document.
    pub(crate) fn mark_truncated(&mut self) {
        self.truncated = true;
    }

    /// Adds a section, cut to the remaining budget.
    pub(crate) fn push(&mut self, kind: SectionKind, label: String, text: String) {
        if self.is_full() {
            self.truncated = true;
            return;
        }
        let text = match text.char_indices().nth(self.remaining_chars) {
            Some((end, _)) => {
                self.truncated = true;
                self.remaining_chars = 0;
                text[..end].to_owned()
            }
            None => {
                self.remaining_chars -= text.chars().count();
                text
            }
        };
        self.sections.push(Section { kind, label, text });
    }
}
