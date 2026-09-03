/* Copyright 2026 CheersAI Team. Licensed under Apache-2.0 */

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;
use quick_xml::Writer;
use rand::RngCore;
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub mod table_reader;

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct CellKey {
    pub sheet: String,
    pub row: u32,
    pub col: u32,
}

#[derive(Debug, Error)]
pub enum EsError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("XML error: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("Cell reference parse error: {0}")]
    CellRef(String),
    #[error("Engine error: {0}")]
    Engine(String),
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct RewriteOutcome {
    pub hits: u64,
    pub conflicts: u64,
    pub downgrade_used: bool,
    pub covered_cells: u64,
    pub warnings: Vec<String>,
}

pub fn parse_cell_ref_a1(sheet: &str, r: &str) -> Option<(u32, u32)> {
    let input = r;
    let (sheet_part, reference_part) = if let Some(idx) = input.rfind('!') {
        let sp = &input[..idx];
        let rp = &input[idx + 1..];
        (Some(sp), rp)
    } else {
        (None, input)
    };

    if let Some(sp) = sheet_part {
        let candidate = if sp.starts_with('\'') && sp.ends_with('\'') && sp.len() >= 2 {
            &sp[1..sp.len() - 1]
        } else {
            sp
        };
        if !candidate.is_empty() && candidate != sheet {
            return None;
        }
    }

    let cleaned: String = reference_part.chars().filter(|c| *c != '$').collect();

    let mut col_str = String::new();
    let mut row_str = String::new();
    let mut saw_digit = false;
    for c in cleaned.chars() {
        if c.is_ascii_alphabetic() {
            if saw_digit {
                return None;
            }
            col_str.push(c);
        } else if c.is_ascii_digit() {
            saw_digit = true;
            row_str.push(c);
        } else {
            return None;
        }
    }

    if col_str.is_empty() || row_str.is_empty() {
        return None;
    }

    let mut col: u32 = 0;
    for c in col_str.chars() {
        let v = (c.to_ascii_uppercase() as u32)
            .checked_sub('A' as u32)?
            .checked_add(1)?;
        col = col.checked_mul(26)?.checked_add(v)?;
    }

    let row: u32 = row_str.parse().ok()?;
    if row == 0 || col == 0 || row > 1_048_576 || col > 16_384 {
        return None;
    }

    Some((row, col))
}

fn sheet_index_from_path(path: &str) -> Option<u32> {
    if !path.starts_with("xl/worksheets/sheet") || !path.ends_with(".xml") {
        return None;
    }
    let rest = path
        .strip_prefix("xl/worksheets/sheet")?
        .strip_suffix(".xml")?;
    rest.parse::<u32>().ok()
}

fn attr_value<'a>(e: &'a quick_xml::events::BytesStart<'a>, key: &[u8]) -> Option<String> {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == key {
            return Some(String::from_utf8_lossy(&attr.value).into_owned());
        }
    }
    None
}

