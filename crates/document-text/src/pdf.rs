//! PDF documents: one section per page, in page order.

use std::panic::{AssertUnwindSafe, catch_unwind};

use lopdf::Document;

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
/// whether either limit left pages unread.
fn page_texts(bytes: &[u8], limits: &Limits) -> Result<(Vec<(u32, String)>, bool), ExtractError> {
    let document = Document::load_mem(bytes).map_err(|_| ExtractError::Malformed)?;
    // Files that only restrict printing or copying open with the empty
    // password; any other encrypted file stays unreadable.
    if document.is_encrypted() && document.encryption_state.is_none() {
        return Err(ExtractError::Encrypted);
    }
    let page_numbers = document.get_pages().into_keys().collect::<Vec<_>>();
    let mut dropped = page_numbers.len() > limits.max_pdf_pages;
    let mut pages = Vec::new();
    let mut chars = 0_usize;
    for number in page_numbers.into_iter().take(limits.max_pdf_pages) {
        if chars >= limits.max_output_chars {
            dropped = true;
            break;
        }
        // One unreadable page does not hide the rest of the document.
        let Ok(text) =
            document.extract_text_with_limit(&[number], limits.max_pdf_page_stream_bytes)
        else {
            continue;
        };
        let text = text.trim().to_owned();
        chars += text.chars().count();
        pages.push((number, text));
    }
    Ok((pages, dropped))
}
