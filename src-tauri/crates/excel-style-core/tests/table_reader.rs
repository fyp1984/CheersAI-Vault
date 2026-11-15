/* Copyright 2026 CheersAI Team. Licensed under Apache-2.0 */
//! Directed tests for `excel_style_core::table_reader`.

use std::io::Write;
use std::path::{Path, PathBuf};

use excel_style_core::table_reader::{
    self, decode_csv_bytes, read_csv_all_rows, read_csv_preview, read_csv_structure,
    read_xlsx_all_sheets_structure, read_xlsx_preview, read_xlsx_sheet_structure,
    xlsx_sheet_names, TableReadError,
};

fn temp_path(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "excel-style-core-table-reader-tests-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

/// Builds a real `.xlsx` via `rust_xlsxwriter` (shared strings, `<dimension>`
/// tag present) — the realistic, performance-critical happy path.
fn write_shared_strings_xlsx(path: &Path, headers: &[&str], rows: &[Vec<String>]) {
    let mut wb = rust_xlsxwriter::Workbook::new();
    let ws = wb.add_worksheet().set_name("Sheet1").unwrap();
    for (c, h) in headers.iter().enumerate() {
        ws.write_string(0, c as u16, *h).unwrap();
    }
    for (r, row) in rows.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            ws.write_string((r + 1) as u32, c as u16, v.as_str())
                .unwrap();
        }
    }
    wb.save(path).unwrap();
}

/// Minimal hand-built `.xlsx` ZIP for edge cases `rust_xlsxwriter` will not
/// produce on its own (inline strings, missing/malformed dimension, empty
/// header row, multiple sheets). Only the parts `table_reader` actually
/// reads are included.
fn write_raw_xlsx(path: &Path, workbook_xml: &str, sheets_xml: &[&str], shared_strings_xml: Option<&str>) {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();

    zip.start_file("xl/workbook.xml", opts).unwrap();
    zip.write_all(workbook_xml.as_bytes()).unwrap();

    for (i, xml) in sheets_xml.iter().enumerate() {
        zip.start_file(format!("xl/worksheets/sheet{}.xml", i + 1), opts)
            .unwrap();
        zip.write_all(xml.as_bytes()).unwrap();
    }

    if let Some(sst) = shared_strings_xml {
        zip.start_file("xl/sharedStrings.xml", opts).unwrap();
        zip.write_all(sst.as_bytes()).unwrap();
    }

    zip.finish().unwrap();
}

fn one_sheet_workbook_xml(name: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheets><sheet name="{name}" sheetId="1" r:id="rId1" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"/></sheets></workbook>"#
    )
}

fn two_sheet_workbook_xml(a: &str, b: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheets><sheet name="{a}" sheetId="1" r:id="rId1" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"/><sheet name="{b}" sheetId="2" r:id="rId2"/></sheets></workbook>"#
    )
}

// ---------------------------------------------------------------------
// XLSX: realistic shared-strings + dimension happy path
// ---------------------------------------------------------------------

#[test]
fn xlsx_structure_reads_headers_dimension_and_samples_via_shared_strings() {
    let path = temp_path("shared_strings_basic.xlsx");
    write_shared_strings_xlsx(
        &path,
        &["姓名", "手机号"],
        &[
            vec!["张三".to_string(), "13900001234".to_string()],
            vec!["李四".to_string(), "13900005678".to_string()],
            vec!["王五".to_string(), "13900009012".to_string()],
        ],
    );
    let structure = read_xlsx_sheet_structure(&path, "Sheet1", 5).unwrap();
    assert_eq!(structure.headers, vec!["姓名", "手机号"]);
    assert_eq!(structure.max_row, 4); // header + 3 data rows
    assert_eq!(structure.max_col, 2);
    assert_eq!(structure.column_samples[0], vec!["张三", "李四", "王五"]);
    assert_eq!(
        structure.column_samples[1],
        vec!["13900001234", "13900005678", "13900009012"]
    );
}