fn worksheet_names_from_workbook<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
) -> HashMap<u32, String> {
    let mut result = HashMap::new();
    let workbook_content = match archive.by_name("xl/workbook.xml") {
        Ok(mut f) => {
            let mut s = String::new();
            if f.read_to_string(&mut s).is_err() {
                return result;
            }
            s
        }
        Err(_) => return result,
    };

    let mut reader = Reader::from_str(&workbook_content);
    let mut buf = Vec::new();
    let mut sheet_index: u32 = 0;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                if e.name().as_ref() == b"sheet" {
                    sheet_index += 1;
                    let name =
                        attr_value(&e, b"name").unwrap_or_else(|| format!("Sheet{}", sheet_index));
                    result.insert(sheet_index, name);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    result
}

fn escape_xml_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '\'' => out.push_str("&apos;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

fn process_sheet_xml(
    sheet_name: &str,
    xml_input: &[u8],
    replacements: &HashMap<CellKey, String>,
    outcome: &mut RewriteOutcome,
) -> Result<Vec<u8>, EsError> {
    let mut reader = Reader::from_reader(xml_input);
    let mut writer = Writer::new(Vec::with_capacity(xml_input.len()));

    let mut buf = Vec::new();
    let mut in_c_element = false;
    let mut cell_rewrites_as_inline: bool = false;
    let mut in_v_element = false;
    let mut skip_v_text = false;
    let mut skip_inline_depth = 0usize;
    let mut pending_v_replacement: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name_bytes = qname.as_ref();
                if name_bytes == b"c" {
                    in_c_element = true;
                    let current_cell_r = attr_value(&e, b"r");
                    let mut key_match: Option<String> = None;
                    if let Some(ref r_val) = current_cell_r {
                        if let Some((row, col)) = parse_cell_ref_a1(sheet_name, r_val) {
                            outcome.covered_cells += 1;
                            let key = CellKey {
                                sheet: sheet_name.to_string(),
                                row,
                                col,
                            };
                            if let Some(nv) = replacements.get(&key) {
                                outcome.hits += 1;
                                key_match = Some(nv.clone());
                            }
                        }
                    }
                    // A replaced cell is always written as an inline-string
                    // cell, regardless of its original type (shared string,
                    // inline string, plain number, boolean, date, or formula
                    // result). Writing a string into a numeric `<v>` would
                    // leave the cell typed as a number and make the final
                    // workbook invalid for real readers (Excel asks to
                    // repair). The original value/formula element is skipped
                    // and a fresh `<is><t>` is emitted on the cell close.
                    cell_rewrites_as_inline = key_match.is_some();
                    skip_inline_depth = 0;
                    if cell_rewrites_as_inline {
                        let mut attrs_vec: Vec<Vec<u8>> = Vec::new();
                        let mut wrote_t = false;
                        for attr in e.attributes().flatten() {
                            let key_bytes = attr.key.as_ref();
                            let val_bytes: &[u8] = &attr.value;
                            if key_bytes == b"t" {
                                attrs_vec.push(b"t=\"inlineStr\"".to_vec());
                                wrote_t = true;
                            } else {
                                let mut pair =
                                    Vec::with_capacity(key_bytes.len() + val_bytes.len() + 3);
                                pair.extend_from_slice(key_bytes);
                                pair.extend_from_slice(b"=\"");
                                pair.extend_from_slice(val_bytes);
                                pair.push(b'"');
                                attrs_vec.push(pair);
                            }
                        }
                        if !wrote_t {
                            attrs_vec.push(b"t=\"inlineStr\"".to_vec());
                        }
                        let content: String = if attrs_vec.is_empty() {
                            "c".to_string()
                        } else {
                            let mut s: String = "c ".to_string();
                            for (i, a) in attrs_vec.iter().enumerate() {
                                if i > 0 {
                                    s.push(' ');
                                }
                                s.push_str(&String::from_utf8_lossy(a));
                            }
                            s
                        };
                        let se = quick_xml::events::BytesStart::from_content(&content, 1);
                        writer.write_event(Event::Start(se))?;
                    } else {
                        writer.write_event(Event::Start(e))?;
                    }
                    pending_v_replacement = key_match;
                } else if in_c_element
                    && cell_rewrites_as_inline
                    && (name_bytes == b"is" || name_bytes == b"f")
                {
                    // Skip the original inline-string content and any formula
                    // element of a replaced cell — both are replaced by the
                    // fresh `<is><t>` written on the cell close.
                    skip_inline_depth = 1;
                } else if name_bytes == b"v" && in_c_element {
                    in_v_element = true;
                    skip_v_text = cell_rewrites_as_inline;
                    if !cell_rewrites_as_inline {
                        writer.write_event(Event::Start(e))?;
                    }
                } else if skip_inline_depth > 0 {
                    skip_inline_depth += 1;
                } else {
                    writer.write_event(Event::Start(e))?;
                }
            }
            Ok(Event::End(e)) => {
                let qname = e.name();
                let name_bytes = qname.as_ref();
                if name_bytes == b"c" {
                    if cell_rewrites_as_inline {
                        if let Some(ref new_val) = pending_v_replacement {
                            let escaped = escape_xml_text(new_val);
                            writer.write_event(Event::Start(
                                quick_xml::events::BytesStart::new("is"),
                            ))?;
                            writer.write_event(Event::Start(
                                quick_xml::events::BytesStart::new("t"),
                            ))?;
                            writer.write_event(Event::Text(quick_xml::events::BytesText::new(
                                &escaped,
                            )))?;
                            writer
                                .write_event(Event::End(quick_xml::events::BytesEnd::new("t")))?;
                            writer
                                .write_event(Event::End(quick_xml::events::BytesEnd::new("is")))?;
                        }
                    }
                    in_c_element = false;
                    cell_rewrites_as_inline = false;
                    pending_v_replacement = None;
                    skip_inline_depth = 0;
                    writer.write_event(Event::End(e))?;
                } else if skip_inline_depth > 0 {
                    skip_inline_depth = skip_inline_depth.saturating_sub(1);
                } else if name_bytes == b"v" && in_v_element {
                    if !cell_rewrites_as_inline {
                        if let Some(ref new_val) = pending_v_replacement {
                            let escaped = escape_xml_text(new_val);
                            writer.write_event(Event::Text(quick_xml::events::BytesText::new(
                                &escaped,
                            )))?;
                        }
                        writer.write_event(Event::End(e))?;
                    }
                    in_v_element = false;
                    skip_v_text = false;
                } else {
                    writer.write_event(Event::End(e))?;
                }
            }
            Ok(Event::Empty(e)) => {
                if skip_inline_depth == 0 {
                    writer.write_event(Event::Empty(e))?;
                }
            }
            Ok(Event::Text(e)) => {
                if (in_v_element && skip_v_text) || skip_inline_depth > 0 {
                    // skip
                } else {
                    writer.write_event(Event::Text(e))?;
                }
            }
            Ok(Event::Comment(e)) => {
                writer.write_event(Event::Comment(e))?;
            }
            Ok(Event::CData(e)) => {
                if (in_v_element && skip_v_text) || skip_inline_depth > 0 {
                    // skip
                } else {
                    writer.write_event(Event::CData(e))?;
                }
            }
            Ok(Event::Decl(e)) => {
                writer.write_event(Event::Decl(e))?;
            }
            Ok(Event::PI(e)) => {
                writer.write_event(Event::PI(e))?;
            }
            Ok(Event::DocType(e)) => {
                writer.write_event(Event::DocType(e))?;
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(EsError::Xml(e)),
        }
        buf.clear();
    }

    Ok(writer.into_inner())
}

