//! Rewrites a YAML document to a new value by editing only the nodes that
//! change, so the author's comments, key order, quoting and spacing survive.
//!
//! Supported changes are the ones a rewrite of references makes: replacing
//! scalars (including renamed strings inside a JSON object or array held in a
//! string), renaming one mapping key and adding scalar entries beside it, and
//! removing scalar items from a sequence. Anything else, or an edit whose
//! result does not parse back to the target, yields `None` so the caller can
//! serialize the value instead.

use std::{collections::BTreeMap, ops::Range};

use granit_parser::{Event, Parser, ScalarStyle, Span, StructureStyle};
use serde_json::Value;

/// Returns `source` edited so it parses to a value equivalent to `target`, or
/// `None` when the change cannot be made in place. A string holding a JSON
/// object or array is equivalent to one with the same parsed JSON.
pub fn rewrite_preserving_layout(source: &str, target: &Value) -> Option<String> {
    let original: Value = crate::from_str(source).ok()?;
    let root = parse_tree(source)?;
    let mut edits = Vec::new();
    diff(source, &root, &original, target, None, &mut edits)?;
    edits.sort_by_key(|(range, _)| range.start);
    if edits.windows(2).any(|pair| pair[0].0.end > pair[1].0.start) {
        return None;
    }
    let mut rewritten = source.to_owned();
    for (range, text) in edits.into_iter().rev() {
        rewritten.replace_range(range, &text);
    }
    let parsed: Value = crate::from_str(&rewritten).ok()?;
    equivalent(&parsed, target).then_some(rewritten)
}

type Edit = (Range<usize>, String);

struct Node {
    span: Range<usize>,
    kind: Kind,
}

enum Kind {
    Scalar(ScalarStyle),
    Sequence { flow: bool, items: Vec<Node> },
    Mapping { flow: bool, entries: Vec<Entry> },
    Alias,
}

struct Entry {
    key: String,
    key_span: Range<usize>,
    value: Node,
}

fn parse_tree(source: &str) -> Option<Node> {
    let mut events = Vec::new();
    for event in Parser::new_from_str(source) {
        let (event, span) = event.ok()?;
        if !matches!(event, Event::Comment(..)) {
            events.push((event, span));
        }
    }
    let mut events = events.into_iter();
    let (Some((Event::StreamStart, _)), Some((Event::DocumentStart(..), _))) =
        (events.next(), events.next())
    else {
        return None;
    };
    let root = node(events.next()?, &mut events)?;
    matches!(
        (events.next(), events.next(), events.next()),
        (
            Some((Event::DocumentEnd, _)),
            Some((Event::StreamEnd, _)),
            None
        )
    )
    .then_some(root)
}

fn byte_range(span: &Span) -> Option<Range<usize>> {
    Some(span.start.byte_offset()?..span.end.byte_offset()?)
}