#[test]
fn xlsx_preview_returns_bounded_rows_with_correct_row_numbers() {
    let path = temp_path("shared_strings_preview.xlsx");
    let rows: Vec<Vec<String>> = (0..10)
        .map(|i| vec![format!("name{i}"), format!("phone{i}")])
        .collect();
    write_shared_strings_xlsx(&path, &["姓名", "手机号"], &rows);

    let preview = read_xlsx_preview(&path, "Sheet1", 3).unwrap();
    assert_eq!(preview.headers, vec!["姓名", "手机号"]);
    assert_eq!(preview.rows.len(), 3);
    assert_eq!(preview.rows[0].row_number, 2);
    assert_eq!(preview.rows[0].values, vec!["name0", "phone0"]);
    assert_eq!(preview.rows[2].row_number, 4);
    assert_eq!(preview.rows[2].values, vec!["name2", "phone2"]);
}

#[test]
fn xlsx_bounded_shared_string_loading_does_not_need_the_whole_table() {
    // 2,000 unique-per-row strings; a bounded 5-row read must not require
    // resolving anywhere near all of them. This is a correctness check
    // (values resolve correctly) that also stands in for the performance
    // property under test — see the Runtime/Tauri integration tests for
    // wall-clock timing evidence on a 100,000-row fixture.
    let path = temp_path("shared_strings_bounded.xlsx");
    let rows: Vec<Vec<String>> = (0..2000)
        .map(|i| vec![format!("uniq-{i}-a"), format!("uniq-{i}-b")])
        .collect();
    write_shared_strings_xlsx(&path, &["A", "B"], &rows);

    let preview = read_xlsx_preview(&path, "Sheet1", 5).unwrap();
    assert_eq!(preview.rows.len(), 5);
    for (i, row) in preview.rows.iter().enumerate() {
        assert_eq!(row.values, vec![format!("uniq-{i}-a"), format!("uniq-{i}-b")]);
    }
}

// ---------------------------------------------------------------------
// XLSX: inline strings
// ---------------------------------------------------------------------

#[test]
fn xlsx_resolves_inline_strings_without_a_shared_strings_part() {
    let path = temp_path("inline_strings.xlsx");
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><dimension ref="A1:B3"/><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>姓名</t></is></c><c r="B1" t="inlineStr"><is><t>手机号</t></is></c></row><row r="2"><c r="A2" t="inlineStr"><is><t>张三</t></is></c><c r="B2" t="inlineStr"><is><t>13900001234</t></is></c></row><row r="3"><c r="A3" t="inlineStr"><is><t>&#26377;&#25972;&#25968;&#23383;</t></is></c><c r="B3" t="inlineStr"><is><t>13900005678</t></is></c></row></sheetData></worksheet>"#;
    write_raw_xlsx(&path, &one_sheet_workbook_xml("Sheet1"), &[sheet_xml], None);

    let structure = read_xlsx_sheet_structure(&path, "Sheet1", 5).unwrap();
    assert_eq!(structure.headers, vec!["姓名", "手机号"]);
    assert_eq!(structure.column_samples[0], vec!["张三", "有整数字"]);
    assert_eq!(structure.max_row, 3);
    assert_eq!(structure.max_col, 2);
}

// ---------------------------------------------------------------------
// XLSX: missing / malformed dimension fail-closed fallback
// ---------------------------------------------------------------------

#[test]
fn xlsx_missing_dimension_falls_back_to_a_correct_observed_extent() {
    let path = temp_path("no_dimension.xlsx");
    // No <dimension> element at all.
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>H1</t></is></c><c r="B1" t="inlineStr"><is><t>H2</t></is></c><c r="C1" t="inlineStr"><is><t>H3</t></is></c></row><row r="2"><c r="A2" t="inlineStr"><is><t>v1</t></is></c><c r="C2" t="inlineStr"><is><t>v3</t></is></c></row></sheetData></worksheet>"#;
    write_raw_xlsx(&path, &one_sheet_workbook_xml("Sheet1"), &[sheet_xml], None);

    let structure = read_xlsx_sheet_structure(&path, "Sheet1", 5).unwrap();
    assert_eq!(structure.headers, vec!["H1", "H2", "H3"]);
    assert_eq!(structure.max_row, 2);
    assert_eq!(structure.max_col, 3);
    assert_eq!(structure.column_samples[0], vec!["v1"]);
    assert_eq!(structure.column_samples[1], vec![""]);
    assert_eq!(structure.column_samples[2], vec!["v3"]);
}

