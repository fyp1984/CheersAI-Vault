const http = require("http");
const fs = require("fs");
const path = require("path");
const crypto = require("crypto");

const PORT = Number(process.env.VAULT_RUNTIME_PORT || 8787);
const HOST = process.env.VAULT_RUNTIME_BIND_HOST || "0.0.0.0";
const DATA_DIR =
  process.env.VAULT_RUNTIME_DATA_DIR ||
  "/var/lib/cheersai-vault/runtime-data";
const STATE_PATH = path.join(DATA_DIR, "runtime-state.json");

const DEFAULT_RULES = [
  { id: "pii-name", name: "姓名脱敏", enabled_by_default: true },
  { id: "pii-phone", name: "手机号脱敏", enabled_by_default: true },
  { id: "pii-id-card", name: "证件号脱敏", enabled_by_default: false },
];

function ensureStateDir() {
  fs.mkdirSync(DATA_DIR, { recursive: true });
}

function defaultState() {
  return {
    batches: {},
    artifacts: {},
    rules: DEFAULT_RULES,
    updated_at: new Date().toISOString(),
  };
}

function loadState() {
  ensureStateDir();
  if (!fs.existsSync(STATE_PATH)) {
    const state = defaultState();
    saveState(state);
    return state;
  }
  try {
    return JSON.parse(fs.readFileSync(STATE_PATH, "utf8"));
  } catch (_error) {
    const state = defaultState();
    saveState(state);
    return state;
  }
}

function saveState(state) {
  state.updated_at = new Date().toISOString();
  fs.writeFileSync(STATE_PATH, JSON.stringify(state, null, 2));
}

function json(res, statusCode, payload) {
  const body = JSON.stringify(payload);
  res.writeHead(statusCode, {
    "Content-Type": "application/json; charset=utf-8",
    "Content-Length": Buffer.byteLength(body),
    "Access-Control-Allow-Origin": "*",
    "Access-Control-Allow-Methods": "GET,POST,OPTIONS",
    "Access-Control-Allow-Headers": "Content-Type, Authorization, Accept",
    "Cache-Control": "no-store",
  });
  res.end(body);
}

function textDownload(res, statusCode, filename, content, extraHeaders = {}) {
  const body = Buffer.from(content, "utf8");
  res.writeHead(statusCode, {
    "Content-Type": "text/markdown; charset=utf-8",
    "Content-Length": body.length,
    "Content-Disposition": `attachment; filename="${filename}"`,
    "Access-Control-Allow-Origin": "*",
    ...extraHeaders,
  });
  res.end(body);
}

function badRequest(res, message) {
  json(res, 400, {
    code: "BAD_REQUEST",
    message,
    retryable: false,
  });
}

function notFound(res, message = "资源不存在。") {
  json(res, 404, {
    code: "NOT_FOUND",
    message,
    retryable: false,
  });
}

function inferFormat(filename) {
  const ext = path.extname(filename).toLowerCase();
  const mapping = {
    ".txt": "text",
    ".md": "markdown",
    ".markdown": "markdown",
    ".csv": "csv",
    ".xls": "excel",
    ".xlsx": "excel",
    ".docx": "docx",
    ".pdf": "pdf",
    ".ppt": "powerpoint",
    ".pptx": "powerpoint",
  };
  return mapping[ext] || "text";
}

function maskedMarkdown(filename, rules) {
  return [
    "# CheersAI Vault Masked Artifact",
    "",
    `- source: ${filename}`,
    `- generated_at: ${new Date().toISOString()}`,
    `- applied_rules: ${rules.join(", ") || "none"}`,
    "",
    "> 这是用于 Docker 本地验证的最小运行时产物。",
    "",
    "本文档用于证明前端与 Runtime API 链路可用。",
  ].join("\n");
}

function restoredMarkdown(filename) {
  return [
    "# CheersAI Vault Restored Artifact",
    "",
    `- source: ${filename}`,
    `- restored_at: ${new Date().toISOString()}`,
    "",
    "这是 Docker 本地验证产生的示例恢复文件。",
  ].join("\n");
}