fn node<'a>(
    (event, span): (Event<'a>, Span),
    events: &mut impl Iterator<Item = (Event<'a>, Span)>,
) -> Option<Node> {
    let start = byte_range(&span)?;
    let (kind, end) = match event {
        Event::Scalar(_, style, ..) => (Kind::Scalar(style), start.end),
        Event::Alias(_) => (Kind::Alias, start.end),
        Event::SequenceStart(style, ..) => {
            let mut items = Vec::new();
            let end = loop {
                let next = events.next()?;
                if matches!(next.0, Event::SequenceEnd) {
                    break byte_range(&next.1)?.end;
                }
                items.push(node(next, events)?);
            };
            let flow = style == StructureStyle::Flow;
            (Kind::Sequence { flow, items }, end)
        }
        Event::MappingStart(style, ..) => {
            let mut entries = Vec::new();
            let end = loop {
                let (key, key_span) = events.next()?;
                let key = match key {
                    Event::MappingEnd => break byte_range(&key_span)?.end,
                    Event::Scalar(key, ..) => key.into_owned(),
                    _ => return None,
                };
                let value = node(events.next()?, events)?;
                entries.push(Entry {
                    key,
                    key_span: byte_range(&key_span)?,
                    value,
                });
            };
            let flow = style == StructureStyle::Flow;
            (Kind::Mapping { flow, entries }, end)
        }
        _ => return None,
    };
    Some(Node {
        span: start.start..end,
        kind,
    })
}

/// Records the edits that turn `node` (parsed as `old`) into `new`.
/// `parent_key` is the key `node` is the value of, when it has one.
fn diff(
    source: &str,
    node: &Node,
    old: &Value,
    new: &Value,
    parent_key: Option<&Range<usize>>,
    edits: &mut Vec<Edit>,
) -> Option<()> {
    if old == new {
        return Some(());
    }
    match (&node.kind, old, new) {
        (Kind::Mapping { flow, entries }, Value::Object(old), Value::Object(new)) => {
            diff_mapping(source, *flow, entries, old, new, edits)
        }
        (Kind::Sequence { flow, items }, Value::Array(old), Value::Array(new)) => {
            diff_sequence(source, node, *flow, items, old, new, parent_key, edits)
        }
        (Kind::Scalar(style), _, new) if !new.is_object() && !new.is_array() => {
            let text = replaced_scalar(&source[node.span.clone()], *style, old, new)?;
            edits.push((node.span.clone(), text));
            Some(())
        }
        _ => None,
    }
}

fn diff_mapping(
    source: &str,
    flow: bool,
    entries: &[Entry],
    old: &serde_json::Map<String, Value>,
    new: &serde_json::Map<String, Value>,
    edits: &mut Vec<Edit>,
) -> Option<()> {
    for entry in entries {
        if let (Some(old), Some(new)) = (old.get(&entry.key), new.get(&entry.key)) {
            diff(source, &entry.value, old, new, Some(&entry.key_span), edits)?;
        }
    }
    let removed = entries
        .iter()
        .filter(|entry| !new.contains_key(&entry.key))
        .collect::<Vec<_>>();
    let mut added = new.iter().filter(|(key, _)| !old.contains_key(*key));
    let anchor = match removed.as_slice() {
        [] => entries.last()?,
        [renamed] => {
            let (key, value) = added.next()?;
            edits.push((renamed.key_span.clone(), rendered_key(key)?));
            diff(
                source,
                &renamed.value,
                old.get(&renamed.key)?,
                value,
                Some(&renamed.key_span),
                edits,
            )?;
            renamed
        }
        _ => return None,
    };
    let added = added.collect::<Vec<_>>();
    if added.is_empty() {
        return Some(());
    }
    let after_value = anchor.value.span.end;
    let mut text = String::new();
    if flow {
        for (key, value) in added {
            let value = serde_json::to_string(value).ok()?;
            text.push_str(&format!(", {}: {value}", rendered_key(key)?));
        }
        edits.push((after_value..after_value, text));
        return Some(());
    }
    // A block entry goes on its own line after the anchor's, at its indent.
    if !matches!(
        anchor.value.kind,
        Kind::Scalar(ScalarStyle::Plain | ScalarStyle::SingleQuoted | ScalarStyle::DoubleQuoted)
    ) {
        return None;
    }
    let line_start = line_start(source, anchor.key_span.start);
    let indent = " ".repeat(source[line_start..anchor.key_span.start].chars().count());
    let at = match source[after_value..].find('\n') {
        Some(offset) => after_value + offset + 1,
        None => {
            text.push('\n');
            source.len()
        }
    };
    for (key, value) in added {
        let value = rendered_scalar(value, ScalarStyle::Plain)?;
        text.push_str(&format!("{indent}{}: {value}\n", rendered_key(key)?));
    }
    edits.push((at..at, text));
    Some(())
}

#[allow(clippy::too_many_arguments)]
fn diff_sequence(
    source: &str,
    node: &Node,
    flow: bool,
    items: &[Node],
    old: &[Value],
    new: &[Value],
    parent_key: Option<&Range<usize>>,
    edits: &mut Vec<Edit>,
) -> Option<()> {
    if old.len() == new.len() {
        for ((item, old), new) in items.iter().zip(old).zip(new) {
            diff(source, item, old, new, None, edits)?;
        }
        return Some(());
    }
    // Otherwise `new` must be `old` with some items removed.
    let mut remaining = new.iter().peekable();
    let kept = old
        .iter()
        .map(|item| remaining.next_if(|next| *next == item).is_some())
        .collect::<Vec<_>>();
    if remaining.next().is_some() {
        return None;
    }
    if flow {
        let text = items
            .iter()
            .zip(&kept)
            .filter(|(_, kept)| **kept)
            .map(|(item, _)| &source[item.span.clone()])
            .collect::<Vec<_>>()
            .join(", ");
        edits.push((node.span.clone(), format!("[{text}]")));
        return Some(());
    }
    if items.iter().any(|item| {
        !matches!(
            item.kind,
            Kind::Scalar(
                ScalarStyle::Plain | ScalarStyle::SingleQuoted | ScalarStyle::DoubleQuoted
            )
        )
    }) {
        return None;
    }
    if !kept.contains(&true) {
        // A block sequence cannot be empty; the key's value becomes `[]`.
        let key = parent_key?;
        let colon = key.end + source[key.end..].find(|c| c != ' ')?;
        if !source[colon..].starts_with(':') {
            return None;
        }
        let last = items.last()?.span.end;
        let end = source[last..]
            .find('\n')
            .map_or(source.len(), |offset| last + offset);
        edits.push((colon + 1..end, " []".to_owned()));
        return Some(());
    }
    for (item, kept) in items.iter().zip(&kept) {
        if *kept {
            continue;
        }
        let start = line_start(source, item.span.start);
        if source[start..item.span.start].trim() != "-" {
            return None;
        }
        let end = source[item.span.end..]
            .find('\n')
            .map_or(source.len(), |offset| item.span.end + offset + 1);
        edits.push((start..end, String::new()));
    }
    Some(())
}

fn line_start(source: &str, at: usize) -> usize {
    source[..at].rfind('\n').map_or(0, |newline| newline + 1)
}

fn replaced_scalar(raw: &str, style: ScalarStyle, old: &Value, new: &Value) -> Option<String> {
    if let (Value::String(old), Value::String(new)) = (old, new)
        && let Some(text) = renamed_json_strings(raw, style, old, new)
    {
        return Some(text);
    }
    if matches!(style, ScalarStyle::Literal | ScalarStyle::Folded) {
        return None;
    }
    rendered_scalar(new, style)
}

/// Edits the raw text of a scalar holding JSON when the only changes are
/// renamed strings, keeping the author's JSON formatting.
fn renamed_json_strings(raw: &str, style: ScalarStyle, old: &str, new: &str) -> Option<String> {
    let old = serde_json::from_str::<Value>(old).ok()?;
    let new = serde_json::from_str::<Value>(new).ok()?;
    if !old.is_object() && !old.is_array() {
        return None;
    }
    let mut renames = BTreeMap::new();
    collect_renames(&old, &new, &mut renames)?;
    if !renames
        .iter()
        .all(|(old, new)| is_bare(old) && is_bare(new))
    {
        return None;
    }
    let quote = if style == ScalarStyle::DoubleQuoted {
        "\\\""
    } else {
        "\""
    };
    let renames = renames
        .into_iter()
        .map(|(old, new)| {
            (
                format!("{quote}{old}{quote}"),
                format!("{quote}{new}{quote}"),
            )
        })
        .collect::<Vec<_>>();
    let mut text = String::with_capacity(raw.len());
    let mut rest = raw;
    'scan: while let Some(next) = rest.chars().next() {
        for (old, new) in &renames {
            if let Some(after) = rest.strip_prefix(old.as_str()) {
                text.push_str(new);
                rest = after;
                continue 'scan;
            }
        }
        text.push(next);
        rest = &rest[next.len_utf8()..];
    }
    Some(text)
}

