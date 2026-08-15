use std::{collections::HashMap, io::Cursor, io::Read, io::Seek, panic};

use calamine::{Data, Reader, SheetType, SheetVisible, Xls, Xlsx};
use quick_xml::{events::Event, Reader as XmlReader};
use serde::{Deserialize, Serialize};
use zip::ZipArchive;

use crate::{AppError, InputFormat};

const MAX_PDF_PAGES: usize = 1000;
const MAX_CSV_RECORDS: usize = 100_000;
const MAX_CSV_COLUMNS: usize = 256;
const MAX_CSV_FIELD_BYTES: usize = 4 * 1024 * 1024;
const MAX_CSV_BYTES: usize = 128 * 1024 * 1024;

const MAX_EXCEL_BYTES: usize = 128 * 1024 * 1024;
const MAX_EXCEL_SHEETS: usize = 100;
const MAX_EXCEL_ROWS: usize = 100_000;
const MAX_EXCEL_COLUMNS: usize = 256;
const MAX_EXCEL_CELLS: usize = 1_000_000;
const MAX_EXCEL_ZIP_ENTRIES: usize = 1_000;
const MAX_EXCEL_UNCOMPRESSED_BYTES: u64 = 1024 * 1024 * 1024;

const MAX_PPTX_BYTES: usize = 128 * 1024 * 1024;
const MAX_PPTX_SLIDES: usize = 1_000;
const MAX_PPTX_CHARS: usize = 16 * 1024 * 1024;
const MAX_PPTX_TEXT_BLOCK_BYTES: usize = 4 * 1024 * 1024;
const MAX_PPTX_XML_DEPTH: usize = 64;
const MAX_PPTX_ZIP_ENTRIES: usize = 2_000;
const MAX_PPTX_UNCOMPRESSED_BYTES: u64 = 1024 * 1024 * 1024;

