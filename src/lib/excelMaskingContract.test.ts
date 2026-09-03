/* Copyright 2026 CheersAI Team. Licensed under Apache-2.0 */
import test from "node:test";
import assert from "node:assert/strict";

import {
  ExcelMaskingContractError,
  classifyDesktopExcelApplyError,
  classifyDesktopExcelPreviewError,
  toCanonicalExcelMaskPreview,
  toTauriExcelMaskingConfig,
} from "./excelMaskingContract";
import type {
  ExcelMaskingConfig,
  ExcelMaskPreview,
} from "@/types/commands";
import type { TauriExcelMaskPreviewCell } from "./excelMaskingContract";

function baseConfig(overrides: Partial<ExcelMaskingConfig> = {}): ExcelMaskingConfig {
  return {
    file_path: "/tmp/contacts.xlsx",
    sheet_policies: [
      {
        sheet: "Sheet1",
        column_rules: [
          {
            sheet: "Sheet1",
            colIndex: 2,
            headerText: "手机号",
            strategy: "PHONE_MID4",
            replacement: undefined,
          },
        ],
        cell_overrides: [
          {
            sheet: "Sheet1",
            row: 4,
            col: 1,
            strategy: "IDCARD_MID10",
            replacement: undefined,
          },
        ],
      },
    ],
    retain_encrypted_source: true,
    key_mode: "SANDBOX_REUSED",
    ...overrides,
  };
}

test("converts file path, sheets, column rules and cell overrides into the Tauri native shape", () => {
  const native = toTauriExcelMaskingConfig(baseConfig(), { sandboxPassphrase: "sbx-pass" });

  assert.equal(native.input_file_path, "/tmp/contacts.xlsx");
  assert.equal(native.generate_ecmap, true);
  assert.equal(native.retain_encrypted_source, true);
  assert.equal(native.passphrase, "sbx-pass");
  assert.deepEqual(native.source_pass_mode, { type: "SandboxReused" });

  assert.equal(native.sheets.length, 1);
  const sheet = native.sheets[0];
  assert.equal(sheet.sheet_name, "Sheet1");
  assert.equal(sheet.header_row, 0);

  assert.deepEqual(sheet.column_rules["手机号"], {
    strategy_id: "PHONE_MID4",
    replacement: undefined,
    enabled: true,
    rule_mode: "CANONICAL",
  });

  // row 4 / col 1 (0-based) -> B5 (1-based A1 notation).
  assert.equal(sheet.cell_overrides.length, 1);
  assert.deepEqual(sheet.cell_overrides[0], {
    cell_ref: "B5",
    strategy_id: "IDCARD_MID10",
    replacement: undefined,
  });
});

test("converts all three key modes, carrying the secondary passphrase only for SECONDARY_PASSPHRASE", () => {
  const sandbox = toTauriExcelMaskingConfig(
    baseConfig({ key_mode: "SANDBOX_REUSED" }),
    { sandboxPassphrase: "sbx" }
  );
  assert.deepEqual(sandbox.source_pass_mode, { type: "SandboxReused" });

  const device = toTauriExcelMaskingConfig(baseConfig({ key_mode: "DEVICE_KEY" }));
  assert.deepEqual(device.source_pass_mode, { type: "DeviceKey" });

  const secondary = toTauriExcelMaskingConfig(
    baseConfig({
      key_mode: "SECONDARY_PASSPHRASE",
      secondary_passphrase: "s3cr3t",
    })
  );
  assert.deepEqual(secondary.source_pass_mode, {
    type: "SecondaryPhrase",
    value: "s3cr3t",
  });
});

test("rejects SECONDARY_PASSPHRASE mode with an empty or missing secondary passphrase", () => {
  assert.throws(
    () =>
      toTauriExcelMaskingConfig(
        baseConfig({ key_mode: "SECONDARY_PASSPHRASE", secondary_passphrase: undefined })
      ),
    ExcelMaskingContractError
  );
  assert.throws(
    () =>
      toTauriExcelMaskingConfig(
        baseConfig({ key_mode: "SECONDARY_PASSPHRASE", secondary_passphrase: "   " })
      ),
    ExcelMaskingContractError
  );
});