fn collect_renames<'a>(
    old: &'a Value,
    new: &'a Value,
    renames: &mut BTreeMap<&'a str, &'a str>,
) -> Option<()> {
    match (old, new) {
        (Value::String(old), Value::String(new)) if old != new => {
            (*renames.entry(old).or_insert(new) == new).then_some(())
        }
        (Value::Object(old), Value::Object(new)) if old.len() == new.len() => {
            for (key, old) in old {
                collect_renames(old, new.get(key)?, renames)?;
            }
            Some(())
        }
        (Value::Array(old), Value::Array(new)) if old.len() == new.len() => {
            for (old, new) in old.iter().zip(new) {
                collect_renames(old, new, renames)?;
            }
            Some(())
        }
        _ => (old == new).then_some(()),
    }
}

/// A string made only of characters that need no escaping or quoting.
fn is_bare(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

/// Whether `value` reads back as the same string when written unquoted.
fn is_plain(value: &str) -> bool {
    is_bare(value)
        && value.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && crate::from_str::<Value>(value).ok() == Some(Value::String(value.to_owned()))
}

fn rendered_key(key: &str) -> Option<String> {
    rendered_scalar(&Value::String(key.to_owned()), ScalarStyle::Plain)
}

/// Writes a scalar in the original style when it allows the value; JSON
/// strings and literals are valid YAML everywhere else.
fn rendered_scalar(value: &Value, style: ScalarStyle) -> Option<String> {
    match value {
        Value::String(value) if style == ScalarStyle::Plain && is_plain(value) => {
            Some(value.clone())
        }
        Value::String(value) if style == ScalarStyle::SingleQuoted && !value.contains('\n') => {
            Some(format!("'{}'", value.replace('\'', "''")))
        }
        Value::Object(_) | Value::Array(_) => None,
        value => serde_json::to_string(value).ok(),
    }
}

fn equivalent(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::String(left), Value::String(right)) => {
            left == right
                || json_container(left).is_some_and(|left| json_container(right) == Some(left))
        }
        (Value::Object(left), Value::Object(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .all(|(key, left)| right.get(key).is_some_and(|right| equivalent(left, right)))
        }
        (Value::Array(left), Value::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| equivalent(left, right))
        }
        _ => left == right,
    }
}

