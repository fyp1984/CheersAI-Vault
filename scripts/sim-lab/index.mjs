import http from "node:http";
import net from "node:net";
import os from "node:os";
import tls from "node:tls";
import path from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

import {
  buildMaskedMarkdown,
  buildRestoredMarkdown,
  createDefaultMockState,
  createId,
  deepClone,
  ensureDir,
  jsonBody,
  markdownDownloadBody,
  matchRoute,
  normalizeRoute,
  nowIso,
  paginate,
  parseMultipart,
  readJsonIfExists,
  readRequestBody,
  safeJsonParse,
  scenarioCatalog,
  scenarioOptions,
  sleep,
  textBody,
  writeJson,
} from "./shared.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const workspaceRoot = path.resolve(__dirname, "..", "..");
const exampleConfigPath = path.join(__dirname, "team-config.example.json");
const simDir = path.join(workspaceRoot, ".local", "sim-lab");
const configPath = path.join(simDir, "config.json");
const statePath = path.join(simDir, "state.json");

function parseArg(name, fallback) {
  const index = process.argv.indexOf(name);
  if (index === -1 || index === process.argv.length - 1) {
    return fallback;
  }
  return process.argv[index + 1];
}

const clientMode = parseArg("--client", "browser-dev");
const autoBuild = parseArg("--build-preview", "false") === "true";

await ensureDir(simDir);

const exampleConfig = await readJsonIfExists(exampleConfigPath, {});
const localConfig = await readJsonIfExists(configPath, null);
if (!localConfig) {
  await writeJson(configPath, exampleConfig);
}
const runtimeConfig = localConfig ?? exampleConfig;

const normalizedRoutes = runtimeConfig.routes.map((route) =>
  normalizeRoute(route, runtimeConfig.ports)
);

const persistedState = await readJsonIfExists(statePath, null);

function createInitialState() {
  return {
    meta: {
      startedAt: nowIso(),
      workspaceRoot,
      simDir,
      configPath,
      statePath,
      clientMode,
      controlVersion: "sim-lab-v1",
    },
    settings: {
      globalScenarioId: runtimeConfig.globalScenarioId ?? "normal",
      logLimit: runtimeConfig.logLimit ?? 400,
      routes: deepClone(normalizedRoutes),
    },
    requestLogs: [],
    mock: createDefaultMockState(),
  };
}

const state = persistedState
  ? {
      ...createInitialState(),
      ...persistedState,
      meta: {
        ...createInitialState().meta,
        ...(persistedState.meta ?? {}),
        startedAt: nowIso(),
        clientMode,
      },
      settings: {
        ...createInitialState().settings,
        ...(persistedState.settings ?? {}),
        routes: normalizedRoutes.map((route) => {
          const saved = (persistedState.settings?.routes ?? []).find((item) => item.id === route.id);
          return saved ? { ...route, ...saved } : route;
        }),
      },
      mock: {
        ...createDefaultMockState(),
        ...(persistedState.mock ?? {}),
      },
    }
  : createInitialState();

let saveTimer = null;
function schedulePersist() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    void writeJson(statePath, state);
  }, 120);
}

function pushLog(entry) {
  state.requestLogs.unshift({
    id: createId(),
    timestamp: nowIso(),
    ...entry,
  });
  state.requestLogs = state.requestLogs.slice(0, state.settings.logLimit);
  schedulePersist();
}

function pushOperationEvent(event) {
  state.mock.operationEvents.unshift({
    event_id: createId(),
    timestamp: nowIso(),
    ...event,
  });
  state.mock.operationEvents = state.mock.operationEvents.slice(0, 600);
  schedulePersist();
}

function resetEnvironment() {
  state.settings.globalScenarioId = runtimeConfig.globalScenarioId ?? "normal";
  state.settings.routes = deepClone(normalizedRoutes);
  state.requestLogs = [];
  state.mock = createDefaultMockState();
  state.mock.mockStats.resets += 1;
  schedulePersist();
}

function clearLogs() {
  state.requestLogs = [];
  schedulePersist();
}

function clearMockData() {
  state.mock = createDefaultMockState();
  schedulePersist();
}

function summarizeLogs() {
  const total = state.requestLogs.length;
  const durations = state.requestLogs
    .filter((item) => typeof item.durationMs === "number")
    .map((item) => item.durationMs)
    .sort((left, right) => left - right);
  const errorCount = state.requestLogs.filter((item) => (item.status ?? 200) >= 400).length;
  const avgDuration =
    durations.length === 0
      ? 0
      : Number((durations.reduce((sum, value) => sum + value, 0) / durations.length).toFixed(2));
  const p95Duration =
    durations.length === 0
      ? 0
      : durations[Math.min(durations.length - 1, Math.floor(durations.length * 0.95))];
  const proxyOverheads = state.requestLogs
    .map((item) => item.proxyAddedLatencyMs)
    .filter((value) => typeof value === "number")
    .sort((left, right) => left - right);
  const p95ProxyOverhead =
    proxyOverheads.length === 0
      ? 0
      : proxyOverheads[Math.min(proxyOverheads.length - 1, Math.floor(proxyOverheads.length * 0.95))];

  return {
    totalRequests: total,
    errorCount,
    avgDurationMs: avgDuration,
    p95DurationMs: p95Duration,
    p95ProxyOverheadMs: p95ProxyOverhead,
  };
}

function resolveScenario(route) {
  if (route.scenarioId && route.scenarioId !== "normal") {
    return scenarioCatalog[route.scenarioId] ?? scenarioCatalog.normal;
  }
  return scenarioCatalog[state.settings.globalScenarioId] ?? scenarioCatalog.normal;
}

function selectProtocolForHttpRequest(req, pathname) {
  if ((req.headers["content-type"] ?? "").startsWith("application/grpc")) {
    return "grpc";
  }
  if (pathname.startsWith("/grpc")) {
    return "grpc";
  }
  return "http";
}

function buildScenarioHeaders(route, scenario, requestId) {
  return {
    "x-sim-request-id": requestId,
    "x-sim-route-id": route.id,
    "x-sim-scenario": scenario.id,
  };
}

function buildErrorResponseForScenario(route, scenario, requestId) {
  const headers = buildScenarioHeaders(route, scenario, requestId);
  if (scenario.errorStatus) {
    const response = jsonBody(
      scenario.errorStatus,
      {
        code: scenario.errorCode,
        message: scenario.errorMessage,
        retryable: scenario.retryable ?? false,
      },
      scenario.retryAfterSeconds
        ? { ...headers, "retry-after": String(scenario.retryAfterSeconds) }
        : headers
    );
    return response;
  }
  return null;
}

function applyCors(headers) {
  return {
    "access-control-allow-origin": "*",
    "access-control-allow-methods": "GET,POST,PUT,DELETE,OPTIONS,PATCH",
    "access-control-allow-headers": "content-type, authorization, accept, x-sim-request-id, x-sim-route-id, x-sim-scenario",
    ...headers,
  };
}

function writeNodeResponse(res, response) {
  res.writeHead(response.status, applyCors(response.headers));
  res.end(response.body);
}

