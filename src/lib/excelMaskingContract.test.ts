/* Copyright 2026 CheersAI Team. Licensed under Apache-2.0 */
import test from "node:test";
import assert from "node:assert/strict";

import {
  ExcelMaskingContractError,
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

test("rejects SANDBOX_REUSED mode when the sandbox passphrase is missing, empty, or whitespace-only", () => {
  for (const sandboxPassphrase of [undefined, "", "   \t\n"]) {
    assert.throws(
      () =>
        toTauriExcelMaskingConfig(baseConfig({ key_mode: "SANDBOX_REUSED" }), {
          sandboxPassphrase,
        }),
      (error: unknown) =>
        error instanceof ExcelMaskingContractError &&
        error.message === "沙箱口令不能为空"
    );
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