fn deflate_options() -> SimpleFileOptions {
    SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(6))
}

pub fn patch_chart_caches<R: Read + std::io::Seek>(
    archive_in: &mut ZipArchive<R>,
    writer: &mut ZipWriter<fs::File>,
    _outcome: &mut RewriteOutcome,
) -> Result<(), EsError> {
    let chart_files: Vec<String> = (0..archive_in.len())
        .filter_map(|i| {
            let f = archive_in.by_index(i).ok()?;
            let name = f.name().to_string();
            if (name.starts_with("xl/charts/chart") && name.ends_with(".xml"))
                || (name.starts_with("xl/charts/_rels/") && name.ends_with(".rels"))
            {
                Some(name)
            } else {
                None
            }
        })
        .collect();

    let options = deflate_options();

    for name in chart_files {
        let mut file = archive_in.by_name(&name)?;
        let mut content = Vec::new();
        file.read_to_end(&mut content)?;
        writer.start_file(name, options)?;
        writer.write_all(&content)?;
    }

    Ok(())
}

struct EntryDump {
    name: String,
    bytes: Vec<u8>,
}

fn dump_all_entries<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<Vec<EntryDump>, EsError> {
    let n = archive.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let mut f = archive.by_index(i)?;
        let name = f.name().to_string();
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes)?;
        out.push(EntryDump { name, bytes });
    }
    Ok(out)
}

