/* Copyright 2026 CheersAI Team. Licensed under Apache-2.0 */

use std::collections::HashMap;


use calamine::{open_workbook_auto, Data, Reader};
use excel_style_core::{parse_cell_ref_a1, rewrite_clone_inject, CellKey, RewriteOutcome};
use hkdf::Hkdf;
use rust_xlsxwriter::{Workbook, XlsxError};
use sha2::Sha256;
use tempfile::tempdir;
use zip::ZipArchive;

fn make_sample_xlsx(path: &std::path::Path) -> Result<(), XlsxError> {
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet().set_name("Sheet1")?;

    let headers = ["ID", "姓名", "部门", "手机", "邮箱"];
    for (c, h) in headers.iter().enumerate() {
        ws.write_string(0, c as u16, *h)?;
    }

    let sample_rows: [&[&str]; 10] = [
        &["1", "张敏", "研发", "13800000001", "zhangmin@example.com"],
        &["2", "李华", "市场", "13800000002", "lihua@example.com"],
        &["3", "王强", "销售", "13800000003", "wangqiang@example.com"],
        &["4", "赵敏", "财务", "13800000004", "zhaomin@example.com"],
        &["5", "陈刚", "HR", "13800000005", "chengang@example.com"],
        &["6", "刘洋", "研发", "13800000006", "liuyang@example.com"],
        &["7", "孙丽", "市场", "13800000007", "sunli@example.com"],
        &["8", "周杰", "销售", "13800000008", "zhoujie@example.com"],
        &["9", "吴芳", "财务", "13800000009", "wufang@example.com"],
        &["10", "郑勇", "HR", "13800000010", "zhengyong@example.com"],
    ];

    for (r, row) in sample_rows.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            ws.write_string((r + 1) as u32, c as u16, *v)?;
        }
    }

    wb.save(path)?;
    Ok(())
}

#[test]
fn test_rewrite_clone_inject_b2_replace() {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("sample.xlsx");
    let output = dir.path().join("masked.xlsx");

    make_sample_xlsx(&input).expect("make sample xlsx");

    let mut replacements = HashMap::new();
    replacements.insert(
        CellKey {
            sheet: "Sheet1".to_string(),
            row: 2,
            col: 2,
        },
        "张*".to_string(),
    );

    let outcome: RewriteOutcome =
        rewrite_clone_inject(&input, &output, &replacements).expect("rewrite ok");
    assert!(outcome.hits >= 1, "至少命中 1 次，实际 {}", outcome.hits);

    let file_bytes = std::fs::read(&output).expect("read output");
    let cursor = std::io::Cursor::new(&file_bytes);
    let mut zip = ZipArchive::new(cursor).expect("valid zip");
    for i in 0..zip.len() {
        let f = zip.by_index(i).expect("zip entry");
        assert!(f.crc32() != 0 || f.size() == 0, "CRC 有效 (非空)");
    }

    let mut wb = open_workbook_auto(output.to_str().unwrap()).expect("open workbook");
    let sheets = wb.sheet_names().to_vec();
    let sheet_name = sheets.first().expect("有 sheet");
    let range = wb.worksheet_range(sheet_name).expect("range ok");

    let b2 = range.get((1, 1)).expect("B2 exists");
    match b2 {
        Data::String(s) => {
            assert_eq!(s, "张*", "B2 应当等于 张*，实际是 {}", s);
        }
        other => {
            panic!("B2 不是字符串: {:?}", other);
        }
    }
}

#[test]
fn test_parse_cell_ref_a1_all_cases() {
    assert_eq!(parse_cell_ref_a1("S", "A1"), Some((1, 1)));
    assert_eq!(parse_cell_ref_a1("Sheet", "$Z$26"), Some((26, 26)));
    assert_eq!(parse_cell_ref_a1("Sheet", "Sheet!$Z$26"), Some((26, 26)));
    assert_eq!(
        parse_cell_ref_a1("Sheet 2", "'Sheet 2'!AA100"),
        Some((100, 27))
    );
    assert_eq!(parse_cell_ref_a1("A", "XFD1048576"), Some((1048576, 16384)));
    assert_eq!(parse_cell_ref_a1("X", "Wrong$A1"), None);
}

#[test]
fn test_hkdf_domain_separation_mutual_independence() {
    let passphrase = b"correct horse battery staple";
    let mut salt = [0u8; 32];
    for i in 0..salt.len() {
        salt[i] = (i as u8).wrapping_mul(7).wrapping_add(3);
    }

    let tags: [&[u8]; 3] = [b"CMAP_V1\0", b"ECMAP_V1\0", b"ENCSRC_V1\0"];

    let mut keys: Vec<[u8; 32]> = Vec::with_capacity(3);

    for tag in tags.iter() {
        let hk = Hkdf::<Sha256>::new(Some(&salt), passphrase);
        let mut info = tag.to_vec();
        info.extend_from_slice(b"kdf-info");
        let mut key = [0u8; 32];
        hk.expand(&info, &mut key).expect("HKDF expand 必须成功");
        keys.push(key);
    }

    assert_ne!(keys[0], keys[1], "CMAP_V1 与 ECMAP_V1 必须不同");
    assert_ne!(keys[0], keys[2], "CMAP_V1 与 ENCSRC_V1 必须不同");
    assert_ne!(keys[1], keys[2], "ECMAP_V1 与 ENCSRC_V1 必须不同");

    let mut collisions = 0usize;
    for i in 0..keys.len() {
        for j in (i + 1)..keys.len() {
            if keys[i] == keys[j] {
                collisions += 1;
            }
        }
    }
    assert_eq!(collisions, 0, "互解碰撞次数必须为 0，实际 {}", collisions);

    let hk = Hkdf::<Sha256>::new(Some(&salt), passphrase);
    for tag in tags.iter() {
        let mut k1 = [0u8; 32];
        let mut k2 = [0u8; 32];
        let mut info1 = tag.to_vec();
        info1.extend_from_slice(b"kdf-info");
        let mut info2 = tag.to_vec();
        info2.extend_from_slice(b"kdf-info");
        hk.expand(&info1, &mut k1).unwrap();
        hk.expand(&info2, &mut k2).unwrap();
        assert_eq!(k1, k2, "同 tag 两次派生必须相等（确定性）");
    }
}