test("SANDBOX_REUSED without a sandbox passphrase still converts (preview never encrypts; apply is enforced in Rust)", () => {
  // UI-STATE-003: the preview command never encrypts, so an unset sandbox
  // passphrase (isolated/first-run environment) must not block config
  // conversion. The apply path enforces it in Rust, whose error string is
  // classified into the actionable "沙箱口令不能为空" message.
  for (const sandboxPassphrase of [undefined, "", "   \t\n"]) {
    const native = toTauriExcelMaskingConfig(
      baseConfig({ key_mode: "SANDBOX_REUSED" }),
      { sandboxPassphrase }
    );
    // Conversion succeeds; the passphrase field carries the fallback verbatim
    // (bytes preserved), and the mode stays SandboxReused.
    assert.equal(native.passphrase, sandboxPassphrase ?? "");
    assert.deepEqual(native.source_pass_mode, { type: "SandboxReused" });
  }
});

test("preserves every byte of a non-empty SANDBOX_REUSED passphrase", () => {
  const sandboxPassphrase = "  fixture sandbox passphrase  ";
  const native = toTauriExcelMaskingConfig(
    baseConfig({ key_mode: "SANDBOX_REUSED" }),
    { sandboxPassphrase }
  );
  assert.equal(native.passphrase, sandboxPassphrase);
});

test("rejects an unknown key mode instead of silently downgrading", () => {
  assert.throws(
    () =>
      toTauriExcelMaskingConfig(
        baseConfig({ key_mode: "NOT_A_REAL_MODE" as ExcelMaskingConfig["key_mode"] })
      ),
    ExcelMaskingContractError
  );
});

test("rejects an unknown masking strategy on a column rule instead of silently downgrading", () => {
  const config = baseConfig();
  config.sheet_policies[0].column_rules[0] = {
    ...config.sheet_policies[0].column_rules[0],
    strategy: "NOT_A_REAL_STRATEGY" as ExcelMaskingConfig["sheet_policies"][number]["column_rules"][number]["strategy"],
  };
  assert.throws(() => toTauriExcelMaskingConfig(config), ExcelMaskingContractError);
});

test("rejects an unknown masking strategy on a cell override instead of silently downgrading", () => {
  const config = baseConfig();
  config.sheet_policies[0].cell_overrides[0] = {
    ...config.sheet_policies[0].cell_overrides[0],
    strategy: "NOT_A_REAL_STRATEGY" as ExcelMaskingConfig["sheet_policies"][number]["cell_overrides"][number]["strategy"],
  };
  assert.throws(() => toTauriExcelMaskingConfig(config), ExcelMaskingContractError);
});

test("rejects duplicate header text bound to two different column indexes instead of silently dropping one", () => {
  const config = baseConfig();
  config.sheet_policies[0].column_rules.push({
    sheet: "Sheet1",
    colIndex: 5,
    headerText: "手机号",
    strategy: "FULL_MASK",
    replacement: undefined,
  });
  assert.throws(() => toTauriExcelMaskingConfig(config), ExcelMaskingContractError);
});

test("rejects an empty file_path", () => {
  const config = baseConfig({ file_path: "" });
  assert.throws(() => toTauriExcelMaskingConfig(config), ExcelMaskingContractError);
});

test("does not log or echo the secondary passphrase anywhere in the produced config besides the phrase field itself", () => {
  const native = toTauriExcelMaskingConfig(
    baseConfig({ key_mode: "SECONDARY_PASSPHRASE", secondary_passphrase: "top-secret" })
  );
  const serialized = JSON.stringify(native);
  const occurrences = serialized.split("top-secret").length - 1;
  assert.equal(occurrences, 1, "the secondary passphrase must appear exactly once, in source_pass_mode.value");
});

function nativePreviewCell(
  overrides: Partial<TauriExcelMaskPreviewCell> = {}
): TauriExcelMaskPreviewCell {
  return {
    original_preview: "fixture-original",
    masked: "fixture-masked",
    strategy_id: "default:identity",
    row: 1,
    col: 1,
    cell_ref: "A1",
    ...overrides,
  };
}