function notFoundResponse(message = "模拟环境中未匹配到请求资源。") {
  return jsonBody(404, {
    code: "SIM_NOT_FOUND",
    message,
    retryable: false,
  });
}

function buildPreviewFiles(files, ruleIds) {
  return files.map((file) => {
    const failed = file.name.toLowerCase().includes("fail");
    return {
      file_id: createId(),
      display_name: file.name,
      input_format: path.extname(file.name).replace(/^\./, "") || "text",
      status: failed ? "Failed" : "Ready",
      masked_entity_count: failed ? null : 3,
      error_code: failed ? "SIMULATED_FAILURE" : null,
      error_message: failed ? "文件名命中 fail，模拟单文件失败。" : null,
      content_available: !failed,
      content: failed ? null : buildMaskedMarkdown(file.name, ruleIds),
    };
  });
}

function createPreviewSession(files, ruleIds) {
  const previewId = createId();
  const createdAt = nowIso();
  const previewFiles = buildPreviewFiles(files, ruleIds);
  const readyCount = previewFiles.filter((file) => file.status === "Ready").length;
  const failedCount = previewFiles.filter((file) => file.status === "Failed").length;
  const status = failedCount > 0 ? (readyCount > 0 ? "ReadyWithErrors" : "Failed") : "Ready";
  const preview = {
    preview_id: previewId,
    status,
    file_count: previewFiles.length,
    ready_count: readyCount,
    failed_count: failedCount,
    masked_entity_count: readyCount * 3,
    created_at: createdAt,
    expires_at: new Date(Date.now() + 30 * 60 * 1000).toISOString(),
    files: previewFiles.map((file) => ({
      file_id: file.file_id,
      display_name: file.display_name,
      input_format: file.input_format,
      status: file.status,
      masked_entity_count: file.masked_entity_count,
      error_code: file.error_code,
      error_message: file.error_message,
      content_available: file.content_available,
    })),
    fileContents: Object.fromEntries(
      previewFiles.filter((file) => file.content).map((file) => [file.file_id, file.content])
    ),
    ruleIds,
  };
  state.mock.previews[previewId] = preview;
  schedulePersist();
  return preview;
}

function createBatchFromFiles(files, ruleIds, origin) {
  const batchId = createId();
  const createdAt = nowIso();
  const batchFiles = files.map((file) => {
    const artifactId = createId();
    const maskedCount = file.status === "Failed" ? null : 3;
    state.mock.artifacts[artifactId] = {
      artifact_id: artifactId,
      display_name: file.display_name,
      masked_content: file.content ?? buildMaskedMarkdown(file.display_name, ruleIds),
      restored_content: buildRestoredMarkdown(file.display_name),
      restored_entity_count: 3,
      created_at: createdAt,
    };

    const batchFile = {
      file_id: file.file_id,
      display_name: file.display_name,
      input_format: file.input_format,
      status: file.status === "Failed" ? "Failed" : "Completed",
      attempt: 1,
      masked_entity_count: maskedCount,
      artifact_id: file.status === "Failed" ? null : artifactId,
      error_code: file.error_code,
      error_message: file.error_message,
      restore_available: file.status !== "Failed",
    };

    pushOperationEvent({
      event_type: origin === "preview-confirm" ? "preview_confirmed" : "batch_created",
      level: batchFile.status === "Completed" ? "success" : "error",
      batch_id: batchId,
      file_id: batchFile.file_id,
      display_name: batchFile.display_name,
      input_format: batchFile.input_format,
      status: batchFile.status,
      masked_entity_count: batchFile.masked_entity_count,
      error_code: batchFile.error_code,
      restored_entity_count: null,
    });

    return batchFile;
  });

  const completedCount = batchFiles.filter((file) => file.status === "Completed").length;
  const failedCount = batchFiles.filter((file) => file.status === "Failed").length;
  const batch = {
    batch_id: batchId,
    status: failedCount > 0 ? (completedCount > 0 ? "CompletedWithErrors" : "Failed") : "Completed",
    file_count: batchFiles.length,
    completed_count: completedCount,
    failed_count: failedCount,
    masked_entity_count: batchFiles.reduce((sum, file) => sum + (file.masked_entity_count ?? 0), 0),
    created_at: createdAt,
    updated_at: createdAt,
  };

  state.mock.batches[batchId] = {
    batch,
    files: batchFiles,
  };
  schedulePersist();
  return state.mock.batches[batchId];
}

function listCompletedBatchCandidates(batchId) {
  const batch = state.mock.batches[batchId];
  if (!batch) return [];
  return batch.files
    .filter((file) => file.status === "Completed" && file.artifact_id)
    .map((file) => ({
      artifact_id: file.artifact_id,
      display_name: file.display_name,
      remote_path: `${batchId}/${file.display_name}.masked.md`,
    }));
}

function getSandboxStatusResponse() {
  const sandbox = state.mock.sandbox;
  const rateLimitedUntil = sandbox.rate_limited_until ? new Date(sandbox.rate_limited_until).getTime() : 0;
  const now = Date.now();
  const rateLimited = rateLimitedUntil > now;
  return {
    pin_configured: sandbox.pin_configured,
    locked: sandbox.locked,
    storage_mode: "server_system_user",
    rate_limited: rateLimited,
    retry_after_seconds: rateLimited ? Math.ceil((rateLimitedUntil - now) / 1000) : null,
  };
}

