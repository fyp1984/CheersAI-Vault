/* Copyright 2026 CheersAI Team. Licensed under Apache-2.0 */
import test from "node:test";
import assert from "node:assert/strict";

import {
  DROPZONE_OUTPUT_FORMAT_NOTE,
  EXCEL_APPLY_FAILURE_MESSAGE,
  EXCEL_KEY_MATERIAL_MISSING_MESSAGE,
  PROTECTED_EXCEL_ARTIFACT_INPUT_ERROR,
  executeExcelApplyRouting,
  isExcelFile,
  isProtectedExcelArtifact,
  partitionDropPaths,
  regularInputsAfterExcelFlow,
  toExcelOutputSummary,
} from "./DropZone";
import type { ExcelApplyResult, ExcelMaskingConfig } from "@/types/commands";

function config(filePath: string): ExcelMaskingConfig {
  return {
    file_path: filePath,
    sheet_policies: [],
    retain_encrypted_source: true,
    key_mode: "SANDBOX_REUSED",
  };
}

function appliedResult(stem: string): ExcelApplyResult {
  return {
    masked_path: `/safe/output/${stem}_masked.xlsx`,
    ecmap_path: `/safe/output/${stem}_masked.ecmap`,
    encrypted_source_path: `/safe/output/${stem}.encrypted_src`,
    report_md: "",
    status: "APPLIED",
  };
}

// R5: the dropzone must not claim a single uniform Markdown output for
// every format — Excel/CSV go through the enhanced .xlsx flow.

test("isExcelFile classifies Excel/CSV extensions and nothing else", () => {
  for (const excelPath of ["a.xlsx", "A.XLSX", "b.xls", "c.xlsm", "d.csv", "D.CSV"]) {
    assert.equal(isExcelFile(excelPath), true, excelPath);
  }
  for (const otherPath of ["a.docx", "b.pdf", "c.md", "d.txt", "e.pptx", "f.json"]) {
    assert.equal(isExcelFile(otherPath), false, otherPath);
  }
});

test("output format note mentions both .xlsx (Excel/CSV) and Markdown (other formats), not a single uniform format", () => {
  assert.match(DROPZONE_OUTPUT_FORMAT_NOTE, /\.xlsx/);
  assert.match(DROPZONE_OUTPUT_FORMAT_NOTE, /Markdown|\.md/);
  assert.doesNotMatch(
    DROPZONE_OUTPUT_FORMAT_NOTE,
    /统一保存为\s*Markdown/,
    "must not claim every format is uniformly saved as Markdown"
  );
});

test("protected Excel artifacts are recognized case-insensitively and direct input has a safe recovery hint", () => {
  for (const artifact of ["masked.ecmap", "SOURCE.ENCRYPTED_SRC"]) {
    assert.equal(isProtectedExcelArtifact(artifact), true, artifact);
  }
  for (const input of ["source.xlsx", "notes.txt", "report.md"]) {
    assert.equal(isProtectedExcelArtifact(input), false, input);
  }
  assert.match(PROTECTED_EXCEL_ARTIFACT_INPUT_ERROR, /不能作为新的脱敏输入/);
  assert.match(PROTECTED_EXCEL_ARTIFACT_INPUT_ERROR, /文件反脱敏/);
  assert.doesNotMatch(PROTECTED_EXCEL_ARTIFACT_INPUT_ERROR, /UTF-8|stack|invoke/i);
});

test("direct protected artifact input is rejected while unrelated original files remain accepted", () => {
  assert.deepEqual(
    partitionDropPaths([
      "/input/sample.ecmap",
      "/input/sample.encrypted_src",
      "/input/readme.txt",
    ]),
    {
      acceptedPaths: ["/input/readme.txt"],
      protectedArtifacts: [
        "/input/sample.ecmap",
        "/input/sample.encrypted_src",
      ],
    }
  );
});

test("Excel-only completion routes nothing into the normal input queue", () => {
  assert.deepEqual(
    regularInputsAfterExcelFlow([
      "/tmp/sample.xlsx",
      "/tmp/sample_masked.xlsx",
      "/tmp/sample_masked.ecmap",
      "/tmp/sample.encrypted_src",
    ]),
    []
  );
});

test("mixed Excel and TXT input keeps only original non-Excel files in the normal queue", () => {
  assert.deepEqual(
    regularInputsAfterExcelFlow([
      "/tmp/sample.xlsx",
      "/tmp/readme.txt",
      "/tmp/notes.md",
      "/tmp/sample_masked.ecmap",
    ]),
    ["/tmp/readme.txt", "/tmp/notes.md"]
  );
});

