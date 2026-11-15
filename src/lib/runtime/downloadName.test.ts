import test from "node:test";
import assert from "node:assert/strict";

import {
  artifactDownloadName,
  parseContentDispositionFilename,
  restoreDownloadName,
  safeStem,
} from "./downloadName";

test("safeStem removes path separators, control chars, and extension", () => {
  assert.equal(safeStem("../张三-13900000000.txt"), "张三-13900000000");
  assert.equal(safeStem("demo\0name.md"), "demoname");
  assert.equal(safeStem("张*-138****5678-合同_脱敏.md"), "张*-138****5678-合同_脱敏");
});

test("parseContentDispositionFilename supports quoted and RFC5987 filenames", () => {
  assert.equal(
    parseContentDispositionFilename('attachment; filename="姓名1-PHONE2_脱敏.md"'),
    "姓名1-PHONE2_脱敏.md"
  );
  assert.equal(
    parseContentDispositionFilename("attachment; filename*=UTF-8''%E5%A7%93%E5%90%8D1_%E8%84%B1%E6%95%8F.md"),
    "姓名1_脱敏.md"
  );
});

test("artifactDownloadName prefers runtime-provided masked filename", () => {
  assert.equal(
    artifactDownloadName(
      "张三-13900000000.txt",
      'attachment; filename="姓名1-PHONE2_脱敏.md"'
    ),
    "姓名1-PHONE2_脱敏.md"
  );
  assert.equal(
    artifactDownloadName("张三-13900000000.txt", null),
    "张三-13900000000_脱敏.md"
  );
});

test("restoreDownloadName prefers runtime-provided filename and falls back safely", () => {
  assert.equal(
    restoreDownloadName(
      "姓名1-PHONE2.txt",
      'attachment; filename="姓名1-PHONE2_还原.md"'
    ),
    "姓名1-PHONE2_还原.md"
  );
  assert.equal(
    restoreDownloadName("姓名1-PHONE2.txt", null),
    "姓名1-PHONE2_还原.md"
  );
});