function parseMultipart(bodyBuffer, contentType) {
  const match = /boundary=(?:"([^"]+)"|([^;]+))/i.exec(contentType || "");
  if (!match) return { files: [], ruleIds: [] };

  const boundary = `--${match[1] || match[2]}`;
  const body = bodyBuffer.toString("latin1");
  const segments = body.split(boundary);
  const files = [];
  let ruleIds = [];

  for (const segment of segments) {
    if (!segment || segment === "--\r\n" || segment === "--") continue;
    const trimmed = segment.replace(/^\r\n/, "").replace(/\r\n$/, "");
    const splitIndex = trimmed.indexOf("\r\n\r\n");
    if (splitIndex < 0) continue;

    const rawHeaders = trimmed.slice(0, splitIndex);
    let content = trimmed.slice(splitIndex + 4);
    content = content.replace(/\r\n--$/, "").replace(/\r\n$/, "");

    const nameMatch = /name="([^"]+)"/i.exec(rawHeaders);
    const filenameMatch = /filename="([^"]*)"/i.exec(rawHeaders);
    const fieldName = nameMatch ? nameMatch[1] : "";

    if (fieldName === "files" && filenameMatch && filenameMatch[1]) {
      files.push({ name: path.basename(filenameMatch[1]) });
      continue;
    }

    if (fieldName === "rule_ids") {
      try {
        const parsed = JSON.parse(content);
        if (Array.isArray(parsed)) ruleIds = parsed.map(String);
      } catch (_error) {
        ruleIds = [];
      }
    }
  }

  return { files, ruleIds };
}

function recalcBatch(batchState) {
  const files = batchState.files;
  const completed = files.filter((file) => file.status === "Completed").length;
  const failed = files.filter((file) => file.status === "Failed").length;
  const running = files.some((file) => file.status === "Running");
  batchState.batch.file_count = files.length;
  batchState.batch.completed_count = completed;
  batchState.batch.failed_count = failed;
  batchState.batch.masked_entity_count = files.reduce(
    (sum, file) => sum + (file.masked_entity_count || 0),
    0
  );
  batchState.batch.status = running
    ? "Running"
    : failed > 0
      ? completed > 0
        ? "CompletedWithErrors"
        : "Failed"
      : "Completed";
  batchState.batch.updated_at = new Date().toISOString();
}

function createBatch(state, files, ruleIds) {
  const batchId = crypto.randomUUID();
  const createdAt = new Date().toISOString();

  const batchState = {
    batch: {
      batch_id: batchId,
      status: "Completed",
      file_count: files.length,
      completed_count: files.length,
      failed_count: 0,
      masked_entity_count: 0,
      updated_at: createdAt,
    },
    files: files.map((file) => {
      const fileId = crypto.randomUUID();
      const artifactId = crypto.randomUUID();
      const maskedCount = Math.max(
        1,
        Math.min(6, Math.ceil(file.name.length / 4))
      );

      state.artifacts[artifactId] = {
        artifact_id: artifactId,
        source_name: file.name,
        masked_content: maskedMarkdown(file.name, ruleIds),
        restored_content: restoredMarkdown(file.name),
        restored_entity_count: maskedCount,
      };

      return {
        file_id: fileId,
        display_name: file.name,
        input_format: inferFormat(file.name),
        status: "Completed",
        attempt: 1,
        masked_entity_count: maskedCount,
        error_code: null,
        error_message: null,
        artifact_id: artifactId,
        restore_available: true,
      };
    }),
  };

  recalcBatch(batchState);
  state.batches[batchId] = batchState;
  saveState(state);
  return batchId;
}

function findFileById(state, fileId) {
  for (const batchState of Object.values(state.batches)) {
    const file = batchState.files.find((item) => item.file_id === fileId);
    if (file) return { batchState, file };
  }
  return null;
}