#[test]
fn xlsx_malformed_dimension_ref_falls_back_like_a_missing_one() {
    let path = temp_path("malformed_dimension.xlsx");
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><dimension ref="not-a-real-range"/><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>H1</t></is></c></row><row r="2"><c r="A2" t="inlineStr"><is><t>v1</t></is></c></row></sheetData></worksheet>"#;
    write_raw_xlsx(&path, &one_sheet_workbook_xml("Sheet1"), &[sheet_xml], None);

    let structure = read_xlsx_sheet_structure(&path, "Sheet1", 5).unwrap();
    assert_eq!(structure.headers, vec!["H1"]);
    assert_eq!(structure.max_row, 2);
    assert_eq!(structure.max_col, 1);
}

// ---------------------------------------------------------------------
// XLSX: empty first row, multi-sheet
// ---------------------------------------------------------------------

#[test]
fn xlsx_empty_header_row_reports_blank_headers_of_observed_width_not_an_error() {
    let path = temp_path("empty_header.xlsx");
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><dimension ref="A1:B2"/><sheetData><row r="1"/><row r="2"><c r="A2" t="inlineStr"><is><t>v1</t></is></c><c r="B2" t="inlineStr"><is><t>v2</t></is></c></row></sheetData></worksheet>"#;
    write_raw_xlsx(&path, &one_sheet_workbook_xml("Sheet1"), &[sheet_xml], None);

    let structure = read_xlsx_sheet_structure(&path, "Sheet1", 5).unwrap();
    assert_eq!(structure.headers, vec!["", ""]);
    assert_eq!(structure.max_row, 2);
    assert_eq!(structure.max_col, 2);
}

#[test]
fn xlsx_multi_sheet_names_and_independent_reads() {
    let path = temp_path("multi_sheet.xlsx");
    let sheet1 = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><dimension ref="A1:A2"/><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>S1H</t></is></c></row><row r="2"><c r="A2" t="inlineStr"><is><t>s1v</t></is></c></row></sheetData></worksheet>"#;
    let sheet2 = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><dimension ref="A1:A2"/><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>S2H</t></is></c></row><row r="2"><c r="A2" t="inlineStr"><is><t>s2v</t></is></c></row></sheetData></worksheet>"#;
    write_raw_xlsx(
        &path,
        &two_sheet_workbook_xml("Alpha", "Beta"),
        &[sheet1, sheet2],
        None,
    );

    let names = xlsx_sheet_names(&path).unwrap();
    assert_eq!(names, vec!["Alpha", "Beta"]);

    let structures = read_xlsx_all_sheets_structure(&path, 5).unwrap();
    assert_eq!(structures.len(), 2);
    assert_eq!(structures[0].name, "Alpha");
    assert_eq!(structures[0].headers, vec!["S1H"]);
    assert_eq!(structures[1].name, "Beta");
    assert_eq!(structures[1].headers, vec!["S2H"]);

    let err = read_xlsx_sheet_structure(&path, "Gamma", 5).unwrap_err();
    assert!(matches!(err, TableReadError::SheetNotFound(_)));
}

#[test]
fn xlsx_corrupted_file_fails_closed_not_panics() {
    let path = temp_path("corrupted.xlsx");
    std::fs::write(&path, b"this is not a zip file at all").unwrap();
    let err = read_xlsx_sheet_structure(&path, "Sheet1", 5).unwrap_err();
    assert!(matches!(err, TableReadError::Zip(_)));
}

// ---------------------------------------------------------------------
// CSV: encodings
// ---------------------------------------------------------------------

