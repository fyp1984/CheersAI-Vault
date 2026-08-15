import fs from "node:fs/promises";
import path from "node:path";
import { spawn } from "node:child_process";

const workspaceRoot = process.cwd();
const timestamp = new Date().toISOString().replace(/[:.]/g, "-");
const reportDir = path.join(workspaceRoot, "test-results", `sim-lab-validation-${timestamp}`);
const reportJsonPath = path.join(reportDir, "report.json");
const reportMdPath = path.join(reportDir, "report.md");

await fs.mkdir(reportDir, { recursive: true });

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

async function fetchJson(url, init) {
  const startedAt = Date.now();
  const response = await fetch(url, init);
  const contentType = response.headers.get("content-type") ?? "";
  const payload = contentType.includes("application/json") ? await response.json() : await response.text();
  return {
    ok: response.ok,
    status: response.status,
    payload,
    durationMs: Date.now() - startedAt,
    headers: Object.fromEntries(response.headers.entries()),
  };
}

async function waitForConsole(baseUrl) {
  for (let attempt = 0; attempt < 60; attempt += 1) {
    try {
      const state = await fetchJson(`${baseUrl}/api/console/state`);
      if (state.ok) return state.payload;
    } catch (_error) {
      // ignore and retry
    }
    await sleep(1000);
  }
  throw new Error("Sim Lab 控制台在 60 秒内未就绪。");
}

function collectChildOutput(stream, filePath) {
  stream.on("data", async (chunk) => {
    await fs.appendFile(filePath, chunk);
  });
}

const processLogPath = path.join(reportDir, "sim-lab-process.log");
const child = spawn("node", ["scripts/sim-lab/index.mjs", "--client", "none"], {
  cwd: workspaceRoot,
  stdio: ["ignore", "pipe", "pipe"],
});

collectChildOutput(child.stdout, processLogPath);
collectChildOutput(child.stderr, processLogPath);

const dashboardBase = "http://127.0.0.1:9090";
const proxyBase = "http://127.0.0.1:9091";
const mockBase = "http://127.0.0.1:9092";
const results = [];

async function runStep(name, fn) {
  const startedAt = Date.now();
  try {
    const detail = await fn();
    results.push({
      name,
      status: "passed",
      durationMs: Date.now() - startedAt,
      detail,
    });
  } catch (error) {
    results.push({
      name,
      status: "failed",
      durationMs: Date.now() - startedAt,
      detail: String(error),
    });
    throw error;
  }
}

