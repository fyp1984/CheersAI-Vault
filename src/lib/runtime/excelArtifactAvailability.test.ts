/* Copyright 2026 CheersAI Team. Licensed under Apache-2.0 */
/**
 * R-closeout (TASK-EXCEL-OUTPUT-RECOVERY-CONSISTENCY-CLOSEOUT-001,
 * 工作包 C): 浏览器结果页只能渲染 Runtime manifest 中真实存在的 Excel
 * 成员。本文件以纯函数方式钉住 `excelMemberActionsForKinds`：未保留加密源
 * 时绝不返回「下载加密源」动作（也就永远不会发出该成员的下载请求）；
 * 加载中/失败/缺失清单一律零动作（fail-safe）。
 *
 * 说明：本工程前端 `.test.*` 文件统一使用 `node:test`（见 vitest.config.ts
 * 的 include/exclude），无 jsdom，无法直接挂载 React 页面，因此把该判定
 * 逻辑提取为 `excelClient.ts` 中的纯函数并以本文件覆盖，页面只负责渲染。
 */
import test from "node:test";
import assert from "node:assert/strict";
import {
  EXCEL_ARTIFACT_ACTION_LABELS,
  excelMemberActionsForKinds,
} from "./excelClient";
import type { RuntimeExcelPersistedFile } from "@/types/runtime";

function member(kind: RuntimeExcelPersistedFile["kind"]): RuntimeExcelPersistedFile {
  return { kind, display_name: `${kind}.bin`, size_bytes: 1 };
}

test("all four members present renders all four download actions", () => {
  const actions = excelMemberActionsForKinds([
    member("masked_workbook"),
    member("report"),
    member("ecmap"),
    member("encrypted_source"),
  ]);
  assert.deepEqual([...actions], ["masked_workbook", "report", "ecmap", "encrypted_source"]);
});

test("unretained manifest never offers the encrypted_source download action", () => {
  const actions = excelMemberActionsForKinds([
    member("masked_workbook"),
    member("report"),
    member("ecmap"),
  ]);
  assert.deepEqual([...actions], ["masked_workbook", "report", "ecmap"]);
  assert.ok(!actions.includes("encrypted_source"));
});

test("a manifest with only the masked workbook offers only that action", () => {
  const actions = excelMemberActionsForKinds([member("masked_workbook")]);
  assert.deepEqual([...actions], ["masked_workbook"]);
});

test("loading, error, undefined and empty memberships render zero actions (fail-safe)", () => {
  assert.deepEqual([...excelMemberActionsForKinds("loading")], []);
  assert.deepEqual([...excelMemberActionsForKinds("error")], []);
  assert.deepEqual([...excelMemberActionsForKinds(undefined)], []);
  assert.deepEqual([...excelMemberActionsForKinds([])], []);
});

test("action labels cover exactly the four Excel member kinds in a stable order", () => {
  assert.deepEqual(
    EXCEL_ARTIFACT_ACTION_LABELS.map((action) => action.kind),
    ["masked_workbook", "report", "ecmap", "encrypted_source"]
  );
  for (const action of EXCEL_ARTIFACT_ACTION_LABELS) {
    assert.ok(action.label.length > 0, "every action must have a label");
  }
});