async function handleMockRequest({ req, url, route, bodyBuffer, requestId }) {
  const startedAt = Date.now();
  const method = req.method ?? "GET";

  if (method === "OPTIONS") {
    return {
      response: textBody(204, "", { "x-sim-elapsed-ms": "0" }),
      durationMs: 0,
    };
  }

  if (method === "GET" && url.pathname === "/__sim/health") {
    return {
      response: jsonBody(200, { ok: true, service: "mock-runtime" }, { "x-sim-elapsed-ms": "0" }),
      durationMs: 0,
    };
  }

  let response = null;

  if (method === "GET" && url.pathname === "/api/v1/health") {
    response = jsonBody(200, {
      status: "ok",
      version: "sim-runtime-1.0.0",
    });
  } else if (method === "GET" && url.pathname === "/api/v1/rules") {
    response = jsonBody(200, {
      rules: state.mock.rules,
    });
  } else if (method === "GET" && url.pathname === "/api/v1/ocr/status") {
    response = jsonBody(200, {
      status: "ready",
      model_ready: true,
      timeout_secs: 30,
      max_pages: 10,
    });
  } else if (method === "POST" && url.pathname === "/api/v1/previews") {
    const { files, fields } = parseMultipart(bodyBuffer, req.headers["content-type"]);
    if (files.length === 0) {
      response = jsonBody(400, {
        code: "BAD_REQUEST",
        message: "未解析到上传文件。",
        retryable: false,
      });
    } else {
      const ruleIds = safeJsonParse(fields.rule_ids ?? "[]", []);
      const preview = createPreviewSession(files, ruleIds);
      response = jsonBody(
        202,
        {
          preview_id: preview.preview_id,
          files: preview.files.map((file) => ({
            file_id: file.file_id,
            display_name: file.display_name,
          })),
          expires_at: preview.expires_at,
        }
      );
    }
  } else if (method === "GET" && /^\/api\/v1\/previews\/[^/]+$/.test(url.pathname)) {
    const previewId = url.pathname.split("/").at(-1);
    const preview = state.mock.previews[previewId];
    response = preview
      ? jsonBody(200, {
          preview_id: preview.preview_id,
          status: preview.status,
          file_count: preview.file_count,
          ready_count: preview.ready_count,
          failed_count: preview.failed_count,
          masked_entity_count: preview.masked_entity_count,
          created_at: preview.created_at,
          expires_at: preview.expires_at,
          files: preview.files,
        })
      : notFoundResponse("预览会话不存在。");
  } else if (
    method === "GET" &&
    /^\/api\/v1\/previews\/[^/]+\/files\/[^/]+\/content$/.test(url.pathname)
  ) {
    const parts = url.pathname.split("/");
    const previewId = parts[4];
    const fileId = parts[6];
    const preview = state.mock.previews[previewId];
    const content = preview?.fileContents?.[fileId];
    response = content
      ? textBody(200, content, { "content-type": "text/markdown; charset=utf-8" })
      : notFoundResponse("预览内容不存在。");
  } else if (method === "POST" && /^\/api\/v1\/previews\/[^/]+\/confirm$/.test(url.pathname)) {
    const previewId = url.pathname.split("/")[4];
    const preview = state.mock.previews[previewId];
    if (!preview) {
      response = notFoundResponse("预览会话不存在。");
    } else {
      const batchState = createBatchFromFiles(
        preview.files.map((file) => ({
          ...file,
          content: preview.fileContents[file.file_id] ?? null,
        })),
        preview.ruleIds,
        "preview-confirm"
      );
      preview.status = "Confirmed";
      response = jsonBody(200, {
        preview_id: preview.preview_id,
        batch_id: batchState.batch.batch_id,
      });
    }
  } else if (method === "DELETE" && /^\/api\/v1\/previews\/[^/]+$/.test(url.pathname)) {
    const previewId = url.pathname.split("/").at(-1);
    delete state.mock.previews[previewId];
    response = textBody(204, "");
  } else if (method === "POST" && url.pathname === "/api/v1/batches") {
    const { files, fields } = parseMultipart(bodyBuffer, req.headers["content-type"]);
    if (files.length === 0) {
      response = jsonBody(400, {
        code: "BAD_REQUEST",
        message: "未解析到上传文件。",
        retryable: false,
      });
    } else {
      const ruleIds = safeJsonParse(fields.rule_ids ?? "[]", []);
      const batchState = createBatchFromFiles(buildPreviewFiles(files, ruleIds), ruleIds, "direct-batch");
      response = jsonBody(202, {
        batch_id: batchState.batch.batch_id,
        files: batchState.files.map((file) => ({
          file_id: file.file_id,
          display_name: file.display_name,
        })),
      });
    }
  } else if (method === "GET" && url.pathname === "/api/v1/batches") {
    const batches = Object.values(state.mock.batches)
      .map((entry) => entry.batch)
      .sort((left, right) => right.updated_at.localeCompare(left.updated_at));
    response = jsonBody(200, { batches });
  } else if (method === "GET" && /^\/api\/v1\/batches\/[^/]+$/.test(url.pathname)) {
    const batchId = url.pathname.split("/").at(-1);
    const batch = state.mock.batches[batchId];
    response = batch ? jsonBody(200, batch) : notFoundResponse("批次不存在。");
  } else if (method === "POST" && /^\/api\/v1\/files\/[^/]+\/retry$/.test(url.pathname)) {
    const fileId = url.pathname.split("/")[4];
    let target = null;
    for (const batch of Object.values(state.mock.batches)) {
      const file = batch.files.find((item) => item.file_id === fileId);
      if (file) {
        target = { batch, file };
        break;
      }
    }
    if (!target) {
      response = notFoundResponse("文件记录不存在。");
    } else {
      target.file.status = "Completed";
      target.file.error_code = null;
      target.file.error_message = null;
      target.file.attempt += 1;
      target.file.masked_entity_count = target.file.masked_entity_count ?? 3;
      target.file.restore_available = true;
      target.batch.batch.status = "Completed";
      target.batch.batch.completed_count = target.batch.files.filter((file) => file.status === "Completed").length;
      target.batch.batch.failed_count = target.batch.files.filter((file) => file.status === "Failed").length;
      target.batch.batch.updated_at = nowIso();
      pushOperationEvent({
        event_type: "file_retried",
        level: "success",
        batch_id: target.batch.batch.batch_id,
        file_id: target.file.file_id,
        display_name: target.file.display_name,
        input_format: target.file.input_format,
        status: target.file.status,
        masked_entity_count: target.file.masked_entity_count,
        error_code: null,
        restored_entity_count: null,
      });
      response = jsonBody(200, {
        file_id: target.file.file_id,
        status: target.file.status,
        attempt: target.file.attempt,
      });
    }
  } else if (method === "GET" && /^\/api\/v1\/artifacts\/[^/]+$/.test(url.pathname)) {
    const artifactId = url.pathname.split("/").at(-1);
    const artifact = state.mock.artifacts[artifactId];
    response = artifact
      ? markdownDownloadBody(`${artifact.display_name}.masked.md`, artifact.masked_content)
      : notFoundResponse("脱敏产物不存在。");
  } else if (method === "POST" && /^\/api\/v1\/artifacts\/[^/]+\/restore$/.test(url.pathname)) {
    const artifactId = url.pathname.split("/")[4];
    const artifact = state.mock.artifacts[artifactId];
    if (!artifact) {
      response = notFoundResponse("恢复产物不存在。");
    } else {
      pushOperationEvent({
        event_type: "artifact_restored",
        level: "success",
        batch_id: null,
        file_id: null,
        display_name: artifact.display_name,
        input_format: "markdown",
        status: "Completed",
        masked_entity_count: null,
        error_code: null,
        restored_entity_count: artifact.restored_entity_count,
      });
      response = markdownDownloadBody(
        `${artifact.display_name}.restored.md`,
        artifact.restored_content,
        {
          "x-restored-entity-count": String(artifact.restored_entity_count),
        }
      );
    }
  } else if (method === "GET" && url.pathname === "/api/v1/sensitive-terms") {
    const category = url.searchParams.get("category");
    const query = url.searchParams.get("query");
    const enabledOnly = url.searchParams.get("enabled_only") === "true";
    const terms = state.mock.sensitiveTerms.filter((term) => {
      if (category && term.category !== category) return false;
      if (enabledOnly && !term.enabled) return false;
      if (query && !term.term.includes(query) && !(term.description ?? "").includes(query)) return false;
      return true;
    });
    response = jsonBody(200, { terms });
  } else if (method === "GET" && url.pathname === "/api/v1/sensitive-terms/categories") {
    const categories = Array.from(new Set(state.mock.sensitiveTerms.map((term) => term.category))).sort();
    response = jsonBody(200, { categories });
  } else if (method === "GET" && url.pathname === "/api/v1/sensitive-terms/stats") {
    const total = state.mock.sensitiveTerms.length;
    const enabled = state.mock.sensitiveTerms.filter((term) => term.enabled).length;
    response = jsonBody(200, {
      total,
      enabled,
      disabled: total - enabled,
      categories: new Set(state.mock.sensitiveTerms.map((term) => term.category)).size,
    });
  } else if (method === "POST" && url.pathname === "/api/v1/sensitive-terms") {
    const payload = safeJsonParse(bodyBuffer.toString("utf8"), {});
    const item = {
      id: createId(),
      term: String(payload.term ?? "").trim(),
      category: String(payload.category ?? "未分类").trim(),
      description: payload.description ? String(payload.description) : null,
      enabled: true,
      created_at: nowIso(),
      updated_at: nowIso(),
    };
    state.mock.sensitiveTerms.unshift(item);
    schedulePersist();
    response = jsonBody(201, item);
  } else if (method === "PUT" && /^\/api\/v1\/sensitive-terms\/[^/]+$/.test(url.pathname)) {
    const id = url.pathname.split("/").at(-1);
    const payload = safeJsonParse(bodyBuffer.toString("utf8"), {});
    const item = state.mock.sensitiveTerms.find((term) => term.id === id);
    if (!item) {
      response = notFoundResponse("敏感词不存在。");
    } else {
      if (payload.term !== undefined) item.term = String(payload.term);
      if (payload.category !== undefined) item.category = String(payload.category);
      if (payload.description !== undefined) item.description = payload.description ? String(payload.description) : null;
      if (payload.enabled !== undefined) item.enabled = Boolean(payload.enabled);
      item.updated_at = nowIso();
      schedulePersist();
      response = jsonBody(200, item);
    }
  } else if (method === "DELETE" && /^\/api\/v1\/sensitive-terms\/[^/]+$/.test(url.pathname)) {
    const id = url.pathname.split("/").at(-1);
    const before = state.mock.sensitiveTerms.length;
    state.mock.sensitiveTerms = state.mock.sensitiveTerms.filter((term) => term.id !== id);
    response = before === state.mock.sensitiveTerms.length ? notFoundResponse("敏感词不存在。") : textBody(204, "");
    schedulePersist();
  } else if (method === "POST" && url.pathname === "/api/v1/sensitive-terms/import") {
    const { fields, files } = parseMultipart(bodyBuffer, req.headers["content-type"]);
    const csv = files[0]?.content ?? fields.file ?? "";
    const imported = csv
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean)
      .slice(1);
    for (const line of imported) {
      const [term, category, description] = line.split(",");
      state.mock.sensitiveTerms.unshift({
        id: createId(),
        term: term?.trim() ?? "未命名词条",
        category: category?.trim() ?? "导入分类",
        description: description?.trim() || null,
        enabled: true,
        created_at: nowIso(),
        updated_at: nowIso(),
      });
    }
    schedulePersist();
    response = jsonBody(200, { imported_count: imported.length });
  } else if (method === "GET" && url.pathname === "/api/v1/sensitive-terms/export") {
    const lines = ["term,category,description"];
    for (const item of state.mock.sensitiveTerms) {
      lines.push([item.term, item.category, item.description ?? ""].join(","));
    }
    response = {
      ...textBody(200, lines.join("\n"), {
        "content-type": "text/csv; charset=utf-8",
        "content-disposition": 'attachment; filename="sensitive_terms.csv"',
      }),
    };
  } else if (method === "GET" && url.pathname === "/api/v1/operation-logs") {
    const page = Number(url.searchParams.get("page") ?? "1");
    const pageSize = Number(url.searchParams.get("page_size") ?? "20");
    const level = url.searchParams.get("level");
    const status = url.searchParams.get("status");
    const batchId = url.searchParams.get("batch_id");
    const filtered = state.mock.operationEvents.filter((item) => {
      if (level && item.level !== level) return false;
      if (status && item.status !== status) return false;
      if (batchId && item.batch_id !== batchId) return false;
      return true;
    });
    response = jsonBody(200, paginate(filtered, page, pageSize));
  } else if (method === "GET" && url.pathname === "/api/v1/operation-logs/statistics") {
    const entries = state.mock.operationEvents;
    const totalFiles = entries.length;
    const successfulFiles = entries.filter((entry) => entry.level === "success").length;
    const totalMaskedItems = entries.reduce((sum, entry) => sum + (entry.masked_entity_count ?? 0), 0);
    response = jsonBody(200, {
      total_files: totalFiles,
      successful_files: successfulFiles,
      failed_files: totalFiles - successfulFiles,
      total_masked_items: totalMaskedItems,
      success_rate: totalFiles === 0 ? 100 : Number(((successfulFiles / totalFiles) * 100).toFixed(2)),
      recent_files_7days: totalFiles,
      average_processing_time_ms: 120,
    });
  } else if (method === "GET" && url.pathname === "/api/v1/operation-logs/storage-status") {
    response = jsonBody(200, {
      status: "ready",
      event_count: state.mock.operationEvents.length,
      runtime_version: "sim-runtime-1.0.0",
    });
  } else if (method === "DELETE" && url.pathname === "/api/v1/operation-logs") {
    const deletedJobEvents = state.mock.operationEvents.filter((item) => !item.event_type.includes("restore")).length;
    const deletedRestoreEvents = state.mock.operationEvents.length - deletedJobEvents;
    state.mock.operationEvents = [];
    schedulePersist();
    response = jsonBody(200, {
      deleted_job_events: deletedJobEvents,
      deleted_restore_events: deletedRestoreEvents,
    });
  } else if (method === "GET" && url.pathname === "/api/v1/sandbox/status") {
    response = jsonBody(200, getSandboxStatusResponse());
  } else if (method === "PUT" && url.pathname === "/api/v1/sandbox/pin") {
    const payload = safeJsonParse(bodyBuffer.toString("utf8"), {});
    const sandbox = state.mock.sandbox;
    if (sandbox.pin_configured && payload.current_pin !== sandbox.pin) {
      response = jsonBody(403, {
        code: "INVALID_CURRENT_PIN",
        message: "当前 PIN 不正确。",
        retryable: false,
      });
    } else {
      sandbox.pin = String(payload.new_pin ?? "");
      sandbox.pin_configured = true;
      sandbox.locked = false;
      sandbox.failedAttempts = 0;
      sandbox.rate_limited_until = null;
      schedulePersist();
      response = jsonBody(200, getSandboxStatusResponse());
    }
  } else if (method === "POST" && url.pathname === "/api/v1/sandbox/lock") {
    state.mock.sandbox.locked = true;
    schedulePersist();
    response = jsonBody(200, getSandboxStatusResponse());
  } else if (method === "POST" && url.pathname === "/api/v1/sandbox/unlock") {
    const sandbox = state.mock.sandbox;
    const status = getSandboxStatusResponse();
    if (status.rate_limited) {
      response = jsonBody(
        429,
        {
          code: "SANDBOX_RATE_LIMITED",
          message: "模拟环境下的 PIN 解锁已被限流。",
          retryable: true,
        },
        { "retry-after": String(status.retry_after_seconds ?? 30) }
      );
    } else {
      const payload = safeJsonParse(bodyBuffer.toString("utf8"), {});
      if (payload.pin !== sandbox.pin) {
        sandbox.failedAttempts += 1;
        if (sandbox.failedAttempts >= 3) {
          sandbox.rate_limited_until = new Date(Date.now() + 30_000).toISOString();
        }
        schedulePersist();
        response = jsonBody(403, {
          code: "INVALID_SANDBOX_PIN",
          message: "PIN 不正确。",
          retryable: false,
        });
      } else {
        sandbox.locked = false;
        sandbox.failedAttempts = 0;
        sandbox.rate_limited_until = null;
        schedulePersist();
        response = jsonBody(200, getSandboxStatusResponse());
      }
    }
  } else if (method === "DELETE" && url.pathname === "/api/v1/sandbox/pin") {
    const payload = safeJsonParse(bodyBuffer.toString("utf8"), {});
    const sandbox = state.mock.sandbox;
    if (payload.current_pin !== sandbox.pin) {
      response = jsonBody(403, {
        code: "INVALID_SANDBOX_PIN",
        message: "当前 PIN 不正确。",
        retryable: false,
      });
    } else {
      sandbox.pin = null;
      sandbox.pin_configured = false;
      sandbox.locked = false;
      sandbox.failedAttempts = 0;
      sandbox.rate_limited_until = null;
      schedulePersist();
      response = jsonBody(200, getSandboxStatusResponse());
    }
  } else if (method === "GET" && url.pathname === "/api/v1/filebay/status") {
    response = jsonBody(200, state.mock.filebay);
  } else if (method === "POST" && url.pathname === "/api/v1/filebay/test") {
    response = jsonBody(200, {
      repository_exists: state.mock.filebay.repository_exists,
    });
  } else if (method === "POST" && url.pathname === "/api/v1/filebay/repository") {
    state.mock.filebay.repository_exists = true;
    schedulePersist();
    response = jsonBody(200, {
      status: "ready",
    });
  } else if (method === "GET" && /^\/api\/v1\/filebay\/batches\/[^/]+\/candidates$/.test(url.pathname)) {
    const batchId = url.pathname.split("/")[5];
    response = jsonBody(200, {
      candidates: listCompletedBatchCandidates(batchId),
    });
  } else if (method === "POST" && url.pathname === "/api/v1/filebay/uploads") {
    const payload = safeJsonParse(bodyBuffer.toString("utf8"), {});
    const items = Array.isArray(payload.artifact_ids)
      ? payload.artifact_ids.map((artifactId) => {
          const artifact = state.mock.artifacts[artifactId];
          return {
            artifact_id: artifactId,
            remote_path: artifact ? `uploads/${artifact.display_name}.masked.md` : `uploads/${artifactId}.masked.md`,
            success: Boolean(artifact),
            url: artifact ? `https://mock-filebay.local/sim-team/vault-artifacts/raw/${artifactId}` : null,
            error_code: artifact ? null : "ARTIFACT_NOT_FOUND",
          };
        })
      : [];
    response = jsonBody(200, { items });
  } else {
    response = notFoundResponse();
  }

  const durationMs = Date.now() - startedAt;
  response.headers = {
    ...response.headers,
    ...buildScenarioHeaders(route, scenarioCatalog.normal, requestId),
    "x-sim-elapsed-ms": String(durationMs),
    "x-sim-upstream": "mock-runtime",
  };

  pushLog({
    stage: "mock",
    requestId,
    method,
    path: url.pathname,
    query: url.search,
    routeId: route.id,
    scenarioId: "normal",
    strategy: "mock-service",
    target: "mock-runtime",
    status: response.status,
    durationMs,
  });

  return {
    response,
    durationMs,
  };
}

