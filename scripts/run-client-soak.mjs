import fs from "node:fs/promises";
import path from "node:path";

function parseArg(name, defaultValue) {
  const index = process.argv.indexOf(name);
  if (index === -1 || index === process.argv.length - 1) {
    return defaultValue;
  }
  return process.argv[index + 1];
}

const durationHours = Number(parseArg("--duration-hours", "72"));
const intervalSeconds = Number(parseArg("--interval-seconds", "60"));
const batchEvery = Number(parseArg("--batch-every", "30"));
const baseUrl = parseArg("--base-url", process.env.PLAYWRIGHT_BASE_URL ?? "http://127.0.0.1:5173");
const outputDir = path.resolve(parseArg("--output-dir", "./test-results/soak"));

await fs.mkdir(outputDir, { recursive: true });

const startedAt = Date.now();
const endAt = startedAt + durationHours * 60 * 60 * 1000;
const samples = [];

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function timedFetch(url, options) {
  const begin = Date.now();
  const response = await fetch(url, options);
  return {
    response,
    durationMs: Date.now() - begin,
  };
}

async function createBatchSample(cycle) {
  const form = new FormData();
  form.append(
    "files",
    new Blob([`Cycle ${cycle}\nPhone: 13900000000\nEmail: demo@example.com\n`], { type: "text/plain" }),
    `soak-${cycle}.txt`
  );
  form.append("rule_ids", JSON.stringify(["phone", "email"]));

  const { response, durationMs } = await timedFetch(`${baseUrl}/api/v1/batches`, {
    method: "POST",
    body: form,
  });

  const body = await response.text();
  return {
    ok: response.ok,
    durationMs,
    status: response.status,
    body,
  };
}

let cycle = 0;
while (Date.now() < endAt) {
  cycle += 1;
  const sample = {
    cycle,
    startedAt: new Date().toISOString(),
    checks: [],
  };

  for (const endpoint of ["/api/v1/health", "/api/v1/rules", "/api/v1/batches", "/api/v1/ocr/status"]) {
    try {
      const { response, durationMs } = await timedFetch(`${baseUrl}${endpoint}`);
      sample.checks.push({
        endpoint,
        ok: response.ok,
        status: response.status,
        durationMs,
      });
    } catch (error) {
      sample.checks.push({
        endpoint,
        ok: false,
        status: 0,
        durationMs: 0,
        error: String(error),
      });
    }
  }

  if (batchEvery > 0 && cycle % batchEvery === 0) {
    try {
      sample.batchProbe = await createBatchSample(cycle);
    } catch (error) {
      sample.batchProbe = {
        ok: false,
        durationMs: 0,
        status: 0,
        error: String(error),
      };
    }
  }

  samples.push(sample);
  await fs.writeFile(
    path.join(outputDir, "soak-latest.json"),
    JSON.stringify(
      {
        durationHours,
        intervalSeconds,
        baseUrl,
        startedAt: new Date(startedAt).toISOString(),
        updatedAt: new Date().toISOString(),
        cyclesCompleted: cycle,
        samples,
      },
      null,
      2
    )
  );

  if (Date.now() + intervalSeconds * 1000 >= endAt) {
    break;
  }
  await sleep(intervalSeconds * 1000);
}

const allChecks = samples.flatMap((item) => item.checks);
const failedChecks = allChecks.filter((item) => !item.ok);
const durations = allChecks.map((item) => item.durationMs).sort((a, b) => a - b);
const p95 = durations.length === 0 ? 0 : durations[Math.min(durations.length - 1, Math.floor(durations.length * 0.95))];

const summary = {
  baseUrl,
  durationHours,
  intervalSeconds,
  startedAt: new Date(startedAt).toISOString(),
  finishedAt: new Date().toISOString(),
  cyclesCompleted: cycle,
  totalChecks: allChecks.length,
  failedChecks: failedChecks.length,
  p95DurationMs: p95,
  batchProbeRuns: samples.filter((item) => item.batchProbe).length,
  note: "该脚本用于当前终端环境的候选稳定性长跑，不等价于已完成的 72 小时正式验收。",
};

await fs.writeFile(path.join(outputDir, "soak-summary.json"), JSON.stringify(summary, null, 2), "utf8");

console.log(JSON.stringify(summary, null, 2));

if (failedChecks.length > 0) {
  process.exit(1);
}