const PPTX_SLIDE_RELATIONSHIP_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentFormat {
    Docx,
    Pdf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedDocument {
    pub markdown: String,
    pub input_format: InputFormat,
    pub page_count: Option<usize>,
    pub warnings: Vec<String>,
}

pub fn parse_document(bytes: &[u8], format: DocumentFormat) -> Result<ParsedDocument, AppError> {
    parse_input(
        bytes,
        match format {
            DocumentFormat::Docx => InputFormat::Docx,
            DocumentFormat::Pdf => InputFormat::Pdf,
        },
    )
}

/// Shared parser boundary for every currently supported enterprise format.
/// The caller supplies controlled bytes and a catalog-approved logical input;
/// no adapter, path, HTTP, database, or UI state crosses this boundary.
pub fn parse_input(bytes: &[u8], format: InputFormat) -> Result<ParsedDocument, AppError> {
    match format {
        InputFormat::Text | InputFormat::Markdown => parse_text(bytes, format),
        InputFormat::Csv => parse_csv(bytes),
        InputFormat::Excel => parse_excel(bytes),
        InputFormat::Docx => parse_docx(bytes),
        InputFormat::Pdf => parse_pdf(bytes),
        InputFormat::Powerpoint => parse_powerpoint(bytes),
    }
}

fn parse_text(bytes: &[u8], input_format: InputFormat) -> Result<ParsedDocument, AppError> {
    if looks_like_binary_document(bytes) {
        return Err(error(
            "INPUT_CORRUPTED",
            "Text input has an invalid binary signature",
        ));
    }
    let markdown = std::str::from_utf8(bytes)
        .map_err(|_| error("INPUT_CORRUPTED", "Input is not valid UTF-8 text"))?
        .to_string();
    Ok(ParsedDocument {
        markdown,
        input_format,
        page_count: None,
        warnings: vec![],
    })
}

fn parse_csv(bytes: &[u8]) -> Result<ParsedDocument, AppError> {
    if bytes.is_empty() {
        return Err(error("INPUT_NO_CONTENT", "CSV input is empty"));
    }
    if bytes.len() > MAX_CSV_BYTES {
        return Err(error(
            "INPUT_LIMIT_EXCEEDED",
            "CSV input exceeds the size limit",
        ));
    }

    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    let text = std::str::from_utf8(bytes)
        .map_err(|_| error("INPUT_CORRUPTED", "CSV input is not valid UTF-8"))?;
    if text.trim().is_empty() {
        return Err(error("INPUT_NO_CONTENT", "CSV input is empty"));
    }

    let rows = parse_csv_rows(text.as_bytes())?;
    let columns = rows.first().map(Vec::len).unwrap_or(0);
    if columns == 0 {
        return Err(error("INPUT_NO_CONTENT", "CSV input has no columns"));
    }

    let mut markdown = String::new();
    let header = (1..=columns)
        .map(|index| format!("Column {index}"))
        .collect::<Vec<_>>();
    markdown.push_str(&render_csv_row(&header));
    markdown.push('\n');
    markdown.push_str(&format!("| {} |", vec!["---"; columns].join(" | ")));
    for row in rows {
        markdown.push('\n');
        markdown.push_str(&render_csv_row(&row));
    }

    Ok(ParsedDocument {
        markdown,
        input_format: InputFormat::Csv,
        page_count: None,
        warnings: vec![],
    })
}

fn parse_csv_rows(bytes: &[u8]) -> Result<Vec<Vec<String>>, AppError> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum State {
        FieldStart,
        Unquoted,
        Quoted,
        AfterQuote,
    }

    let mut state = State::FieldStart;
    let mut field = Vec::new();
    let mut row = Vec::new();
    let mut rows = Vec::new();
    let mut expected_columns = None;
    let mut index = 0;

    let push_field = |field: &mut Vec<u8>, row: &mut Vec<Vec<u8>>| {
        row.push(std::mem::take(field));
    };
    let push_row = |row: &mut Vec<Vec<u8>>,
                    rows: &mut Vec<Vec<String>>,
                    expected_columns: &mut Option<usize>|
     -> Result<(), AppError> {
        if row.is_empty() {
            return Err(error("INPUT_CORRUPTED", "CSV record has no fields"));
        }
        if row.len() > MAX_CSV_COLUMNS {
            return Err(error(
                "INPUT_LIMIT_EXCEEDED",
                "CSV column count exceeds the limit",
            ));
        }
        if let Some(expected) = *expected_columns {
            if row.len() != expected {
                return Err(error(
                    "INPUT_CORRUPTED",
                    "CSV records have inconsistent column counts",
                ));
            }
        } else {
            *expected_columns = Some(row.len());
        }
        let converted = row
            .drain(..)
            .map(|value| {
                String::from_utf8(value)
                    .map_err(|_| error("INPUT_CORRUPTED", "CSV field is not valid UTF-8"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        rows.push(converted);
        if rows.len() > MAX_CSV_RECORDS {
            return Err(error(
                "INPUT_LIMIT_EXCEEDED",
                "CSV record count exceeds the limit",
            ));
        }
        Ok(())
    };

    while index < bytes.len() {
        let byte = bytes[index];
        match state {
            State::FieldStart => match byte {
                b'"' => state = State::Quoted,
                b',' => push_field(&mut field, &mut row),
                b'\n' => {
                    push_field(&mut field, &mut row);
                    push_row(&mut row, &mut rows, &mut expected_columns)?;
                }
                b'\r' => {
                    if bytes.get(index + 1) != Some(&b'\n') {
                        return Err(error(
                            "INPUT_CORRUPTED",
                            "CSV contains an invalid line ending",
                        ));
                    }
                    push_field(&mut field, &mut row);
                    push_row(&mut row, &mut rows, &mut expected_columns)?;
                    index += 1;
                }
                _ => {
                    field.push(byte);
                    state = State::Unquoted;
                }
            },
            State::Unquoted => match byte {
                b',' => {
                    push_field(&mut field, &mut row);
                    state = State::FieldStart;
                }
                b'"' => return Err(error("INPUT_CORRUPTED", "CSV contains an unexpected quote")),
                b'\n' => {
                    push_field(&mut field, &mut row);
                    push_row(&mut row, &mut rows, &mut expected_columns)?;
                    state = State::FieldStart;
                }
                b'\r' => {
                    if bytes.get(index + 1) != Some(&b'\n') {
                        return Err(error(
                            "INPUT_CORRUPTED",
                            "CSV contains an invalid line ending",
                        ));
                    }
                    push_field(&mut field, &mut row);
                    push_row(&mut row, &mut rows, &mut expected_columns)?;
                    state = State::FieldStart;
                    index += 1;
                }
                _ => field.push(byte),
            },
            State::Quoted => match byte {
                b'"' => state = State::AfterQuote,
                _ => field.push(byte),
            },
            State::AfterQuote => match byte {
                b'"' => {
                    field.push(b'"');
                    state = State::Quoted;
                }
                b',' => {
                    push_field(&mut field, &mut row);
                    state = State::FieldStart;
                }
                b'\n' => {
                    push_field(&mut field, &mut row);
                    push_row(&mut row, &mut rows, &mut expected_columns)?;
                    state = State::FieldStart;
                }
                b'\r' => {
                    if bytes.get(index + 1) != Some(&b'\n') {
                        return Err(error(
                            "INPUT_CORRUPTED",
                            "CSV contains an invalid line ending",
                        ));
                    }
                    push_field(&mut field, &mut row);
                    push_row(&mut row, &mut rows, &mut expected_columns)?;
                    state = State::FieldStart;
                    index += 1;
                }
                _ => {
                    return Err(error(
                        "INPUT_CORRUPTED",
                        "CSV contains data after a closing quote",
                    ))
                }
            },
        }
        if field.len() > MAX_CSV_FIELD_BYTES {
            return Err(error(
                "INPUT_LIMIT_EXCEEDED",
                "CSV field exceeds the size limit",
            ));
        }
        index += 1;
    }

    match state {
        State::Quoted => return Err(error("INPUT_CORRUPTED", "CSV contains an unclosed quote")),
        State::AfterQuote | State::Unquoted | State::FieldStart => {
            if !field.is_empty()
                || !row.is_empty()
                || state == State::FieldStart
                    && index > 0
                    && bytes.last() != Some(&b'\n')
                    && bytes.last() != Some(&b'\r')
            {
                push_field(&mut field, &mut row);
                push_row(&mut row, &mut rows, &mut expected_columns)?;
            }
        }
    }

    if rows.is_empty() {
        return Err(error("INPUT_NO_CONTENT", "CSV input has no records"));
    }
    Ok(rows)
}

fn render_csv_row(row: &[String]) -> String {
    let cells = row
        .iter()
        .map(|value| escape_markdown_cell(value))
        .collect::<Vec<_>>();
    format!("| {} |", cells.join(" | "))
}

fn escape_markdown_cell(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "<br>")
        .replace('`', "\\`")
}

fn parse_excel(bytes: &[u8]) -> Result<ParsedDocument, AppError> {
    if bytes.is_empty() {
        return Err(error("INPUT_NO_CONTENT", "Excel input is empty"));
    }
    if bytes.len() > MAX_EXCEL_BYTES {
        return Err(error(
            "INPUT_LIMIT_EXCEEDED",
            "Excel input exceeds the size limit",
        ));
    }

    let outcome = panic::catch_unwind(|| parse_excel_inner(bytes));
    outcome.unwrap_or_else(|_| Err(error("INPUT_CORRUPTED", "Excel parser failed safely")))
}

fn parse_excel_inner(bytes: &[u8]) -> Result<ParsedDocument, AppError> {
    if bytes.starts_with(b"PK\x03\x04") {
        validate_xlsx_zip(bytes)?;
        let cursor = Cursor::new(bytes);
        let mut workbook = Xlsx::new(cursor).map_err(map_xlsx_error)?;
        return read_excel_workbook(&mut workbook);
    }

    if bytes.starts_with(&[0xd0, 0xcf, 0x11, 0xe0]) {
        let cursor = Cursor::new(bytes);
        match Xls::new(cursor) {
            Ok(mut workbook) => return read_excel_workbook(&mut workbook),
            Err(calamine::XlsError::Password) => {
                return Err(error(
                    "INPUT_ENCRYPTED",
                    "Encrypted Excel files are not supported",
                ))
            }
            Err(_) => {
                // Some password-protected .xlsx files use a CFB (OLE2) container
                // rather than a ZIP container; give the Xlsx reader a chance to
                // detect the encryption before treating the file as corrupted.
                let cursor = Cursor::new(bytes);
                return match Xlsx::new(cursor) {
                    Ok(mut workbook) => read_excel_workbook(&mut workbook),
                    Err(calamine::XlsxError::Password) => Err(error(
                        "INPUT_ENCRYPTED",
                        "Encrypted Excel files are not supported",
                    )),
                    Err(_) => Err(error(
                        "INPUT_CORRUPTED",
                        "XLS structure or content is invalid",
                    )),
                };
            }
        }
    }

    Err(error(
        "INPUT_CORRUPTED",
        "Excel input has an invalid binary signature",
    ))
}

fn validate_xlsx_zip(bytes: &[u8]) -> Result<(), AppError> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|_| error("INPUT_CORRUPTED", "XLSX zip structure is invalid"))?;
    let entries = archive.len();
    if entries > MAX_EXCEL_ZIP_ENTRIES {
        return Err(error(
            "INPUT_LIMIT_EXCEEDED",
            "XLSX zip entry count exceeds the limit",
        ));
    }
    let mut total: u64 = 0;
    for index in 0..entries {
        let file = archive
            .by_index(index)
            .map_err(|_| error("INPUT_CORRUPTED", "XLSX zip entry cannot be read"))?;
        total = total.saturating_add(file.size());
        if total > MAX_EXCEL_UNCOMPRESSED_BYTES {
            return Err(error(
                "INPUT_LIMIT_EXCEEDED",
                "XLSX uncompressed size exceeds the limit",
            ));
        }
        let name = file.name();
        if name.contains('\\') || name.starts_with('/') || name.contains("..") {
            return Err(error(
                "INPUT_CORRUPTED",
                "XLSX zip contains an invalid entry path",
            ));
        }
    }
    Ok(())
}

fn map_xlsx_error(err: calamine::XlsxError) -> AppError {
    use calamine::XlsxError;
    match err {
        XlsxError::Password => error("INPUT_ENCRYPTED", "Encrypted Excel files are not supported"),
        _ => error("INPUT_CORRUPTED", "XLSX structure or content is invalid"),
    }
}

fn read_excel_workbook<R, RB>(workbook: &mut RB) -> Result<ParsedDocument, AppError>
where
    R: Read + Seek,
    RB: Reader<R>,
{
    let sheets: Vec<(String, SheetType, SheetVisible)> = workbook
        .sheets_metadata()
        .iter()
        .map(|sheet| (sheet.name.clone(), sheet.typ, sheet.visible))
        .collect();
    if sheets.len() > MAX_EXCEL_SHEETS {
        return Err(error(
            "INPUT_LIMIT_EXCEEDED",
            "Excel workbook sheet count exceeds the limit",
        ));
    }

    let mut markdown = String::new();
    let mut total_cells: usize = 0;
    let mut has_content = false;
    let mut formula_present = false;

    for (name, typ, visible) in sheets {
        if typ != SheetType::WorkSheet || visible != SheetVisible::Visible {
            continue;
        }
        let range = workbook
            .worksheet_range(&name)
            .map_err(|_| error("INPUT_CORRUPTED", "Excel worksheet cannot be read"))?;
        let (height, width) = range.get_size();
        if width > MAX_EXCEL_COLUMNS {
            return Err(error(
                "INPUT_LIMIT_EXCEEDED",
                "Excel worksheet column count exceeds the limit",
            ));
        }
        if height > MAX_EXCEL_ROWS {
            return Err(error(
                "INPUT_LIMIT_EXCEEDED",
                "Excel worksheet row count exceeds the limit",
            ));
        }
        let cells = height.saturating_mul(width);
        total_cells = total_cells.saturating_add(cells);
        if total_cells > MAX_EXCEL_CELLS {
            return Err(error(
                "INPUT_LIMIT_EXCEEDED",
                "Excel total cell count exceeds the limit",
            ));
        }
        if height == 0 || width == 0 {
            continue;
        }

        if !markdown.is_empty() {
            markdown.push_str("\n\n");
        }
        markdown.push_str(&format!("## Sheet: {name}\n\n"));
        let header: Vec<String> = (1..=width).map(|index| format!("Column {index}")).collect();
        markdown.push_str(&render_excel_row(&header));
        markdown.push('\n');
        markdown.push_str(&format!("| {} |", vec!["---"; width].join(" | ")));
        for row in range.rows() {
            markdown.push('\n');
            let cells: Vec<String> = row.iter().map(format_excel_cell).collect();
            markdown.push_str(&render_excel_row(&cells));
        }
        has_content = true;

        if let Ok(formula_range) = workbook.worksheet_formula(&name) {
            if formula_range
                .rows()
                .any(|row| row.iter().any(|formula| !formula.is_empty()))
            {
                formula_present = true;
            }
        }
    }

    if !has_content {
        return Err(error(
            "INPUT_CORRUPTED",
            "Excel workbook contains no readable text",
        ));
    }

    let mut warnings = Vec::new();
    if formula_present {
        warnings.push(
            "Formula cells were not recalculated; cached workbook values are preserved."
                .to_string(),
        );
    }

    Ok(ParsedDocument {
        markdown,
        input_format: InputFormat::Excel,
        page_count: None,
        warnings,
    })
}

fn render_excel_row(row: &[String]) -> String {
    let cells = row
        .iter()
        .map(|value| escape_markdown_cell(value))
        .collect::<Vec<_>>();
    format!("| {} |", cells.join(" | "))
}

fn format_excel_cell(cell: &Data) -> String {
    match cell {
        Data::Int(value) => value.to_string(),
        Data::Float(value) => value.to_string(),
        Data::String(value) => value.clone(),
        Data::Bool(value) => value.to_string(),
        Data::DateTime(value) => value.to_string(),
        Data::DateTimeIso(value) => value.clone(),
        Data::DurationIso(value) => value.clone(),
        Data::Error(value) => value.to_string(),
        Data::Empty => String::new(),
    }
}

fn looks_like_binary_document(bytes: &[u8]) -> bool {
    bytes.starts_with(b"%PDF-")
        || bytes.starts_with(b"PK\x03\x04")
        || bytes.starts_with(&[0xd0, 0xcf, 0x11, 0xe0])
        || bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(b"GIF87a")
        || bytes.starts_with(b"GIF89a")
        || bytes.starts_with(b"RIFF")
}

/// Error code vocabulary produced by this parser boundary:
///
/// - `INPUT_CORRUPTED` — the input's structure cannot be parsed at all
///   (invalid ZIP/XML/PDF structure, illegal binary signature, invalid
///   UTF-8, malformed CSV shape, a required archive part is missing, ...).
/// - `INPUT_NO_CONTENT` — the input's structure is fully valid and was
///   parsed successfully, but it contains no extractable content (a
///   zero-byte file, a CSV with no records, a PPTX with slides that carry
///   no text, ...). This is distinct from `INPUT_CORRUPTED`: nothing is
///   broken, there is simply nothing to mask. Mirrors the
///   `component-runtime::OcrError::NoText` (`OCR_NO_TEXT`) precedent for
///   the OCR path.
/// - `INPUT_ENCRYPTED`, `INPUT_LIMIT_EXCEEDED`, `INPUT_FORMAT_UNSUPPORTED`,
///   `OCR_COMPONENT_REQUIRED` — unchanged, existing semantics.
///
/// Only `OCR_COMPONENT_REQUIRED` is retryable; every other code —
/// including `INPUT_NO_CONTENT` — resubmitting the exact same bytes can
/// never turn success, so it must not be retryable.
fn error(code: &str, message: &str) -> AppError {
    AppError {
        code: code.to_string(),
        message: message.to_string(),
        retryable: code == "OCR_COMPONENT_REQUIRED",
        safe_details: None,
    }
}

fn parse_powerpoint(bytes: &[u8]) -> Result<ParsedDocument, AppError> {
    if bytes.is_empty() {
        return Err(error("INPUT_NO_CONTENT", "PowerPoint input is empty"));
    }
    if bytes.len() > MAX_PPTX_BYTES {
        return Err(error(
            "INPUT_LIMIT_EXCEEDED",
            "PowerPoint input exceeds the size limit",
        ));
    }
    if bytes.starts_with(&[0xd0, 0xcf, 0x11, 0xe0]) {
        return Err(error(
            "INPUT_ENCRYPTED",
            "Encrypted PowerPoint files are not supported",
        ));
    }
    if !bytes.starts_with(b"PK\x03\x04") {
        return Err(error(
            "INPUT_CORRUPTED",
            "PowerPoint input has an invalid binary signature",
        ));
    }
    let outcome = panic::catch_unwind(|| parse_powerpoint_inner(bytes));
    outcome.unwrap_or_else(|_| Err(error("INPUT_CORRUPTED", "PowerPoint parser failed safely")))
}

fn parse_powerpoint_inner(bytes: &[u8]) -> Result<ParsedDocument, AppError> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|_| error("INPUT_CORRUPTED", "PPTX zip structure is invalid"))?;

    if archive.by_name("[Content_Types].xml").is_err() {
        return Err(error("INPUT_CORRUPTED", "PPTX structure is invalid"));
    }
    if archive.by_name("ppt/presentation.xml").is_err() {
        return Err(error("INPUT_CORRUPTED", "PPTX structure is invalid"));
    }

    let entries = archive.len();
    if entries > MAX_PPTX_ZIP_ENTRIES {
        return Err(error(
            "INPUT_LIMIT_EXCEEDED",
            "PPTX zip entry count exceeds the limit",
        ));
    }
    let mut total_uncompressed: u64 = 0;
    for index in 0..entries {
        let file = archive
            .by_index(index)
            .map_err(|_| error("INPUT_CORRUPTED", "PPTX zip entry cannot be read"))?;
        total_uncompressed = total_uncompressed.saturating_add(file.size());
        if total_uncompressed > MAX_PPTX_UNCOMPRESSED_BYTES {
            return Err(error(
                "INPUT_LIMIT_EXCEEDED",
                "PPTX uncompressed size exceeds the limit",
            ));
        }
        let name = file.name();
        if name.contains('\\') || name.starts_with('/') || name.contains("..") {
            return Err(error(
                "INPUT_CORRUPTED",
                "PPTX zip contains an invalid entry path",
            ));
        }
    }

    let slide_rels = read_presentation_slide_rels(&mut archive)?;
    let (slide_ids, has_hidden) = read_presentation_slide_ids(&mut archive)?;
    if slide_ids.is_empty() {
        return Err(error("INPUT_NO_CONTENT", "PPTX contains no slides"));
    }
    if slide_ids.len() > MAX_PPTX_SLIDES {
        return Err(error(
            "INPUT_LIMIT_EXCEEDED",
            "PPTX slide count exceeds the limit",
        ));
    }

    let mut markdown = String::new();
    let mut total_chars: usize = 0;
    let mut has_any_text = false;
    for (index, slide_rid) in slide_ids.iter().enumerate() {
        let slide_number = index + 1;
        let target = slide_rels
            .get(slide_rid)
            .ok_or_else(|| error("INPUT_CORRUPTED", "PPTX slide relationship is missing"))?;
        let slide_xml = read_zip_entry_to_string(&mut archive, target)?;
        let slide_text = extract_slide_text(&slide_xml)?;
        if slide_text.len() > MAX_PPTX_TEXT_BLOCK_BYTES {
            return Err(error(
                "INPUT_LIMIT_EXCEEDED",
                "PPTX text block exceeds the size limit",
            ));
        }
        total_chars = total_chars.saturating_add(slide_text.chars().count());
        if total_chars > MAX_PPTX_CHARS {
            return Err(error(
                "INPUT_LIMIT_EXCEEDED",
                "PPTX total text exceeds the limit",
            ));
        }

        if !markdown.is_empty() {
            markdown.push_str("\n\n");
        }
        markdown.push_str(&format!("## 幻灯片 {slide_number}\n\n"));
        if !slide_text.is_empty() {
            markdown.push_str(&slide_text);
            has_any_text = true;
        }
    }

    if !has_any_text {
        return Err(error("INPUT_NO_CONTENT", "PPTX contains no readable text"));
    }

    let mut warnings = Vec::new();
    if has_hidden {
        warnings.push("Hidden slides were included in extraction.".to_string());
    }

    Ok(ParsedDocument {
        markdown,
        input_format: InputFormat::Powerpoint,
        page_count: Some(slide_ids.len()),
        warnings,
    })
}