function renderDashboardHtml() {
  const routesTableRows = state.settings.routes
    .map(
      (route) => `
        <tr>
          <td>${route.label}</td>
          <td><code>${route.pathPrefix}</code></td>
          <td>${route.protocol}</td>
          <td>
            <select data-route-id="${route.id}" data-field="strategy">
              <option value="mock" ${route.strategy === "mock" ? "selected" : ""}>mock</option>
              <option value="forward" ${route.strategy === "forward" ? "selected" : ""}>forward</option>
            </select>
          </td>
          <td>
            <select data-route-id="${route.id}" data-field="scenarioId">
              ${scenarioOptions
                .map(
                  (scenario) =>
                    `<option value="${scenario.id}" ${route.scenarioId === scenario.id ? "selected" : ""}>${scenario.label}</option>`
                )
                .join("")}
            </select>
          </td>
          <td><input data-route-id="${route.id}" data-field="forwardTarget" value="${route.forwardTarget}" /></td>
          <td><input data-route-id="${route.id}" data-field="enabled" type="checkbox" ${route.enabled ? "checked" : ""} /></td>
          <td><button data-save-route="${route.id}">保存</button></td>
        </tr>
      `
    )
    .join("");

  return `<!doctype html>
  <html lang="zh-CN">
    <head>
      <meta charset="utf-8" />
      <meta name="viewport" content="width=device-width, initial-scale=1" />
      <title>CheersAI Vault Sim Lab</title>
      <style>
        :root {
          color-scheme: dark;
          font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
          background: #0f172a;
          color: #e2e8f0;
        }
        body { margin: 0; padding: 24px; background: #0f172a; }
        h1, h2 { margin: 0 0 12px; }
        p, li { color: #cbd5e1; }
        .grid { display: grid; gap: 16px; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); margin-bottom: 16px; }
        .card { background: #111827; border: 1px solid #334155; border-radius: 14px; padding: 16px; box-shadow: 0 12px 30px rgba(15, 23, 42, 0.22); }
        .muted { color: #94a3b8; font-size: 12px; }
        .actions { display: flex; flex-wrap: wrap; gap: 12px; margin: 12px 0 20px; }
        button {
          border: 0; border-radius: 10px; padding: 10px 14px;
          background: #2563eb; color: white; cursor: pointer;
        }
        button.secondary { background: #334155; }
        table { width: 100%; border-collapse: collapse; }
        th, td { border-bottom: 1px solid #233047; padding: 10px 8px; text-align: left; vertical-align: top; }
        input, select {
          width: 100%; box-sizing: border-box; background: #020617; color: #e2e8f0;
          border: 1px solid #334155; border-radius: 8px; padding: 8px;
        }
        .log-table { max-height: 420px; overflow: auto; }
        code { color: #93c5fd; }
        .pill { display: inline-block; padding: 3px 8px; border-radius: 999px; background: #1e293b; font-size: 12px; }
      </style>
    </head>
    <body>
      <h1>CheersAI Vault Sim Lab</h1>
      <p>统一控制台：查看链路日志、切换 mock / forward、触发异常场景、重置环境与导出共享配置。</p>

      <div class="grid">
        <div class="card"><div class="muted">控制台</div><div id="dashboard-port"></div></div>
        <div class="card"><div class="muted">代理层</div><div id="proxy-port"></div></div>
        <div class="card"><div class="muted">Mock 服务层</div><div id="mock-port"></div></div>
        <div class="card"><div class="muted">客户端模式</div><div id="client-mode"></div></div>
        <div class="card"><div class="muted">总请求数</div><div id="total-requests"></div></div>
        <div class="card"><div class="muted">P95 转发附加时延</div><div id="proxy-p95"></div></div>
      </div>

      <div class="card">
        <h2>全局场景</h2>
        <div class="actions">
          ${scenarioOptions
            .map(
              (scenario) =>
                `<button class="${scenario.id === "normal" ? "" : "secondary"}" data-global-scenario="${scenario.id}">${scenario.label}</button>`
            )
            .join("")}
          <button class="secondary" id="reset-env">一键重置环境</button>
          <button class="secondary" id="clear-logs">清空请求日志</button>
          <button class="secondary" id="clear-data">清空模拟数据</button>
          <button class="secondary" id="export-config">导出共享配置</button>
        </div>
        <div class="muted">当前全局场景：<span id="global-scenario-name"></span></div>
      </div>

      <div class="card">
        <h2>路由策略</h2>
        <table>
          <thead>
            <tr>
              <th>模块</th>
              <th>路径前缀</th>
              <th>协议</th>
              <th>策略</th>
              <th>场景</th>
              <th>Forward 目标</th>
              <th>启用</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>${routesTableRows}</tbody>
        </table>
      </div>

      <div class="card">
        <h2>最近请求</h2>
        <div class="log-table">
          <table>
            <thead>
              <tr>
                <th>时间</th>
                <th>阶段</th>
                <th>方法</th>
                <th>路径</th>
                <th>策略</th>
                <th>场景</th>
                <th>状态码</th>
                <th>耗时</th>
                <th>附加时延</th>
              </tr>
            </thead>
            <tbody id="log-body"></tbody>
          </table>
        </div>
      </div>

      <script>
        async function request(path, options) {
          const response = await fetch(path, {
            ...options,
            headers: { "content-type": "application/json", ...(options?.headers || {}) },
          });
          if (!response.ok) {
            throw new Error(await response.text());
          }
          const type = response.headers.get("content-type") || "";
          return type.includes("application/json") ? response.json() : response.text();
        }

        async function refresh() {
          const payload = await request("/api/console/state");
          document.getElementById("dashboard-port").textContent = payload.ports.dashboard;
          document.getElementById("proxy-port").textContent = payload.ports.proxy;
          document.getElementById("mock-port").textContent = payload.ports.mock;
          document.getElementById("client-mode").textContent = payload.meta.clientMode;
          document.getElementById("total-requests").textContent = String(payload.metrics.totalRequests);
          document.getElementById("proxy-p95").textContent = payload.metrics.p95ProxyOverheadMs + " ms";
          document.getElementById("global-scenario-name").textContent = payload.globalScenario.label;

          const logBody = document.getElementById("log-body");
          logBody.innerHTML = payload.logs.map((item) => \`
            <tr>
              <td>\${new Date(item.timestamp).toLocaleTimeString()}</td>
              <td><span class="pill">\${item.stage}</span></td>
              <td>\${item.method}</td>
              <td><code>\${item.path}</code></td>
              <td>\${item.strategy}</td>
              <td>\${item.scenarioId}</td>
              <td>\${item.status}</td>
              <td>\${item.durationMs ?? 0} ms</td>
              <td>\${item.proxyAddedLatencyMs ?? "-"} </td>
            </tr>
          \`).join("");
        }

        document.getElementById("reset-env").onclick = async () => { await request("/api/console/reset", { method: "POST" }); await refresh(); };
        document.getElementById("clear-logs").onclick = async () => { await request("/api/console/clear-logs", { method: "POST" }); await refresh(); };
        document.getElementById("clear-data").onclick = async () => { await request("/api/console/clear-data", { method: "POST" }); await refresh(); };
        document.getElementById("export-config").onclick = async () => {
          const response = await fetch("/api/console/export-config");
          const blob = await response.blob();
          const objectUrl = URL.createObjectURL(blob);
          const anchor = document.createElement("a");
          anchor.href = objectUrl;
          anchor.download = "sim-lab-team-config.json";
          document.body.append(anchor);
          anchor.click();
          anchor.remove();
          URL.revokeObjectURL(objectUrl);
        };

        document.querySelectorAll("[data-global-scenario]").forEach((button) => {
          button.onclick = async () => {
            await request("/api/console/global-scenario", {
              method: "POST",
              body: JSON.stringify({ scenarioId: button.dataset.globalScenario }),
            });
            await refresh();
          };
        });

        document.querySelectorAll("[data-save-route]").forEach((button) => {
          button.onclick = async () => {
            const routeId = button.dataset.saveRoute;
            const strategy = document.querySelector('[data-route-id="' + routeId + '"][data-field="strategy"]').value;
            const scenarioId = document.querySelector('[data-route-id="' + routeId + '"][data-field="scenarioId"]').value;
            const forwardTarget = document.querySelector('[data-route-id="' + routeId + '"][data-field="forwardTarget"]').value;
            const enabled = document.querySelector('[data-route-id="' + routeId + '"][data-field="enabled"]').checked;
            await request('/api/console/routes/' + routeId, {
              method: 'PUT',
              body: JSON.stringify({ strategy, scenarioId, forwardTarget, enabled }),
            });
            await refresh();
          };
        });

        refresh();
        setInterval(refresh, 2500);
      </script>
    </body>
  </html>`;
}

