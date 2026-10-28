//! Excel workbooks: one section per worksheet, its rows as tab-separated
//! values. Cells hold the values Excel last calculated, not their formulas.

use std::io::Cursor;

use calamine::{DataRef, Reader, SheetType, Xlsx, open_workbook_from_rs};

use crate::{ExtractError, Limits, Output, SectionKind, archive::Archive};

/// Appended to a sheet cut short by the row limit.
const MORE_ROWS_MARKER: &str = "[more rows not shown]";

pub(crate) fn extract(
    bytes: &[u8],
    limits: &Limits,
    output: &mut Output,
) -> Result<(), ExtractError> {
    // The workbook reader trusts the sizes the archive declares, so measure
    // every entry first.
    Archive::open(bytes, limits)?;
    let mut workbook: Xlsx<_> =
        open_workbook_from_rs(Cursor::new(bytes)).map_err(|_| ExtractError::Malformed)?;
    let sheets = workbook
        .sheets_metadata()
        .iter()
        .filter(|sheet| sheet.typ == SheetType::WorkSheet)
        .map(|sheet| sheet.name.clone())
        .collect::<Vec<_>>();
    for name in sheets {
        if output.is_full() {
            output.mark_truncated();
            break;
        }
        let text = sheet_text(&mut workbook, &name, limits, output)?;
        output.push(SectionKind::Sheet, name, text);
    }
    Ok(())
}

fn sheet_text(
    workbook: &mut Xlsx<Cursor<&[u8]>>,
    name: &str,
    limits: &Limits,
    output: &mut Output,
) -> Result<String, ExtractError> {
    let budget = output.remaining_chars();
    let mut reader = workbook
        .worksheet_cells_reader(name)
        .map_err(|_| ExtractError::Malformed)?;
    let mut lines: Vec<String> = Vec::new();
    let mut chars = 0_usize;
    let mut row_index = None;
    let mut row: Vec<String> = Vec::new();
    // Cells arrive in row order and only where a sheet has content.
    while let Some(cell) = reader.next_cell().map_err(|_| ExtractError::Malformed)? {
        let (cell_row, column) = cell.get_position();
        if row_index != Some(cell_row) {
            if row_index.is_some() {
                chars += flush_row(&mut row, &mut lines);
            }
            if lines.len() >= limits.max_sheet_rows || chars >= budget {
                lines.push(MORE_ROWS_MARKER.to_owned());
                output.mark_truncated();
                return Ok(lines.join("\n"));
            }
            row_index = Some(cell_row);
        }
        let value = cell_text(cell.get_value());
        if value.is_empty() {
            continue;
        }
        let column = column as usize;
        if column >= limits.max_sheet_columns {
            output.mark_truncated();
            continue;
        }
        if row.len() <= column {
            row.resize(column + 1, String::new());
        }
        row[column] = value;
    }
    flush_row(&mut row, &mut lines);
    Ok(lines.join("\n"))
}

/// Appends a non-empty row as a TSV line. Returns the characters added.
fn flush_row(row: &mut Vec<String>, lines: &mut Vec<String>) -> usize {
    if row.is_empty() {
        return 0;
    }
    let line = row.join("\t");
    row.clear();
    let chars = line.chars().count() + 1;
    lines.push(line);
    chars
}

fn cell_text(value: &DataRef<'_>) -> String {
    let text = match value {
        DataRef::Empty => return String::new(),
        DataRef::DateTime(value) => match value.as_datetime() {
            Some(datetime) if datetime.time() == chrono::NaiveTime::MIN => {
                datetime.date().to_string()
            }
            Some(datetime) => datetime.to_string(),
            None => value.to_string(),
        },
        DataRef::SharedString(text) => (*text).to_owned(),
        other => calamine::Data::from(other.clone()).to_string(),
    };
    // A cell must not break the row and column structure.
    text.replace(['\t', '\n', '\r'], " ").trim().to_owned()
}