fn read_presentation_slide_rels<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<HashMap<String, String>, AppError> {
    let mut xml = String::new();
    archive
        .by_name("ppt/_rels/presentation.xml.rels")
        .map_err(|_| error("INPUT_CORRUPTED", "PPTX relationships are missing"))?
        .read_to_string(&mut xml)
        .map_err(|_| error("INPUT_CORRUPTED", "PPTX relationships cannot be read"))?;

    let mut reader = XmlReader::from_str(&xml);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut rels = HashMap::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(event)) | Ok(Event::Start(event))
                if local_name(event.name().as_ref()) == b"Relationship" =>
            {
                let mut id = None;
                let mut rel_type = None;
                let mut target = None;
                for attribute in event.attributes() {
                    let attribute = attribute.map_err(|_| {
                        error("INPUT_CORRUPTED", "PPTX relationship XML is invalid")
                    })?;
                    let name = attribute.key.as_ref();
                    if name.eq_ignore_ascii_case(b"Id") {
                        id = Some(String::from_utf8_lossy(&attribute.value).to_string());
                    } else if name.eq_ignore_ascii_case(b"Type") {
                        rel_type = Some(String::from_utf8_lossy(&attribute.value).to_string());
                    } else if name.eq_ignore_ascii_case(b"Target") {
                        target = Some(String::from_utf8_lossy(&attribute.value).to_string());
                    }
                }
                if let (Some(id), Some(rel_type), Some(target)) = (id, rel_type, target) {
                    if rel_type == PPTX_SLIDE_RELATIONSHIP_TYPE {
                        rels.insert(id, resolve_slide_target(&target));
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return Err(error("INPUT_CORRUPTED", "PPTX relationship XML is invalid")),
            _ => {}
        }
        buf.clear();
    }
    Ok(rels)
}

