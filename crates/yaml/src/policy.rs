//! Rules the YAML library cannot be configured to enforce, checked on the
//! parser's events before deserialization:
//!
//! - Mapping keys that read as the same string are duplicates even when one is
//!   an integer and the other is quoted (`1` and `"1"`), because string-keyed
//!   maps would otherwise keep only the last one.
//! - A plain decimal integer must be exact: a leading zero (`012`, YAML 1.1
//!   octal but YAML 1.2 decimal) or a value outside the 64-bit range would
//!   otherwise become a lossy float. Authors quote such values to keep text.
//! - Only YAML core-schema tags (`!!str`, `!!int`, `!!float`, `!!bool`,
//!   `!!null`, `!!seq`, `!!map`) and the non-specific `!` are accepted;
//!   `!!binary`, `!!timestamp` and local tags are rejected.
//!
//! Syntax errors are left for the deserializer to report.

use std::collections::HashSet;

use granit_parser::{Event, Marker, Parser, ScalarStyle, Tag};

use crate::Error;

/// Core-schema tag suffixes a definition may name explicitly.
const SUPPORTED_CORE_TAGS: [&str; 7] = ["str", "int", "float", "bool", "null", "seq", "map"];

/// Tag suffix of the YAML core integer type.
const INT_TAG: &str = "int";

/// The non-specific tag `!`, which forces a string.
const NON_SPECIFIC_TAG: &str = "!";

enum Frame {
    Sequence,
    Mapping {
        keys: HashSet<String>,
        expecting_key: bool,
    },
}

pub(crate) fn check(source: &str) -> Result<(), Error> {
    let mut frames: Vec<Frame> = Vec::new();
    for event in Parser::new_from_str(source) {
        let Ok((event, span)) = event else {
            return Ok(());
        };
        let is_key = match &event {
            Event::Scalar(..)
            | Event::Alias(..)
            | Event::SequenceStart(..)
            | Event::MappingStart(..) => match frames.last_mut() {
                Some(Frame::Mapping { expecting_key, .. }) => {
                    let is_key = *expecting_key;
                    *expecting_key = !is_key;
                    is_key
                }
                _ => false,
            },
            _ => false,
        };
        match event {
            Event::Scalar(value, style, _, tag) => {
                if let Some(tag) = tag.as_deref() {
                    check_tag(tag, span.start)?;
                }
                if is_key {
                    if let Some(Frame::Mapping { keys, .. }) = frames.last_mut()
                        && !keys.insert(value.to_string())
                    {
                        return Err(error(format!("duplicate mapping key: {value}"), span.start));
                    }
                } else if is_integer_typed(style, tag.as_deref()) {
                    check_integer(&value, span.start)?;
                }
            }
            Event::SequenceStart(_, _, tag) => {
                if let Some(tag) = tag.as_deref() {
                    check_tag(tag, span.start)?;
                }
                frames.push(Frame::Sequence);
            }
            Event::MappingStart(_, _, tag) => {
                if let Some(tag) = tag.as_deref() {
                    check_tag(tag, span.start)?;
                }
                frames.push(Frame::Mapping {
                    keys: HashSet::new(),
                    expecting_key: true,
                });
            }
            Event::SequenceEnd | Event::MappingEnd => {
                frames.pop();
            }
            _ => {}
        }
    }
    Ok(())
}

fn check_tag(tag: &Tag, at: Marker) -> Result<(), Error> {
    let non_specific = tag.original() == NON_SPECIFIC_TAG;
    let core = tag
        .core_suffix()
        .is_some_and(|suffix| SUPPORTED_CORE_TAGS.contains(&suffix));
    if non_specific || core {
        Ok(())
    } else {
        Err(error(format!("unsupported tag `{}`", tag.original()), at))
    }
}

/// Whether the scalar resolves as an integer if it looks like one: a plain
/// untagged scalar or one tagged `!!int`.
fn is_integer_typed(style: ScalarStyle, tag: Option<&Tag>) -> bool {
    match tag {
        None => style == ScalarStyle::Plain,
        Some(tag) => tag.core_suffix() == Some(INT_TAG),
    }
}

/// Rejects YAML 1.2 core decimal integers (`[-+]?[0-9]+`) that would not be
/// read exactly.
fn check_integer(value: &str, at: Marker) -> Result<(), Error> {
    let digits = value.strip_prefix(['-', '+']).unwrap_or(value);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Ok(());
    }
    if digits.len() > 1 && digits.starts_with('0') {
        return Err(error(
            format!(
                "integer `{value}` has a leading zero; remove it, or quote the value to keep it as text"
            ),
            at,
        ));
    }
    if value.parse::<i64>().is_err() && value.parse::<u64>().is_err() {
        return Err(error(
            format!(
                "integer `{value}` is outside the 64-bit range; quote the value to keep it as text"
            ),
            at,
        ));
    }
    Ok(())
}

/// Formats the location as the deserializer does: 1-based line and column.
fn error(message: String, at: Marker) -> Error {
    Error(format!(
        "{message} at line {}, column {}",
        at.line(),
        at.col() + 1
    ))
}
