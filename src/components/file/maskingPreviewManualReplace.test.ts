import test from "node:test";
import assert from "node:assert/strict";

import {
  applyManualReplacementToPreview,
  applyManualReplacementToPreviewFiles,
} from "./maskingPreviewManualReplace";

test("updates masked rows, markdown, and mapping when replacing an existing masked placeholder", () => {
  const result = applyManualReplacementToPreview(
    {
      original_rows: [["金金在这里"]],
      masked_rows: [["姓名1在这里"]],
      headers: ["内容"],
      original_file_stem: "金金-合同",
      original_file_name: "金金-合同.md",
      masked_file_stem: "demo",
      masked_file_name: "demo.md",
      mapping: [{ original: "金金", masked: "姓名1" }],
      masked_markdown: "姓名1在这里",
      masked_entity_count: 1,
    },
    "姓名1",
    "甲方",
  );

  assert.equal(result.count, 1);
  assert.deepEqual(result.preview.masked_rows, [["甲方在这里"]]);
  assert.equal(result.preview.masked_markdown, "甲方在这里");
  assert.deepEqual(result.preview.mapping, [{ original: "金金", masked: "甲方" }]);
});

test("falls back to original text and keeps saved markdown in sync", () => {
  const result = applyManualReplacementToPreview(
    {
      original_rows: [["金金在这里"]],
      masked_rows: [["姓名1在这里"]],
      headers: ["内容"],
      original_file_stem: "金金-合同",
      original_file_name: "金金-合同.md",
      masked_file_stem: "demo",
      masked_file_name: "demo.md",
      mapping: [{ original: "金金", masked: "姓名1" }],
      masked_markdown: "姓名1在这里",
      masked_entity_count: 1,
    },
    "金金",
    "甲方",
  );

  assert.equal(result.count, 1);
  assert.deepEqual(result.preview.masked_rows, [["甲方在这里"]]);
  assert.equal(result.preview.masked_markdown, "甲方在这里");
  assert.deepEqual(result.preview.mapping, [{ original: "金金", masked: "甲方" }]);
});

test("only updates the active preview file", () => {
  const result = applyManualReplacementToPreviewFiles(
    [
      {
        fileName: "a.md",
        preview: {
          original_rows: [["金金在这里"]],
          masked_rows: [["姓名1在这里"]],
          headers: ["内容"],
          original_file_stem: "金金",
          original_file_name: "a.md",
          masked_file_stem: "a",
          masked_file_name: "a.md",
          masked_markdown: "姓名1在这里",
        },
      },
      {
        fileName: "b.md",
        preview: {
          original_rows: [["手机号1在这里"]],
          masked_rows: [["***PHONE***1在这里"]],
          headers: ["内容"],
          original_file_stem: "手机号1",
          original_file_name: "b.md",
          masked_file_stem: "b",
          masked_file_name: "b.md",
          masked_markdown: "***PHONE***1在这里",
        },
      },
    ],
    1,
    "***PHONE***1",
    "联系电话",
  );

  assert.equal(result.count, 1);
  assert.equal(result.previews[0].preview.masked_markdown, "姓名1在这里");
  assert.equal(result.previews[1].preview.masked_markdown, "联系电话在这里");
});

test("updates masked file stem and file name when replacing original filename mapping", () => {
  const result = applyManualReplacementToPreview(
    {
      original_rows: [["内容"]],
      masked_rows: [["内容"]],
      headers: ["内容"],
      original_file_stem: "张三-13812345678-合同",
      original_file_name: "张三-13812345678-合同.md",
      masked_file_stem: "张*-138****5678-合同_脱敏",
      masked_file_name: "张*-138****5678-合同_脱敏.md",
      mapping: [
        { original: "张三", masked: "张*" },
        { original: "13812345678", masked: "138****5678" },
      ],
      masked_markdown: "内容",
      masked_entity_count: 2,
    },
    "13812345678",
    "联系电话",
  );

  assert.equal(result.count, 1);
  assert.equal(result.preview.masked_file_stem, "张*-联系电话-合同_脱敏");
  assert.equal(result.preview.masked_file_name, "张*-联系电话-合同_脱敏.md");
});