test("converts the real native per-cell preview shape into canonical rows", () => {
  const native = {
    sheets: [
      {
        sheet_name: "Sheet1",
        headers: ["Name", "Phone", "Email"],
        preview_rows: [
          nativePreviewCell({
            original_preview: "mail-fixture",
            masked: "m***",
            strategy_id: "EMAIL_USER_MASK",
            row: 2,
            col: 3,
            cell_ref: "C2",
          }),
          nativePreviewCell({
            original_preview: "name-fixture",
            masked: "******",
            strategy_id: "FULL_MASK",
            row: 1,
            col: 1,
            cell_ref: "A1",
          }),
          nativePreviewCell({
            original_preview: "phone-fixture",
            masked: "p***",
            strategy_id: "PHONE_MID4",
            row: 1,
            col: 2,
            cell_ref: "B1",
          }),
          nativePreviewCell({
            original_preview: "second-name",
            masked: "******",
            strategy_id: "FULL_MASK",
            row: 2,
            col: 1,
            cell_ref: "A2",
          }),
        ],
      },
    ],
  };

  const canonical = toCanonicalExcelMaskPreview(native);
  assert.deepEqual(canonical, {
    preview_rows: [
      {
        original_preview: ["name-fixture", "phone-fixture", null],
        masked: ["******", "p***", ""],
        row_index: 2,
        sheet: "Sheet1",
      },
      {
        original_preview: ["second-name", null, "mail-fixture"],
        masked: ["******", "", "m***"],
        row_index: 3,
        sheet: "Sheet1",
      },
    ],
    conflicts: [],
  } satisfies ExcelMaskPreview);
});

test("preserves native sheet order while grouping multiple rows and columns deterministically", () => {
  const canonical = toCanonicalExcelMaskPreview({
    sheets: [
      {
        sheet_name: "Second",
        headers: ["A", "B"],
        preview_rows: [
          nativePreviewCell({ row: 3, col: 2, cell_ref: "B3", masked: "b3" }),
          nativePreviewCell({ row: 1, col: 2, cell_ref: "B1", masked: "b1" }),
        ],
      },
      {
        sheet_name: "First",
        headers: ["A", "B", "C"],
        preview_rows: [
          nativePreviewCell({ row: 2, col: 3, cell_ref: "C2", masked: "c2" }),
        ],
      },
    ],
  });

  assert.deepEqual(canonical.preview_rows, [
    {
      original_preview: [null, "fixture-original"],
      masked: ["", "b1"],
      row_index: 2,
      sheet: "Second",
    },
    {
      original_preview: [null, "fixture-original"],
      masked: ["", "b3"],
      row_index: 4,
      sheet: "Second",
    },
    {
      original_preview: [null, null, "fixture-original"],
      masked: ["", "", "c2"],
      row_index: 3,
      sheet: "First",
    },
  ]);
});

test("accepts an empty native response and an empty sheet preview", () => {
  assert.deepEqual(toCanonicalExcelMaskPreview({ sheets: [] }), {
    preview_rows: [],
    conflicts: [],
  });
  assert.deepEqual(
    toCanonicalExcelMaskPreview({
      sheets: [{ sheet_name: "Empty", headers: ["A"], preview_rows: [] }],
    }),
    { preview_rows: [], conflicts: [] }
  );
});

