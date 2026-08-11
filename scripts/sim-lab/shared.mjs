import fs from "node:fs/promises";
import path from "node:path";
import crypto from "node:crypto";

export const scenarioCatalog = {
  normal: {
    id: "normal",
    label: "正常流程",
    description: "直接返回正常结果，不额外引入延迟或错误。",
    delayMs: 0,
    jitterMs: 0,
  },
  timeout: {
    id: "timeout",
    label: "网络超时",
    description: "延迟后返回 504，用于模拟前后端等待超时。",
    delayMs: 12000,
    jitterMs: 0,
    errorStatus: 504,
    errorCode: "SIM_TIMEOUT",
    errorMessage: "模拟环境已注入超时场景。",
    retryable: true,
  },
  server_error: {
    id: "server_error",
    label: "服务报错",
    description: "直接返回 503 错误，模拟后端服务异常。",
    delayMs: 20,
    jitterMs: 0,
    errorStatus: 503,
    errorCode: "SIM_SERVER_ERROR",
    errorMessage: "模拟环境已注入服务错误场景。",
    retryable: false,
  },
  rate_limit: {
    id: "rate_limit",
    label: "限流降级",
    description: "返回 429，并携带 Retry-After 响应头。",
    delayMs: 10,
    jitterMs: 0,
    errorStatus: 429,
    errorCode: "SIM_RATE_LIMIT",
    errorMessage: "模拟环境已注入限流场景。",
    retryable: true,
    retryAfterSeconds: 30,
  },
  weak_network: {
    id: "weak_network",
    label: "弱网抖动",
    description: "引入随机延迟和轻微抖动，用于观察页面重试与等待提示。",
    delayMs: 120,
    jitterMs: 220,
  },
};

export const scenarioOptions = Object.values(scenarioCatalog);

export const builtinRules = [
  { id: "chinese_name", name: "中文姓名", enabled_by_default: true },
  { id: "phone", name: "手机号", enabled_by_default: true },
  { id: "email", name: "电子邮箱", enabled_by_default: true },
  { id: "id_card", name: "身份证", enabled_by_default: false },
  { id: "bank_card", name: "银行卡", enabled_by_default: false },
  { id: "ipv4", name: "IPv4 地址", enabled_by_default: false },
  { id: "passport", name: "护照号", enabled_by_default: false },
  { id: "use_sensitive_terms", name: "敏感词库", enabled_by_default: true },
];

export async function ensureDir(dirPath) {
  await fs.mkdir(dirPath, { recursive: true });
}

export async function readJsonIfExists(filePath, fallback) {
  try {
    const raw = await fs.readFile(filePath, "utf8");
    return JSON.parse(raw);
  } catch (_error) {
    return fallback;
  }
}

export async function writeJson(filePath, payload) {
  await ensureDir(path.dirname(filePath));
  await fs.writeFile(filePath, JSON.stringify(payload, null, 2), "utf8");
}

export function nowIso() {
  return new Date().toISOString();
}

export function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export function deepClone(value) {
  return JSON.parse(JSON.stringify(value));
}

export function createId() {
  return crypto.randomUUID();
}

export function normalizeRoute(route, ports) {
  const protocol = route.protocol ?? "http";
  return {
    enabled: route.enabled !== false,
    strategy: route.strategy ?? "mock",
    scenarioId: route.scenarioId ?? "normal",
    protocol,
    mockTarget:
      route.mockTarget ??
      (protocol === "http" ? `http://127.0.0.1:${ports.mock}` : `ws://127.0.0.1:${ports.mock}`),
    ...route,
  };
}

export function matchRoute(routes, pathname, protocol = "http") {
  return routes
    .filter((route) => route.enabled && route.protocol === protocol)
    .sort((left, right) => right.pathPrefix.length - left.pathPrefix.length)
    .find((route) => pathname.startsWith(route.pathPrefix));
}

export async function readRequestBody(req) {
  const chunks = [];
  for await (const chunk of req) {
    chunks.push(chunk);
  }
  return Buffer.concat(chunks);
}