fn json_container(value: &str) -> Option<Value> {
    serde_json::from_str::<Value>(value)
        .ok()
        .filter(|value| value.is_object() || value.is_array())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rewrite(source: &str, edit: impl FnOnce(&mut Value)) -> Option<String> {
        let mut target: Value = crate::from_str(source).unwrap();
        edit(&mut target);
        rewrite_preserving_layout(source, &target)
    }

    #[test]
    fn replaces_scalars_and_keeps_comments_order_and_quoting() {
        let source = "# Orders\ncode: orders # pack code\nname: 'Orders'\nlabel: \"x\"\n";
        let rewritten = rewrite(source, |value| {
            value["code"] = json!("acme_orders");
            value["name"] = json!("Acme's orders");
            value["label"] = json!("y");
        });
        assert_eq!(
            rewritten.unwrap(),
            "# Orders\ncode: acme_orders # pack code\nname: 'Acme''s orders'\nlabel: \"y\"\n"
        );
    }

    #[test]
    fn quotes_a_replacement_that_would_read_as_another_type() {
        let rewritten = rewrite("code: a\n", |value| value["code"] = json!("true"));
        assert_eq!(rewritten.unwrap(), "code: \"true\"\n");
    }

    #[test]
    fn renames_a_key_and_adds_entries_beside_it() {
        let source = "includes:\n  # base first\n  - key: base # shared\n    alias: b\n  - {key: other, alias: o}\n";
        let rewritten = rewrite(source, |value| {
            for (include, code) in value["includes"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .zip(["acme_base", "acme_other"])
            {
                let include = include.as_object_mut().unwrap();
                include.remove("key");
                include.insert("code".into(), json!(code));
                include.insert("version".into(), json!(1));
            }
        });
        assert_eq!(
            rewritten.unwrap(),
            "includes:\n  # base first\n  - code: acme_base # shared\n    version: 1\n    alias: b\n  - {code: acme_other, version: 1, alias: o}\n"
        );
    }

    #[test]
    fn removes_sequence_items() {
        let source = "order:\n  - one # first\n  - two\n  - three\nhidden: [a, b, c]\nall:\n  - x\n  - y # last\nnext: 1\n";
        let rewritten = rewrite(source, |value| {
            value["order"] = json!(["one", "three"]);
            value["hidden"] = json!(["b"]);
            value["all"] = json!([]);
        });
        assert_eq!(
            rewritten.unwrap(),
            "order:\n  - one # first\n  - three\nhidden: [b]\nall: []\nnext: 1\n"
        );
    }

    #[test]
    fn renames_strings_inside_embedded_json() {
        let source = "record_schema: |\n  {\n    \"blueprint\": \"orders\",\n    \"other\": \"kept\"\n  }\nvalue_schema: \"{\\\"blueprint\\\": \\\"orders\\\"}\"\n";
        let rewritten = rewrite(source, |value| {
            value["record_schema"] = json!(r#"{"blueprint":"acme_orders","other":"kept"}"#);
            value["value_schema"] = json!(r#"{"blueprint":"acme_orders"}"#);
        });
        assert_eq!(
            rewritten.unwrap(),
            "record_schema: |\n  {\n    \"blueprint\": \"acme_orders\",\n    \"other\": \"kept\"\n  }\nvalue_schema: \"{\\\"blueprint\\\": \\\"acme_orders\\\"}\"\n"
        );
    }

    #[test]
    fn leaves_an_unchanged_document_untouched() {
        let source = "# note\ncode:   orders\n";
        assert_eq!(rewrite(source, |_| {}).unwrap(), source);
    }

    #[test]
    fn declines_changes_it_cannot_make_in_place() {
        // Removing an entry, editing through an alias and restructuring.
        assert_eq!(
            rewrite("a: 1\nb: 2\n", |value| {
                value.as_object_mut().unwrap().remove("b");
            }),
            None
        );
        assert_eq!(
            rewrite("a: &x orders\nb: *x\n", |value| value["b"] = json!("other")),
            None
        );
        assert_eq!(
            rewrite("a: [1]\n", |value| value["a"] = json!({"b": 1})),
            None
        );
    }

    #[test]
    fn declines_a_rename_that_also_matches_unrenamed_text() {
        // `orders` is a JSON key too; replacing it would change the key.
        let source = "s: '{\"orders\": \"orders\"}'\n";
        assert_eq!(
            rewrite(source, |value| value["s"] = json!(r#"{"orders":"acme"}"#)),
            None
        );
    }
}