test("rejects malformed native preview envelopes and fields", () => {
  const malformedResponses: unknown[] = [
    null,
    {},
    { sheets: "not-an-array" },
    { sheets: [{ sheet_name: "Sheet1", headers: [], preview_rows: "bad" }] },
    { sheets: [{ sheet_name: "Sheet1", headers: [42], preview_rows: [] }] },
    { sheets: [{ sheet_name: "", headers: [], preview_rows: [] }] },
    { sheets: [{ sheet_name: "Sheet1", headers: ["A"], preview_rows: [null] }] },
    {
      sheets: [
        {
          sheet_name: "Sheet1",
          headers: ["A"],
          preview_rows: [nativePreviewCell({ original_preview: 42 as unknown as string })],
        },
      ],
    },
    {
      sheets: [
        {
          sheet_name: "Sheet1",
          headers: ["A"],
          preview_rows: [nativePreviewCell({ masked: 42 as unknown as string })],
        },
      ],
    },
    {
      sheets: [
        {
          sheet_name: "Sheet1",
          headers: ["A"],
          preview_rows: [nativePreviewCell({ strategy_id: 42 as unknown as string })],
        },
      ],
    },
    {
      sheets: [
        {
          sheet_name: "Sheet1",
          headers: ["A"],
          preview_rows: [nativePreviewCell({ cell_ref: 42 as unknown as string })],
        },
      ],
    },
    {
      sheets: [
        {
          sheet_name: "Sheet1",
          headers: ["A"],
          preview_rows: [nativePreviewCell({ row: 0, cell_ref: "A0" })],
        },
      ],
    },
    {
      sheets: [
        {
          sheet_name: "Sheet1",
          headers: ["A"],
          preview_rows: [nativePreviewCell({ col: 2, cell_ref: "B1" })],
        },
      ],
    },
    {
      sheets: [
        {
          sheet_name: "Sheet1",
          headers: ["A"],
          preview_rows: [nativePreviewCell({ cell_ref: "B1" })],
        },
      ],
    },
  ];

  for (const response of malformedResponses) {
    assert.throws(() => toCanonicalExcelMaskPreview(response), ExcelMaskingContractError);
  }
});

test("fails closed for duplicate cells and never echoes an original preview in errors", () => {
  const original = "fixture-original-secret";
  const duplicate = nativePreviewCell({
    original_preview: original,
    row: 1,
    col: 1,
    cell_ref: "A1",
  });
  const response = {
    sheets: [
      {
        sheet_name: "Sheet1",
        headers: ["A"],
        preview_rows: [duplicate, { ...duplicate, masked: "different-mask" }],
      },
    ],
  };

  assert.throws(
    () => toCanonicalExcelMaskPreview(response),
    (error: unknown) => {
      assert.ok(error instanceof ExcelMaskingContractError);
      assert.equal(error.message.includes(original), false);
      return true;
    }
  );

  assert.throws(
    () =>
      toCanonicalExcelMaskPreview({
        sheets: [
          { sheet_name: "Sheet1", headers: [], preview_rows: [] },
          { sheet_name: "sheet1", headers: [], preview_rows: [] },
        ],
      }),
    ExcelMaskingContractError
  );
});

// ---------------------------------------------------------------------
// R-closeout (工作包 D): 桌面 Excel 脱敏失败的安全分类文案
// ---------------------------------------------------------------------

test("classifyDesktopExcelApplyError maps known failure categories to fixed actionable messages", () => {
  const cases: Array<[string, string]> = [
    ["独立二级口令不能为空", "口令不能为空"],
    ["Sandbox passphrase must not be empty", "沙箱口令不能为空"],
    ["Failed to read CSV file: /tmp/x.csv", "无法读取 CSV 文件"],
    ["Failed to open Excel: /tmp/x.xlsx", "无法打开 Excel 文件"],
    ["Failed to process legacy xls file: ...", "无法打开 Excel 文件"],
    ["Permission denied (os error 13)", "没有权限读写文件"],
    ["ECMAP 加密失败: cipher error", "加密相关处理失败"],
    ["Invalid magic bytes", "映射或加密源文件格式无效"],
    ["Data too short", "映射或加密源文件格式无效"],
  ];
  for (const [raw, expectedFragment] of cases) {
    const message = classifyDesktopExcelApplyError(raw);
    assert.ok(
      message.includes(expectedFragment),
      `raw "${raw}" must map to a message containing "${expectedFragment}", got: ${message}`
    );
  }
});

test("classifyDesktopExcelApplyError classifies contract errors by their fixed message before the generic fallback", () => {
  // UI-STATE-003: an `ExcelMaskingContractError` whose message names the
  // sandbox passphrase must classify as sandbox_passphrase, not as the
  // generic contract fallback.
  const message = classifyDesktopExcelApplyError(
    new ExcelMaskingContractError("沙箱口令不能为空")
  );
  assert.equal(message, "沙箱口令不能为空，请在设置中配置沙箱口令后重试。");
});

