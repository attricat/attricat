//! PDF documents: one section per page, in page order.

use std::panic::{AssertUnwindSafe, catch_unwind};

use lopdf::{DecompressError, Document, LoadOptions};

use crate::{ExtractError, Limits, Output, SectionKind};

pub(crate) fn extract(
    bytes: &[u8],
    limits: &Limits,
    output: &mut Output,
) -> Result<(), ExtractError> {
    // The parser panics on some damaged files; treat that as damage, not a crash.
    let (pages, dropped) = catch_unwind(AssertUnwindSafe(|| page_texts(bytes, limits)))
        .map_err(|_| ExtractError::Malformed)??;
    if dropped {
        output.mark_truncated();
    }
    for (number, text) in pages {
        if output.is_full() {
            output.mark_truncated();
            break;
        }
        output.push(SectionKind::Page, number.to_string(), text);
    }
    Ok(())
}

/// The text of each page, up to the page limit and the output budget, and
/// whether a limit left text unread.
fn page_texts(bytes: &[u8], limits: &Limits) -> Result<(Vec<(u32, String)>, bool), ExtractError> {
    // Object and cross-reference streams are decompressed while loading, so
    // they are bounded here rather than only when a page is read.
    let options = LoadOptions::with_max_decompressed_size(limits.max_pdf_page_stream_bytes);
    let document = Document::load_mem_with_options(bytes, options).map_err(|error| {
        if is_limit_error(&error) {
            ExtractError::TooLarge
        } else {
            ExtractError::Malformed
        }
    })?;
    // Files that only restrict printing or copying open with the empty
    // password; any other encrypted file stays unreadable.
    if document.is_encrypted() && document.encryption_state.is_none() {
        return Err(ExtractError::Encrypted);
    }
    let page_numbers = document.get_pages().into_keys().collect::<Vec<_>>();
    let mut dropped = page_numbers.len() > limits.max_pdf_pages;
    let mut pages = Vec::new();
    let mut chars = 0_usize;
    // The first chunk error seen, so a PDF whose pages all fail to read is not
    // mistaken for one without a text layer.
    let mut chunk_error = None;
    for number in page_numbers.into_iter().take(limits.max_pdf_pages) {
        if chars >= limits.max_output_chars {
            dropped = true;
            break;
        }
        // One unreadable page does not hide the rest of the document, and
        // one unreadable font does not hide the rest of its page.
        let mut text = String::new();
        for chunk in
            document.extract_text_chunks_with_limit(&[number], limits.max_pdf_page_stream_bytes)
        {
            match chunk {
                Ok(chunk) => text.push_str(&chunk),
                Err(error) => {
                    let error = if is_limit_error(&error) {
                        dropped = true;
                        ExtractError::TooLarge
                    } else {
                        ExtractError::Malformed
                    };
                    chunk_error.get_or_insert(error);
                }
            }
        }
        let text = text.trim().to_owned();
        chars += text.chars().count();
        pages.push((number, text));
    }
    if let Some(error) = chunk_error
        && pages.iter().all(|(_, text)| text.is_empty())
    {
        return Err(error);
    }
    Ok((pages, dropped))
}

fn is_limit_error(error: &lopdf::Error) -> bool {
    matches!(
        error,
        lopdf::Error::Decompress(DecompressError::MemoryLimitExceeded { .. })
    )
}