const mockServer = http.createServer(async (req, res) => {
  const requestId = req.headers["x-sim-request-id"]?.toString() ?? createId();
  const url = new URL(req.url, `http://${req.headers.host}`);
  const route = matchRoute(state.settings.routes, url.pathname, selectProtocolForHttpRequest(req, url.pathname)) ??
    { id: "mock", pathPrefix: url.pathname, label: "Mock", protocol: "http" };
  const bodyBuffer = await readRequestBody(req);
  const { response } = await handleMockRequest({
    req,
    url,
    route,
    bodyBuffer,
    requestId,
  });
  writeNodeResponse(res, response);
});

const proxyServer = http.createServer(async (req, res) => {
  const requestId = createId();
  const startedAt = Date.now();
  const url = new URL(req.url, `http://${req.headers.host}`);
  const protocol = selectProtocolForHttpRequest(req, url.pathname);
  const route = matchRoute(state.settings.routes, url.pathname, protocol);

  if (req.method === "OPTIONS") {
    writeNodeResponse(res, textBody(204, ""));
    return;
  }

  if (!route) {
    const response = notFoundResponse("代理层未配置该路径的路由。");
    writeNodeResponse(res, response);
    pushLog({
      stage: "proxy",
      requestId,
      method: req.method,
      path: url.pathname,
      routeId: "unmatched",
      scenarioId: "normal",
      strategy: "unmatched",
      target: "none",
      status: response.status,
      durationMs: Date.now() - startedAt,
      proxyAddedLatencyMs: null,
    });
    return;
  }

  const scenario = resolveScenario(route);
  const bodyBuffer = await readRequestBody(req);
  const scenarioError = buildErrorResponseForScenario(route, scenario, requestId);

  if (scenario.delayMs > 0 || scenario.jitterMs > 0) {
    const extra = scenario.jitterMs > 0 ? Math.floor(Math.random() * scenario.jitterMs) : 0;
    await sleep(scenario.delayMs + extra);
  }

  if (scenarioError) {
    writeNodeResponse(res, scenarioError);
    pushLog({
      stage: "proxy",
      requestId,
      method: req.method,
      path: url.pathname,
      routeId: route.id,
      scenarioId: scenario.id,
      strategy: route.strategy,
      target: route.strategy === "forward" ? route.forwardTarget : route.mockTarget,
      status: scenarioError.status,
      durationMs: Date.now() - startedAt,
      proxyAddedLatencyMs: Date.now() - startedAt,
    });
    return;
  }

  const targetBase = route.strategy === "forward" ? route.forwardTarget : route.mockTarget;
  const targetUrl = new URL(`${targetBase}${url.pathname}${url.search}`);
  const headers = new Headers();
  for (const [key, value] of Object.entries(req.headers)) {
    if (value === undefined) continue;
    if (["host", "connection", "content-length"].includes(key.toLowerCase())) continue;
    headers.set(key, Array.isArray(value) ? value.join(", ") : value);
  }
  for (const [key, value] of Object.entries(buildScenarioHeaders(route, scenario, requestId))) {
    headers.set(key, value);
  }
  headers.set("x-sim-route-strategy", route.strategy);

  let upstreamResponse;
  try {
    upstreamResponse = await fetch(targetUrl, {
      method: req.method,
      headers,
      body:
        req.method === "GET" || req.method === "HEAD" || req.method === "OPTIONS"
          ? undefined
          : bodyBuffer,
    });
  } catch (error) {
    const response = jsonBody(502, {
      code: "SIM_PROXY_UPSTREAM_ERROR",
      message: `代理转发失败：${String(error)}`,
      retryable: true,
    });
    writeNodeResponse(res, response);
    pushLog({
      stage: "proxy",
      requestId,
      method: req.method,
      path: url.pathname,
      routeId: route.id,
      scenarioId: scenario.id,
      strategy: route.strategy,
      target: targetUrl.toString(),
      status: 502,
      durationMs: Date.now() - startedAt,
      proxyAddedLatencyMs: Date.now() - startedAt,
    });
    return;
  }

  const upstreamElapsed = Number(upstreamResponse.headers.get("x-sim-elapsed-ms"));
  const buffer = Buffer.from(await upstreamResponse.arrayBuffer());
  const responseHeaders = {};
  upstreamResponse.headers.forEach((value, key) => {
    if (["content-length", "connection", "transfer-encoding", "keep-alive"].includes(key.toLowerCase())) {
      return;
    }
    responseHeaders[key] = value;
  });
  responseHeaders["content-length"] = String(buffer.length);
  responseHeaders["x-sim-request-id"] = requestId;
  responseHeaders["x-sim-route-id"] = route.id;
  responseHeaders["x-sim-scenario"] = scenario.id;
  responseHeaders["x-sim-target"] = targetUrl.origin;
  responseHeaders["x-sim-proxy-added-latency-ms"] = Number.isFinite(upstreamElapsed)
    ? String(Math.max(0, Date.now() - startedAt - upstreamElapsed))
    : "0";

  res.writeHead(upstreamResponse.status, applyCors(responseHeaders));
  res.end(buffer);

  pushLog({
    stage: "proxy",
    requestId,
    method: req.method,
    path: url.pathname,
    routeId: route.id,
    scenarioId: scenario.id,
    strategy: route.strategy,
    target: targetUrl.toString(),
    status: upstreamResponse.status,
    durationMs: Date.now() - startedAt,
    proxyAddedLatencyMs: Number.isFinite(upstreamElapsed) ? Math.max(0, Date.now() - startedAt - upstreamElapsed) : 0,
  });
});