test("classifyDesktopExcelPreviewError classifies a sandbox-passphrase contract error as actionable, not as structure-invalid", () => {
  // UI-STATE-003: if a sandbox-passphrase error ever reaches the preview
  // classifier, it must show the actionable sandbox message instead of the
  // misleading "Excel 预览数据结构无效".
  const message = classifyDesktopExcelPreviewError(
    new ExcelMaskingContractError("沙箱口令不能为空")
  );
  assert.equal(message, "沙箱口令不能为空，请在设置中配置沙箱口令后重试。");
});

test("classifyDesktopExcelApplyError never echoes path, passphrase, stack or ciphertext", () => {
  const secret = "s3cr3t-passphrase-2026";
  const path = "/private/tmp/original-客户.xlsx";
  const stack = "at excel_apply_masking (src-tauri/src/commands/excel_masking.rs:851)";
  const cipher = "VAULT_ENCSRC\x01\x00\x00\x00";
  const raw = `读取原文件失败: ${path} 口令=${secret} ${stack} ${cipher}`;
  const message = classifyDesktopExcelApplyError(raw);
  assert.ok(!message.includes(secret), "must not echo the passphrase");
  assert.ok(!message.includes(path), "must not echo the absolute path");
  assert.ok(!message.includes("excel_masking.rs"), "must not echo stack frames");
  assert.ok(!message.includes("VAULT_ENCSRC"), "must not echo ciphertext");
  assert.ok(!message.includes(raw), "must not echo the raw error");
});

test("classifyDesktopExcelApplyError falls back to a fixed message for unknown and empty errors", () => {
  const fallback = "Excel 脱敏执行失败，请检查配置后重试。";
  assert.equal(classifyDesktopExcelApplyError(new Error("unexpected internal state")), fallback);
  assert.equal(classifyDesktopExcelApplyError(""), fallback);
  assert.equal(classifyDesktopExcelApplyError(undefined), fallback);
  assert.equal(classifyDesktopExcelApplyError(null), fallback);
  assert.equal(classifyDesktopExcelApplyError({ nested: "object" }), fallback);
});

// ---------------------------------------------------------------------
// R-closeout (工作包 D): 桌面 Excel 预览失败的安全分类文案
// ---------------------------------------------------------------------

test("classifyDesktopExcelPreviewError maps known failure categories to fixed preview messages", () => {
  const cases: Array<[string, string]> = [
    ["独立二级口令不能为空", "口令不能为空"],
    ["Sandbox passphrase must not be empty", "沙箱口令不能为空"],
    ["Failed to read CSV file: /tmp/x.csv", "无法读取 CSV 文件"],
    ["Failed to open Excel: /tmp/x.xlsx", "无法打开 Excel 文件"],
    ["Permission denied (os error 13)", "没有权限读写文件"],
    ["ECMAP 加密失败: cipher error", "加密相关处理失败"],
    ["Invalid magic bytes", "文件格式无效或已损坏"],
    ["Data too short", "文件格式无效或已损坏"],
  ];
  for (const [raw, expectedFragment] of cases) {
    const message = classifyDesktopExcelPreviewError(raw);
    assert.ok(
      message.includes(expectedFragment),
      `raw "${raw}" must map to a message containing "${expectedFragment}", got: ${message}`
    );
  }
});

test("classifyDesktopExcelPreviewError maps contract conversion failures to the fixed structure message", () => {
  const message = classifyDesktopExcelPreviewError(
    new ExcelMaskingContractError("Tauri Excel 预览返回结构无效，拒绝继续")
  );
  assert.equal(message, "Excel 预览数据结构无效，请重新选择文件后重试。");
});