pub fn rewrite_clone_inject(
    input: &Path,
    output: &Path,
    replacements: &HashMap<CellKey, String>,
) -> Result<RewriteOutcome, EsError> {
    let mut outcome = RewriteOutcome::default();

    let input_bytes = match fs::read(input) {
        Ok(b) => b,
        Err(e) => {
            outcome.warnings.push(format!("无法读取输入文件: {}", e));
            return Err(EsError::Io(e));
        }
    };

    let file = match fs::File::open(input) {
        Ok(f) => f,
        Err(e) => {
            outcome.warnings.push(format!("无法打开输入文件: {}", e));
            return Err(EsError::Io(e));
        }
    };

    let mut archive = match ZipArchive::new(file) {
        Ok(a) => a,
        Err(e) => {
            outcome
                .warnings
                .push(format!("输入不是合法的 ZIP/XLSX: {}", e));
            return Err(EsError::Zip(e));
        }
    };

    let sheet_names = worksheet_names_from_workbook(&mut archive);
    let entries = match dump_all_entries(&mut archive) {
        Ok(v) => v,
        Err(e) => {
            outcome
                .warnings
                .push(format!("读取全部 zip 条目失败，回退保留原始字节: {}", e));
            let _ = fs::write(output, &input_bytes);
            return Err(e);
        }
    };

    let out_file = match fs::File::create(output) {
        Ok(f) => f,
        Err(e) => {
            outcome.warnings.push(format!("无法创建输出文件: {}", e));
            let _ = fs::write(output, &input_bytes);
            outcome
                .warnings
                .push("回退保留原始 zip 字节失败".to_string());
            return Err(EsError::Io(e));
        }
    };

    let mut writer = ZipWriter::new(out_file);
    let options = deflate_options();

    for entry in entries.iter() {
        let entry_name = &entry.name;
        if let Some(sheet_idx) = sheet_index_from_path(entry_name) {
            let sheet_name = sheet_names
                .get(&sheet_idx)
                .cloned()
                .unwrap_or_else(|| format!("Sheet{}", sheet_idx));

            match process_sheet_xml(&sheet_name, &entry.bytes, replacements, &mut outcome) {
                Ok(new_xml) => {
                    writer.start_file(entry_name, options)?;
                    writer.write_all(&new_xml)?;
                }
                Err(e) => {
                    outcome
                        .warnings
                        .push(format!("处理 {} XML 失败，原样透传: {}", entry_name, e));
                    writer.start_file(entry_name, options)?;
                    writer.write_all(&entry.bytes)?;
                }
            }
        } else {
            writer.start_file(entry_name, options)?;
            writer.write_all(&entry.bytes)?;
        }
    }

    if let Err(e) = writer.finish() {
        outcome.warnings.push(format!("Zip 写入完成失败: {}", e));
        let _ = fs::write(output, &input_bytes);
        outcome
            .warnings
            .push("已回退保留原始 zip 字节（写入失败）".to_string());
        return Err(EsError::Zip(e));
    }

    Ok(outcome)
}

pub fn fallback_xlsxwriter_full(
    headers: &[String],
    rows: &[Vec<String>],
    output: &Path,
) -> Result<RewriteOutcome, EsError> {
    use rust_xlsxwriter::{Workbook, XlsxError};

    let mut outcome = RewriteOutcome {
        downgrade_used: true,
        warnings: vec!["使用 fallback_xlsxwriter_full 纯数据输出（不保证样式）".to_string()],
        ..Default::default()
    };

    let mut workbook = Workbook::new();
    let worksheet = workbook
        .add_worksheet()
        .set_name("Sheet1")
        .map_err(|e: XlsxError| EsError::Engine(format!("Worksheet error: {}", e)))?;

    for (col_idx, header) in headers.iter().enumerate() {
        worksheet
            .write_string(0, col_idx as u16, header.as_str())
            .map_err(|e: XlsxError| EsError::Engine(format!("Write header error: {}", e)))?;
    }

    for (row_idx, row) in rows.iter().enumerate() {
        for (col_idx, cell) in row.iter().enumerate() {
            worksheet
                .write_string((row_idx + 1) as u32, col_idx as u16, cell.as_str())
                .map_err(|e: XlsxError| EsError::Engine(format!("Write cell error: {}", e)))?;
        }
        outcome.covered_cells += row.len() as u64;
    }

    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    workbook
        .save(output)
        .map_err(|e: XlsxError| EsError::Engine(format!("Save workbook error: {}", e)))?;

    Ok(outcome)
}

/// One legacy `.xls` worksheet read in full via calamine: a dense,
/// row-major matrix (row 0 is always the header row, matching the
/// `row_idx`/`col_idx` convention used everywhere else in this crate and
/// its host callers), preserving every coordinate including empty cells
/// (as `""`).
struct LegacySheet {
    name: String,
    rows: Vec<Vec<String>>,
}

