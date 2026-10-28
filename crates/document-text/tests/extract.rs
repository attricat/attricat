//! Fixtures were produced with LibreOffice from small HTML and flat ODS
//! sources; `encrypted.pdf` is `specification.pdf` encrypted with qpdf,
//! `restricted.pdf` is it with only an owner password, and
//! `no-text.pdf` is a vector drawing without a text layer. Adversarial archives
//! are built in the tests.

use std::io::{Cursor, Write};

use attricat_document_text::{ExtractError, Limits, SectionKind, extract};
use attricat_validation::files::{DOCX_MIME, PDF_MIME, XLSX_MIME};
use zip::{ZipWriter, write::SimpleFileOptions};

const SPECIFICATION_DOCX: &[u8] = include_bytes!("fixtures/specification.docx");
const SPECIFICATION_PDF: &[u8] = include_bytes!("fixtures/specification.pdf");
const ENCRYPTED_PDF: &[u8] = include_bytes!("fixtures/encrypted.pdf");
const RESTRICTED_PDF: &[u8] = include_bytes!("fixtures/restricted.pdf");
const NO_TEXT_PDF: &[u8] = include_bytes!("fixtures/no-text.pdf");
const PRICES_XLSX: &[u8] = include_bytes!("fixtures/prices.xlsx");

#[test]
fn word_documents_keep_headings_lists_and_tables() {
    let document = extract(DOCX_MIME, SPECIFICATION_DOCX, &Limits::DEFAULT).unwrap();
    assert_eq!(document.sections.len(), 1);
    let body = &document.sections[0];
    assert_eq!(body.kind, SectionKind::Body);
    assert!(body.text.contains("# Widget specification"));
    assert!(body.text.contains("## Dimensions"));
    assert!(body.text.contains("- Width 120 mm"));
    assert!(body.text.contains("brushed aluminium & steel"));
    assert!(body.text.contains("| Part | Weight |"));
    assert!(body.text.contains("| Lid \\| cover | 40 g |"));
    assert!(!document.truncated);
}

#[test]
fn word_documents_follow_package_relationships() {
    const W: &str = "xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"";
    let relationships = br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document2.xml"/></Relationships>"#;
    let styles = format!(
        r#"<w:styles {W}><w:style w:type="paragraph" w:styleId="Kop1"><w:name w:val="heading 1"/></w:style></w:styles>"#
    );
    let document = format!(
        r#"<w:document {W}><w:body>
        <w:p><w:pPr><w:pStyle w:val="Kop1"/><w:pPrChange><w:pPr><w:pStyle w:val="Normal"/></w:pPr></w:pPrChange></w:pPr><w:r><w:t>Overview</w:t></w:r></w:p>
        <w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="0"/></w:numPr></w:pPr><w:r><w:t>Plain</w:t></w:r></w:p>
        <w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="3"/></w:numPr></w:pPr><w:r><w:t>Item</w:t></w:r></w:p>
        </w:body></w:document>"#
    );
    let package = archive(&[
        ("_rels/.rels", relationships.to_vec()),
        ("word/document2.xml", document.into_bytes()),
        ("word/styles.xml", styles.into_bytes()),
    ]);
    let document = extract(DOCX_MIME, &package, &Limits::DEFAULT).unwrap();
    assert_eq!(document.sections[0].text, "# Overview\n\nPlain\n\n- Item");
}

#[test]
fn workbooks_read_each_sheet_as_tab_separated_rows() {
    let document = extract(XLSX_MIME, PRICES_XLSX, &Limits::DEFAULT).unwrap();
    let labels = document
        .sections
        .iter()
        .map(|section| section.label.as_str())
        .collect::<Vec<_>>();
    assert_eq!(labels, ["Prices", "Notes"]);
    assert!(
        document
            .sections
            .iter()
            .all(|section| section.kind == SectionKind::Sheet)
    );
    assert_eq!(
        document.sections[0].text,
        "SKU\tPrice\tReleased\nW-100\t19.5\t2026-03-01\nW-200\t42"
    );
    assert_eq!(document.sections[1].text, "Prices exclude VAT");
}

#[test]
fn sheet_rows_beyond_the_limit_are_marked() {
    let limits = Limits {
        max_sheet_rows: 1,
        ..Limits::DEFAULT
    };
    let document = extract(XLSX_MIME, PRICES_XLSX, &limits).unwrap();
    assert!(document.truncated);
    assert_eq!(
        document.sections[0].text,
        "SKU\tPrice\tReleased\n[more rows not shown]"
    );
}

#[test]
fn pdfs_read_page_by_page() {
    let document = extract(PDF_MIME, SPECIFICATION_PDF, &Limits::DEFAULT).unwrap();
    assert_eq!(document.sections[0].kind, SectionKind::Page);
    assert_eq!(document.sections[0].label, "1");
    let text = &document.sections[0].text;
    assert!(text.contains("Widget specification"), "{text}");
    assert!(text.contains("brushed aluminium & steel"), "{text}");
    assert!(text.contains("Width 120 mm"), "{text}");
}

