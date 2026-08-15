// Copyright 2026 CheersAI. Licensed under Apache-2.0.
import { describe, it, expect } from "vitest";
import {
  safeStem,
  parseContentDispositionFilename,
  artifactDownloadName,
  restoreDownloadName,
} from "@/lib/runtime/downloadName";

describe("downloadName util (vitest spec port)", () => {
  it("safeStem removes path separators, control chars, and extension", () => {
    expect(safeStem("../张三-13900000000.txt")).toBe("张三-13900000000");
    expect(safeStem("demo\0name.md")).toBe("demoname");
    expect(safeStem("张*-138****5678-合同_脱敏.md")).toBe("张*-138****5678-合同_脱敏");
  });

  it("parseContentDispositionFilename supports quoted and RFC5987 filenames", () => {
    expect(
      parseContentDispositionFilename('attachment; filename="姓名1-PHONE2_脱敏.md"'),
    ).toBe("姓名1-PHONE2_脱敏.md");
    expect(
      parseContentDispositionFilename(
        "attachment; filename*=UTF-8''%E5%A7%93%E5%90%8D1_%E8%84%B1%E6%95%8F.md",
      ),
    ).toBe("姓名1_脱敏.md");
  });

  it("artifactDownloadName prefers runtime-provided masked filename", () => {
    expect(
      artifactDownloadName(
        "张三-13900000000.txt",
        'attachment; filename="姓名1-PHONE2_脱敏.md"',
      ),
    ).toBe("姓名1-PHONE2_脱敏.md");
    expect(artifactDownloadName("张三-13900000000.txt", null)).toBe("张三-13900000000_脱敏.md");
  });

  it("restoreDownloadName prefers runtime-provided filename and falls back safely", () => {
    expect(
      restoreDownloadName("姓名1-PHONE2.txt", 'attachment; filename="姓名1-PHONE2_还原.md"'),
    ).toBe("姓名1-PHONE2_还原.md");
    expect(restoreDownloadName("姓名1-PHONE2.txt", null)).toBe("姓名1-PHONE2_还原.md");
  });
});