const server = http.createServer((req, res) => {
  if (req.method === "OPTIONS") {
    res.writeHead(204, {
      "Access-Control-Allow-Origin": "*",
      "Access-Control-Allow-Methods": "GET,POST,OPTIONS",
      "Access-Control-Allow-Headers": "Content-Type, Authorization, Accept",
      "Access-Control-Max-Age": "3600",
    });
    res.end();
    return;
  }

  const url = new URL(req.url, `http://${req.headers.host}`);
  const state = loadState();

  if (req.method === "GET" && url.pathname === "/api/v1/health") {
    json(res, 200, {
      success: true,
      message: "Vault runtime validation server is running",
    });
    return;
  }

  if (req.method === "GET" && url.pathname === "/api/v1/rules") {
    json(res, 200, { rules: state.rules || DEFAULT_RULES });
    return;
  }

  if (req.method === "GET" && url.pathname === "/api/v1/ocr/status") {
    json(res, 200, {
      status: "ready",
      provider: "docker-validation-runtime",
      message: "OCR validation endpoint is available",
    });
    return;
  }

  if (req.method === "GET" && url.pathname === "/api/v1/batches") {
    const batches = Object.values(state.batches)
      .map((entry) => entry.batch)
      .sort((a, b) => b.updated_at.localeCompare(a.updated_at));
    json(res, 200, { batches });
    return;
  }

  const batchMatch = url.pathname.match(/^\/api\/v1\/batches\/([^/]+)$/);
  if (req.method === "GET" && batchMatch) {
    const batchState = state.batches[batchMatch[1]];
    if (!batchState) {
      notFound(res, "批次不存在。");
      return;
    }
    json(res, 200, batchState);
    return;
  }

  const retryMatch = url.pathname.match(/^\/api\/v1\/files\/([^/]+)\/retry$/);
  if (req.method === "POST" && retryMatch) {
    const found = findFileById(state, retryMatch[1]);
    if (!found) {
      notFound(res, "文件记录不存在。");
      return;
    }
    found.file.status = "Completed";
    found.file.error_code = null;
    found.file.error_message = null;
    found.file.attempt += 1;
    recalcBatch(found.batchState);
    saveState(state);
    json(res, 200, { success: true });
    return;
  }

  const artifactMatch = url.pathname.match(/^\/api\/v1\/artifacts\/([^/]+)$/);
  if (req.method === "GET" && artifactMatch) {
    const artifact = state.artifacts[artifactMatch[1]];
    if (!artifact) {
      notFound(res, "产物不存在。");
      return;
    }
    const baseName = path.basename(artifact.source_name, path.extname(artifact.source_name));
    textDownload(
      res,
      200,
      `${baseName}.masked.md`,
      artifact.masked_content
    );
    return;
  }

  const restoreMatch = url.pathname.match(/^\/api\/v1\/artifacts\/([^/]+)\/restore$/);
  if (req.method === "POST" && restoreMatch) {
    const artifact = state.artifacts[restoreMatch[1]];
    if (!artifact) {
      notFound(res, "恢复产物不存在。");
      return;
    }
    const baseName = path.basename(artifact.source_name, path.extname(artifact.source_name));
    textDownload(
      res,
      200,
      `${baseName}.restored.md`,
      artifact.restored_content,
      {
        "X-Restored-Entity-Count": String(artifact.restored_entity_count || 0),
      }
    );
    return;
  }

  if (req.method === "POST" && url.pathname === "/api/v1/batches") {
    const chunks = [];
    req.on("data", (chunk) => chunks.push(chunk));
    req.on("end", () => {
      const body = Buffer.concat(chunks);
      const { files, ruleIds } = parseMultipart(body, req.headers["content-type"]);
      if (files.length === 0) {
        badRequest(res, "未解析到上传文件。");
        return;
      }
      const batchId = createBatch(state, files, ruleIds);
      json(res, 201, { batch_id: batchId });
    });
    return;
  }

  notFound(res);
});

server.listen(PORT, HOST, () => {
  ensureStateDir();
  console.log(
    `[vault-runtime-api] validation server listening on http://${HOST}:${PORT}`
  );
});