fn resolve_slide_target(target: &str) -> String {
    if let Some(stripped) = target.strip_prefix('/') {
        stripped.to_string()
    } else if target.starts_with("ppt/") {
        target.to_string()
    } else {
        format!("ppt/{target}")
    }
}

fn read_presentation_slide_ids<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<(Vec<String>, bool), AppError> {
    let mut xml = Vec::new();
    archive
        .by_name("ppt/presentation.xml")
        .map_err(|_| error("INPUT_CORRUPTED", "PPTX presentation is missing"))?
        .read_to_end(&mut xml)
        .map_err(|_| error("INPUT_CORRUPTED", "PPTX presentation cannot be read"))?;

    let mut reader = XmlReader::from_reader(&xml[..]);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut slide_ids = Vec::new();
    let mut inside_slide_id_list = false;
    let mut depth: usize = 0;
    let mut has_hidden = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => {
                depth = depth.saturating_add(1);
                if depth > MAX_PPTX_XML_DEPTH {
                    return Err(error(
                        "INPUT_LIMIT_EXCEEDED",
                        "PPTX presentation XML depth exceeds the limit",
                    ));
                }
                if local_name(event.name().as_ref()) == b"sldIdLst" {
                    inside_slide_id_list = true;
                } else if inside_slide_id_list && local_name(event.name().as_ref()) == b"sldId" {
                    parse_slide_id_attributes(&event, &mut slide_ids, &mut has_hidden)?;
                }
            }
            Ok(Event::Empty(event)) => {
                if inside_slide_id_list && local_name(event.name().as_ref()) == b"sldId" {
                    parse_slide_id_attributes(&event, &mut slide_ids, &mut has_hidden)?;
                }
            }
            Ok(Event::End(event)) => {
                if local_name(event.name().as_ref()) == b"sldIdLst" {
                    inside_slide_id_list = false;
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Err(_) => return Err(error("INPUT_CORRUPTED", "PPTX presentation XML is invalid")),
            _ => {}
        }
        buf.clear();
    }
    Ok((slide_ids, has_hidden))
}

fn parse_slide_id_attributes(
    event: &quick_xml::events::BytesStart,
    slide_ids: &mut Vec<String>,
    has_hidden: &mut bool,
) -> Result<(), AppError> {
    let mut rid = None;
    let mut hidden = false;
    for attribute in event.attributes() {
        let attribute =
            attribute.map_err(|_| error("INPUT_CORRUPTED", "PPTX slide ID XML is invalid"))?;
        let key = attribute.key;
        if matches!(key.prefix(), Some(prefix) if prefix.as_ref() == b"r")
            && key.local_name().as_ref() == b"id"
        {
            rid = Some(String::from_utf8_lossy(&attribute.value).to_string());
        } else if key.local_name().as_ref() == b"show" {
            hidden = attribute.value.as_ref() == b"0";
        }
    }
    if let Some(rid) = rid {
        slide_ids.push(rid);
        if hidden {
            *has_hidden = true;
        }
    }
    Ok(())
}

fn read_zip_entry_to_string<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Result<String, AppError> {
    let mut bytes = Vec::new();
    archive
        .by_name(name)
        .map_err(|_| error("INPUT_CORRUPTED", "PPTX slide part is missing"))?
        .read_to_end(&mut bytes)
        .map_err(|_| error("INPUT_CORRUPTED", "PPTX slide part cannot be read"))?;
    String::from_utf8(bytes)
        .map_err(|_| error("INPUT_CORRUPTED", "PPTX slide part is not valid UTF-8"))
}

fn extract_slide_text(xml: &str) -> Result<String, AppError> {
    let mut reader = XmlReader::from_str(xml);
    reader.trim_text(false);
    let mut buf = Vec::new();
    let mut paragraphs = Vec::new();
    let mut current = String::new();
    let mut inside_text = false;
    let mut depth: usize = 0;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => {
                depth = depth.saturating_add(1);
                if depth > MAX_PPTX_XML_DEPTH {
                    return Err(error(
                        "INPUT_LIMIT_EXCEEDED",
                        "PPTX slide XML depth exceeds the limit",
                    ));
                }
                if local_name(event.name().as_ref()) == b"t" {
                    inside_text = true;
                }
            }
            Ok(Event::Text(event)) if inside_text => {
                let text = event
                    .unescape()
                    .map_err(|_| error("INPUT_CORRUPTED", "PPTX slide XML is invalid"))?;
                current.push_str(&text);
            }
            Ok(Event::End(event)) => {
                if local_name(event.name().as_ref()) == b"t" {
                    inside_text = false;
                } else if local_name(event.name().as_ref()) == b"p" {
                    let trimmed = current.trim();
                    if !trimmed.is_empty() {
                        paragraphs.push(trimmed.to_string());
                    }
                    current.clear();
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Err(_) => return Err(error("INPUT_CORRUPTED", "PPTX slide XML is invalid")),
            _ => {}
        }
        buf.clear();
    }
    if !current.trim().is_empty() {
        paragraphs.push(current.trim().to_string());
    }

    Ok(paragraphs
        .into_iter()
        .map(|paragraph| escape_markdown_text(&paragraph))
        .collect::<Vec<_>>()
        .join("\n\n"))
}

fn escape_markdown_text(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('#', "\\#")
        .replace('|', "\\|")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace("\r\n", "\n")
        .replace('\r', "\n")
}

fn parse_docx(bytes: &[u8]) -> Result<ParsedDocument, AppError> {
    if bytes.starts_with(&[0xd0, 0xcf, 0x11, 0xe0]) {
        return Err(error("INPUT_ENCRYPTED", "Encrypted DOCX is not supported"));
    }
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|_| error("INPUT_CORRUPTED", "DOCX structure is invalid"))?;
    if archive.by_name("[Content_Types].xml").is_err() {
        return Err(error("INPUT_CORRUPTED", "DOCX structure is invalid"));
    }
    let mut xml = String::new();
    archive
        .by_name("word/document.xml")
        .map_err(|_| error("INPUT_CORRUPTED", "DOCX structure is invalid"))?
        .read_to_string(&mut xml)
        .map_err(|_| error("INPUT_CORRUPTED", "DOCX content cannot be read"))?;

    let markdown = docx_xml_to_markdown(&xml)?;
    Ok(ParsedDocument {
        markdown,
        input_format: InputFormat::Docx,
        page_count: None,
        warnings: vec![],
    })
}

fn local_name(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
}

fn docx_xml_to_markdown(xml: &str) -> Result<String, AppError> {
    let mut reader = XmlReader::from_str(xml);
    reader.trim_text(false);
    let mut buf = Vec::new();
    let mut paragraph = String::new();
    let mut row = Vec::<String>::new();
    let mut rows = Vec::<Vec<String>>::new();
    let mut blocks = Vec::<String>::new();
    let mut in_table = false;
    let mut in_cell = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => match local_name(event.name().as_ref()) {
                b"tbl" => in_table = true,
                b"tc" => {
                    in_cell = true;
                    paragraph.clear();
                }
                _ => {}
            },
            Ok(Event::Empty(event)) if local_name(event.name().as_ref()) == b"tab" => {
                paragraph.push('\t')
            }
            Ok(Event::Text(text)) => {
                paragraph.push_str(
                    &text
                        .unescape()
                        .map_err(|_| error("INPUT_CORRUPTED", "DOCX XML is invalid"))?,
                );
            }
            Ok(Event::End(event)) => match local_name(event.name().as_ref()) {
                b"p" if !in_cell && !in_table => {
                    let value = paragraph.trim().to_string();
                    if !value.is_empty() {
                        blocks.push(value);
                    }
                    paragraph.clear();
                }
                b"tc" => {
                    row.push(paragraph.trim().to_string());
                    paragraph.clear();
                    in_cell = false;
                }
                b"tr" => {
                    if !row.is_empty() {
                        rows.push(std::mem::take(&mut row));
                    }
                }
                b"tbl" => {
                    if !rows.is_empty() {
                        blocks.push(table_to_markdown(&rows));
                        rows.clear();
                    }
                    in_table = false;
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(_) => return Err(error("INPUT_CORRUPTED", "DOCX XML is invalid")),
            _ => {}
        }
        buf.clear();
    }
    let mut markdown = blocks.join("\n\n");
    if !markdown.is_empty() {
        markdown.push('\n');
    }
    Ok(markdown)
}

