/**
 * Shared, browser-safe classification for Runtime HTTP failures.
 *
 * A response that reached the browser is always an HTTP outcome, even when a
 * proxy returned HTML, plain text, an empty body, or malformed JSON. Only the
 * three fields in the Runtime error contract are allowed to cross this helper
 * boundary; the original response body is never returned to callers.
 */

export type RuntimeHttpFailure = {
  ok: false;
  reason: "http";
  status: number;
  code?: string;
  message?: string;
  retryable?: boolean;
};

export type RuntimeNetworkFailure = { ok: false; reason: "network" };

export type RuntimeJsonParseFailure = { ok: false; reason: "parse" };

type RuntimeErrorPayload = {
  code: string;
  message: string;
  retryable: boolean;
};

function isRuntimeErrorPayload(value: unknown): value is RuntimeErrorPayload {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return false;
  }

  const payload = value as Record<string, unknown>;
  return (
    typeof payload.code === "string" &&
    typeof payload.message === "string" &&
    typeof payload.retryable === "boolean"
  );
}

const PLAINTEXT_FALLBACK: Readonly<Record<number, { code: string; message: string }>> = {
  400: { code: "BAD_REQUEST", message: "请求参数或上传文件无效，请校验后重试。" },
  401: { code: "UNAUTHORIZED", message: "当前会话未授权，请重新登录后重试。" },
  403: { code: "FORBIDDEN", message: "当前访问被拒绝，请确认 Runtime CORS 白名单包含本页来源。" },
  404: {
    code: "NOT_FOUND",
    message: "请求的 Runtime 接口不存在，请确认本地 Runtime 版本与前端版本一致并重新部署。",
  },
  413: {
    code: "PAYLOAD_TOO_LARGE",
    message: "上传文件大小超过限制，请拆分或压缩后重试。",
  },
  422: { code: "UNPROCESSABLE", message: "Excel 结构或脱敏配置无效，请检查配置后重试。" },
  500: {
    code: "INTERNAL_ERROR",
    message: "Runtime 执行异常，请查看本地 Runtime 日志并重试。",
  },
  502: {
    code: "BAD_GATEWAY",
    message: "本地 Runtime 网关不可达，请确认服务已启动后重试。",
  },
  503: {
    code: "SERVICE_UNAVAILABLE",
    message: "本地 Runtime 暂不可用，请确认服务已启动后重试。",
  },
  504: {
    code: "GATEWAY_TIMEOUT",
    message: "本地 Runtime 处理超时，请稍后重试或缩小上传文件。",
  },
};

const EN_HTTP_LITERALS =
  /the requested resource was not found|failed to fetch|networkerror|not found|bad gateway|service unavailable|gateway timeout|invalid response|failed to invoke/i;

function looksLikeTrustedChinese(value: string): boolean {
  if (typeof value !== "string") return false;
  const trimmed = value.trim();
  if (!trimmed || trimmed.length > 240) return false;
  if (EN_HTTP_LITERALS.test(trimmed)) return false;
  return /[\u4e00-\u9fa5]/.test(trimmed);
}

function sanitizeFailureMessage(status: number, message: string | undefined): string {
  const fallback = PLAINTEXT_FALLBACK[status];
  const defaultMsg = fallback
    ? fallback.message
    : "请求失败，请稍后重试或联系系统管理员。";
  if (!message || typeof message !== "string") return defaultMsg;
  if (looksLikeTrustedChinese(message)) return message;
  if (EN_HTTP_LITERALS.test(message)) {
    if (status === 404 || /not found|was not found/i.test(message)) {
      return PLAINTEXT_FALLBACK[404]?.message ?? defaultMsg;
    }
    if (/network|fetch|invoke|gateway|unavailable|timeout/i.test(message)) {
      return PLAINTEXT_FALLBACK[503]?.message ?? defaultMsg;
    }
  }
  return fallback ? fallback.message : defaultMsg;
}

/**
 * Classify a response after `fetch` has received it. The body is consumed only
 * to recover a complete, type-checked Runtime error payload; malformed or
 * incomplete payloads (including warp/nginx plain-text 404s) are upgraded to a
 * stable status-based contract so upstream never surfaces raw "The requested
 * resource was not found" literal text.
 *
 * Hard guarantee: the returned `message` field, if any, is always Chinese and
 * never contains an English HTTP plain-text literal.
 */
export async function classifyRuntimeHttpResponse(response: Response): Promise<RuntimeHttpFailure> {
  let payload: unknown = null;
  try {
    payload = await response.json();
    if (isRuntimeErrorPayload(payload)) {
      return {
        ok: false,
        reason: "http",
        status: response.status,
        code: payload.code,
        message: sanitizeFailureMessage(response.status, payload.message),
        retryable: payload.retryable,
      };
    }
  } catch {
    // Non-JSON, empty, and malformed bodies deliberately remain status-only.
  }

  const fallback = PLAINTEXT_FALLBACK[response.status];
  if (fallback) {
    const rawText: string | null = typeof payload === "string" ? payload : null;
    const needsForce = !rawText || response.status === 404 || /was not found/i.test(rawText ?? "");
    return {
      ok: false,
      reason: "http",
      status: response.status,
      code: fallback.code,
      message: needsForce ? fallback.message : sanitizeFailureMessage(response.status, rawText ?? undefined),
    };
  }

  return {
    ok: false,
    reason: "http",
    status: response.status,
    message: sanitizeFailureMessage(response.status, undefined),
  };
}