export function safeJsonParse(raw, fallback = {}) {
  try {
    return JSON.parse(raw);
  } catch (_error) {
    return fallback;
  }
}

export function jsonBody(status, payload, extraHeaders = {}) {
  const body = Buffer.from(JSON.stringify(payload), "utf8");
  return {
    status,
    headers: {
      "content-type": "application/json; charset=utf-8",
      "content-length": String(body.length),
      "cache-control": "no-store",
      ...extraHeaders,
    },
    body,
  };
}

export function textBody(status, content, extraHeaders = {}) {
  const body = Buffer.from(content, "utf8");
  return {
    status,
    headers: {
      "content-type": "text/plain; charset=utf-8",
      "content-length": String(body.length),
      "cache-control": "no-store",
      ...extraHeaders,
    },
    body,
  };
}

export function markdownDownloadBody(filename, content, extraHeaders = {}) {
  const body = Buffer.from(content, "utf8");
  return {
    status: 200,
    headers: {
      "content-type": "text/markdown; charset=utf-8",
      "content-length": String(body.length),
      "content-disposition": `attachment; filename="${filename}"`,
      "cache-control": "no-store",
      ...extraHeaders,
    },
    body,
  };
}

export function createDefaultMockState() {
  const now = nowIso();
  return {
    rules: deepClone(builtinRules),
    previews: {},
    previewFiles: {},
    batches: {},
    artifacts: {},
    sensitiveTerms: [
      {
        id: createId(),
        term: "保密项目A",
        category: "项目",
        description: "默认示例词条",
        enabled: true,
        created_at: now,
        updated_at: now,
      },
    ],
    sandbox: {
      pin: null,
      pin_configured: false,
      locked: false,
      failedAttempts: 0,
      rate_limited_until: null,
    },
    filebay: {
      status: "configured",
      configured: true,
      has_token: true,
      target_origin: "https://mock-filebay.local",
      owner: "sim-team",
      repo: "vault-artifacts",
      repository_exists: true,
    },
    operationEvents: [],
    mockStats: {
      resets: 0,
    },
  };
}

export function buildMaskedMarkdown(displayName, ruleIds) {
  return [
    "# CheersAI Vault Simulated Masked Artifact",
    "",
    `- source: ${displayName}`,
    `- generated_at: ${nowIso()}`,
    `- applied_rules: ${ruleIds.length > 0 ? ruleIds.join(", ") : "none"}`,
    "",
    "客户姓名：姓名1",
    "联系电话：***PHONE***2",
    "电子邮箱：***EMAIL***3",
    "",
    "> 该文件由本地仿真环境生成，用于链路验证与回归测试。",
  ].join("\n");
}

export function buildRestoredMarkdown(displayName) {
  return [
    "# CheersAI Vault Simulated Restored Artifact",
    "",
    `- source: ${displayName}`,
    `- restored_at: ${nowIso()}`,
    "",
    "客户姓名：张三",
    "联系电话：13900000000",
    "电子邮箱：zhangsan@example.com",
  ].join("\n");
}

export function parseMultipart(bodyBuffer, contentType) {
  const match = /boundary=(?:"([^"]+)"|([^;]+))/i.exec(contentType || "");
  if (!match) return { files: [], fields: {} };

  const boundary = `--${match[1] || match[2]}`;
  const body = bodyBuffer.toString("latin1");
  const segments = body.split(boundary);
  const files = [];
  const fields = {};

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
      files.push({
        name: path.basename(filenameMatch[1]),
        content,
      });
      continue;
    }

    if (fieldName) {
      fields[fieldName] = content;
    }
  }

  return { files, fields };
}

export function paginate(items, page, pageSize) {
  const totalCount = items.length;
  const totalPages = Math.max(1, Math.ceil(totalCount / pageSize));
  const safePage = Math.min(Math.max(page, 1), totalPages);
  const start = (safePage - 1) * pageSize;
  return {
    page: safePage,
    page_size: pageSize,
    total_count: totalCount,
    total_pages: totalPages,
    entries: items.slice(start, start + pageSize),
  };
}
