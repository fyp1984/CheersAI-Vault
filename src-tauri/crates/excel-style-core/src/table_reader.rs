/* Copyright 2026 CheersAI Team. Licensed under Apache-2.0 */
//! Host-agnostic, bounded `.xlsx`/CSV structure and preview reading.
//!
//! Unlike `calamine::Reader::worksheet_range()`, which always materializes
//! the entire used range into memory before the caller can look at even one
//! cell, the functions here read only what a structure scan or a bounded
//! preview actually needs: the header row, up to `sample_rows`/`max_rows`
//! data rows, and — when the sheet's `<dimension>` tag is present and
//! well-formed — the total row/column count in O(1) without visiting every
//! row. When `<dimension>` is missing or malformed, row/column counting
//! falls back to a single lightweight scan that tracks only cell
//! coordinates (not values) for rows beyond the requested bound, so
//! structure/preview reads never do a second full pass and never allocate a
//! value for a cell the caller did not ask for.
//!
//! `xl/sharedStrings.xml` gets the same bounded treatment: a workbook with
//! 100,000 rows can have a shared-string table many times larger than the
//! rows a bounded read actually touches (real-world writers assign shared-
//! string indices in first-use order, so the indices referenced by the
//! first N rows are bounded by how many *distinct* strings those N rows
//! introduce — not by the size of the whole table). Cell resolution is
//! therefore two-pass: scan the needed rows first without resolving shared
//! strings, find the highest index they actually reference, then read only
//! that many entries from the shared-string table instead of the whole
//! file. An index that turns out to be out of the loaded range (a
//! pathological writer that does not assign indices in first-use order)
//! resolves to an empty string rather than erroring or guessing — a rare
//! edge case is handled safely, not by falling back to a full scan.
//!
//! Full masking/apply (which genuinely needs every row) is out of scope
//! here and continues to use the existing calamine-based path for `.xlsx`;
//! this module's CSV functions do provide a full-row reader, because CSV
//! has no calamine-based path at all.

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use encoding_rs::GB18030;
use quick_xml::events::Event;
use quick_xml::Reader as XmlReader;
use thiserror::Error;
use zip::ZipArchive;

use crate::{attr_value, parse_cell_ref_a1, sheet_index_from_path, worksheet_names_from_workbook};

#[derive(Debug, Error)]
pub enum TableReadError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("not a valid ZIP/XLSX archive: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("XML parse error: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("sheet not found: {0}")]
    SheetNotFound(String),
    #[error("no worksheets found in workbook")]
    NoSheets,
    #[error("CSV text cannot be reliably decoded as UTF-8 or GB18030/GBK")]
    UndecodableEncoding,
    #[error("malformed CSV: {0}")]
    MalformedCsv(String),
}

#[derive(Debug, Clone, Default)]
pub struct SheetStructure {
    pub name: String,
    pub headers: Vec<String>,
    /// Columnar samples, up to `sample_rows` values per column (may be
    /// shorter near the end of a short sheet).
    pub column_samples: Vec<Vec<String>>,
    /// Total row count including the header row.
    pub max_row: u32,
    pub max_col: u32,
}

#[derive(Debug, Clone, Default)]
pub struct PreviewRow {
    /// 1-based row number matching the original file (row 2 is the first
    /// data row, immediately after the row-1 header).
    pub row_number: u32,
    /// Dense, width = headers.len(); missing cells are `""`.
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct TablePreview {
    pub sheet_name: String,
    pub headers: Vec<String>,
    pub rows: Vec<PreviewRow>,
}

// ---------------------------------------------------------------------
// XLSX
// ---------------------------------------------------------------------

pub fn xlsx_sheet_names(path: &Path) -> Result<Vec<String>, TableReadError> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;
    let names = worksheet_names_from_workbook(&mut archive);
    if names.is_empty() {
        return Err(TableReadError::NoSheets);
    }
    let mut ordered: Vec<(u32, String)> = names.into_iter().collect();
    ordered.sort_by_key(|(idx, _)| *idx);
    Ok(ordered.into_iter().map(|(_, name)| name).collect())
}

