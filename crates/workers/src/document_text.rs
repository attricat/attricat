//! Runs document text extraction off the async runtime under a deadline. The
//! file worker stores the result; the agent runtime calls this directly for
//! files the worker has not reached yet.

use std::time::Duration;

use attricat_document_text::{Document, ExtractError, Limits, extract};
use attricat_domain::states::TextExtractionFailure;
use bytes::Bytes;

/// How long one document may take to extract. A parser past its deadline
/// keeps its blocking thread until it returns, but nobody waits for it.
pub const TEXT_EXTRACTION_TIMEOUT: Duration = Duration::from_secs(60);

/// Extracts a document's text on a blocking thread.
pub async fn extract_text(mime: String, bytes: Bytes) -> Result<Document, TextExtractionFailure> {
    let extraction = tokio::task::spawn_blocking(move || extract(&mime, &bytes, &Limits::DEFAULT));
    match tokio::time::timeout(TEXT_EXTRACTION_TIMEOUT, extraction).await {
        Ok(Ok(result)) => result.map_err(failure),
        // The parser panicked outside its own guard.
        Ok(Err(_)) => Err(TextExtractionFailure::Malformed),
        Err(_) => Err(TextExtractionFailure::TimedOut),
    }
}

fn failure(error: ExtractError) -> TextExtractionFailure {
    match error {
        ExtractError::Encrypted => TextExtractionFailure::Encrypted,
        ExtractError::NoTextLayer => TextExtractionFailure::NoTextLayer,
        ExtractError::TooLarge => TextExtractionFailure::TooLarge,
        ExtractError::Malformed => TextExtractionFailure::Malformed,
        ExtractError::Unsupported => TextExtractionFailure::Unsupported,
    }
}