try {
  const consoleState = await waitForConsole(dashboardBase);

  await runStep("console-health", async () => {
    assert(consoleState.ports.dashboard === 9090, "控制台端口不正确。");
    return consoleState;
  });

  await runStep("mock-health", async () => {
    const response = await fetchJson(`${mockBase}/__sim/health`);
    assert(response.ok, "Mock 服务健康检查失败。");
    return response;
  });

  await runStep("proxy-normal-flow", async () => {
    await fetchJson(`${dashboardBase}/api/console/reset`, { method: "POST" });
    const rules = await fetchJson(`${proxyBase}/api/v1/rules`);
    assert(rules.ok, "代理层 rules 请求失败。");
    assert(Array.isArray(rules.payload.rules), "rules 响应结构不正确。");

    const form = new FormData();
    form.append(
      "files",
      new Blob(["客户姓名：张三\n联系电话：13900000000\n电子邮箱：zhangsan@example.com\n"], {
        type: "text/plain",
      }),
      "preview-case.txt"
    );
    form.append("rule_ids", JSON.stringify(["chinese_name", "phone", "email"]));
    const preview = await fetchJson(`${proxyBase}/api/v1/previews`, {
      method: "POST",
      body: form,
    });
    assert(preview.status === 202, "预览创建未返回 202。");
    const previewId = preview.payload.preview_id;
    const previewDetail = await fetchJson(`${proxyBase}/api/v1/previews/${previewId}`);
    assert(previewDetail.ok, "预览详情读取失败。");
    const firstFileId = previewDetail.payload.files[0].file_id;
    const previewContent = await fetch(`${proxyBase}/api/v1/previews/${previewId}/files/${firstFileId}/content`);
    assert(previewContent.ok, "预览正文读取失败。");
    const confirm = await fetchJson(`${proxyBase}/api/v1/previews/${previewId}/confirm`, { method: "POST" });
    assert(confirm.ok, "预览确认失败。");
    const batch = await fetchJson(`${proxyBase}/api/v1/batches/${confirm.payload.batch_id}`);
    assert(batch.ok, "正式批次详情读取失败。");
    const artifactId = batch.payload.files.find((file) => file.artifact_id)?.artifact_id;
    assert(artifactId, "正式批次未生成 artifact。");
    const restoreResponse = await fetch(`${proxyBase}/api/v1/artifacts/${artifactId}/restore`, { method: "POST" });
    assert(restoreResponse.ok, "反脱敏下载失败。");
    assert(Number(restoreResponse.headers.get("x-restored-entity-count")) > 0, "恢复数量头不正确。");
    return {
      previewId,
      batchId: confirm.payload.batch_id,
      artifactId,
    };
  });

  await runStep("forward-strategy", async () => {
    await fetchJson(`${dashboardBase}/api/console/routes/health`, {
      method: "PUT",
      body: JSON.stringify({
        strategy: "forward",
        scenarioId: "normal",
        forwardTarget: mockBase,
        enabled: true,
      }),
    });
    const response = await fetchJson(`${proxyBase}/api/v1/health`);
    assert(response.ok, "forward 策略未能成功转发。");
    return response;
  });

  await runStep("server-error-scenario", async () => {
    await fetchJson(`${dashboardBase}/api/console/routes/previews`, {
      method: "PUT",
      body: JSON.stringify({
        strategy: "mock",
        scenarioId: "server_error",
        enabled: true,
      }),
    });
    const form = new FormData();
    form.append("files", new Blob(["x"], { type: "text/plain" }), "error-case.txt");
    form.append("rule_ids", JSON.stringify(["phone"]));
    const response = await fetchJson(`${proxyBase}/api/v1/previews`, {
      method: "POST",
      body: form,
    });
    assert(response.status === 503, "服务报错场景未返回 503。");
    return response;
  });

  await runStep("rate-limit-scenario", async () => {
    await fetchJson(`${dashboardBase}/api/console/routes/sensitive-terms`, {
      method: "PUT",
      body: JSON.stringify({
        strategy: "mock",
        scenarioId: "rate_limit",
        enabled: true,
      }),
    });
    const response = await fetchJson(`${proxyBase}/api/v1/sensitive-terms`);
    assert(response.status === 429, "限流场景未返回 429。");
    assert(response.headers["retry-after"], "限流场景未携带 retry-after。");
    return response;
  });

  await runStep("weak-network-scenario", async () => {
    await fetchJson(`${dashboardBase}/api/console/routes/health`, {
      method: "PUT",
      body: JSON.stringify({
        strategy: "mock",
        scenarioId: "weak_network",
        enabled: true,
      }),
    });
    const response = await fetchJson(`${proxyBase}/api/v1/health`);
    assert(response.ok, "弱网场景下 health 不应失败。");
    assert(response.durationMs >= 100, "弱网场景未体现额外延迟。");
    return response;
  });

  await runStep("proxy-overhead", async () => {
    await fetchJson(`${dashboardBase}/api/console/clear-logs`, { method: "POST" });
    await fetchJson(`${dashboardBase}/api/console/routes/health`, {
      method: "PUT",
      body: JSON.stringify({
        strategy: "forward",
        scenarioId: "normal",
        forwardTarget: mockBase,
        enabled: true,
      }),
    });
    for (let index = 0; index < 20; index += 1) {
      const response = await fetchJson(`${proxyBase}/api/v1/health`);
      assert(response.ok, "健康检查压测中出现失败。");
    }
    const state = await fetchJson(`${dashboardBase}/api/console/state`);
    assert(state.ok, "读取控制台状态失败。");
    assert(state.payload.metrics.p95ProxyOverheadMs <= 10, "代理附加时延超过 10ms。");
    return state.payload.metrics;
  });
} finally {
  child.kill("SIGTERM");
}

const summary = {
  generatedAt: new Date().toISOString(),
  dashboardBase,
  proxyBase,
  mockBase,
  total: results.length,
  passed: results.filter((item) => item.status === "passed").length,
  failed: results.filter((item) => item.status === "failed").length,
  processLogPath,
  results,
  note: "72 小时稳定性需执行长跑命令持续观测，本次脚本完成的是交付前快速验收。",
};

await fs.writeFile(reportJsonPath, JSON.stringify(summary, null, 2), "utf8");

const markdown = [
  "# Sim Lab 验证报告",
  "",
  `- 生成时间: ${summary.generatedAt}`,
  `- 控制台: ${dashboardBase}`,
  `- 代理: ${proxyBase}`,
  `- Mock: ${mockBase}`,
  `- 通过: ${summary.passed}`,
  `- 失败: ${summary.failed}`,
  `- 进程日志: \`${processLogPath}\``,
  "",
  "## 步骤结果",
  ...results.map(
    (item) =>
      `- ${item.name}: ${item.status} | ${item.durationMs} ms | ${typeof item.detail === "string" ? item.detail : JSON.stringify(item.detail)}`
  ),
  "",
  "## 说明",
  "- 本报告已覆盖控制台、代理转发、Mock 服务、异常场景注入与代理附加时延。",
  "- 72 小时长稳需结合持续运行脚本另行执行。",
  "",
].join("\n");

await fs.writeFile(reportMdPath, markdown, "utf8");

console.log(`Sim Lab 验证完成，JSON: ${reportJsonPath}`);
console.log(`Markdown: ${reportMdPath}`);
console.log(`通过 ${summary.passed} / 失败 ${summary.failed}`);

if (summary.failed > 0) {
  process.exit(1);
}
