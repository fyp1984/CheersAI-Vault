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
                    let t_attr = attr_value(&e, b"t");
                    let cell_was_shared = t_attr.as_deref() == Some("s");
                    let cell_was_inline = t_attr.as_deref() == Some("inlineStr");
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
                    cell_rewrites_as_inline =
                        key_match.is_some() && (cell_was_shared || cell_was_inline);
                    skip_inline_depth = 0;
                    if cell_rewrites_as_inline && cell_was_shared {
                        let mut attrs_vec: Vec<Vec<u8>> = Vec::new();
                        for attr in e.attributes().flatten() {
                            let key_bytes = attr.key.as_ref();
                            let val_bytes: &[u8] = &attr.value;
                            if key_bytes == b"t" {
                                attrs_vec.push(b"t=\"inlineStr\"".to_vec());
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
                } else if in_c_element && cell_rewrites_as_inline && name_bytes == b"is" {
                    skip_inline_depth = 1;
                } else if name_bytes == b"v" && in_c_element {
                    in_v_element = true;
                    skip_v_text = false;
                    if pending_v_replacement.is_some() {
                        skip_v_text = true;
                    }
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
}