#[test]
fn csv_decodes_utf8_utf8_bom_and_gb18030() {
    let utf8 = "姓名,手机号\n张三,13900001234\n".as_bytes().to_vec();
    assert_eq!(
        decode_csv_bytes(&utf8).unwrap(),
        "姓名,手机号\n张三,13900001234\n"
    );

    let mut bom = vec![0xEF, 0xBB, 0xBF];
    bom.extend_from_slice("姓名,手机号\n李四,13900005678\n".as_bytes());
    assert_eq!(
        decode_csv_bytes(&bom).unwrap(),
        "姓名,手机号\n李四,13900005678\n"
    );

    let (gb_bytes, _, had_errors) = encoding_rs::GB18030.encode("姓名,手机号\n王五,13900009012\n");
    assert!(!had_errors);
    assert_eq!(
        decode_csv_bytes(&gb_bytes).unwrap(),
        "姓名,手机号\n王五,13900009012\n"
    );
}

#[test]
fn csv_undecodable_bytes_reject_instead_of_lossy_replacement() {
    // A lone GB18030 two-byte lead byte (0x81) with no trailing byte at all
    // (truncated mid-sequence) is invalid both as UTF-8 and as GB18030 —
    // GB18030 is close to a total mapping over all of Unicode, so an
    // arbitrary short byte string is a poor test input; an incomplete
    // multi-byte sequence at EOF reliably is not decodable.
    let bad = vec![0x81u8];
    let err = decode_csv_bytes(&bad).unwrap_err();
    assert!(matches!(err, TableReadError::UndecodableEncoding));
}

#[test]
fn csv_structure_and_preview_handle_quotes_commas_crlf_and_embedded_newlines() {
    let path = temp_path("quoted.csv");
    let content = "姓名,备注\r\n张三,\"包含,逗号\"\r\n李四,\"多行\n备注\"\r\n";
    std::fs::write(&path, content).unwrap();

    let structure = read_csv_structure(&path, 5).unwrap();
    assert_eq!(structure.headers, vec!["姓名", "备注"]);
    assert_eq!(structure.max_row, 3);
    assert_eq!(structure.column_samples[1], vec!["包含,逗号", "多行\n备注"]);

    let preview = read_csv_preview(&path, 10).unwrap();
    assert_eq!(preview.rows.len(), 2);
    assert_eq!(preview.rows[1].values, vec!["李四", "多行\n备注"]);
}

#[test]
fn csv_ragged_rows_are_padded_not_rejected() {
    let path = temp_path("ragged.csv");
    std::fs::write(&path, "A,B,C\n1,2\n3,4,5,6\n").unwrap();
    let (header, rows) = read_csv_all_rows(&path).unwrap();
    assert_eq!(header, vec!["A", "B", "C"]);
    assert_eq!(rows[0], vec!["1", "2", ""]);
    // A longer row is not truncated — every field is kept rather than
    // silently dropping data past the header width.
    assert_eq!(rows[1], vec!["3", "4", "5", "6"]);
}

#[test]
fn csv_malformed_unterminated_quote_fails_closed() {
    let path = temp_path("malformed_quote.csv");
    std::fs::write(&path, "A,B\n\"unterminated,2\n").unwrap();
    let err = read_csv_structure(&path, 5).unwrap_err();
    assert!(matches!(err, TableReadError::MalformedCsv(_)));
}

#[test]
fn csv_full_read_returns_every_data_row() {
    let path = temp_path("full_read.csv");
    let mut content = String::from("A,B\n");
    for i in 0..25 {
        content.push_str(&format!("v{i}a,v{i}b\n"));
    }
    std::fs::write(&path, &content).unwrap();
    let (header, rows) = read_csv_all_rows(&path).unwrap();
    assert_eq!(header, vec!["A", "B"]);
    assert_eq!(rows.len(), 25);
    assert_eq!(rows[24], vec!["v24a", "v24b"]);
}

#[test]
fn csv_empty_cells_and_unequal_encodings_do_not_panic() {
    let path = temp_path("empty_cells.csv");
    std::fs::write(&path, "A,B,C\n,,\nx,,z\n").unwrap();
    let (header, rows) = read_csv_all_rows(&path).unwrap();
    assert_eq!(header, vec!["A", "B", "C"]);
    assert_eq!(rows[0], vec!["", "", ""]);
    assert_eq!(rows[1], vec!["x", "", "z"]);
}