proxyServer.on("upgrade", async (req, socket, head) => {
  const url = new URL(req.url, `http://${req.headers.host}`);
  const route = matchRoute(state.settings.routes, url.pathname, "websocket");
  const requestId = createId();
  const startedAt = Date.now();

  if (!route) {
    socket.write("HTTP/1.1 404 Not Found\r\n\r\n");
    socket.destroy();
    return;
  }

  const scenario = resolveScenario(route);
  const scenarioError = buildErrorResponseForScenario(route, scenario, requestId);
  if (scenario.delayMs > 0 || scenario.jitterMs > 0) {
    const extra = scenario.jitterMs > 0 ? Math.floor(Math.random() * scenario.jitterMs) : 0;
    await sleep(scenario.delayMs + extra);
  }
  if (scenarioError) {
    socket.write(
      `HTTP/1.1 ${scenarioError.status} Simulated Error\r\nContent-Type: application/json\r\nContent-Length: ${scenarioError.body.length}\r\n\r\n`
    );
    socket.write(scenarioError.body);
    socket.destroy();
    pushLog({
      stage: "proxy-ws",
      requestId,
      method: "UPGRADE",
      path: url.pathname,
      routeId: route.id,
      scenarioId: scenario.id,
      strategy: route.strategy,
      target: route.strategy === "forward" ? route.forwardTarget : route.mockTarget,
      status: scenarioError.status,
      durationMs: Date.now() - startedAt,
      proxyAddedLatencyMs: Date.now() - startedAt,
    });
    return;
  }

  const target = new URL(route.strategy === "forward" ? route.forwardTarget : route.mockTarget);
  const connectPort = Number(target.port || (target.protocol === "wss:" ? 443 : 80));
  const upstream =
    target.protocol === "wss:"
      ? tls.connect(connectPort, target.hostname)
      : net.connect(connectPort, target.hostname);

  upstream.once("connect", () => {
    const headerLines = [`GET ${req.url} HTTP/1.1`];
    for (const [key, value] of Object.entries(req.headers)) {
      if (value === undefined) continue;
      if (key.toLowerCase() === "host") {
        headerLines.push(`Host: ${target.host}`);
      } else {
        headerLines.push(`${key}: ${Array.isArray(value) ? value.join(", ") : value}`);
      }
    }
    headerLines.push(`x-sim-request-id: ${requestId}`);
    headerLines.push(`x-sim-route-id: ${route.id}`);
    headerLines.push(`x-sim-scenario: ${scenario.id}`);
    upstream.write(`${headerLines.join("\r\n")}\r\n\r\n`);
    if (head && head.length > 0) {
      upstream.write(head);
    }
    socket.pipe(upstream).pipe(socket);
    pushLog({
      stage: "proxy-ws",
      requestId,
      method: "UPGRADE",
      path: url.pathname,
      routeId: route.id,
      scenarioId: scenario.id,
      strategy: route.strategy,
      target: target.toString(),
      status: 101,
      durationMs: Date.now() - startedAt,
      proxyAddedLatencyMs: Date.now() - startedAt,
    });
  });

  upstream.on("error", () => {
    socket.destroy();
  });
});

