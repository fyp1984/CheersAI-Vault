import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

// 沙箱安全日志定向测试：只读 SandboxManagerDesktop.tsx 源文件文本，
// 断言不再向控制台输出口令值、是否记住口令、口令长度三类调试日志。
const here = dirname(fileURLToPath(import.meta.url));
const sourcePath = resolve(here, "SandboxManagerDesktop.tsx");
const source = readFileSync(sourcePath, "utf8");

describe("SandboxManagerDesktop 安全日志", () => {
  it("不输出 Passphrase loaded（口令值）调试日志", () => {
    assert.ok(
      !source.includes('"Passphrase loaded:"'),
      "SandboxManagerDesktop.tsx 不得输出口令值调试日志 Passphrase loaded"
    );
  });

  it("不输出 Remember passphrase（是否记住口令）调试日志", () => {
    assert.ok(
      !source.includes('"Remember passphrase:"'),
      "SandboxManagerDesktop.tsx 不得输出是否记住口令调试日志 Remember passphrase"
    );
  });

  it("不输出 Passphrase length（口令长度）调试日志", () => {
    assert.ok(
      !source.includes('"Passphrase length:"'),
      "SandboxManagerDesktop.tsx 不得输出口令长度调试日志 Passphrase length"
    );
  });

  it("不再把 file-store 持久化对象（含 passphrase）解析后传给 console", () => {
    assert.ok(
      !source.includes('"LocalStorage file-store:"'),
      "SandboxManagerDesktop.tsx 不得把 file-store 持久化对象（含 passphrase）解析后输出到控制台"
    );
    assert.ok(
      !/console\.(log|info|debug|warn|error)\([^)]*JSON\.parse[^)]*stored/.test(source),
      "SandboxManagerDesktop.tsx 不得把 file-store 解析对象作为参数传给 console"
    );
  });
});
