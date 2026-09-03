/* Copyright 2026 CheersAI Team. Licensed under Apache-2.0 */
// Scope note: this file was originally restore-invoke-argument-contract-only
// (TASK-EXCEL-P0-RESTORE-IPC-PASSPHRASE-CLOSEOUT-001's initial approval). Its
// scope was explicitly widened by the same task's follow-up authorization to
// also cover the `excel_parse_structure` response-envelope contract in
// `tauri.ts`, rather than introduce a new, unwhitelisted test file.
import test from "node:test";
import assert from "node:assert/strict";

import {
  EXCEL_RESTORE_FROM_ECMAP_COMMAND,
  ExcelStructureResponseError,
  buildExcelRestoreInvokeArgs,
  extractSheetsFromExcelStructureResponse,
} from "./tauri";
import type { ExcelRestoreReq, SheetDef } from "@/types/commands";

// R6/AC-11/AC-12: the Rust command `excel_restore_from_ecmap`
// (src-tauri/src/commands/excel_masking.rs) takes a single named parameter
// `restore: ExcelRestoreReq`. Tauri's invoke() matches JS object keys to Rust
// parameter names by exact name, so a `req` key (or any name other than
// `restore`) is a different, unrecognized argument and the real desktop call
// fails with a missing-required-argument error — this was reproduced on the
// final .app for both restore path A and path B before this fix.

function baseRestoreReq(overrides: Partial<ExcelRestoreReq> = {}): ExcelRestoreReq {
  return {
    restore_mode: "A",
    masked_file_path: "/tmp/fixture/masked.xlsx",
    ecmap_file_path: "/tmp/fixture/masked.ecmap",
    encrypted_source_path: "/tmp/fixture/masked.encrypted_src",
    output_path: "/tmp/fixture/restored.xlsx",
    passphrase: "fixture-restore-test-pass-only-for-testing",
    ...overrides,
  };
}

test("EXCEL_RESTORE_FROM_ECMAP_COMMAND is the exact Rust command name", () => {
  assert.equal(EXCEL_RESTORE_FROM_ECMAP_COMMAND, "excel_restore_from_ecmap");
});

test("buildExcelRestoreInvokeArgs wraps the request under a top-level `restore` key", () => {
  const req = baseRestoreReq();
  const args = buildExcelRestoreInvokeArgs(req);
  assert.deepEqual(args, { restore: req });
});

test("buildExcelRestoreInvokeArgs never produces a `req` key (regression guard for the original bug)", () => {
  const req = baseRestoreReq();
  const args = buildExcelRestoreInvokeArgs(req) as Record<string, unknown>;
  assert.equal("req" in args, false, "a stray `req` key must never reappear");
  assert.equal("restore" in args, true);
  assert.equal(Object.keys(args).length, 1, "the payload must carry exactly one top-level key");
});

test("buildExcelRestoreInvokeArgs does not mutate or rewrite the request content", () => {
  const req = baseRestoreReq({ restore_mode: "B", user_original_file_path: "/tmp/fixture/original.xlsx", encrypted_source_path: undefined });
  const args = buildExcelRestoreInvokeArgs(req);
  assert.equal(args.restore, req, "must carry the exact same request object, not a copy with altered fields");
  assert.deepEqual(args.restore, {
    restore_mode: "B",
    masked_file_path: "/tmp/fixture/masked.xlsx",
    ecmap_file_path: "/tmp/fixture/masked.ecmap",
    encrypted_source_path: undefined,
    user_original_file_path: "/tmp/fixture/original.xlsx",
    output_path: "/tmp/fixture/restored.xlsx",
    passphrase: "fixture-restore-test-pass-only-for-testing",
  });
});

