import fs from "node:fs/promises";
import path from "node:path";
import os from "node:os";
import { spawnSync } from "node:child_process";

const rootDir = process.cwd();
const timestamp = new Date().toISOString().replace(/[:.]/g, "-");
const reportDir = path.join(rootDir, "test-results", `client-matrix-${timestamp}`);
const reportJsonPath = path.join(reportDir, "report.json");
const reportMdPath = path.join(reportDir, "report.md");
const env = {
  ...process.env,
  PATH: path.join(os.homedir(), ".cargo", "bin") + path.delimiter + process.env.PATH,
};

await fs.mkdir(reportDir, { recursive: true });

function runCommand(command, args, options = {}) {
  const startedAt = Date.now();
  const result = spawnSync(command, args, {
    cwd: rootDir,
    env,
    encoding: "utf8",
    stdio: "pipe",
    timeout: options.timeoutMs,
  });

  return {
    command,
    args,
    exitCode: result.status ?? 1,
    signal: result.signal ?? null,
    durationMs: Date.now() - startedAt,
    stdout: result.stdout ?? "",
    stderr: result.stderr ?? "",
    error: result.error ? String(result.error) : null,
  };
}

async function writeLogFile(name, execution) {
  const logPath = path.join(reportDir, `${name}.log`);
  const content = [
    `$ ${[execution.command, ...execution.args].join(" ")}`,
    "",
    execution.stdout.trimEnd(),
    execution.stderr ? `\n[stderr]\n${execution.stderr.trimEnd()}` : "",
    execution.error ? `\n[error]\n${execution.error}` : "",
  ]
    .filter(Boolean)
    .join("\n");
  await fs.writeFile(logPath, `${content}\n`, "utf8");
  return logPath;
}

function commandExists(command) {
  const result = spawnSync("bash", ["-lc", `command -v ${command}`], {
    cwd: rootDir,
    env,
    encoding: "utf8",
    stdio: "pipe",
  });
  return result.status === 0;
}

async function runStep(step) {
  if (step.requiredCommand && !commandExists(step.requiredCommand)) {
    return {
      name: step.name,
      status: "skipped",
      reason: `缺少命令 ${step.requiredCommand}`,
    };
  }

  const execution = runCommand(step.command, step.args, step.options);
  const logPath = await writeLogFile(step.name, execution);
  return {
    name: step.name,
    status: execution.exitCode === 0 ? "passed" : "failed",
    reason: execution.exitCode === 0 ? null : `退出码 ${execution.exitCode}`,
    command: [execution.command, ...execution.args].join(" "),
    durationMs: execution.durationMs,
    logPath,
  };
}

async function collectPreflightFacts() {
  const facts = {};
  for (const [name, command, args] of [
    ["node", "node", ["-v"]],
    ["pnpm", "corepack", ["pnpm", "-v"]],
    ["rustc", "rustc", ["-V"]],
    ["cargo", "cargo", ["-V"]],
    ["xcodeSelect", "xcode-select", ["-p"]],
    ["xcodebuild", "xcodebuild", ["-version"]],
  ]) {
    const result = runCommand(command, args);
    facts[name] = {
      available: result.exitCode === 0,
      output: `${result.stdout}${result.stderr}`.trim(),
    };
  }

  try {
    const response = await fetch(`${process.env.PLAYWRIGHT_BASE_URL ?? "http://127.0.0.1:5173"}/api/v1/health`);
    facts.runtimeHealth = {
      available: response.ok,
      output: JSON.stringify(await response.json()),
    };
  } catch (error) {
    facts.runtimeHealth = {
      available: false,
      output: String(error),
    };
  }

  return facts;
}

const steps = [
  {
    name: "preflight",
    command: "bash",
    args: ["./scripts/check-macos-release-env.sh"],
  },
  {
    name: "unit",
    command: "corepack",
    args: ["pnpm", "run", "test:unit"],
  },
  {
    name: "runtime-rust",
    command: "cargo",
    args: ["test", "--manifest-path", "apps/vault-runtime-api/Cargo.toml", "--lib", "--", "--test-threads=1"],
    requiredCommand: "cargo",
    options: { timeoutMs: 20 * 60 * 1000 },
  },
  {
    name: "desktop-rust",
    command: "cargo",
    args: ["test", "--manifest-path", "src-tauri/Cargo.toml", "--lib", "--", "--test-threads=1"],
    requiredCommand: "cargo",
    options: { timeoutMs: 20 * 60 * 1000 },
  },
  {
    name: "e2e",
    command: "corepack",
    args: ["pnpm", "run", "test:e2e"],
    options: { timeoutMs: 20 * 60 * 1000 },
  },
];

const facts = await collectPreflightFacts();
const results = [];

for (const step of steps) {
  results.push(await runStep(step));
}

const summary = {
  generatedAt: new Date().toISOString(),
  reportDir,
  facts,
  total: results.length,
  passed: results.filter((item) => item.status === "passed").length,
  failed: results.filter((item) => item.status === "failed").length,
  skipped: results.filter((item) => item.status === "skipped").length,
  results,
  note: "72 小时稳定性测试为持续运行项，请单独执行 pnpm test:stability。",
};

await fs.writeFile(reportJsonPath, JSON.stringify(summary, null, 2), "utf8");

const md = [
  "# 客户端测试矩阵报告",
  "",
  `- 生成时间: ${summary.generatedAt}`,
  `- 报告目录: \`${reportDir}\``,
  `- 通过: ${summary.passed}`,
  `- 失败: ${summary.failed}`,
  `- 跳过: ${summary.skipped}`,
  "",
  "## 环境事实",
  ...Object.entries(facts).map(([name, fact]) => `- ${name}: ${fact.available ? "可用" : "不可用"}${fact.output ? ` | ${fact.output}` : ""}`),
  "",
  "## 执行结果",
  ...results.map((item) => {
    const base = `- ${item.name}: ${item.status}`;
    if (item.status === "skipped") {
      return `${base} | ${item.reason}`;
    }
    return `${base} | ${item.reason ?? "通过"} | ${item.durationMs} ms | 日志: \`${item.logPath}\``;
  }),
  "",
  "## 说明",
  "- 当前报告覆盖预检、逻辑单测、Rust 测试和 Playwright E2E。",
  "- 72 小时稳定性测试需另行启动并等待完整跑完后补最终结论。",
  "",
].join("\n");

await fs.writeFile(reportMdPath, md, "utf8");

console.log(`测试矩阵已完成，JSON: ${reportJsonPath}`);
console.log(`Markdown: ${reportMdPath}`);
console.log(`通过 ${summary.passed} / 失败 ${summary.failed} / 跳过 ${summary.skipped}`);

if (summary.failed > 0) {
  process.exit(1);
}
