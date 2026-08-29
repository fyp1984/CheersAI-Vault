/* Copyright 2026 CheersAI Team. Licensed under Apache-2.0 */
/**
 * R-closeout (TASK-EXCEL-OUTPUT-RECOVERY-CONSISTENCY-CLOSEOUT-001,
 * 工作包 C): 企业 Excel 恢复错误文案契约。所有文案必须是固定安全文案，
 * 绝不回显口令、路径、堆栈、SQL 或密文正文；错误码未知时也不得透传原始
 * 响应 message 之外的内容。
 */
import test from "node:test";
import assert from "node:assert/strict";
import { excelRestoreErrorMessage } from "./excelClient";
import { parseContentDispositionFilename } from "./downloadName";

test("content-disposition with RFC 5987 filename* decodes Chinese download names", () => {
  // Runtime R-closeout shape: ASCII quoted fallback + filename*=UTF-8''…
  // The browser parser must prefer filename* and percent-decode the Chinese
  // name (member names are ASCII; restored names are `{stem}_还原.{ext}`).
  const memberHeader = 'attachment; filename="restored_file.xlsx"; filename*=UTF-8\'\'%E5%91%98%E5%B7%A5%E5%B7%A5%E8%B5%84%E8%A1%A8_%E8%BF%98%E5%8E%9F.xlsx';
  assert.equal(
    parseContentDispositionFilename(memberHeader),
    "员工工资表_还原.xlsx"
  );
});

test("content-disposition ASCII-only quoted fallback stays parseable", () => {
  const header = 'attachment; filename="workbook_masked.xlsx"';
  assert.equal(parseContentDispositionFilename(header), "workbook_masked.xlsx");
});

test("content-disposition missing filename falls back to null for callers", () => {
  assert.equal(parseContentDispositionFilename(null), null);
  assert.equal(parseContentDispositionFilename(""), null);
});


test("known failure codes map to fixed safe, actionable messages", () => {
  const cases: Array<[string | undefined, string]> = [
    ["EXCEL_RESTORE_DECRYPT_FAILED", "口令不正确"],
    ["EXCEL_RESTORE_MISMATCH", "材料校验未通过"],
    ["EXCEL_RESTORE_MATERIAL_MISSING", "缺少所需材料"],
    ["EXCEL_RESTORE_MODE_INVALID", "路径参数无效"],
    ["EXCEL_RESTORE_MODE_REQUIRED", "路径参数无效"],
    ["NOT_FOUND", "已经不存在"],
  ];
  for (const [code, expectedFragment] of cases) {
    const message = excelRestoreErrorMessage("http", code, "raw server detail");
    assert.ok(
      message.includes(expectedFragment),
      `code ${code} must mention "${expectedFragment}", got: ${message}`
    );
  }
});

test("network and invalid-count reasons map to fixed messages", () => {
  assert.equal(
    excelRestoreErrorMessage("network", undefined, undefined),
    "当前连不上本地服务，请确认服务已启动后再试。"
  );
  assert.equal(
    excelRestoreErrorMessage("invalid-count", undefined, undefined),
    "恢复结果异常，没有生成任何文件。请稍后再试。"
  );
});

test("error text never echoes the raw message, passphrase, path, stack or ciphertext", () => {
  const secret = "super-secret-passphrase-9f8e7d6c";
  const path = "/private/tmp/user-original-2026.xlsx";
  const stack = "at excel_restore_handler (src/excel.rs:1234)";
  const cipher = "VAULT_ENCSRC\x01".concat("\u0000".repeat(64));
  const raw = `decrypt failed for ${path} with ${secret} ${stack} ${cipher}`;
  const message = excelRestoreErrorMessage("http", "EXCEL_RESTORE_DECRYPT_FAILED", raw);
  assert.ok(!message.includes(secret), "must not echo the passphrase");
  assert.ok(!message.includes(path), "must not echo the absolute path");
  assert.ok(!message.includes("src/excel.rs"), "must not echo stack frames");
  assert.ok(!message.includes("VAULT_ENCSRC"), "must not echo ciphertext magic");
  assert.ok(!message.includes(raw), "must not echo the raw server message");
});

test("unknown codes fall back to the server message or a fixed fallback, never raw internals", () => {
  const fallback = excelRestoreErrorMessage("http", "SOME_UNKNOWN_CODE", undefined);
  assert.equal(fallback, "恢复没有成功，请稍后再试。");
  const passthrough = excelRestoreErrorMessage("http", "SOME_UNKNOWN_CODE", "服务端返回了一个说明");
  assert.equal(passthrough, "服务端返回了一个说明");
});