test("buildExcelRestoreInvokeArgs round-trips path B (no encrypted_source_path, SHA-256-paired original) unchanged", () => {
  const req = baseRestoreReq({
    restore_mode: "B",
    encrypted_source_path: undefined,
    user_original_file_path: "/tmp/fixture/original.xlsx",
  });
  const { restore } = buildExcelRestoreInvokeArgs(req);
  assert.equal(restore.restore_mode, "B");
  assert.equal(restore.encrypted_source_path, undefined);
  assert.equal(restore.user_original_file_path, "/tmp/fixture/original.xlsx");
});

// excel_parse_structure response envelope contract
//
// Rust returns `ExcelStructure { sheets: Vec<SheetDef> }`. The old wrapper in
// tauri.ts typed the invoke() result as a bare `SheetDef[]`, which type-checks
// but is wrong at runtime — `sheets` state ends up holding the envelope
// object, and any later `.find`/`.map` on it throws "X.find is not a
// function". These tests pin the fix: a normal envelope is unwrapped to its
// `sheets` array, an empty `sheets` array is accepted as legal (zero-sheet
// files are not malformed), and any response missing a legal `sheets` array
// throws a clear, safe `ExcelStructureResponseError` instead of silently
// producing a shape that crashes later.

function sheet(name: string): SheetDef {
  return {
    name,
    headers: ["姓名", "手机号"],
    column_samples: [["测试甲"], ["13900000001"]],
    max_row: 1,
    max_col: 2,
  };
}

test("extractSheetsFromExcelStructureResponse unwraps a normal { sheets } envelope", () => {
  const sheets = [sheet("Sheet1"), sheet("Sheet2")];
  const result = extractSheetsFromExcelStructureResponse({ sheets });
  assert.equal(result, sheets, "must return the same array reference, not a copy");
  assert.equal(result.length, 2);
});

test("extractSheetsFromExcelStructureResponse accepts an empty sheets array as legal", () => {
  const result = extractSheetsFromExcelStructureResponse({ sheets: [] });
  assert.deepEqual(result, []);
});

test("extractSheetsFromExcelStructureResponse rejects a response missing the sheets key", () => {
  assert.throws(
    () => extractSheetsFromExcelStructureResponse({}),
    ExcelStructureResponseError
  );
});

test("extractSheetsFromExcelStructureResponse rejects a response whose sheets is not an array", () => {
  assert.throws(
    () => extractSheetsFromExcelStructureResponse({ sheets: "not-an-array" }),
    ExcelStructureResponseError
  );
  assert.throws(
    () => extractSheetsFromExcelStructureResponse({ sheets: { name: "Sheet1" } }),
    ExcelStructureResponseError
  );
  assert.throws(
    () => extractSheetsFromExcelStructureResponse({ sheets: null }),
    ExcelStructureResponseError
  );
});

test("extractSheetsFromExcelStructureResponse rejects null, undefined, and non-object responses", () => {
  assert.throws(() => extractSheetsFromExcelStructureResponse(null), ExcelStructureResponseError);
  assert.throws(() => extractSheetsFromExcelStructureResponse(undefined), ExcelStructureResponseError);
  assert.throws(() => extractSheetsFromExcelStructureResponse("oops"), ExcelStructureResponseError);
  assert.throws(() => extractSheetsFromExcelStructureResponse(42), ExcelStructureResponseError);
});

test("extractSheetsFromExcelStructureResponse also rejects the old bare-array shape (no silent backward-compat guess)", () => {
  // Before this fix, a bare array was exactly the (wrong) shape callers used
  // to assume; it must now be rejected explicitly rather than accepted as if
  // it were still the contract.
  assert.throws(
    () => extractSheetsFromExcelStructureResponse([sheet("Sheet1")]),
    ExcelStructureResponseError
  );
});

test("extractSheetsFromExcelStructureResponse error message leaks no path, passphrase, or stack detail", () => {
  try {
    extractSheetsFromExcelStructureResponse({ sheets: "not-an-array" });
    assert.fail("expected a throw");
  } catch (err) {
    const message = (err as Error).message;
    assert.doesNotMatch(message, /\//);
    assert.doesNotMatch(message, /passphrase/i);
  }
});
