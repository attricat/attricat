//! Word documents: the main document part as Markdown-like text, with heading
//! styles as `#` headings, numbered or bulleted paragraphs as `- ` items, and
//! tables as Markdown tables. Headers, footers, notes and comments are skipped.

use std::collections::HashMap;

use quick_xml::{
    Reader, XmlVersion,
    escape::resolve_predefined_entity,
    events::{BytesStart, Event},
};

use crate::{ExtractError, Limits, Output, SectionKind, archive::Archive};

const DOCUMENT_PART: &str = "word/document.xml";
const STYLES_PART: &str = "word/styles.xml";
/// Section label of the single section a Word document produces.
pub(crate) const BODY_LABEL: &str = "body";
/// Deepest Markdown heading level.
const MAX_HEADING_LEVEL: usize = 6;

pub(crate) fn extract(
    bytes: &[u8],
    limits: &Limits,
    output: &mut Output,
) -> Result<(), ExtractError> {
    let mut archive = Archive::open(bytes, limits)?;
    let document = archive
        .read(DOCUMENT_PART)?
        .ok_or(ExtractError::Malformed)?;
    let headings = match archive.read(STYLES_PART)? {
        Some(styles) => heading_styles(&styles)?,
        None => HashMap::new(),
    };
    let (text, truncated) = body_text(&document, &headings, limits.max_output_chars)?;
    if truncated {
        output.mark_truncated();
    }
    output.push(SectionKind::Body, BODY_LABEL.to_owned(), text);
    Ok(())
}