const dashboardServer = http.createServer(async (req, res) => {
  const url = new URL(req.url, `http://${req.headers.host}`);

  if (req.method === "GET" && url.pathname === "/") {
    res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
    res.end(renderDashboardHtml());
    return;
  }

  if (req.method === "GET" && url.pathname === "/api/console/state") {
    writeNodeResponse(
      res,
      jsonBody(200, {
        meta: state.meta,
        ports: runtimeConfig.ports,
        globalScenario: scenarioCatalog[state.settings.globalScenarioId] ?? scenarioCatalog.normal,
        settings: state.settings,
        metrics: summarizeLogs(),
        logs: state.requestLogs.slice(0, 80),
        mock: {
          previewCount: Object.keys(state.mock.previews).length,
          batchCount: Object.keys(state.mock.batches).length,
          sensitiveTermCount: state.mock.sensitiveTerms.length,
          operationEventCount: state.mock.operationEvents.length,
          sandbox: getSandboxStatusResponse(),
        },
      })
    );
    return;
  }

  if (req.method === "GET" && url.pathname === "/api/console/export-config") {
    const body = Buffer.from(
      JSON.stringify(
        {
          ports: runtimeConfig.ports,
          globalScenarioId: state.settings.globalScenarioId,
          logLimit: state.settings.logLimit,
          routes: state.settings.routes,
        },
        null,
        2
      ),
      "utf8"
    );
    res.writeHead(200, {
      "content-type": "application/json; charset=utf-8",
      "content-disposition": 'attachment; filename="sim-lab-team-config.json"',
      "content-length": String(body.length),
    });
    res.end(body);
    return;
  }

  const bodyBuffer = await readRequestBody(req);
  const payload = safeJsonParse(bodyBuffer.toString("utf8"), {});

  if (req.method === "POST" && url.pathname === "/api/console/reset") {
    resetEnvironment();
    writeNodeResponse(res, jsonBody(200, { success: true }));
    return;
  }

  if (req.method === "POST" && url.pathname === "/api/console/clear-logs") {
    clearLogs();
    writeNodeResponse(res, jsonBody(200, { success: true }));
    return;
  }

  if (req.method === "POST" && url.pathname === "/api/console/clear-data") {
    clearMockData();
    writeNodeResponse(res, jsonBody(200, { success: true }));
    return;
  }

  if (req.method === "POST" && url.pathname === "/api/console/global-scenario") {
    if (!scenarioCatalog[payload.scenarioId]) {
      writeNodeResponse(res, jsonBody(400, { message: "未知场景。" }));
      return;
    }
    state.settings.globalScenarioId = payload.scenarioId;
    schedulePersist();
    writeNodeResponse(res, jsonBody(200, { success: true }));
    return;
  }

  const routeMatch = url.pathname.match(/^\/api\/console\/routes\/([^/]+)$/);
  if (req.method === "PUT" && routeMatch) {
    const route = state.settings.routes.find((item) => item.id === routeMatch[1]);
    if (!route) {
      writeNodeResponse(res, jsonBody(404, { message: "路由不存在。" }));
      return;
    }
    if (payload.strategy) route.strategy = payload.strategy;
    if (payload.scenarioId) route.scenarioId = payload.scenarioId;
    if (payload.forwardTarget) route.forwardTarget = payload.forwardTarget;
    if (payload.enabled !== undefined) route.enabled = Boolean(payload.enabled);
    schedulePersist();
    writeNodeResponse(res, jsonBody(200, { success: true, route }));
    return;
  }

  writeNodeResponse(res, notFoundResponse("控制台接口不存在。"));
});