fn legacy_cell_to_string(cell: &calamine::Data) -> String {
    use calamine::Data;
    match cell {
        Data::Int(i) => i.to_string(),
        Data::Float(f) => f.to_string(),
        Data::String(s) => s.clone(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(dt) => dt.to_string(),
        Data::DateTimeIso(s) => s.clone(),
        Data::DurationIso(s) => s.clone(),
        Data::Error(e) => format!("Error: {:?}", e),
        Data::Empty => String::new(),
    }
}

fn read_legacy_xls_workbook(path: &Path) -> Result<Vec<LegacySheet>, EsError> {
    use calamine::{open_workbook_auto, Reader};

    let mut workbook = open_workbook_auto(path)
        .map_err(|e| EsError::Engine(format!("Failed to open legacy workbook: {e}")))?;
    let sheet_names = workbook.sheet_names().to_vec();
    let mut sheets = Vec::with_capacity(sheet_names.len());

    for name in sheet_names {
        let range = workbook
            .worksheet_range(&name)
            .map_err(|e| EsError::Engine(format!("Worksheet '{name}' error: {e}")))?;
        let (height, width) = range.get_size();
        let mut rows = Vec::with_capacity(height);
        for r in 0..height {
            let mut row = Vec::with_capacity(width);
            for c in 0..width {
                row.push(
                    range
                        .get((r, c))
                        .map(legacy_cell_to_string)
                        .unwrap_or_default(),
                );
            }
            rows.push(row);
        }
        sheets.push(LegacySheet { name, rows });
    }

    Ok(sheets)
}

/// One cell actually changed by [`rewrite_legacy_xls_with_mask`] — the
/// single source of truth a caller should build `.ecmap` entries from,
/// instead of a second independent traversal of the file that could drift
/// from what was actually written to the output workbook.
#[derive(Debug, Clone)]
pub struct LegacyCellChange {
    pub sheet: String,
    /// 0-based row index within the sheet's dense matrix; row 0 is always
    /// the header and never appears here (the header is copied through
    /// unmasked, matching every other masking path in this codebase).
    pub row_idx: usize,
    pub col_idx: usize,
    pub original: String,
    pub masked: String,
}

/// The legacy OLE `.xls` masking path (R3). `.xls` is not an OOXML ZIP
/// archive, so [`rewrite_clone_inject`] can never clone-inject into it —
/// this instead reads every sheet in full via calamine, calls the host's
/// `mask_cell` callback for every data cell (row 0, the header, is always
/// copied through verbatim and never passed to the callback), and writes a
/// brand-new multi-sheet `.xlsx` via `rust_xlsxwriter` that preserves sheet
/// order, sheet names, every row/column coordinate — including untouched
/// and empty cells — and changes only the cells the callback actually
/// masked. The returned [`LegacyCellChange`] list is exactly what was
/// written to `output`, so a caller building `.ecmap` entries from it
/// cannot drift from the produced artifact.
///
/// The result always reports `downgrade_used = true`: `.xls` carries no
/// OOXML style/formula/chart data to preserve, so the output is explicitly
/// a pure-data `.xlsx`, never a style-preservation claim. Any failure to
/// read a sheet, set a sheet name, write a cell, or save the workbook
/// returns `Err` — this function never falls back to a partial/empty-rows
/// "success".
pub fn rewrite_legacy_xls_with_mask<F>(
    input: &Path,
    output: &Path,
    mut mask_cell: F,
) -> Result<(RewriteOutcome, Vec<LegacyCellChange>), EsError>
where
    F: FnMut(&str, usize, usize, &str) -> String,
{
    use rust_xlsxwriter::{Workbook, XlsxError};

    let sheets = read_legacy_xls_workbook(input)?;
    if sheets.is_empty() {
        return Err(EsError::Engine(
            "legacy workbook has no worksheets".to_string(),
        ));
    }

    let mut workbook = Workbook::new();
    let mut changes = Vec::new();
    let mut covered_cells: u64 = 0;

    for sheet in &sheets {
        let worksheet = workbook
            .add_worksheet()
            .set_name(&sheet.name)
            .map_err(|e: XlsxError| EsError::Engine(format!("Worksheet name error: {e}")))?;

        for (row_idx, row) in sheet.rows.iter().enumerate() {
            for (col_idx, original) in row.iter().enumerate() {
                let value = if row_idx == 0 {
                    original.clone()
                } else {
                    let masked = mask_cell(&sheet.name, row_idx, col_idx, original);
                    if !original.is_empty() && masked != *original {
                        changes.push(LegacyCellChange {
                            sheet: sheet.name.clone(),
                            row_idx,
                            col_idx,
                            original: original.clone(),
                            masked: masked.clone(),
                        });
                    }
                    masked
                };
                if !value.is_empty() {
                    worksheet
                        .write_string(row_idx as u32, col_idx as u16, value.as_str())
                        .map_err(|e: XlsxError| {
                            EsError::Engine(format!("Write cell error: {e}"))
                        })?;
                }
                covered_cells += 1;
            }
        }
    }

    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    workbook
        .save(output)
        .map_err(|e: XlsxError| EsError::Engine(format!("Save workbook error: {e}")))?;

    let outcome = RewriteOutcome {
        hits: changes.len() as u64,
        conflicts: 0,
        downgrade_used: true,
        covered_cells,
        warnings: vec![
            "旧 .xls 已转换为 .xlsx 纯数据输出：样式、公式、图表不保证保留。".to_string(),
        ],
    };
    Ok((outcome, changes))
}

pub fn build_report_md(outcome: &RewriteOutcome) -> String {
    let mut md = String::new();
    md.push_str("# Excel 脱敏处理报告\n\n");
    md.push_str(&format!("**命中单元格数:** {}\n\n", outcome.hits));
    md.push_str(&format!("**冲突/错误数:** {}\n\n", outcome.conflicts));
    md.push_str(&format!(
        "**扫描覆盖单元格:** {}\n\n",
        outcome.covered_cells
    ));

    if outcome.downgrade_used {
        md.push_str("## ⚠️ 降级提示\n\n");
        md.push_str("> 本次输出使用纯数据回退写入器（fallback_xlsxwriter_full），\n");
        md.push_str("> **样式无法保证**（字体/颜色/边框/列宽/公式/图表等可能丢失）。\n\n");
    } else {
        md.push_str("## ✅ 样式路径\n\n");
        md.push_str("> 已使用样式保留克隆注入（rewrite_clone_inject），\n");
        md.push_str("> 原始 Excel 样式、公式骨架与图表缓存条目已尽量保留。\n\n");
    }

    let coverage_ratio = if outcome.covered_cells > 0 {
        (outcome.hits as f64) / (outcome.covered_cells as f64)
    } else {
        0.0
    };
    md.push_str(&format!(
        "**样式覆盖率估算:** {:.1}% （命中 / 扫描覆盖，估算）\n\n",
        coverage_ratio * 100.0
    ));

    if !outcome.warnings.is_empty() {
        md.push_str("## 警告/备注\n\n");
        for (i, w) in outcome.warnings.iter().enumerate() {
            md.push_str(&format!("{}. {}\n", i + 1, w));
        }
    }

    md
}

fn _sha256(b: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b);
    let r = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&r);
    out
}