test("classifyDesktopExcelPreviewError never echoes path, passphrase, stack or ciphertext", () => {
  const secret = "s3cr3t-passphrase-2026";
  const path = "/private/tmp/original-客户.xlsx";
  const stack = "at toCanonicalExcelMaskPreview (src/lib/excelMaskingContract.ts:226)";
  const cipher = "VAULT_ENCSRC\x01\x00\x00\x00";
  const raw = `预览失败: ${path} 口令=${secret} ${stack} ${cipher}`;
  const message = classifyDesktopExcelPreviewError(raw);
  assert.ok(!message.includes(secret), "must not echo the passphrase");
  assert.ok(!message.includes(path), "must not echo the absolute path");
  assert.ok(!message.includes("excelMaskingContract.ts"), "must not echo stack frames");
  assert.ok(!message.includes("VAULT_ENCSRC"), "must not echo ciphertext");
  assert.ok(!message.includes(raw), "must not echo the raw error");
});

test("classifyDesktopExcelPreviewError falls back to a fixed preview message for unknown errors", () => {
  const fallback = "预览生成失败，请检查配置后重试。";
  assert.equal(classifyDesktopExcelPreviewError(new Error("boom")), fallback);
  assert.equal(classifyDesktopExcelPreviewError(""), fallback);
  assert.equal(classifyDesktopExcelPreviewError(undefined), fallback);
  assert.equal(classifyDesktopExcelPreviewError({ nested: "object" }), fallback);
});

// ---------------------------------------------------------------------
// R-closeout (preview root-cause lock): the REAL native preview response
// captured from the current .app (preview-diagnostics-responses.log) must
// keep passing the canonical contract. This pins the exact shape the Rust
// command produces so a future native/contract drift is caught here.
// ---------------------------------------------------------------------

const REAL_NATIVE_PREVIEW_RESPONSE = {
  sheets: [
    {
      sheet_name: "Sheet1",
      headers: ["Name", "Phone", "Email"],
      preview_rows: [
        { original_preview: "Alice", masked: "Alice", strategy_id: "default:identity", row: 1, col: 1, cell_ref: "A1" },
        { original_preview: "13900000", masked: "***********", strategy_id: "FULL_MASK", row: 1, col: 2, cell_ref: "B1" },
        { original_preview: "alice@ex", masked: "alice@example.invalid", strategy_id: "default:identity", row: 1, col: 3, cell_ref: "C1" },
        { original_preview: "Bob", masked: "Bob", strategy_id: "default:identity", row: 2, col: 1, cell_ref: "A2" },
        { original_preview: "13800000", masked: "***********", strategy_id: "FULL_MASK", row: 2, col: 2, cell_ref: "B2" },
        { original_preview: "bob@exam", masked: "bob@example.invalid", strategy_id: "default:identity", row: 2, col: 3, cell_ref: "C2" },
        { original_preview: "Line\nbre", masked: "Line\nbreak", strategy_id: "default:identity", row: 3, col: 1, cell_ref: "A3" },
        { original_preview: "中文", masked: "中文", strategy_id: "default:identity", row: 4, col: 1, cell_ref: "A4" },
      ],
    },
    {
      sheet_name: "Sheet2",
      headers: ["Phone", "Number"],
      preview_rows: [
        { original_preview: "13900000", masked: "13900000000", strategy_id: "default:identity", row: 1, col: 1, cell_ref: "A1" },
        { original_preview: "42", masked: "42", strategy_id: "default:identity", row: 1, col: 2, cell_ref: "B1" },
      ],
    },
  ],
};

test("real native preview response from the current .app converts to the canonical shape", () => {
  const canonical = toCanonicalExcelMaskPreview(REAL_NATIVE_PREVIEW_RESPONSE);
  assert.equal(canonical.conflicts.length, 0);
  assert.equal(canonical.preview_rows.length, 5);
  const sheet1Rows = canonical.preview_rows.filter((row) => row.sheet === "Sheet1");
  assert.deepEqual(
    sheet1Rows.map((row) => row.row_index),
    [2, 3, 4, 5]
  );
  // FULL_MASK phone cells become the canonical masked values.
  const phoneRow = sheet1Rows[0];
  assert.equal(phoneRow.masked[1], "***********");
  assert.equal(phoneRow.original_preview[1], "13900000");
  // Sheet2 contributes one canonical row (row_index 2).
  assert.equal(
    canonical.preview_rows.filter((row) => row.sheet === "Sheet2").length,
    1
  );
});
