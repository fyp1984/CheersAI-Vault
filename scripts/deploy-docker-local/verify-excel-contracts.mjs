/* Copyright 2026 CheersAI Team. Licensed under Apache-2.0 */
/**
 * Pure-JS smoke (requires no TS loader). Verify in-process behavior:
 *
 *  (a) resolveColumnSamples produces column-indexed samples from both the
 *      modern `column_samples: string[][]` contract and from legacy row-based
 *      `data_hint: string[]` (strings joined with " | ").
 *  (b) 404/HTTP literal sanitization blacklist rejects raw English warp
 *      plain-text bodies; callers that pass these strings into the UI
 *      normalization layer must get a stable Chinese replacement instead.
 *
 * Run with:  node scripts/deploy-docker-local/verify-excel-contracts.mjs
 */

const EN_HTTP_LITERALS =
  /the requested resource was not found|failed to fetch|networkerror|not found|bad gateway|service unavailable|gateway timeout|invalid response|failed to invoke/i;

function looksLikeTrustedChinese(value) {
  if (typeof value !== "string") return false;
  const trimmed = value.trim();
  if (!trimmed || trimmed.length > 240) return false;
  if (EN_HTTP_LITERALS.test(trimmed)) return false;
  return /[\u4e00-\u9fa5]/.test(trimmed);
}

function normalizeCaughtErrorMessage(error, fallback) {
  const defaultMsg =
    fallback && typeof fallback === "string" && fallback.trim().length > 0
      ? fallback
      : "Excel 脱敏执行失败，请稍后重试。";
  const raw =
    error && error instanceof Error
      ? error.message
      : typeof error === "string"
        ? error
        : "";
  if (!raw) return defaultMsg;
  if (looksLikeTrustedChinese(raw)) return raw;
  if (EN_HTTP_LITERALS.test(raw)) {
    return "请求的 Runtime 接口不存在，请确认本地 Runtime 版本与前端版本一致并重新部署。";
  }
  return defaultMsg;
}

function resolveColumnSamples(sheet) {
  const headers = sheet?.headers ?? [];
  const column_samples = sheet?.column_samples;
  const data_hint = sheet?.data_hint;
  const width = headers.length;
  if (
    Array.isArray(column_samples) &&
    column_samples.length === width &&
    column_samples.every(Array.isArray)
  ) {
    return column_samples;
  }
  if (Array.isArray(data_hint) && data_hint.length > 0) {
    const perCol = [];
    for (let c = 0; c < width; c += 1) {
      const values = [];
      for (const rowBlob of data_hint.slice(0, 5)) {
        const cells =
          typeof rowBlob === "string" && rowBlob.includes(" | ")
            ? rowBlob.split(" | ")
            : [typeof rowBlob === "string" ? rowBlob : ""];
        const v = cells[c] ?? "";
        if (typeof v === "string" && v.trim().length > 0) values.push(v);
      }
      perCol.push(values);
    }
    return perCol;
  }
  return new Array(width).fill(undefined);
}

function assertEqual(actual, expected, msg) {
  if (actual !== expected) {
    throw new Error(`${msg} — expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
  }
}

function main() {
  const warp404Raw = "The requested resource was not found";
  const normalized = normalizeCaughtErrorMessage(warp404Raw, "Excel 脱敏执行失败，请稍后重试。");
  if (!normalized || /was not found/i.test(normalized)) {
    throw new Error(`404 message should be rewritten, got ${normalized}`);
  }
  if (!normalized.includes("Runtime") || !/不存在|版本|重新部署/.test(normalized)) {
    throw new Error(`404 normalized message is not the expected Chinese contract, got ${normalized}`);
  }
  console.log("404 translate OK =>", normalized);

  const tawnyInvokeErr = new Error("failed to invoke handler `excel_apply_masking`: io error");
  const normInvoke = normalizeCaughtErrorMessage(tawnyInvokeErr, "Excel 脱敏执行失败，请稍后重试。");
  if (/failed to invoke|io error/i.test(normInvoke)) {
    throw new Error(`Tauri invoke literal leaked into UI message: ${normInvoke}`);
  }
  console.log("tauri invoke literal sanitized OK =>", normInvoke);

  const legacy = {
    name: "评分",
    headers: ["维度", "1分", "3分", "5分"],
    column_samples: [],
    data_hint: [
      "业务价值 | 低频、影响人少、收益难说明 | 有明确痛点，可估算局部收益 | 战略级价值，可量化全局影响",
      "知识可用性 | 资料缺失或版本冲突 | 资料基本可得，仍需整理确认 | 权威资料齐备，版本与边界明确",
      "边界清晰度 | 输入输出和排除项不清 | 可圈定主要流程，但边界仍有争议 | 范围与排除项无歧义，共识已对齐",
    ],
    max_row: 4,
    max_col: 4,
  };
  const samples = resolveColumnSamples(legacy);
  const c0 = samples[0] ?? [];
  if (!c0.includes("业务价值") || !c0.includes("知识可用性") || !c0.includes("边界清晰度")) {
    throw new Error(`column0 should be legacy 维度 column, got ${JSON.stringify(c0)}`);
  }
  const c1 = samples[1] ?? [];
  if (!c1.join("|").includes("低频")) {
    throw new Error(`column1 should be legacy 1分, got ${JSON.stringify(c1)}`);
  }
  assertEqual(c0.length, 3, "legacy c0 expected 3 non-empty row values from 3 data_hint rows");
  console.log("legacy data_hint correction OK =>", { c0, c1, c2: samples[2] });

  const modern = {
    name: "评分",
    headers: ["维度", "1分", "3分", "5分"],
    column_samples: [
      ["业务价值", "知识可用性", "边界清晰度"],
      ["低频、影响人少", "资料缺失或版本冲突", "输入输出和排除项不清"],
      ["有明确痛点，可估算局部收益", "资料基本可得，仍需整理确认", "可圈定主要流程，但边界仍有争议"],
      ["战略级价值，可量化全局影响", "权威资料齐备，版本与边界明确", "范围与排除项无歧义"],
    ],
    max_row: 4,
    max_col: 4,
  };
  const ms = resolveColumnSamples(modern);
  assertEqual(ms.length, 4, "modern samples length must match headers length (=4)");
  if (!Array.isArray(ms[0]) || ms[0][1] !== "知识可用性") {
    throw new Error(`modern column_samples[0][1] broken: ${JSON.stringify(ms)}`);
  }
  assertEqual((ms[3] ?? []).length, 3, "modern 5分 column expected 3 sample values");
  console.log("modern column_samples OK");

  const emptySamples = {
    name: "EmptySheet",
    headers: ["A", "B", "C"],
    column_samples: [[], [], []],
    data_hint: [],
    max_row: 0,
    max_col: 3,
  };
  const es = resolveColumnSamples(emptySamples);
  assertEqual(es.length, 3, "empty column_samples array length must still match headers (=3)");
  console.log("empty column_samples length-vs-headers OK (will render as — in UI)");
}

try {
  main();
  console.log("verify-excel-contracts: ALL PASSED");
  process.exit(0);
} catch (err) {
  console.error("verify-excel-contracts: FAIL", err && err.message ? err.message : err);
  process.exit(1);
}