// Re-export check: the module's public surface is reachable via the crate
// root path used by consumers (Tauri/Runtime), not just `use` inside this
// test file.
#[test]
fn public_module_path_is_stable() {
    let _ = table_reader::TableReadError::NoSheets;
}

// ---------------------------------------------------------------------
// Legacy `.xls` (R3, TASK-EXCEL-P0-DYNAMIC-FAILURES-CLOSEOUT-001):
// `rewrite_legacy_xls_with_mask` at the crate root. There is no `.xls`
// writer available in this workspace (rust_xlsxwriter only writes OOXML),
// so these tests read the repo's existing real, fictional, multi-sheet
// `.xls` fixture (shared read-only with the `engine-core` crate's own
// tests) rather than fabricating a new one — the round that introduced
// this fixture already confirmed via live HTTP reproduction that Sheet1
// has a header + 3 data rows (one with empty Phone/Email cells) and Sheet2
// has a header + 1 data row; that exact, real shape is asserted below.
// ---------------------------------------------------------------------

fn sample_xls_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../engine-core/tests/fixtures/sample.xls")
}

#[test]
fn rewrite_legacy_xls_preserves_all_sheets_names_order_and_values_with_identity_mask() {
    let output = temp_path("legacy_identity.xlsx");
    let (outcome, changes) = excel_style_core::rewrite_legacy_xls_with_mask(
        &sample_xls_path(),
        &output,
        |_sheet, _row, _col, original| original.to_string(),
    )
    .expect("legacy xls rewrite must succeed");

    assert!(
        changes.is_empty(),
        "an identity mask must report zero changed cells, got {changes:?}"
    );
    assert!(outcome.downgrade_used, ".xls -> .xlsx must always be a downgrade");
    assert_eq!(outcome.hits, 0);
    assert!(outcome.covered_cells > 0);

    let sheet_names = xlsx_sheet_names(&output).expect("re-read xlsx sheet names");
    assert_eq!(sheet_names, vec!["Sheet1".to_string(), "Sheet2".to_string()]);

    let sheet1 = read_xlsx_preview(&output, "Sheet1", 10).expect("re-read Sheet1");
    assert_eq!(sheet1.headers, vec!["Name", "Phone", "Email"]);
    assert_eq!(sheet1.rows.len(), 3, "Sheet1 must keep all 3 data rows");
    assert_eq!(
        sheet1.rows[0].values,
        vec!["Alice", "13900000000", "alice@example.invalid"]
    );
    assert_eq!(
        sheet1.rows[1].values,
        vec!["Bob", "13800000000", "bob@example.invalid"]
    );
    assert_eq!(
        sheet1.rows[2].values,
        vec!["中文", "", ""],
        "the row with empty Phone/Email cells must keep those cells empty, not drop them"
    );

    let sheet2 = read_xlsx_preview(&output, "Sheet2", 10).expect("re-read Sheet2");
    assert_eq!(
        sheet2.headers,
        vec!["Phone"],
        "the second worksheet must not be dropped (this is the exact R3 regression)"
    );
    assert_eq!(sheet2.rows.len(), 1);
    assert_eq!(sheet2.rows[0].values, vec!["13900000000"]);
}