test("successful Excel results become terminal output summaries without secret or mapping fields", () => {
  const summary = toExcelOutputSummary({
    masked_path: "/safe/output/sample_masked.xlsx",
    ecmap_path: "/safe/output/sample_masked.ecmap",
    encrypted_source_path: "/safe/output/sample.encrypted_src",
    report_md: "",
    status: "APPLIED",
  });
  assert.deepEqual(summary, {
    maskedPath: "/safe/output/sample_masked.xlsx",
    ecmapPath: "/safe/output/sample_masked.ecmap",
    encryptedSourcePath: "/safe/output/sample.encrypted_src",
  });
  assert.doesNotMatch(JSON.stringify(summary), /passphrase|entries|originalPreview/i);
});

test("missing required Excel artifacts fails closed instead of falling back to the original input", () => {
  assert.throws(
    () =>
      toExcelOutputSummary({
        masked_path: "/safe/output/sample_masked.xlsx",
        ecmap_path: null,
        encrypted_source_path: null,
        report_md: "",
        status: "ERROR",
      }),
    /未生成完整的工作簿和映射产物/
  );
  assert.match(EXCEL_APPLY_FAILURE_MESSAGE, /未加入普通处理队列/);
  assert.doesNotMatch(EXCEL_APPLY_FAILURE_MESSAGE, /UTF-8|stack|invoke/i);
});

test("an ERROR status fails closed even if backend paths are accidentally present", () => {
  assert.throws(
    () =>
      toExcelOutputSummary({
        ...appliedResult("sample"),
        status: "ERROR",
      }),
    /未生成完整的工作簿和映射产物/
  );
});

test("successful Excel apply returns terminal products while onFilesDropped receives only original TXT input", async () => {
  const calls: string[] = [];
  const routing = await executeExcelApplyRouting({
    configs: [config("/input/sample.xlsx")],
    pendingPaths: ["/input/sample.xlsx", "/input/readme.txt"],
    outputDir: "/safe/output",
    sandboxPassphrase: "fictional-test-passphrase",
    applyMasking: async (_config, _outputDir, passphrase) => {
      assert.equal(passphrase, "fictional-test-passphrase");
      calls.push("apply");
      return appliedResult("sample");
    },
  });

  assert.deepEqual(calls, ["apply"]);
  assert.equal(routing.failureCount, 0);
  assert.deepEqual(routing.normalQueuePaths, ["/input/readme.txt"]);
  assert.deepEqual(routing.outputs, [
    {
      maskedPath: "/safe/output/sample_masked.xlsx",
      ecmapPath: "/safe/output/sample_masked.ecmap",
      encryptedSourcePath: "/safe/output/sample.encrypted_src",
    },
  ]);
});

test("failed Excel apply never falls back to the original Excel or partial artifacts", async () => {
  const routing = await executeExcelApplyRouting({
    configs: [config("/input/sample.xlsx")],
    pendingPaths: ["/input/sample.xlsx", "/input/readme.txt"],
    outputDir: "/safe/output",
    sandboxPassphrase: "fictional-test-passphrase",
    applyMasking: async () => {
      throw new Error("technical details must not be routed");
    },
  });

  assert.equal(routing.failureCount, 1);
  assert.deepEqual(routing.outputs, []);
  assert.deepEqual(routing.normalQueuePaths, ["/input/readme.txt"]);
});

// R-closeout (工作包 D): 桌面路径不再吞掉底层错误后只显示无法定位的通用
// 文案 — 首个失败会以安全分类文案透出，且绝不回显路径/口令/堆栈。
test("failed Excel apply exposes a safe classified first error, never raw internals", async () => {
  const secret = "top-secret-passphrase";
  const routing = await executeExcelApplyRouting({
    configs: [config("/input/sample.xlsx")],
    pendingPaths: ["/input/sample.xlsx"],
    outputDir: "/safe/output",
    sandboxPassphrase: "fictional-test-passphrase",
    applyMasking: async () => {
      throw new Error(
        `读取原文件失败: /private/tmp/secret.xlsx 口令=${secret} at excel_masking.rs:851`
      );
    },
  });

  assert.equal(routing.failureCount, 1);
  const message = routing.firstErrorMessage;
  assert.ok(typeof message === "string" && message.length > 0);
  assert.ok(!message.includes(secret), "must not echo the passphrase");
  assert.ok(!message.includes("/private/tmp"), "must not echo the absolute path");
  assert.ok(!message.includes("excel_masking.rs"), "must not echo stack frames");
  assert.ok(!message.includes("读取原文件失败"), "must not echo the raw error");
});

test("successful Excel apply exposes no first error message", async () => {
  const routing = await executeExcelApplyRouting({
    configs: [config("/input/sample.xlsx")],
    pendingPaths: ["/input/sample.xlsx"],
    outputDir: "/safe/output",
    sandboxPassphrase: "fictional-test-passphrase",
    applyMasking: async () => appliedResult("sample"),
  });
  assert.equal(routing.failureCount, 0);
  assert.equal(routing.firstErrorMessage, undefined);
});