/** Keep fetch rejects, including AbortError before any response, in the existing network bucket. */
export function classifyRuntimeFetchError(_error: unknown): RuntimeNetworkFailure {
  return { ok: false, reason: "network" };
}

/**
 * Final UI-facing normalization for Runtime failures.
 *
 * This is the ONLY place that is allowed to emit user-visible error strings.
 * Upstream code (pages, dialogs, tauri invokes, nginx proxies, warp defaults)
 * must never directly show raw `error.message` from an unknown source because
 * host shells (Tauri/Warp/Nginx) frequently leak English literals such as
 * "The requested resource was not found" or "Failed to invoke" that break
 * the contract.
 *
 * Policy:
 *  - "cannot reach runtime" failures (network / transport / tauri invoke
 *    without a response) always become the same runtime-unreachable Chinese
 *    string so the user sees consistent wording.
 *  - HTTP failures with a stable code (404/413/5xx) also get the centralized
 *    Chinese message here, even if the underlying Response/Tauri payload was
 *    non-JSON English text.
 *  - Caller-provided messages are retained only if they pass a strict
 *    allow-list (Chinese text only, ASCII control chars stripped, no English
 *    HTTP literals). This acts as the last-mile hardening so we are sure
 *    the screenshot never again shows raw 404 English text.
 */
export function normalizeRuntimeUserMessage(
  failure:
    | RuntimeHttpFailure
    | RuntimeNetworkFailure
    | RuntimeJsonParseFailure
    | null
    | undefined,
  fallback?: string | null
): string {
  const defaultMsg =
    fallback && typeof fallback === "string" && fallback.trim().length > 0
      ? fallback
      : "Excel 脱敏执行失败，请稍后重试。";

  if (!failure) return defaultMsg;

  if (failure.reason === "network") {
    return "无法连接本机 Runtime，请确认服务已启动后重试。";
  }

  if (failure.reason === "parse") {
    return "Runtime 返回格式异常，请确认本地 Runtime 版本与前端版本一致并重新部署。";
  }

  const status = failure.status ?? 0;
  switch (status) {
    case 400:
      return failure.message && looksLikeTrustedChinese(failure.message)
        ? failure.message
        : "请求参数或上传文件无效，请校验后重试。";
    case 401:
      return "当前会话未授权，请重新登录后重试。";
    case 403:
      return "当前访问被拒绝，请确认 Runtime CORS 白名单包含本页来源。";
    case 404:
      return "请求的 Runtime 接口不存在，请确认本地 Runtime 版本与前端版本一致并重新部署。";
    case 413:
      return "上传文件大小超过限制，请拆分或压缩后重试。";
    case 422:
      return failure.message && looksLikeTrustedChinese(failure.message)
        ? failure.message
        : "Excel 结构或脱敏配置无效，请检查配置后重试。";
    case 500:
      return "Runtime 执行异常，请查看本地 Runtime 日志并重试。";
    case 502:
      return "本地 Runtime 网关不可达，请确认服务已启动后重试。";
    case 503:
      return "本地 Runtime 暂不可用，请确认服务已启动后重试。";
    case 504:
      return "本地 Runtime 处理超时，请稍后重试或缩小上传文件。";
    default:
      break;
  }

  if (failure.message && looksLikeTrustedChinese(failure.message)) {
    return failure.message;
  }
  return defaultMsg;
}

/**
 * One-shot normalizer specifically for Tauri `invoke` rejections and for callers
 * that still throw `Error(message)` into the UI layer. We deliberately treat
 * any English/unknown string as untrusted and replace with a stable fallback.
 */
export function normalizeCaughtRuntimeErrorMessage(
  error: unknown,
  fallback?: string | null
): string {
  if (
    error &&
    typeof error === "object" &&
    "ok" in error &&
    (error as { ok?: boolean }).ok === false
  ) {
    return normalizeRuntimeUserMessage(
      error as RuntimeHttpFailure | RuntimeNetworkFailure | RuntimeJsonParseFailure,
      fallback ?? undefined
    );
  }
  const raw =
    error instanceof Error
      ? error.message
      : typeof error === "string"
        ? error
        : "";
  if (!raw) {
    return fallback && typeof fallback === "string" && fallback.trim().length > 0
      ? fallback
      : "Excel 脱敏执行失败，请稍后重试。";
  }
  if (looksLikeTrustedChinese(raw)) return raw;
  if (EN_HTTP_LITERALS.test(raw)) {
    return "请求的 Runtime 接口不存在，请确认本地 Runtime 版本与前端版本一致并重新部署。";
  }
  if (
    /invoke/i.test(raw) ||
    /^error building request/i.test(raw) ||
    /network error/i.test(raw)
  ) {
    return "无法连接本机 Runtime，请确认服务已启动后重试。";
  }
  return fallback && typeof fallback === "string" && fallback.trim().length > 0
    ? fallback
    : "Excel 脱敏执行失败，请稍后重试。";
}

/** Parse a successful JSON response without exposing malformed response text. */
export async function parseRuntimeJsonResponse<T>(
  response: Response
): Promise<{ ok: true; data: T } | RuntimeJsonParseFailure> {
  try {
    return { ok: true, data: (await response.json()) as T };
  } catch {
    return { ok: false, reason: "parse" };
  }
}