fn _random_bytes<const N: usize>() -> [u8; N] {
    let mut out = [0u8; N];
    rand::thread_rng().fill_bytes(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parse_cell_ref_basic() {
        assert_eq!(parse_cell_ref_a1("Sheet1", "A1"), Some((1, 1)));
        assert_eq!(parse_cell_ref_a1("Sheet1", "$A$1"), Some((1, 1)));
        assert_eq!(parse_cell_ref_a1("Sheet1", "Z26"), Some((26, 26)));
        assert_eq!(parse_cell_ref_a1("Sheet1", "AA100"), Some((100, 27)));
        assert_eq!(parse_cell_ref_a1("Sheet1", "Sheet1!B2"), Some((2, 2)));
        assert_eq!(
            parse_cell_ref_a1("Sheet 2", "'Sheet 2'!AA100"),
            Some((100, 27))
        );
        assert_eq!(parse_cell_ref_a1("Sheet", "Other!A1"), None);
    }

    // -----------------------------------------------------------------
    // R-closeout (TASK-EXCEL-OUTPUT-RECOVERY-CONSISTENCY-CLOSEOUT-001):
    // a numeric cell replaced by a string must be written as a legal
    // string cell (t="inlineStr" + <is><t>), never as a numeric <v>
    // holding a non-numeric value; formulas of replaced cells are dropped;
    // unmasked numbers/formulas/styles pass through untouched.
    // -----------------------------------------------------------------

    fn unique_temp_dir(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "excel-style-core-closeout-{tag}-{nanos}-{n}-{}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create temp test dir");
        dir
    }

    /// Extracts the full `<c ...>...</c>` (or `<c .../>`) element whose `r`
    /// attribute equals `cell_ref` from raw worksheet XML.
    fn extract_cell_element(xml: &str, cell_ref: &str) -> Option<String> {
        let mut rest = xml;
        loop {
            let Some(start) = rest.find("<c ") else {
                return None;
            };
            let after_start = &rest[start..];
            let Some(end_gt) = after_start.find('>') else {
                return None;
            };
            let tag = &after_start[..=end_gt];
            if tag.contains(&format!("r=\"{cell_ref}\"")) {
                if tag.ends_with("/>") {
                    return Some(tag.to_string());
                }
                let body = &after_start[end_gt + 1..];
                let Some(end_close) = body.find("</c>") else {
                    return None;
                };
                return Some(format!("{tag}{}</c>", &body[..end_close]));
            }
            rest = &after_start[end_gt + 1..];
        }
    }

    #[test]
    fn numeric_cell_replaced_with_string_is_written_as_inline_string_cell() {
        let dir = unique_temp_dir("numeric-to-inline");
        let input = dir.join("nums.xlsx");
        {
            let mut workbook = rust_xlsxwriter::Workbook::new();
            let worksheet = workbook.add_worksheet().set_name("Sheet1").unwrap();
            worksheet.write_string(0, 0, "数量").unwrap();
            worksheet.write_string(0, 1, "金额").unwrap();
            worksheet.write_number(1, 0, 3.0).unwrap();
            worksheet.write_number(1, 1, 9.5).unwrap();
            worksheet.write_string(2, 0, "备注").unwrap();
            workbook.save(&input).unwrap();
        }

        let mut replacements = HashMap::new();
        replacements.insert(
            CellKey {
                sheet: "Sheet1".to_string(),
                row: 2,
                col: 1,
            },
            "***".to_string(),
        );
        let output = dir.join("masked.xlsx");
        let outcome = rewrite_clone_inject(&input, &output, &replacements).expect("rewrite");
        assert_eq!(outcome.hits, 1);

        // A real workbook reader opens it and sees the string replacement
        // plus the untouched numeric cell.
        use calamine::{open_workbook_auto, Data, Reader};
        let mut workbook = open_workbook_auto(&output).expect("masked workbook must open");
        let range = workbook.worksheet_range("Sheet1").expect("Sheet1");
        match range.get((1, 0)) {
            Some(Data::String(s)) => assert_eq!(s, "***"),
            other => panic!("A2 must be a string cell, got {other:?}"),
        }
        assert!(matches!(range.get((1, 1)), Some(Data::Float(f)) if (*f - 9.5).abs() < 1e-9));

        // The raw XML is well-formed and type-consistent.
        let zip_bytes = fs::read(&output).expect("read masked zip");
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).expect("open zip");
        let mut sheet_xml = String::new();
        zip.by_name("xl/worksheets/sheet1.xml")
            .expect("sheet1 part")
            .read_to_string(&mut sheet_xml)
            .expect("read sheet xml");
        let a2 = extract_cell_element(&sheet_xml, "A2").expect("A2 cell element");
        assert!(
            a2.contains("t=\"inlineStr\""),
            "A2 must be declared as inline string, got: {a2}"
        );
        assert!(
            a2.contains("<is><t>***</t></is>"),
            "A2 must carry the inline string value, got: {a2}"
        );
        assert!(
            !a2.contains("<v>"),
            "A2 must not keep a numeric <v>, got: {a2}"
        );
        let b2 = extract_cell_element(&sheet_xml, "B2").expect("B2 cell element");
        assert!(
            !b2.contains("t=\"inlineStr\"") && b2.contains("<v>9.5</v>"),
            "unmasked numeric B2 must stay a number, got: {b2}"
        );
    }

    #[test]
    fn formula_cell_replaced_with_string_drops_formula_and_writes_inline_string() {
        let dir = unique_temp_dir("formula-to-inline");
        let input = dir.join("formulas.xlsx");
        {
            let mut workbook = rust_xlsxwriter::Workbook::new();
            let worksheet = workbook.add_worksheet().set_name("Sheet1").unwrap();
            worksheet.write_number(1, 0, 3.0).unwrap();
            worksheet.write_number(1, 1, 9.5).unwrap();
            worksheet
                .write_formula(1, 2, "=A2+B2")
                .expect("write formula");
            workbook.save(&input).unwrap();
        }

        let mut replacements = HashMap::new();
        replacements.insert(
            CellKey {
                sheet: "Sheet1".to_string(),
                row: 2,
                col: 3,
            },
            "REDACTED".to_string(),
        );
        let output = dir.join("masked.xlsx");
        let outcome = rewrite_clone_inject(&input, &output, &replacements).expect("rewrite");
        assert_eq!(outcome.hits, 1);

        let zip_bytes = fs::read(&output).expect("read masked zip");
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).expect("open zip");
        let mut sheet_xml = String::new();
        zip.by_name("xl/worksheets/sheet1.xml")
            .expect("sheet1 part")
            .read_to_string(&mut sheet_xml)
            .expect("read sheet xml");
        let c2 = extract_cell_element(&sheet_xml, "C2").expect("C2 cell element");
        assert!(
            c2.contains("t=\"inlineStr\"") && c2.contains("<is><t>REDACTED</t></is>"),
            "replaced formula cell must become a string cell, got: {c2}"
        );
        assert!(
            !c2.contains("<f>"),
            "formula of a replaced cell must be dropped (a cell cannot hold both <f> and <is>), got: {c2}"
        );
        // Unmasked numeric cells stay numbers.
        let a2 = extract_cell_element(&sheet_xml, "A2").expect("A2 cell element");
        assert!(
            !a2.contains("t=\"inlineStr\"") && a2.contains("<v>"),
            "unmasked A2 must stay a number, got: {a2}"
        );
    }

    // -----------------------------------------------------------------
    // R-closeout (preview root-cause): the bounded structure scan (cap 5)
    // and the preview scan (cap 20) must agree on `max_col`/headers width;
    // a row beyond the structure cap that introduces a NEW column must not
    // make the preview emit a cell the structure headers cannot cover.
    // -----------------------------------------------------------------

    #[test]
    fn structure_and_preview_scans_agree_on_max_col_even_with_late_new_columns() {
        use std::io::Write;
        let dir = unique_temp_dir("scan-maxcol");
        let path = dir.join("late-col.xlsx");
        {
            // 7 data rows; the 7th data row (row 8) introduces column D.
            let mut sheet = String::from(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><dimension ref="A1:D8"/><sheetData>"#,
            );
            sheet.push_str(r#"<row r="1"><c r="A1" t="inlineStr"><is><t>Name</t></is></c><c r="B1" t="inlineStr"><is><t>Phone</t></is></c><c r="C1" t="inlineStr"><is><t>Email</t></is></c></row>"#);
            for i in 2..=7u32 {
                sheet.push_str(&format!(
                    r#"<row r="{i}"><c r="A{i}" t="inlineStr"><is><t>u{i}</t></is></c><c r="B{i}" t="inlineStr"><is><t>139{i:04}</t></is></c><c r="C{i}" t="inlineStr"><is><t>u{i}@x.invalid</t></is></c></row>"#
                ));
            }
            sheet.push_str(
                r#"<row r="8"><c r="A8" t="inlineStr"><is><t>u8</t></is></c><c r="B8" t="inlineStr"><is><t>13980000</t></is></c><c r="C8" t="inlineStr"><is><t>u8@x.invalid</t></is></c><c r="D8" t="inlineStr"><is><t>EXTRA</t></is></c></row>"#,
            );
            sheet.push_str("</sheetData></worksheet>");

            let file = fs::File::create(&path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let opts = zip::write::SimpleFileOptions::default();
            zip.start_file("xl/workbook.xml", opts).unwrap();
            zip.write_all(
                br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets></workbook>"#,
            ).unwrap();
            zip.start_file("xl/worksheets/sheet1.xml", opts).unwrap();
            zip.write_all(sheet.as_bytes()).unwrap();
            zip.finish().unwrap();
        }

        let structure = table_reader::read_xlsx_sheet_structure(&path, "Sheet1", 5)
            .expect("structure scan");
        let preview = table_reader::read_xlsx_preview(&path, "Sheet1", 20).expect("preview scan");
        // The desktop preview response reports `headers` from the structure
        // scan while emitting cells from the preview scan; both must agree
        // on width so the frontend `cell.col <= headers.length` contract
        // check can never reject a real native response. Today both scans
        // observe every row (the early-break optimization does not fire for
        // this fixture), so they must agree on `max_col`.
        assert_eq!(
            structure.max_col, preview.headers.len() as u32,
            "structure width (headers) and preview width must agree"
        );
        let max_preview_col = preview
            .rows
            .iter()
            .map(|row| row.values.len() as u32)
            .max()
            .unwrap_or(0);
        assert!(
            max_preview_col <= structure.max_col,
            "preview rows must stay within the reported header width"
        );
        // And the preview scan must have observed the late new column (D),
        // otherwise the fixture does not exercise the disagreement case.
        assert_eq!(preview.headers.len(), 4, "fixture must introduce a late new column");
    }
}