fn table_to_markdown(rows: &[Vec<String>]) -> String {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(1);
    let render = |row: &[String]| {
        let cells = (0..columns)
            .map(|index| {
                row.get(index)
                    .map(String::as_str)
                    .unwrap_or("")
                    .replace('|', "\\|")
            })
            .collect::<Vec<_>>();
        format!("| {} |", cells.join(" | "))
    };
    let mut lines = vec![render(&rows[0])];
    lines.push(format!("| {} |", vec!["---"; columns].join(" | ")));
    lines.extend(rows.iter().skip(1).map(|row| render(row)));
    lines.join("\n")
}

/// Extract each page's text via `parangi` (a Rust port of Apache PDFBox's
/// text-stripping engine), returning one string per page in page order.
///
/// Unlike `pdf-extract` 0.7 (the previous library), `parangi`'s font and
/// CMap handling returns `Result`/`Option` throughout instead of asserting
/// or indexing unchecked, so composite Type0/CID fonts — the standard PDF
/// mechanism for embedding CJK and other large glyph sets — no longer crash
/// the extractor (see TASK-PDF-CJK-TYPE0-TEXT-LAYER-COMPATIBILITY-001).
fn extract_pdf_text_by_pages(bytes: &[u8]) -> Result<Vec<String>, AppError> {
    let document = parangi::PdfDocument::from_bytes(bytes)
        .map_err(|_| error("INPUT_CORRUPTED", "PDF text layer cannot be read"))?;
    let doc_arc = document.inner_arc();
    let font_cache: parangi::stream::engine::FontCache = Default::default();
    let config = parangi::StripperConfig::default();

    let mut pages = Vec::with_capacity(document.page_count() as usize);
    for page_index in 0..document.page_count() {
        let page = document
            .page(page_index)
            .map_err(|_| error("INPUT_CORRUPTED", "PDF text layer cannot be read"))?;
        let content_bytes = page
            .content_bytes()
            .map_err(|_| error("INPUT_CORRUPTED", "PDF text layer cannot be read"))?;
        if content_bytes.is_empty() {
            pages.push(String::new());
            continue;
        }
        let resources = page
            .resources()
            .map_err(|_| error("INPUT_CORRUPTED", "PDF text layer cannot be read"))?;
        // A missing/invalid MediaBox does not prevent text extraction; fall
        // back to the default US Letter box used elsewhere in this parser.
        let media_box = page.media_box().unwrap_or([0.0, 0.0, 612.0, 792.0]);
        let rotation = page.rotation().unwrap_or(0) as i32;

        let mut engine = parangi::stream::engine::StreamEngine::with_font_cache(
            std::sync::Arc::clone(&doc_arc),
            font_cache.clone(),
        );
        engine.set_page_info(rotation, media_box[2], media_box[3]);
        if let Some(res) = &resources {
            engine.load_resources(res.dictionary());
        }
        engine
            .process_content(&content_bytes)
            .map_err(|_| error("INPUT_CORRUPTED", "PDF text layer cannot be read"))?;
        let mut positions = engine.into_text_positions();
        pages.push(parangi::text::assemble_text(&mut positions, &config));
    }
    Ok(pages)
}