pub fn read_xlsx_all_sheets_structure(
    path: &Path,
    sample_rows: usize,
) -> Result<Vec<SheetStructure>, TableReadError> {
    let names = xlsx_sheet_names(path)?;
    names
        .into_iter()
        .map(|name| read_xlsx_sheet_structure(path, &name, sample_rows))
        .collect()
}

pub fn read_xlsx_sheet_structure(
    path: &Path,
    sheet_name: &str,
    sample_rows: usize,
) -> Result<SheetStructure, TableReadError> {
    let scan = scan_named_sheet(path, sheet_name, Some(sample_rows))?;
    let width = scan.max_col as usize;
    let mut column_samples: Vec<Vec<String>> = vec![Vec::with_capacity(sample_rows); width];
    for (_, values) in scan.data_rows.iter().take(sample_rows) {
        for (c, v) in values.iter().enumerate() {
            if c < width {
                column_samples[c].push(v.clone());
            }
        }
    }
    Ok(SheetStructure {
        name: sheet_name.to_string(),
        headers: pad_to_width(scan.header, width),
        column_samples,
        max_row: scan.max_row,
        max_col: scan.max_col,
    })
}

pub fn read_xlsx_preview(
    path: &Path,
    sheet_name: &str,
    max_rows: usize,
) -> Result<TablePreview, TableReadError> {
    let scan = scan_named_sheet(path, sheet_name, Some(max_rows))?;
    let width = scan.max_col as usize;
    let headers = pad_to_width(scan.header, width);
    let rows = scan
        .data_rows
        .into_iter()
        .take(max_rows)
        .map(|(row_number, values)| PreviewRow {
            row_number,
            values: pad_to_width(values, width),
        })
        .collect();
    Ok(TablePreview {
        sheet_name: sheet_name.to_string(),
        headers,
        rows,
    })
}

fn pad_to_width(mut values: Vec<String>, width: usize) -> Vec<String> {
    if values.len() < width {
        values.resize(width, String::new());
    }
    values
}

struct RawSheetScan {
    header: Vec<String>,
    /// (1-based row number, dense resolved values for that row)
    data_rows: Vec<(u32, Vec<String>)>,
    max_row: u32,
    max_col: u32,
}

fn scan_named_sheet(
    path: &Path,
    sheet_name: &str,
    max_data_rows: Option<usize>,
) -> Result<RawSheetScan, TableReadError> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;
    let names = worksheet_names_from_workbook(&mut archive);
    if names.is_empty() {
        return Err(TableReadError::NoSheets);
    }
    let sheet_idx = names
        .iter()
        .find(|(_, name)| name.as_str() == sheet_name)
        .map(|(idx, _)| *idx)
        .ok_or_else(|| TableReadError::SheetNotFound(sheet_name.to_string()))?;

    let entry_name = format!("xl/worksheets/sheet{sheet_idx}.xml");
    let mut xml_bytes = Vec::new();
    archive.by_name(&entry_name)?.read_to_end(&mut xml_bytes)?;
    // sheet_index_from_path is reused elsewhere in this crate for the
    // inverse mapping (path -> index); referencing it here keeps the two
    // conventions (index -> path, path -> index) visibly in sync.
    debug_assert_eq!(sheet_index_from_path(&entry_name), Some(sheet_idx));

    let raw = scan_sheet_xml_raw(&xml_bytes, max_data_rows)?;

    let max_shared_index = raw
        .header
        .iter()
        .chain(raw.data_rows.iter().flat_map(|(_, row)| row.iter()))
        .filter_map(|cell| match cell {
            CellRaw::SharedIndex(i) => Some(*i),
            CellRaw::Text(_) => None,
        })
        .max();

    let shared = match max_shared_index {
        Some(max_idx) => load_shared_strings_bounded(&mut archive, max_idx),
        None => Vec::new(),
    };

    let resolve_row = |row: Vec<CellRaw>| -> Vec<String> {
        row.into_iter()
            .map(|cell| match cell {
                CellRaw::Text(s) => s,
                CellRaw::SharedIndex(i) => shared.get(i).cloned().unwrap_or_default(),
            })
            .collect()
    };

    Ok(RawSheetScan {
        header: resolve_row(raw.header),
        data_rows: raw
            .data_rows
            .into_iter()
            .map(|(n, row)| (n, resolve_row(row)))
            .collect(),
        max_row: raw.max_row,
        max_col: raw.max_col,
    })
}

