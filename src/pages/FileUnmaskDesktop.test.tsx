/* Copyright 2026 CheersAI Team. Licensed under Apache-2.0 */
import test from "node:test";
import assert from "node:assert/strict";

import { describeExcelRestoreSuccess } from "./FileUnmaskDesktop";
import type { ExcelRestoreResult } from "@/types/commands";

// TASK-EXCEL-P0-RESTORE-IPC-PASSPHRASE-CLOSEOUT-001: the independent tester's
// third-round desktop regression found path A/B restore itself succeeding,
// but the success card reading nonexistent `restored_count`/`matched` fields
// (the real Rust DTO is `{ restored_path, sha256_verified }`), which made the
// count always show blank and — since `!undefined` is `true` — always
// claimed "SHA 未匹配" even when `sha256_verified` was `true`. These tests pin
// the fix at the display-contract boundary, independent of TypeScript's own
// (compile-time only) shape check.

test("describeExcelRestoreSuccess reports SHA-256 verified when the DTO says so", () => {
  const result: ExcelRestoreResult = {
    restored_path: "/tmp/fixture/restore_test_已还原.xlsx",
    sha256_verified: true,
  };
  const display = describeExcelRestoreSuccess(result);
  assert.equal(display.statusText, "SHA-256 校验通过");
  assert.equal(display.outputPath, "/tmp/fixture/restore_test_已还原.xlsx");
});

test("describeExcelRestoreSuccess reports SHA-256 not verified when the DTO says so, without fabricating a count", () => {
  const result: ExcelRestoreResult = {
    restored_path: "/tmp/fixture/restore_test_已还原.xlsx",
    sha256_verified: false,
  };
  const display = describeExcelRestoreSuccess(result);
  assert.equal(display.statusText, "SHA-256 未通过校验");
});

test("describeExcelRestoreSuccess never reads a restored_count or matched field (regression guard for the original bug)", () => {
  // A result shaped exactly like the DTO (no extra fields) must still work —
  // proves the function does not silently depend on fields the real Rust
  // command never sends.
  const result = { restored_path: "/tmp/x.xlsx", sha256_verified: true } as ExcelRestoreResult;
  assert.deepEqual(Object.keys(result).sort(), ["restored_path", "sha256_verified"]);
  const display = describeExcelRestoreSuccess(result);
  assert.equal(display.statusText, "SHA-256 校验通过");
  assert.ok(
    !("restored_count" in display) && !("matched" in display),
    "display object must not carry the removed fields forward"
  );
});

test("describeExcelRestoreSuccess passes the real output path through unmodified", () => {
  const paths = [
    "/Users/fixture/Desktop/output/small_functional_已还原.xlsx",
    "/private/tmp/cheers-restore-fixture/restore_test_已还原.xlsx",
  ];
  for (const restored_path of paths) {
    const display = describeExcelRestoreSuccess({ restored_path, sha256_verified: true });
    assert.equal(display.outputPath, restored_path);
  }
});