/// The heading level of each paragraph style ID whose name is `Title` or
/// `heading N`. Style IDs are localised; style names are not.
fn heading_styles(styles: &[u8]) -> Result<HashMap<String, usize>, ExtractError> {
    let mut reader = Reader::from_reader(styles);
    let mut buffer = Vec::new();
    let mut headings = HashMap::new();
    let mut style_id = None;
    loop {
        match reader
            .read_event_into(&mut buffer)
            .map_err(|_| ExtractError::Malformed)?
        {
            Event::Start(element) if element.local_name().as_ref() == b"style" => {
                style_id = attribute(&element, b"styleId");
            }
            Event::End(element) if element.local_name().as_ref() == b"style" => style_id = None,
            Event::Empty(element) if element.local_name().as_ref() == b"name" => {
                if let (Some(id), Some(name)) = (&style_id, attribute(&element, b"val")) {
                    let name = name.to_ascii_lowercase();
                    let level = if name == "title" {
                        Some(1)
                    } else {
                        name.strip_prefix("heading ")
                            .and_then(|level| level.trim().parse::<usize>().ok())
                    };
                    if let Some(level) = level.filter(|level| *level >= 1) {
                        headings.insert(id.clone(), level.min(MAX_HEADING_LEVEL));
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    Ok(headings)
}

#[derive(Default)]
struct Paragraph {
    text: String,
    style: Option<String>,
    list_item: bool,
}

type Table = Vec<Vec<String>>;

/// Walks the document part. Returns the text, and whether `max_chars` cut it.
fn body_text(
    document: &[u8],
    headings: &HashMap<String, usize>,
    max_chars: usize,
) -> Result<(String, bool), ExtractError> {
    let mut reader = Reader::from_reader(document);
    let mut buffer = Vec::new();
    let mut blocks = Blocks::new(max_chars);
    // Text boxes nest paragraphs inside a paragraph's runs, and tables nest
    // inside table cells, so both are stacks.
    let mut paragraphs: Vec<Paragraph> = Vec::new();
    let mut tables: Vec<Table> = Vec::new();
    let mut in_text = false;
    // Alternate content repeats a text box as a legacy fallback; read it once.
    let mut fallback_depth = 0_usize;
    loop {
        if blocks.full {
            break;
        }
        let event = reader
            .read_event_into(&mut buffer)
            .map_err(|_| ExtractError::Malformed)?;
        if fallback_depth > 0 {
            match event {
                Event::Start(element) if element.local_name().as_ref() == b"Fallback" => {
                    fallback_depth += 1
                }
                Event::End(element) if element.local_name().as_ref() == b"Fallback" => {
                    fallback_depth -= 1
                }
                Event::Eof => break,
                _ => {}
            }
            buffer.clear();
            continue;
        }
        match event {
            Event::Start(element) => match element.local_name().as_ref() {
                b"Fallback" => fallback_depth = 1,
                b"p" => paragraphs.push(Paragraph::default()),
                b"t" => in_text = true,
                b"numPr" => set_list_item(&mut paragraphs),
                b"tbl" => tables.push(Vec::new()),
                b"tr" => {
                    if let Some(table) = tables.last_mut() {
                        table.push(Vec::new());
                    }
                }
                b"tc" => {
                    if let Some(row) = tables.last_mut().and_then(|table| table.last_mut()) {
                        row.push(String::new());
                    }
                }
                _ => {}
            },
            Event::Empty(element) => match element.local_name().as_ref() {
                b"pStyle" => {
                    if let Some(paragraph) = paragraphs.last_mut() {
                        paragraph.style = attribute(&element, b"val");
                    }
                }
                b"numPr" => set_list_item(&mut paragraphs),
                b"tab" => push_text(&mut paragraphs, "\t"),
                b"br" | b"cr" => push_text(&mut paragraphs, "\n"),
                _ => {}
            },
            Event::Text(text) if in_text => {
                push_text(
                    &mut paragraphs,
                    &text.decode().map_err(|_| ExtractError::Malformed)?,
                );
            }
            Event::GeneralRef(reference) if in_text => {
                let character = match reference
                    .resolve_char_ref()
                    .map_err(|_| ExtractError::Malformed)?
                {
                    Some(character) => character.to_string(),
                    None => {
                        let name = reference.decode().map_err(|_| ExtractError::Malformed)?;
                        resolve_predefined_entity(&name)
                            .unwrap_or_default()
                            .to_owned()
                    }
                };
                push_text(&mut paragraphs, &character);
            }
            Event::End(element) => match element.local_name().as_ref() {
                b"t" => in_text = false,
                b"p" => {
                    let paragraph = paragraphs.pop().unwrap_or_default();
                    let cell = tables
                        .last_mut()
                        .and_then(|table| table.last_mut())
                        .and_then(|row| row.last_mut());
                    match cell {
                        Some(cell) => {
                            let text = paragraph.text.trim();
                            if !text.is_empty() {
                                if !cell.is_empty() {
                                    cell.push(' ');
                                }
                                cell.push_str(text);
                            }
                        }
                        None => blocks.push(paragraph_block(paragraph, headings)),
                    }
                }
                b"tbl" => {
                    let table = tables.pop().unwrap_or_default();
                    let cell = tables
                        .last_mut()
                        .and_then(|table| table.last_mut())
                        .and_then(|row| row.last_mut());
                    match cell {
                        // A nested table reads as its cells in order.
                        Some(cell) => {
                            for text in table.iter().flatten().filter(|text| !text.is_empty()) {
                                if !cell.is_empty() {
                                    cell.push(' ');
                                }
                                cell.push_str(text);
                            }
                        }
                        None => blocks.push(markdown_table(&table)),
                    }
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    Ok((blocks.text, blocks.full))
}

fn set_list_item(paragraphs: &mut [Paragraph]) {
    if let Some(paragraph) = paragraphs.last_mut() {
        paragraph.list_item = true;
    }
}

fn push_text(paragraphs: &mut [Paragraph], text: &str) {
    if let Some(paragraph) = paragraphs.last_mut() {
        paragraph.text.push_str(text);
    }
}

fn paragraph_block(paragraph: Paragraph, headings: &HashMap<String, usize>) -> String {
    let text = paragraph.text.trim();
    if text.is_empty() {
        return String::new();
    }
    if let Some(level) = paragraph
        .style
        .as_ref()
        .and_then(|style| headings.get(style))
    {
        format!("{} {text}", "#".repeat(*level))
    } else if paragraph.list_item {
        format!("- {text}")
    } else {
        text.to_owned()
    }
}

fn markdown_table(table: &Table) -> String {
    let columns = table.iter().map(Vec::len).max().unwrap_or_default();
    if columns == 0 {
        return String::new();
    }
    let row_line = |row: &[String]| {
        let cells = (0..columns)
            .map(|index| {
                row.get(index)
                    .map(|cell| cell.replace('|', "\\|").replace('\n', " "))
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();
        format!("| {} |", cells.join(" | "))
    };
    let mut lines = Vec::with_capacity(table.len() + 1);
    for (index, row) in table.iter().enumerate() {
        lines.push(row_line(row));
        if index == 0 {
            lines.push(format!("|{}", " --- |".repeat(columns)));
        }
    }
    lines.join("\n")
}

/// Non-empty blocks separated by blank lines, read until a character budget.
struct Blocks {
    text: String,
    remaining_chars: usize,
    full: bool,
}

impl Blocks {
    fn new(max_chars: usize) -> Self {
        Self {
            text: String::new(),
            remaining_chars: max_chars,
            full: false,
        }
    }

    fn push(&mut self, block: String) {
        if block.is_empty() {
            return;
        }
        let separator = if self.text.is_empty() { "" } else { "\n\n" };
        let chars = separator.len() + block.chars().count();
        if chars > self.remaining_chars {
            self.full = true;
            return;
        }
        self.remaining_chars -= chars;
        self.text.push_str(separator);
        self.text.push_str(&block);
    }
}

fn attribute(element: &BytesStart<'_>, local_name: &[u8]) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.local_name().as_ref() == local_name)
        .and_then(|attribute| {
            attribute
                .normalized_value(XmlVersion::Implicit1_0)
                .ok()
                .map(|value| value.into_owned())
        })
}