let clientProcess = null;

function pipePrefixed(stream, prefix) {
  stream.on("data", (chunk) => {
    process.stdout.write(`${prefix}${chunk}`);
  });
}

function startClientProcess() {
  if (clientMode === "none") return;

  const cargoBin = path.join(os.homedir(), ".cargo", "bin");
  const env = {
    ...process.env,
    PATH: process.env.PATH
      ? `${cargoBin}${path.delimiter}${process.env.PATH}`
      : cargoBin,
    VITE_DESKTOP_SIM_LAB_PROXY: `http://127.0.0.1:${runtimeConfig.ports.proxy}`,
    VITE_RUNTIME_PROXY_TARGET: `http://127.0.0.1:${runtimeConfig.ports.proxy}`,
  };

  if (clientMode === "browser-dev") {
    env.VITE_DEV_PORT = String(runtimeConfig.ports.clientDev);
    clientProcess = spawn("corepack", ["pnpm", "dev", "--host", "127.0.0.1"], {
      cwd: workspaceRoot,
      env,
      stdio: ["ignore", "pipe", "pipe"],
    });
  } else if (clientMode === "browser-preview") {
    if (autoBuild) {
      const build = spawn("corepack", ["pnpm", "build"], {
        cwd: workspaceRoot,
        env,
        stdio: "inherit",
      });
      build.on("close", (code) => {
        if (code !== 0) {
          process.exit(code ?? 1);
        }
      });
    }
    env.VITE_PREVIEW_PORT = String(runtimeConfig.ports.clientPreview);
    clientProcess = spawn("corepack", ["pnpm", "preview", "--host", "127.0.0.1"], {
      cwd: workspaceRoot,
      env,
      stdio: ["ignore", "pipe", "pipe"],
    });
  } else if (clientMode === "tauri-dev") {
    clientProcess = spawn("corepack", ["pnpm", "tauri", "dev"], {
      cwd: workspaceRoot,
      env,
      stdio: ["ignore", "pipe", "pipe"],
    });
  }

  if (clientProcess) {
    pipePrefixed(clientProcess.stdout, "[sim-client] ");
    pipePrefixed(clientProcess.stderr, "[sim-client] ");
  }
}

async function listen(server, port, host) {
  await new Promise((resolve) => server.listen(port, host, resolve));
}

await listen(mockServer, runtimeConfig.ports.mock, "127.0.0.1");
await listen(proxyServer, runtimeConfig.ports.proxy, "127.0.0.1");
await listen(dashboardServer, runtimeConfig.ports.dashboard, "127.0.0.1");
startClientProcess();

console.log(`[sim-lab] dashboard: http://127.0.0.1:${runtimeConfig.ports.dashboard}`);
console.log(`[sim-lab] proxy:     http://127.0.0.1:${runtimeConfig.ports.proxy}`);
console.log(`[sim-lab] mock:      http://127.0.0.1:${runtimeConfig.ports.mock}`);
if (clientMode === "browser-dev") {
  console.log(`[sim-lab] client:    http://127.0.0.1:${runtimeConfig.ports.clientDev}`);
} else if (clientMode === "browser-preview") {
  console.log(`[sim-lab] client:    http://127.0.0.1:${runtimeConfig.ports.clientPreview}`);
} else if (clientMode === "tauri-dev") {
  console.log("[sim-lab] client:    Tauri dev desktop window");
}

async function shutdown() {
  clearTimeout(saveTimer);
  await writeJson(statePath, state);
  mockServer.close();
  proxyServer.close();
  dashboardServer.close();
  if (clientProcess && !clientProcess.killed) {
    clientProcess.kill("SIGTERM");
  }
  process.exit(0);
}

process.on("SIGINT", shutdown);
process.on("SIGTERM", shutdown);