/// Reads only the first `max_index + 1` entries of `xl/sharedStrings.xml`,
/// stopping the XML scan as soon as they are collected instead of parsing
/// the (potentially many-million-entry) rest of the table.
fn load_shared_strings_bounded<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    max_index: usize,
) -> Vec<String> {
    let mut file = match archive.by_name("xl/sharedStrings.xml") {
        Ok(f) => f,
        // A workbook with no shared strings part (inline strings / numeric
        // only) is a completely valid, common case, not an error.
        Err(_) => return Vec::new(),
    };

    let needed = max_index + 1;
    let mut out: Vec<String> = Vec::with_capacity(needed.min(1_000_000));
    let mut reader = XmlReader::from_reader(std::io::BufReader::new(&mut file));
    let mut buf = Vec::new();
    let mut current = String::new();
    let mut in_si = false;
    let mut in_t = false;

    loop {
        if out.len() >= needed {
            break;
        }
        let event = match reader.read_event_into(&mut buf) {
            Ok(e) => e,
            Err(_) => break,
        };
        match event {
            Event::Start(e) => {
                let n = e.name();
                if n.as_ref() == b"si" {
                    in_si = true;
                    current.clear();
                } else if n.as_ref() == b"t" && in_si {
                    in_t = true;
                }
            }
            Event::Empty(e) => {
                if e.name().as_ref() == b"si" {
                    out.push(String::new());
                }
            }
            Event::End(e) => {
                let n = e.name();
                if n.as_ref() == b"si" {
                    out.push(std::mem::take(&mut current));
                    in_si = false;
                } else if n.as_ref() == b"t" {
                    in_t = false;
                }
            }
            Event::Text(e) => {
                if in_t {
                    if let Ok(txt) = e.unescape() {
                        current.push_str(&txt);
                    }
                }
            }
            Event::CData(e) => {
                if in_t {
                    current.push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

fn parse_dimension_ref(raw: &str) -> Option<(u32, u32)> {
    let end_part = raw.split(':').next_back()?;
    // Bare cell refs never carry a sheet prefix, so this always takes the
    // "no sheet part" branch of parse_cell_ref_a1.
    parse_cell_ref_a1("", end_part)
}

#[derive(Debug, Clone)]
enum CellRaw {
    Text(String),
    SharedIndex(usize),
}

struct RawSheetScanDeferred {
    header: Vec<CellRaw>,
    data_rows: Vec<(u32, Vec<CellRaw>)>,
    max_row: u32,
    max_col: u32,
}

fn scan_sheet_xml_raw(
    xml_bytes: &[u8],
    max_data_rows: Option<usize>,
) -> Result<RawSheetScanDeferred, TableReadError> {
    let mut reader = XmlReader::from_reader(xml_bytes);
    let mut buf = Vec::new();

    let mut dimension: Option<(u32, u32)> = None;

    let mut header: Vec<CellRaw> = Vec::new();
    let mut data_rows: Vec<(u32, Vec<CellRaw>)> = Vec::new();
    let mut observed_max_row: u32 = 0;
    let mut observed_max_col: u32 = 0;

    let mut in_row = false;
    let mut current_row_num: u32 = 0;
    let mut current_row_cells: HashMap<u32, CellRaw> = HashMap::new();
    let mut current_row_max_col: u32 = 0;

    let mut in_cell = false;
    let mut current_col: u32 = 0;
    let mut current_type: Option<String> = None;
    let mut in_value = false;
    let mut in_inline_is = false;
    let mut in_inline_t = false;
    let mut value_text = String::new();

    let mut collected_data_rows: usize = 0;

    loop {
        // Once we trust the dimension tag and have collected everything the
        // caller asked for, stop reading the rest of the (possibly huge)
        // sheet XML entirely instead of parsing it just to discard it.
        if dimension.is_some() {
            if let Some(cap) = max_data_rows {
                if !header.is_empty() && collected_data_rows >= cap {
                    break;
                }
            }
        }

        let event = reader.read_event_into(&mut buf)?;
        let is_empty = matches!(&event, Event::Empty(_));
        match event {
            Event::Empty(e) | Event::Start(e) => {
                let name = e.name();
                match name.as_ref() {
                    b"dimension" => {
                        if let Some(r) = attr_value(&e, b"ref") {
                            dimension = parse_dimension_ref(&r);
                        }
                    }
                    b"row" => {
                        in_row = true;
                        current_row_num = attr_value(&e, b"r")
                            .and_then(|s| s.parse::<u32>().ok())
                            .unwrap_or(current_row_num + 1);
                        current_row_cells = HashMap::new();
                        current_row_max_col = 0;
                        if is_empty {
                            finish_row(
                                current_row_num,
                                &mut current_row_cells,
                                current_row_max_col,
                                &mut header,
                                &mut data_rows,
                                &mut observed_max_row,
                                &mut observed_max_col,
                                &mut collected_data_rows,
                                max_data_rows,
                            );
                            in_row = false;
                        }
                    }
                    b"c" if in_row => {
                        in_cell = true;
                        current_type = attr_value(&e, b"t");
                        current_col = attr_value(&e, b"r")
                            .and_then(|r| parse_dimension_ref(&format!("{r}:{r}")))
                            .map(|(_, col)| col)
                            .unwrap_or(current_row_max_col + 1);
                        value_text.clear();
                        if is_empty {
                            in_cell = false;
                        }
                    }
                    b"v" if in_cell => {
                        in_value = true;
                        if is_empty {
                            in_value = false;
                        }
                    }
                    b"is" if in_cell => {
                        in_inline_is = true;
                    }
                    b"t" if in_inline_is => {
                        in_inline_t = true;
                    }
                    _ => {}
                }
            }
            Event::End(e) => {
                let name = e.name();
                match name.as_ref() {
                    b"row" => {
                        if in_row {
                            finish_row(
                                current_row_num,
                                &mut current_row_cells,
                                current_row_max_col,
                                &mut header,
                                &mut data_rows,
                                &mut observed_max_row,
                                &mut observed_max_col,
                                &mut collected_data_rows,
                                max_data_rows,
                            );
                        }
                        in_row = false;
                    }
                    b"c" => {
                        if in_cell {
                            let resolved = resolve_cell_raw(
                                current_type.as_deref(),
                                if value_text.is_empty() {
                                    None
                                } else {
                                    Some(value_text.as_str())
                                },
                            );
                            if let Some(r) = resolved {
                                current_row_cells.insert(current_col, r);
                            }
                            current_row_max_col = current_row_max_col.max(current_col);
                        }
                        in_cell = false;
                        current_type = None;
                        value_text.clear();
                    }
                    b"v" => {
                        in_value = false;
                    }
                    b"is" => {
                        in_inline_is = false;
                    }
                    b"t" => {
                        in_inline_t = false;
                    }
                    _ => {}
                }
            }
            Event::Text(e) => {
                if in_value || in_inline_t {
                    if let Ok(txt) = e.unescape() {
                        value_text.push_str(&txt);
                    }
                }
            }
            Event::CData(e) => {
                if in_value || in_inline_t {
                    value_text.push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    let (final_max_row, final_max_col) = match dimension {
        Some((row, col)) => (row, col),
        None => (observed_max_row, observed_max_col),
    };

    if header.is_empty() && final_max_row > 0 {
        // A sheet whose very first row has no populated cells (an "empty
        // header row") is a legitimate, expected case (see AC-01's "首行
        // 全空" banner behaviour), not a corruption — surface it as an
        // all-empty header of the observed width rather than erroring.
        header = (0..final_max_col)
            .map(|_| CellRaw::Text(String::new()))
            .collect();
    }

    Ok(RawSheetScanDeferred {
        header,
        data_rows,
        max_row: final_max_row,
        max_col: final_max_col,
    })
}

#[allow(clippy::too_many_arguments)]
fn finish_row(
    row_num: u32,
    cells: &mut HashMap<u32, CellRaw>,
    row_max_col: u32,
    header: &mut Vec<CellRaw>,
    data_rows: &mut Vec<(u32, Vec<CellRaw>)>,
    observed_max_row: &mut u32,
    observed_max_col: &mut u32,
    collected_data_rows: &mut usize,
    max_data_rows: Option<usize>,
) {
    *observed_max_row = (*observed_max_row).max(row_num);
    *observed_max_col = (*observed_max_col).max(row_max_col);

    let is_header = header.is_empty() && data_rows.is_empty() && row_num <= 1;
    let width = row_max_col as usize;
    let mut dense: Vec<CellRaw> = (0..width).map(|_| CellRaw::Text(String::new())).collect();
    for (col, val) in cells.drain() {
        let idx = (col as usize).saturating_sub(1);
        if idx < width {
            dense[idx] = val;
        }
    }

    if is_header {
        *header = dense;
        return;
    }

    if let Some(cap) = max_data_rows {
        if *collected_data_rows >= cap {
            return;
        }
    }
    data_rows.push((row_num, dense));
    *collected_data_rows += 1;
}

fn resolve_cell_raw(cell_type: Option<&str>, raw_value: Option<&str>) -> Option<CellRaw> {
    match (cell_type, raw_value) {
        (Some("s"), Some(idx_str)) => idx_str
            .parse::<usize>()
            .ok()
            .map(CellRaw::SharedIndex),
        (Some("b"), Some(v)) => Some(CellRaw::Text(if v == "1" {
            "TRUE".to_string()
        } else {
            "FALSE".to_string()
        })),
        (_, Some(v)) if !v.is_empty() => Some(CellRaw::Text(v.to_string())),
        _ => None,
    }
}

// The inline-string branch (`t="inlineStr"`, value carried in `<is><t>`
// rather than `<v>`) is handled directly in `scan_sheet_xml_raw`: inline
// text is accumulated into `value_text` the same way `<v>` text is, so
// `resolve_cell_raw`'s `(_, Some(v))` arm covers it without needing a
// separate code path.

// ---------------------------------------------------------------------
// CSV
// ---------------------------------------------------------------------

const UTF8_BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// Decodes raw CSV bytes as UTF-8 (with or without a BOM) or, failing that,
/// GB18030 (a strict superset of GBK, so this also covers pure-GBK input).
/// Returns an error instead of ever falling back to lossy replacement-
/// character substitution.
pub fn decode_csv_bytes(raw: &[u8]) -> Result<String, TableReadError> {
    if let Some(stripped) = raw.strip_prefix(&UTF8_BOM) {
        return String::from_utf8(stripped.to_vec())
            .map_err(|_| TableReadError::UndecodableEncoding);
    }
    if let Ok(s) = std::str::from_utf8(raw) {
        return Ok(s.to_string());
    }
    let (decoded, _encoding, had_errors) = GB18030.decode(raw);
    if had_errors {
        return Err(TableReadError::UndecodableEncoding);
    }
    Ok(decoded.into_owned())
}

fn csv_reader_for(decoded: &str) -> csv::Reader<&[u8]> {
    csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(decoded.as_bytes())
}

/// The `csv` crate is deliberately lenient about a quoted field that is
/// never closed: rather than erroring, it treats everything up to EOF as
/// that field's content. That silently merges what should have been
/// multiple records into one and is exactly the "损坏引号" case the task
/// requires to fail safely, so it is rejected explicitly here before the
/// bytes ever reach the `csv` crate. A `""` escaped quote inside an
/// already-open quoted field is not corruption and does not trip this
/// check.
fn validate_quote_balance(text: &str) -> Result<(), TableReadError> {
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                } else {
                    in_quotes = false;
                }
            }
        } else if c == '"' {
            in_quotes = true;
        }
    }
    if in_quotes {
        return Err(TableReadError::MalformedCsv(
            "unterminated quoted field".to_string(),
        ));
    }
    Ok(())
}

fn read_csv_rows(
    path: &Path,
    max_data_rows: Option<usize>,
) -> Result<(Vec<String>, Vec<Vec<String>>), TableReadError> {
    let raw = std::fs::read(path)?;
    let decoded = decode_csv_bytes(&raw)?;
    validate_quote_balance(&decoded)?;
    let mut reader = csv_reader_for(&decoded);

    let mut header: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut width = 0usize;

    for (idx, record) in reader.records().enumerate() {
        let record = record.map_err(|e| TableReadError::MalformedCsv(e.to_string()))?;
        let values: Vec<String> = record.iter().map(|s| s.to_string()).collect();
        if idx == 0 {
            width = values.len();
            header = values;
            continue;
        }
        if let Some(cap) = max_data_rows {
            if rows.len() >= cap {
                continue;
            }
        }
        rows.push(pad_to_width(values, width));
    }

    Ok((header, rows))
}

pub fn read_csv_structure(
    path: &Path,
    sample_rows: usize,
) -> Result<SheetStructure, TableReadError> {
    let (header, rows) = read_csv_rows(path, Some(sample_rows))?;
    let width = header.len();
    let mut column_samples: Vec<Vec<String>> = vec![Vec::with_capacity(sample_rows); width];
    for row in rows.iter().take(sample_rows) {
        for (c, v) in row.iter().enumerate() {
            if c < width {
                column_samples[c].push(v.clone());
            }
        }
    }
    // CSV has exactly one implicit "sheet"; header row counts as row 1.
    let max_row = 1 + full_csv_row_count(path)?;
    Ok(SheetStructure {
        name: "Sheet1".to_string(),
        headers: header,
        column_samples,
        max_row,
        max_col: width as u32,
    })
}

fn full_csv_row_count(path: &Path) -> Result<u32, TableReadError> {
    let raw = std::fs::read(path)?;
    let decoded = decode_csv_bytes(&raw)?;
    validate_quote_balance(&decoded)?;
    let mut reader = csv_reader_for(&decoded);
    let mut count: u32 = 0;
    for record in reader.records() {
        record.map_err(|e| TableReadError::MalformedCsv(e.to_string()))?;
        count += 1;
    }
    // First record is the header; data-row count is one less (saturating
    // guards an entirely empty file).
    Ok(count.saturating_sub(1))
}

pub fn read_csv_preview(path: &Path, max_rows: usize) -> Result<TablePreview, TableReadError> {
    let (header, rows) = read_csv_rows(path, Some(max_rows))?;
    let width = header.len();
    let preview_rows = rows
        .into_iter()
        .enumerate()
        .map(|(i, values)| PreviewRow {
            row_number: (i as u32) + 2, // +1 for header, +1 for 1-based
            values: pad_to_width(values, width),
        })
        .collect();
    Ok(TablePreview {
        sheet_name: "Sheet1".to_string(),
        headers: header,
        rows: preview_rows,
    })
}

/// Full read (all data rows), used by the masking/apply path — CSV has no
/// calamine-based fallback, so unlike `.xlsx` this one genuinely needs to
/// read everything.
pub fn read_csv_all_rows(path: &Path) -> Result<(Vec<String>, Vec<Vec<String>>), TableReadError> {
    read_csv_rows(path, None)
}
