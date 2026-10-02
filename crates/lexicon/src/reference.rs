use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A `{{key}}` or `{{key|context}}` reference to a workspace lexicon entry.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Reference {
    /// Normalized English source text, also the fallback display text.
    pub key: String,
    /// Disambiguation context (gettext `msgctxt`). It is never displayed.
    pub context: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Segment {
    Literal(String),
    Reference(Reference),
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ReferenceError {
    #[error("lexicon reference is not closed with '}}}}'; write '\\{{{{' for literal braces")]
    Unclosed,
    #[error("lexicon reference has an empty key")]
    EmptyKey,
    #[error("lexicon reference has an empty context")]
    EmptyContext,
    #[error("lexicon reference can have only one '|' context separator")]
    MultipleContexts,
    #[error("lexicon reference cannot contain '{{' or '}}'")]
    Brace,
}

impl ReferenceError {
    /// Stable identifier shared with the web parser's conformance fixtures.
    pub fn code(self) -> &'static str {
        match self {
            Self::Unclosed => "unclosed",
            Self::EmptyKey => "empty_key",
            Self::EmptyContext => "empty_context",
            Self::MultipleContexts => "multiple_contexts",
            Self::Brace => "brace",
        }
    }
}

const OPEN: &str = "{{";
const CLOSE: &str = "}}";
const ESCAPE: char = '\\';
const CONTEXT_SEPARATOR: char = '|';

/// Trims and collapses internal whitespace, the comparison form for keys and
/// contexts. Keys stay case-sensitive.
pub fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Splits catalog text into literal runs and lexicon references.
///
/// `\{{` is a literal `{{`; any other backslash is literal. Text without
/// references yields a single literal segment (or none when empty).
pub fn parse(text: &str) -> Result<Vec<Segment>, ReferenceError> {
    let mut segments = Vec::new();
    let mut literal = String::new();
    let mut rest = text;
    while let Some(start) = rest.find(OPEN) {
        if rest[..start].ends_with(ESCAPE) {
            literal.push_str(&rest[..start - ESCAPE.len_utf8()]);
            literal.push_str(OPEN);
            rest = &rest[start + OPEN.len()..];
            continue;
        }
        literal.push_str(&rest[..start]);
        let body = &rest[start + OPEN.len()..];
        let end = body.find(CLOSE).ok_or(ReferenceError::Unclosed)?;
        let reference = parse_reference(&body[..end])?;
        if !literal.is_empty() {
            segments.push(Segment::Literal(std::mem::take(&mut literal)));
        }
        segments.push(Segment::Reference(reference));
        rest = &body[end + CLOSE.len()..];
    }
    literal.push_str(rest);
    if !literal.is_empty() {
        segments.push(Segment::Literal(literal));
    }
    Ok(segments)
}

fn parse_reference(body: &str) -> Result<Reference, ReferenceError> {
    if body.contains(['{', '}']) {
        return Err(ReferenceError::Brace);
    }
    let mut parts = body.split(CONTEXT_SEPARATOR);
    let key = normalize(parts.next().unwrap_or_default());
    let context = parts.next().map(normalize);
    if parts.next().is_some() {
        return Err(ReferenceError::MultipleContexts);
    }
    if key.is_empty() {
        return Err(ReferenceError::EmptyKey);
    }
    if context.as_ref().is_some_and(String::is_empty) {
        return Err(ReferenceError::EmptyContext);
    }
    Ok(Reference { key, context })
}

/// Returns the references in `text`, in order and including duplicates.
pub fn references(text: &str) -> Result<Vec<Reference>, ReferenceError> {
    Ok(parse(text)?
        .into_iter()
        .filter_map(|segment| match segment {
            Segment::Reference(reference) => Some(reference),
            Segment::Literal(_) => None,
        })
        .collect())
}

/// Renders `text`, replacing each reference with `lookup`'s text or, when it
/// has none, the reference key. Resolved text is never parsed again.
pub fn resolve(
    text: &str,
    mut lookup: impl FnMut(&Reference) -> Option<String>,
) -> Result<String, ReferenceError> {
    Ok(parse(text)?
        .into_iter()
        .map(|segment| match segment {
            Segment::Literal(literal) => literal,
            Segment::Reference(reference) => {
                lookup(&reference).unwrap_or_else(|| reference.key.clone())
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_references_without_reinterpreting_translations() {
        let text = resolve("{{Product}} · {{Order|sorting}} \\{{x}}", |reference| {
            (reference.key == "Product").then(|| "{{Produkt}}".to_owned())
        })
        .unwrap();
        assert_eq!(text, "{{Produkt}} · Order {{x}}");
    }
}