fn parse_pdf(bytes: &[u8]) -> Result<ParsedDocument, AppError> {
    let outcome = panic::catch_unwind(|| {
        let document = lopdf::Document::load_mem(bytes)
            .map_err(|_| error("INPUT_CORRUPTED", "PDF structure is invalid"))?;
        // `Document::is_encrypted()` only recognises an `/Encrypt` trailer
        // entry that is an indirect reference; it misses PDFs — such as
        // AES-256 (V5/R6) files produced by PyMuPDF — that embed the
        // encryption dictionary directly (inline) in the trailer. Without
        // this fallback, such a file is not detected as encrypted at all
        // and falls through to text extraction on ciphertext, which
        // surfaces as either a spurious OCR_COMPONENT_REQUIRED or a raw
        // internal error from the OCR subprocess instead of INPUT_ENCRYPTED.
        if document.is_encrypted() || document.trailer.get(b"Encrypt").is_ok() {
            return Err(error("INPUT_ENCRYPTED", "Encrypted PDF is not supported"));
        }
        let page_count = document.get_pages().len();
        if page_count > MAX_PDF_PAGES {
            return Err(error("INPUT_LIMIT_EXCEEDED", "PDF page count exceeds 1000"));
        }
        let pages = extract_pdf_text_by_pages(bytes)?;
        if pages.iter().all(|page| page.trim().is_empty()) {
            return Err(error(
                "OCR_COMPONENT_REQUIRED",
                "PDF has no readable text layer",
            ));
        }
        let markdown = pages
            .iter()
            .enumerate()
            .map(|(index, page)| format!("## Page {}\n\n{}", index + 1, page.trim()))
            .collect::<Vec<_>>()
            .join("\n\n");
        Ok(ParsedDocument {
            markdown: format!("{}\n", markdown),
            input_format: InputFormat::Pdf,
            page_count: Some(page_count),
            warnings: vec![],
        })
    });
    outcome.unwrap_or_else(|_| Err(error("INPUT_CORRUPTED", "PDF parser failed safely")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FormatCatalog, LogicalFormat};
    use lopdf::dictionary;
    use std::io::Write;

    #[test]
    fn catalog_maps_supported_and_future_extensions_to_unique_logical_formats() {
        let cases = [
            ("fixture.TXT", LogicalFormat::Text, true),
            ("fixture.md", LogicalFormat::Markdown, true),
            ("fixture.markdown", LogicalFormat::Markdown, true),
            ("fixture.csv", LogicalFormat::Csv, true),
            ("fixture.xls", LogicalFormat::Excel, true),
            ("fixture.XLSX", LogicalFormat::Excel, true),
            ("fixture.doc", LogicalFormat::Word, false),
            ("fixture.docx", LogicalFormat::Word, true),
            ("fixture.pdf", LogicalFormat::Pdf, true),
            ("fixture.ppt", LogicalFormat::Powerpoint, true),
            ("fixture.PPTX", LogicalFormat::Powerpoint, true),
        ];

        for (filename, logical_format, enterprise_supported) in cases {
            let definition = FormatCatalog::from_filename(filename).unwrap();
            assert_eq!(definition.logical_format, logical_format, "{filename}");
            assert_eq!(
                definition.enterprise_supported, enterprise_supported,
                "{filename}"
            );
        }
        assert!(FormatCatalog::from_filename("fixture.json").is_none());
        assert!(FormatCatalog::from_filename(".txt").is_none());
    }

    #[test]
    fn shared_parser_handles_text_and_rejects_disguised_binary_text_inputs() {
        let text = parse_input(b"Phone 13900000000", InputFormat::Text).unwrap();
        assert_eq!(text.markdown, "Phone 13900000000");
        assert_eq!(text.input_format, InputFormat::Text);

        let markdown = parse_input(b"# Fixture\n", InputFormat::Markdown).unwrap();
        assert_eq!(markdown.markdown, "# Fixture\n");
        assert_eq!(markdown.input_format, InputFormat::Markdown);

        let invalid_utf8 = parse_input(&[0xff, 0xfe], InputFormat::Text).unwrap_err();
        assert_eq!(invalid_utf8.code, "INPUT_CORRUPTED");

        for bytes in [b"%PDF-1.7\n".as_slice(), b"PK\x03\x04".as_slice()] {
            let error = parse_input(bytes, InputFormat::Text).unwrap_err();
            assert_eq!(error.code, "INPUT_CORRUPTED");
        }
    }

    #[test]
    fn unsupported_catalog_formats_are_blocked_before_parsing() {
        for filename in ["fixture.doc", "fixture.DOC", "fixture.json"] {
            let error = FormatCatalog::enterprise_from_filename(filename).unwrap_err();
            assert_eq!(error.code, "INPUT_FORMAT_UNSUPPORTED", "{filename}");
        }
    }

    #[test]
    fn csv_parser_handles_utf8_bom_quotes_empty_fields_and_multiline_cells() {
        let bytes = b"\xef\xbb\xbfName,Notes,Empty\r\n\"Alice, A\",\"line one\r\nline two\",\r\n\"Bob\"\" Jr\",Chinese \xE4\xB8\xAD\xE6\x96\x87,done";
        let parsed = parse_input(bytes, InputFormat::Csv).unwrap();
        assert_eq!(parsed.input_format, InputFormat::Csv);
        assert_eq!(
            parsed.markdown,
            r#"| Column 1 | Column 2 | Column 3 |
| --- | --- | --- |
| Name | Notes | Empty |
| Alice, A | line one<br>line two |  |
| Bob" Jr | Chinese 中文 | done |"#
        );
    }

    #[test]
    fn csv_parser_escapes_markdown_structure_without_parsing_or_masking_values() {
        let parsed = parse_input(
            br#"a|b,c\d,e`f
1|2,3\4,5`6"#,
            InputFormat::Csv,
        )
        .unwrap();
        assert_eq!(
            parsed.markdown,
            r#"| Column 1 | Column 2 | Column 3 |
| --- | --- | --- |
| a\|b | c\\d | e\`f |
| 1\|2 | 3\\4 | 5\`6 |"#
        );
    }

    #[test]
    fn csv_parser_rejects_invalid_encoding_unclosed_quotes_and_shape_errors() {
        for bytes in [
            b"a,b\n1".as_slice(),
            b"a,b\n\"unterminated".as_slice(),
            b"a,b\n\"closed\"x,y".as_slice(),
            &[0xff, 0xfe],
        ] {
            let error = parse_input(bytes, InputFormat::Csv).unwrap_err();
            assert_eq!(error.code, "INPUT_CORRUPTED", "{bytes:?}");
        }
    }

    #[test]
    fn csv_parser_reports_no_content_for_empty_and_whitespace_only_input() {
        for bytes in [b"".as_slice(), b"   \n".as_slice()] {
            let error = parse_input(bytes, InputFormat::Csv).unwrap_err();
            assert_eq!(error.code, "INPUT_NO_CONTENT", "{bytes:?}");
            assert!(!error.retryable);
        }
    }

    #[test]
    fn csv_parser_reports_no_content_when_the_only_field_is_an_unterminated_flush() {
        // A lone empty quoted field ("") is not whitespace, so it passes the
        // empty/whitespace-only check, but the state machine ends in
        // AfterQuote with an empty field and an empty row: the final-flush
        // guard does not push anything, so `rows` stays genuinely empty and
        // this reaches the "no records" branch — a real, reachable path to
        // INPUT_NO_CONTENT, not just the empty/whitespace cases above.
        let error = parse_input(b"\"\"", InputFormat::Csv).unwrap_err();
        assert_eq!(error.code, "INPUT_NO_CONTENT");
        assert!(!error.retryable);
    }

    #[test]
    fn csv_parser_rejects_limits() {
        let too_many_columns = (0..=MAX_CSV_COLUMNS)
            .map(|_| "x")
            .collect::<Vec<_>>()
            .join(",");
        let error = parse_input(too_many_columns.as_bytes(), InputFormat::Csv).unwrap_err();
        assert_eq!(error.code, "INPUT_LIMIT_EXCEEDED");

        let too_long_field = "x".repeat(MAX_CSV_FIELD_BYTES + 1);
        let error = parse_input(too_long_field.as_bytes(), InputFormat::Csv).unwrap_err();
        assert_eq!(error.code, "INPUT_LIMIT_EXCEEDED");
    }

    fn sample_xlsx() -> Vec<u8> {
        include_bytes!("../tests/fixtures/sample.xlsx").to_vec()
    }

    fn sample_xls() -> Vec<u8> {
        include_bytes!("../tests/fixtures/sample.xls").to_vec()
    }

    fn formula_xlsx() -> Vec<u8> {
        include_bytes!("../tests/fixtures/formula.xlsx").to_vec()
    }

    fn encrypted_xlsx() -> Vec<u8> {
        include_bytes!("../tests/fixtures/encrypted.xlsx").to_vec()
    }

    fn wide_xlsx() -> Vec<u8> {
        include_bytes!("../tests/fixtures/wide.xlsx").to_vec()
    }

    #[test]
    fn excel_parser_handles_xlsx_multisheet_with_chinese_and_newlines() {
        let parsed = parse_input(&sample_xlsx(), InputFormat::Excel).unwrap();
        assert_eq!(parsed.input_format, InputFormat::Excel);
        assert!(parsed.markdown.contains("## Sheet: Sheet1"));
        assert!(parsed.markdown.contains("## Sheet: Sheet2"));
        assert!(parsed.markdown.contains("| Name | Phone | Email |"));
        assert!(parsed.markdown.contains("13900000000"));
        assert!(parsed.markdown.contains("alice@example.invalid"));
        assert!(parsed.markdown.contains("Line<br>break"));
        assert!(parsed.markdown.contains("中文"));
        assert!(parsed.warnings.is_empty());
    }

    #[test]
    fn excel_parser_handles_xls() {
        let parsed = parse_input(&sample_xls(), InputFormat::Excel).unwrap();
        assert_eq!(parsed.input_format, InputFormat::Excel);
        assert!(parsed.markdown.contains("## Sheet: Sheet1"));
        assert!(parsed.markdown.contains("## Sheet: Sheet2"));
        assert!(parsed.markdown.contains("13900000000"));
        assert!(parsed.markdown.contains("中文"));
        assert!(parsed.warnings.is_empty());
    }

    #[test]
    fn excel_parser_formula_cells_yield_warning_and_cached_value() {
        let parsed = parse_input(&formula_xlsx(), InputFormat::Excel).unwrap();
        assert_eq!(parsed.input_format, InputFormat::Excel);
        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.contains("Formula cells were not recalculated")));
    }

    #[test]
    fn excel_parser_rejects_encrypted_xlsx() {
        let error = parse_input(&encrypted_xlsx(), InputFormat::Excel).unwrap_err();
        assert_eq!(error.code, "INPUT_ENCRYPTED");
    }

    #[test]
    fn excel_parser_rejects_invalid_and_disguised_inputs() {
        for bytes in [
            b"not excel".as_slice(),
            b"PK\x03\x04".as_slice(),
            &[0xd0, 0xcf, 0x11, 0xe0],
        ] {
            let error = parse_input(bytes, InputFormat::Excel).unwrap_err();
            assert_eq!(error.code, "INPUT_CORRUPTED", "{bytes:?}");
        }

        let cursor = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::FileOptions::default();
        writer.start_file("word/document.xml", options).unwrap();
        writer.write_all(b"not an xlsx").unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let error = parse_input(&bytes, InputFormat::Excel).unwrap_err();
        assert_eq!(error.code, "INPUT_CORRUPTED");
    }

    #[test]
    fn excel_parser_reports_no_content_for_empty_input() {
        let error = parse_input(b"", InputFormat::Excel).unwrap_err();
        assert_eq!(error.code, "INPUT_NO_CONTENT");
        assert!(!error.retryable);
    }

    #[test]
    fn excel_parser_rejects_limits() {
        let error = parse_input(&wide_xlsx(), InputFormat::Excel).unwrap_err();
        assert_eq!(error.code, "INPUT_LIMIT_EXCEEDED");
    }

    #[test]
    fn docx_extracts_paragraphs_and_tables() {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::FileOptions::default();
        writer.start_file("[Content_Types].xml", options).unwrap();
        writer.write_all(b"<Types/>").unwrap();
        writer.start_file("word/document.xml", options).unwrap();
        writer.write_all(br#"<w:document xmlns:w="x"><w:body><w:p><w:r><w:t>Phone 13900000000</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Email</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>fixture@example.invalid</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#).unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let parsed = parse_document(&bytes, DocumentFormat::Docx).unwrap();
        assert!(parsed.markdown.contains("Phone 13900000000"));
        assert!(parsed
            .markdown
            .contains("| Email | fixture@example.invalid |"));
    }

    #[test]
    fn rejects_disguised_or_corrupted_docx() {
        let error = parse_document(b"not a zip", DocumentFormat::Docx).unwrap_err();
        assert_eq!(error.code, "INPUT_CORRUPTED");
        assert!(!error.retryable);
    }

    #[test]
    fn ocr_component_required_is_retryable_and_other_parser_errors_are_not() {
        let mut document = lopdf::Document::with_version("1.5");
        let pages_id = document.new_object_id();
        let page_id = document.new_object_id();
        let content_id = document.add_object(lopdf::Stream::new(dictionary! {}, vec![]));
        let resources_id = document.add_object(dictionary! {});
        document.objects.insert(
            page_id,
            lopdf::Object::Dictionary(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "Contents" => content_id,
                "Resources" => resources_id,
                "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            }),
        );
        document.objects.insert(
            pages_id,
            lopdf::Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![page_id.into()],
                "Count" => 1,
            }),
        );
        let catalog_id = document.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        document.trailer.set("Root", catalog_id);
        let mut bytes = Vec::new();
        document.save_to(&mut bytes).unwrap();

        let ocr_error = parse_document(&bytes, DocumentFormat::Pdf).unwrap_err();
        assert_eq!(ocr_error.code, "OCR_COMPONENT_REQUIRED");
        assert!(ocr_error.retryable);

        for code in [
            "INPUT_CORRUPTED",
            "INPUT_ENCRYPTED",
            "INPUT_LIMIT_EXCEEDED",
            "INPUT_NO_CONTENT",
        ] {
            assert!(!error(code, "safe parser error").retryable, "{code}");
        }
    }

    #[test]
    fn pdf_parser_rejects_structurally_corrupt_input() {
        let error = parse_document(b"not a pdf at all", DocumentFormat::Pdf).unwrap_err();
        assert_eq!(error.code, "INPUT_CORRUPTED");
        assert!(!error.retryable);
    }

    #[test]
    fn pdf_parser_rejects_encrypted_pdf() {
        let mut document = lopdf::Document::with_version("1.5");
        let pages_id = document.new_object_id();
        let page_id = document.new_object_id();
        let content_id = document.add_object(lopdf::Stream::new(dictionary! {}, vec![]));
        let resources_id = document.add_object(dictionary! {});
        document.objects.insert(
            page_id,
            lopdf::Object::Dictionary(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "Contents" => content_id,
                "Resources" => resources_id,
                "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            }),
        );
        document.objects.insert(
            pages_id,
            lopdf::Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![page_id.into()],
                "Count" => 1,
            }),
        );
        let catalog_id = document.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        let encrypt_id = document.add_object(dictionary! {
            "Filter" => "Standard",
            "V" => 1,
            "R" => 2,
            "O" => lopdf::Object::string_literal(vec![0u8; 32]),
            "U" => lopdf::Object::string_literal(vec![0u8; 32]),
            "P" => -4,
        });
        document.trailer.set("Root", catalog_id);
        document.trailer.set("Encrypt", encrypt_id);
        let mut bytes = Vec::new();
        document.save_to(&mut bytes).unwrap();

        let error = parse_document(&bytes, DocumentFormat::Pdf).unwrap_err();
        assert_eq!(error.code, "INPUT_ENCRYPTED");
        assert!(!error.retryable);
    }

    /// Real AES-256 (V5/R6) encrypted PDFs, as produced by PyMuPDF, embed
    /// the `/Encrypt` dictionary directly (inline) in the trailer rather
    /// than as an indirect reference. `lopdf::Document::is_encrypted()`
    /// alone misses this shape; discovered via a real Runtime submission
    /// while validating TASK-PDF-CJK-TYPE0-TEXT-LAYER-COMPATIBILITY-001 —
    /// without the trailer fallback this fell through to text extraction on
    /// ciphertext instead of being rejected as INPUT_ENCRYPTED.
    #[test]
    fn pdf_parser_rejects_encrypted_pdf_with_inline_encrypt_dictionary() {
        let mut document = lopdf::Document::with_version("1.7");
        let pages_id = document.new_object_id();
        let page_id = document.new_object_id();
        let content_id = document.add_object(lopdf::Stream::new(dictionary! {}, vec![]));
        let resources_id = document.add_object(dictionary! {});
        document.objects.insert(
            page_id,
            lopdf::Object::Dictionary(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "Contents" => content_id,
                "Resources" => resources_id,
                "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            }),
        );
        document.objects.insert(
            pages_id,
            lopdf::Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![page_id.into()],
                "Count" => 1,
            }),
        );
        let catalog_id = document.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        document.trailer.set("Root", catalog_id);
        // Inline dictionary value, NOT an indirect reference — this is the
        // shape that `is_encrypted()` alone fails to recognise.
        document.trailer.set(
            "Encrypt",
            lopdf::Object::Dictionary(dictionary! {
                "Filter" => "Standard",
                "V" => 5,
                "R" => 6,
            }),
        );
        let mut bytes = Vec::new();
        document.save_to(&mut bytes).unwrap();

        let error = parse_document(&bytes, DocumentFormat::Pdf).unwrap_err();
        assert_eq!(error.code, "INPUT_ENCRYPTED");
        assert!(!error.retryable);
    }

    #[test]
    fn pdf_parser_rejects_page_count_over_limit() {
        let mut document = lopdf::Document::with_version("1.5");
        let pages_id = document.new_object_id();
        let content_id = document.add_object(lopdf::Stream::new(dictionary! {}, vec![]));
        let resources_id = document.add_object(dictionary! {});
        let kids: Vec<lopdf::Object> = (0..=MAX_PDF_PAGES)
            .map(|_| {
                let page_id = document.new_object_id();
                document.objects.insert(
                    page_id,
                    lopdf::Object::Dictionary(dictionary! {
                        "Type" => "Page",
                        "Parent" => pages_id,
                        "Contents" => content_id,
                        "Resources" => resources_id,
                        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
                    }),
                );
                page_id.into()
            })
            .collect();
        let count = kids.len();
        document.objects.insert(
            pages_id,
            lopdf::Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => kids,
                "Count" => count as i64,
            }),
        );
        let catalog_id = document.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        document.trailer.set("Root", catalog_id);
        let mut bytes = Vec::new();
        document.save_to(&mut bytes).unwrap();

        let error = parse_document(&bytes, DocumentFormat::Pdf).unwrap_err();
        assert_eq!(error.code, "INPUT_LIMIT_EXCEEDED");
        assert!(!error.retryable);
    }

    // --- Type0/CID text-layer PDF compatibility (TASK-PDF-CJK-TYPE0-TEXT-LAYER-COMPATIBILITY-001) ---
    //
    // Real-world fixtures generated with PyMuPDF (constructions 1-3, matching
    // the architect's independently reproduced classification) and real
    // headless LibreOffice (construction 4), verified via PyMuPDF's own font
    // introspection before being committed:
    //   1. Type0, NOT embedded, UniGB-UTF16-H encoding (Chinese)
    //   2. Type0, embedded (subset), Identity-H encoding (Chinese)
    //   3. Type0, embedded (subset), Identity-H encoding (pure ASCII body)
    //   4. Simple TrueType/Type1 subset, not Type0 (Chinese) — must keep working

    fn pdf_type0_unembedded_unigb_chinese() -> Vec<u8> {
        include_bytes!("../tests/fixtures/pdf_type0_unembedded_unigb_chinese.pdf").to_vec()
    }

    fn pdf_type0_embedded_identityh_chinese() -> Vec<u8> {
        include_bytes!("../tests/fixtures/pdf_type0_embedded_identityh_chinese.pdf").to_vec()
    }

    fn pdf_type0_embedded_identityh_ascii() -> Vec<u8> {
        include_bytes!("../tests/fixtures/pdf_type0_embedded_identityh_ascii.pdf").to_vec()
    }

    fn pdf_simple_truetype_chinese() -> Vec<u8> {
        include_bytes!("../tests/fixtures/pdf_simple_truetype_chinese.pdf").to_vec()
    }

    #[test]
    fn pdf_type0_unembedded_cid_font_chinese_extracts_text() {
        let parsed = parse_document(&pdf_type0_unembedded_unigb_chinese(), DocumentFormat::Pdf)
            .expect("Type0/unembedded/UniGB-UTF16-H PDF must not be reported as corrupted");
        assert!(
            parsed.markdown.contains("13988889999"),
            "{}",
            parsed.markdown
        );
        assert!(parsed.markdown.contains("田十三"), "{}", parsed.markdown);
    }

    #[test]
    fn pdf_type0_embedded_identity_h_chinese_extracts_text() {
        let parsed = parse_document(&pdf_type0_embedded_identityh_chinese(), DocumentFormat::Pdf)
            .expect("Type0/embedded/Identity-H (Chinese) PDF must not be reported as corrupted");
        assert!(
            parsed.markdown.contains("13988889999"),
            "{}",
            parsed.markdown
        );
        assert!(parsed.markdown.contains("田十三"), "{}", parsed.markdown);
    }

    #[test]
    fn pdf_type0_embedded_identity_h_ascii_extracts_text() {
        let parsed = parse_document(&pdf_type0_embedded_identityh_ascii(), DocumentFormat::Pdf)
            .expect("Type0/embedded/Identity-H (ASCII body) PDF must not be reported as corrupted");
        assert!(
            parsed.markdown.contains("13988889999"),
            "{}",
            parsed.markdown
        );
    }

    #[test]
    fn pdf_simple_truetype_chinese_still_extracts_text() {
        let parsed = parse_document(&pdf_simple_truetype_chinese(), DocumentFormat::Pdf)
            .expect("simple TrueType/Type1 PDF must keep working");
        assert!(
            parsed.markdown.contains("13988889999"),
            "{}",
            parsed.markdown
        );
        assert!(parsed.markdown.contains("田十三"), "{}", parsed.markdown);
    }

    fn fictional_pptx() -> Vec<u8> {
        include_bytes!("../tests/fixtures/fictional.pptx").to_vec()
    }

    fn scrambled_pptx() -> Vec<u8> {
        include_bytes!("../tests/fixtures/scrambled.pptx").to_vec()
    }

    fn empty_pptx() -> Vec<u8> {
        include_bytes!("../tests/fixtures/empty.pptx").to_vec()
    }

    #[test]
    fn powerpoint_parser_extracts_chinese_phones_emails_lists_and_special_chars() {
        let parsed = parse_input(&fictional_pptx(), InputFormat::Powerpoint).unwrap();
        assert_eq!(parsed.input_format, InputFormat::Powerpoint);
        assert_eq!(parsed.page_count, Some(4));
        assert!(
            parsed
                .warnings
                .iter()
                .any(|warning| warning.contains("Hidden slides")),
            "{:?}",
            parsed.warnings
        );
        let markdown = &parsed.markdown;
        assert!(markdown.contains("## 幻灯片 1"));
        assert!(markdown.contains("## 幻灯片 2"));
        assert!(markdown.contains("## 幻灯片 3"));
        assert!(markdown.contains("## 幻灯片 4"));
        assert!(markdown.contains("项目启动会议"));
        assert!(markdown.contains("13900000000"));
        assert!(markdown.contains("alice@example.invalid"));
        assert!(markdown.contains("13800138000"));
        assert!(markdown.contains("bob@example.invalid"));
        assert!(markdown.contains("13700137000"));
        assert!(markdown.contains("特殊字符测试"));
        // Blank slide retains a heading boundary but contributes no text.
        assert!(markdown.contains("## 幻灯片 2\n\n"));
    }

    #[test]
    fn powerpoint_parser_follows_relationship_order_not_slide_filename_order() {
        let parsed = parse_input(&scrambled_pptx(), InputFormat::Powerpoint).unwrap();
        assert_eq!(parsed.page_count, Some(4));
        let markdown = parsed.markdown;
        let hidden_pos = markdown
            .find("Hidden slide content")
            .expect("slide 4 content first");
        let title_pos = markdown
            .find("项目启动会议")
            .expect("slide 1 content second");
        let agenda_pos = markdown.find("议程安排").expect("slide 3 content last");
        assert!(
            hidden_pos < title_pos && title_pos < agenda_pos,
            "relationship order should be slide 4, 1, 2, 3"
        );
    }

    #[test]
    fn powerpoint_parser_reports_no_content_for_fully_empty_slides() {
        // empty.pptx is structurally valid — real ZIP, real
        // presentation.xml, 2 real slides — it simply has no text on any
        // slide. This must NOT be reported as corrupted (see
        // TASK-EMPTY-CONTENT-ERROR-CLASSIFICATION-001).
        let error = parse_input(&empty_pptx(), InputFormat::Powerpoint).unwrap_err();
        assert_eq!(error.code, "INPUT_NO_CONTENT");
        assert!(!error.retryable);
    }

    #[test]
    fn powerpoint_parser_reports_no_content_for_empty_input() {
        let error = parse_input(b"", InputFormat::Powerpoint).unwrap_err();
        assert_eq!(error.code, "INPUT_NO_CONTENT");
        assert!(!error.retryable);
    }

    #[test]
    fn powerpoint_parser_rejects_encrypted_and_invalid_inputs() {
        for bytes in [
            b"not a powerpoint".as_slice(),
            b"PK\x03\x04".as_slice(),
            &[0xd0, 0xcf, 0x11, 0xe0, 0x00, 0x01],
        ] {
            let error = parse_input(bytes, InputFormat::Powerpoint).unwrap_err();
            assert!(
                matches!(error.code.as_str(), "INPUT_CORRUPTED" | "INPUT_ENCRYPTED"),
                "unexpected code {} for {:?}",
                error.code,
                bytes
            );
        }

        let cursor = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::FileOptions::default();
        writer.start_file("[Content_Types].xml", options).unwrap();
        writer.write_all(b"<Types/>").unwrap();
        writer.start_file("ppt/presentation.xml", options).unwrap();
        writer.write_all(b"<p:presentation/>").unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let error = parse_input(&bytes, InputFormat::Powerpoint).unwrap_err();
        // The required ppt/_rels/presentation.xml.rels part is missing —
        // this is a genuine structural defect (a required archive part is
        // absent), not merely "no slides", so it stays INPUT_CORRUPTED.
        assert_eq!(error.code, "INPUT_CORRUPTED");
    }

    #[test]
    fn powerpoint_parser_reports_no_content_when_presentation_lists_no_slides() {
        // A structurally complete PPTX (all required parts present, valid
        // relationships) whose <p:sldIdLst> is simply empty: nothing is
        // broken, there are just no slides to extract content from.
        let cursor = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::FileOptions::default();
        writer.start_file("[Content_Types].xml", options).unwrap();
        writer.write_all(b"<Types/>").unwrap();
        writer
            .start_file("ppt/_rels/presentation.xml.rels", options)
            .unwrap();
        writer.write_all(b"<Relationships/>").unwrap();
        writer.start_file("ppt/presentation.xml", options).unwrap();
        writer
            .write_all(
                b"<p:presentation xmlns:p=\"p\" xmlns:r=\"r\"><p:sldIdLst/></p:presentation>",
            )
            .unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let error = parse_input(&bytes, InputFormat::Powerpoint).unwrap_err();
        assert_eq!(error.code, "INPUT_NO_CONTENT");
        assert!(!error.retryable);
    }

    #[test]
    fn powerpoint_parser_rejects_zip_slip_and_entry_limit() {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::FileOptions::default();
        writer.start_file("../slide1.xml", options).unwrap();
        writer.write_all(b"<xml/>").unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let error = parse_input(&bytes, InputFormat::Powerpoint).unwrap_err();
        assert_eq!(error.code, "INPUT_CORRUPTED");

        let cursor = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::FileOptions::default();
        writer.start_file("[Content_Types].xml", options).unwrap();
        writer.write_all(b"<Types/>").unwrap();
        writer.start_file("ppt/presentation.xml", options).unwrap();
        writer.write_all(b"<p:presentation/>").unwrap();
        for index in 0..=MAX_PPTX_ZIP_ENTRIES {
            writer
                .start_file(format!("entry{index}.txt"), options)
                .unwrap();
            writer.write_all(b"x").unwrap();
        }
        let bytes = writer.finish().unwrap().into_inner();
        let error = parse_input(&bytes, InputFormat::Powerpoint).unwrap_err();
        assert_eq!(error.code, "INPUT_LIMIT_EXCEEDED");
    }

    #[test]
    fn powerpoint_parser_rejects_slide_count_limit() {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::FileOptions::default();
        writer.start_file("[Content_Types].xml", options).unwrap();
        writer.write_all(b"<Types/>").unwrap();
        writer
            .start_file("ppt/_rels/presentation.xml.rels", options)
            .unwrap();
        writer.write_all(b"<Relationships/>").unwrap();
        writer.start_file("ppt/presentation.xml", options).unwrap();
        let mut ids = String::new();
        for index in 0..=MAX_PPTX_SLIDES {
            ids.push_str(&format!("<p:sldId r:id=\"rId{index}\"/>"));
        }
        writer
            .write_all(format!("<p:presentation xmlns:p=\"p\" xmlns:r=\"r\"><p:sldIdLst>{ids}</p:sldIdLst></p:presentation>").as_bytes())
            .unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let error = parse_input(&bytes, InputFormat::Powerpoint).unwrap_err();
        assert_eq!(error.code, "INPUT_LIMIT_EXCEEDED");
    }
}