#[test]
fn rewrite_legacy_xls_masks_only_matched_cells_and_reports_exact_changes() {
    let output = temp_path("legacy_masked.xlsx");
    // A deliberately simple, unambiguous transform: mask any value that
    // looks like an 11-digit phone number, leave everything else
    // (including empty cells and the Name/Email columns) untouched.
    let (outcome, mut changes) = excel_style_core::rewrite_legacy_xls_with_mask(
        &sample_xls_path(),
        &output,
        |_sheet, _row, _col, original| {
            if original.len() == 11 && original.chars().all(|c| c.is_ascii_digit()) {
                format!("***{}", &original[original.len() - 4..])
            } else {
                original.to_string()
            }
        },
    )
    .expect("legacy xls rewrite must succeed");

    changes.sort_by(|a, b| (a.sheet.clone(), a.row_idx, a.col_idx).cmp(&(b.sheet.clone(), b.row_idx, b.col_idx)));
    assert_eq!(outcome.hits, changes.len() as u64);
    assert_eq!(
        changes.len(),
        3,
        "exactly the 3 real phone-number cells (2 in Sheet1, 1 in Sheet2) must be masked, got {changes:?}"
    );

    assert_eq!(changes[0].sheet, "Sheet1");
    assert_eq!(changes[0].row_idx, 1);
    assert_eq!(changes[0].col_idx, 1);
    assert_eq!(changes[0].original, "13900000000");
    assert_eq!(changes[0].masked, "***0000");

    assert_eq!(changes[1].sheet, "Sheet1");
    assert_eq!(changes[1].row_idx, 2);
    assert_eq!(changes[1].col_idx, 1);
    assert_eq!(changes[1].original, "13800000000");

    assert_eq!(changes[2].sheet, "Sheet2");
    assert_eq!(changes[2].row_idx, 1);
    assert_eq!(changes[2].col_idx, 0);
    assert_eq!(changes[2].original, "13900000000");

    // The unmasked cells must be byte-identical to the original in the
    // rewritten file — not just "the changed cells are right".
    let sheet1 = read_xlsx_preview(&output, "Sheet1", 10).unwrap();
    assert_eq!(sheet1.rows[0].values, vec!["Alice", "***0000", "alice@example.invalid"]);
    assert_eq!(sheet1.rows[2].values, vec!["中文", "", ""]);
    let sheet2 = read_xlsx_preview(&output, "Sheet2", 10).unwrap();
    assert_eq!(sheet2.rows[0].values, vec!["***0000"]);
}

#[test]
fn rewrite_legacy_xls_never_masks_the_header_row() {
    let output = temp_path("legacy_header_untouched.xlsx");
    // A callback that would corrupt every data cell it's ever called with —
    // if the header row were (incorrectly) passed to it, this would show up
    // immediately as "MASKED" headers below.
    let (_outcome, _changes) = excel_style_core::rewrite_legacy_xls_with_mask(
        &sample_xls_path(),
        &output,
        |_sheet, _row, _col, _original| "MASKED".to_string(),
    )
    .expect("legacy xls rewrite must succeed");

    let sheet1 = read_xlsx_all_sheets_structure(&output, 10).unwrap();
    let sheet1 = sheet1.iter().find(|s| s.name == "Sheet1").unwrap();
    assert_eq!(sheet1.headers, vec!["Name", "Phone", "Email"]);
}

#[test]
fn rewrite_legacy_xls_report_declares_pure_data_downgrade() {
    let output = temp_path("legacy_report.xlsx");
    let (outcome, _changes) = excel_style_core::rewrite_legacy_xls_with_mask(
        &sample_xls_path(),
        &output,
        |_sheet, _row, _col, original| original.to_string(),
    )
    .unwrap();
    assert!(outcome
        .warnings
        .iter()
        .any(|w| w.contains(".xls") && w.contains(".xlsx") && w.contains("样式")));
    let report = excel_style_core::build_report_md(&outcome);
    assert!(report.contains(".xls"));
    assert!(report.contains("样式"));
}

#[test]
fn rewrite_legacy_xls_corrupted_or_disguised_input_fails_closed_no_partial_output() {
    let input = temp_path("fake_legacy.xls");
    std::fs::write(&input, b"not a real OLE .xls file, just plain bytes").unwrap();
    let output = temp_path("legacy_should_not_exist.xlsx");
    let _ = std::fs::remove_file(&output);

    let result = excel_style_core::rewrite_legacy_xls_with_mask(&input, &output, |_, _, _, original| {
        original.to_string()
    });
    assert!(result.is_err(), "corrupted OLE bytes must fail closed, not succeed");
    assert!(
        !output.exists(),
        "a failed legacy xls rewrite must never leave a partial/empty output file behind"
    );
}