#[test]
fn password_protected_pdfs_are_reported() {
    assert_eq!(
        extract(PDF_MIME, ENCRYPTED_PDF, &Limits::DEFAULT),
        Err(ExtractError::Encrypted)
    );
}

#[test]
fn pdfs_restricted_only_by_an_owner_password_are_read() {
    let document = extract(PDF_MIME, RESTRICTED_PDF, &Limits::DEFAULT).unwrap();
    assert_eq!(
        document,
        extract(PDF_MIME, SPECIFICATION_PDF, &Limits::DEFAULT).unwrap()
    );
}

#[test]
fn pdfs_without_a_text_layer_are_reported() {
    assert_eq!(
        extract(PDF_MIME, NO_TEXT_PDF, &Limits::DEFAULT),
        Err(ExtractError::NoTextLayer)
    );
}

#[test]
fn output_is_cut_to_the_character_budget() {
    let limits = Limits {
        max_output_chars: 20,
        ..Limits::DEFAULT
    };
    let document = extract(DOCX_MIME, SPECIFICATION_DOCX, &limits).unwrap();
    assert!(document.truncated);
    assert_eq!(document.char_count(), 20);
    assert_eq!(document.sections[0].text, "# Widget specificati");
}

#[test]
fn oversized_sources_are_rejected_before_parsing() {
    let limits = Limits {
        max_source_bytes: 10,
        ..Limits::DEFAULT
    };
    assert_eq!(
        extract(PDF_MIME, SPECIFICATION_PDF, &limits),
        Err(ExtractError::TooLarge)
    );
}

#[test]
fn pdf_streams_past_the_limit_are_too_large_not_textless() {
    let limits = Limits {
        max_pdf_page_stream_bytes: 1,
        ..Limits::DEFAULT
    };
    assert_eq!(
        extract(PDF_MIME, SPECIFICATION_PDF, &limits),
        Err(ExtractError::TooLarge)
    );
}

#[test]
fn decompression_bombs_are_rejected() {
    let bomb = archive(&[("word/document.xml", vec![b' '; 2 * 1024 * 1024])]);
    let limits = Limits {
        max_archive_entry_bytes: 1024 * 1024,
        ..Limits::DEFAULT
    };
    assert_eq!(
        extract(DOCX_MIME, &bomb, &limits),
        Err(ExtractError::TooLarge)
    );
    let limits = Limits {
        max_archive_total_bytes: 1024 * 1024,
        ..Limits::DEFAULT
    };
    assert_eq!(
        extract(XLSX_MIME, &bomb, &limits),
        Err(ExtractError::TooLarge)
    );
}

#[test]
fn archives_with_too_many_entries_are_rejected() {
    let entries = (0..3)
        .map(|index| (format!("part{index}.xml"), Vec::new()))
        .collect::<Vec<_>>();
    let entries = entries
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.clone()))
        .collect::<Vec<_>>();
    let limits = Limits {
        max_archive_entries: 2,
        ..Limits::DEFAULT
    };
    assert_eq!(
        extract(DOCX_MIME, &archive(&entries), &limits),
        Err(ExtractError::TooLarge)
    );
}

#[test]
fn damaged_or_mislabelled_files_are_malformed() {
    let truncated = &SPECIFICATION_DOCX[..SPECIFICATION_DOCX.len() / 2];
    assert_eq!(
        extract(DOCX_MIME, truncated, &Limits::DEFAULT),
        Err(ExtractError::Malformed)
    );
    let not_word = archive(&[("readme.txt", b"hello".to_vec())]);
    assert_eq!(
        extract(DOCX_MIME, &not_word, &Limits::DEFAULT),
        Err(ExtractError::Malformed)
    );
    assert_eq!(
        extract(XLSX_MIME, &not_word, &Limits::DEFAULT),
        Err(ExtractError::Malformed)
    );
    assert_eq!(
        extract(PDF_MIME, b"%PDF-1.7 garbage", &Limits::DEFAULT),
        Err(ExtractError::Malformed)
    );
}

#[test]
fn other_types_are_unsupported() {
    assert_eq!(
        extract("text/plain", b"hello", &Limits::DEFAULT),
        Err(ExtractError::Unsupported)
    );
}

#[test]
fn documents_round_trip_through_json() {
    let document = extract(XLSX_MIME, PRICES_XLSX, &Limits::DEFAULT).unwrap();
    let json = serde_json::to_string(&document).unwrap();
    assert!(json.contains("\"kind\":\"sheet\""));
    assert_eq!(
        serde_json::from_str::<attricat_document_text::Document>(&json).unwrap(),
        document
    );
}

fn archive(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}