// ---------------------------------------------------------------------
// TASK-EXCEL-SANDBOX-PASSPHRASE-CLIENT-CLOSEOUT-001 (AC-8): a submission
// whose selected key mode has no usable key material must fail closed before
// any Rust invocation — zero artifacts, zero routing, fixed safe message.
// ---------------------------------------------------------------------

test("blocked SANDBOX_REUSED apply invokes nothing and reports the fixed safe message (zero artifacts)", async () => {
  let applyCalls = 0;
  const routing = await executeExcelApplyRouting({
    configs: [config("/input/sample.xlsx")],
    pendingPaths: ["/input/sample.xlsx"],
    outputDir: "/safe/output",
    sandboxPassphrase: "   ",
    applyMasking: async () => {
      applyCalls += 1;
      return appliedResult("sample");
    },
  });

  assert.equal(applyCalls, 0, "no Rust apply invocation may happen without usable key material");
  assert.deepEqual(routing.outputs, []);
  assert.deepEqual(routing.normalQueuePaths, []);
  assert.equal(routing.failureCount, 1);
  assert.equal(routing.firstErrorMessage, EXCEL_KEY_MATERIAL_MISSING_MESSAGE);
});

test("blocked apply covers an empty sandbox passphrase in a mixed batch", async () => {
  let applyCalls = 0;
  const routing = await executeExcelApplyRouting({
    configs: [config("/input/one.xlsx"), config("/input/two.xlsx")],
    pendingPaths: ["/input/one.xlsx", "/input/two.xlsx"],
    outputDir: "/safe/output",
    sandboxPassphrase: "",
    applyMasking: async () => {
      applyCalls += 1;
      return appliedResult("sample");
    },
  });

  assert.equal(applyCalls, 0, "an empty sandbox passphrase must invoke nothing");
  assert.deepEqual(routing.outputs, []);
  assert.equal(routing.failureCount, 2);
  assert.equal(routing.firstErrorMessage, EXCEL_KEY_MATERIAL_MISSING_MESSAGE);
});

test("blocked apply covers an empty secondary passphrase in SECONDARY_PASSPHRASE mode", async () => {
  let applyCalls = 0;
  const blockedConfig: ExcelMaskingConfig = {
    ...config("/input/sample.xlsx"),
    key_mode: "SECONDARY_PASSPHRASE",
    secondary_passphrase: "   ",
  };
  const routing = await executeExcelApplyRouting({
    configs: [blockedConfig],
    pendingPaths: ["/input/sample.xlsx"],
    outputDir: "/safe/output",
    sandboxPassphrase: "fictional-test-passphrase",
    applyMasking: async () => {
      applyCalls += 1;
      return appliedResult("sample");
    },
  });

  assert.equal(applyCalls, 0);
  assert.deepEqual(routing.outputs, []);
  assert.equal(routing.failureCount, 1);
  assert.equal(routing.firstErrorMessage, EXCEL_KEY_MATERIAL_MISSING_MESSAGE);
});

test("DEVICE_KEY routing never needs key material and still applies normally", async () => {
  let applyCalls = 0;
  const routing = await executeExcelApplyRouting({
    configs: [{ ...config("/input/sample.xlsx"), key_mode: "DEVICE_KEY" }],
    pendingPaths: ["/input/sample.xlsx"],
    outputDir: "/safe/output",
    sandboxPassphrase: "",
    applyMasking: async () => {
      applyCalls += 1;
      return appliedResult("sample");
    },
  });

  assert.equal(applyCalls, 1);
  assert.equal(routing.failureCount, 0);
  assert.equal(routing.firstErrorMessage, undefined);
});

test("EXCEL_KEY_MATERIAL_MISSING_MESSAGE is fixed safe copy that never echoes values or internals", () => {
  const secret = "top-secret-passphrase";
  assert.ok(!EXCEL_KEY_MATERIAL_MISSING_MESSAGE.includes(secret), "must not echo the passphrase");
  assert.doesNotMatch(
    EXCEL_KEY_MATERIAL_MISSING_MESSAGE,
    /fileStore|passphrase|Error|invoke|stack|at \S+:\d+|\//
  );
  assert.match(EXCEL_KEY_MATERIAL_MISSING_MESSAGE, /未生成任何脱敏产物/);
  assert.match(EXCEL_KEY_MATERIAL_MISSING_MESSAGE, /沙箱管理/);
  assert.match(EXCEL_KEY_MATERIAL_MISSING_MESSAGE, /独立二级口令/);
  assert.match(EXCEL_KEY_MATERIAL_MISSING_MESSAGE, /PIN/);
});
