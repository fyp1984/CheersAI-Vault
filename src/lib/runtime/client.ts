/**
 * 普通浏览器宿主的最小 Runtime HTTP 适配器。
 *
 * 只使用原生 `fetch` 调用 `vault-runtime-api` 现有的 `/api/v1/health` 与
 * `/api/v1/ocr/status`，不复制、不推断服务端状态判断——组件状态原样转发
 * 服务端返回的字段，网络失败与非 2xx 响应统一归类为“断开”，只暴露固定的
 * 安全提示，不把底层错误、堆栈或服务器路径透出给界面。
 *
 * 地址解析：
 * - 默认使用同源相对路径（空 base），适配生产环境同源反向代理部署；
 * - 开发模式可通过 `VITE_RUNTIME_API_URL` 显式指定（例如直连
 *   `http://127.0.0.1:8787`），或者不设置该变量、改用 Vite 开发代理
 *   （见 `vite.config.ts` 的 `server.proxy["/api"]`）。
 * - 地址只允许 `http`/`https`，不得包含用户名、密码或 URL fragment。
 */
import type {
  RuntimeBatchDetail,
  RuntimeBatchListResponse,
  RuntimeClearOperationLogsResponse,
  RuntimeConfirmPreviewResponse,
  RuntimeCreateBatchResponse,
  RuntimeCreatePreviewResponse,
  RuntimeCreateSensitiveTermRequest,
  RuntimeHealthResponse,
  RuntimeOcrStatusResponse,
  RuntimeOperationLogListResponse,
  RuntimeOperationLogStatistics,
  RuntimeOperationLogStorageStatus,
  RuntimeClearSandboxPinRequest,
  RuntimeFileBayCandidatesResponse,
  RuntimeFileBayRepositoryResponse,
  RuntimeFileBayStatusResponse,
  RuntimeFileBayTestResponse,
  RuntimeFileBayUploadRequest,
  RuntimeFileBayUploadResponse,
  RuntimePreviewDetail,
  RuntimeRetryResponse,
  RuntimeRulesResponse,
  RuntimeSandboxStatusResponse,
  RuntimeSensitiveTerm,
  RuntimeSensitiveTermCategoriesResponse,
  RuntimeSensitiveTermsImportResponse,
  RuntimeSensitiveTermsResponse,
  RuntimeSensitiveTermsStats,
  RuntimeSetSandboxPinRequest,
  RuntimeUnlockSandboxRequest,
  RuntimeUpdateSensitiveTermRequest,
} from "@/types/runtime";

function resolveBaseUrl(raw: string | undefined): string {
  if (!raw) {
    return "";
  }

  const url = new URL(raw);
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw new Error("VITE_RUNTIME_API_URL 必须是 http/https 地址");
  }
  if (url.username || url.password || url.hash) {
    throw new Error("VITE_RUNTIME_API_URL 不得包含用户名、密码或 URL fragment");
  }
  return url.origin;
}

export const runtimeBaseUrl = resolveBaseUrl(
  import.meta.env.VITE_RUNTIME_API_URL as string | undefined
);

export type RuntimeFetchResult<T> =
  | { ok: true; data: T }
  | { ok: false; reason: "network" }
  | { ok: false; reason: "http"; status: number; code?: string; message?: string; retryable?: boolean }
  | { ok: false; reason: "parse" };

/**
 * 把非 2xx 响应体解析为结构化 `code/message/retryable`；响应不是合法安全
 * JSON（或缺少这三个字段）时，只返回 `status`，不把原始响应体透出给调用方
 * ——调用方必须使用固定的通用错误文案，不得回显未解析的原文。
 */
async function parseErrorBody(
  response: Response
): Promise<{ code?: string; message?: string; retryable?: boolean }> {
  try {
    const parsed = (await response.json()) as Partial<{
      code: unknown;
      message: unknown;
      retryable: unknown;
    }>;
    if (typeof parsed.code === "string" && typeof parsed.message === "string") {
      return {
        code: parsed.code,
        message: parsed.message,
        retryable: parsed.retryable === true,
      };
    }
  } catch {
    // 响应体不是合法 JSON：保持空对象，调用方回退到固定通用文案。
  }
  return {};
}

async function fetchRuntimeJson<T>(path: string, init?: RequestInit): Promise<RuntimeFetchResult<T>> {
  let response: Response;
  try {
    response = await fetch(`${runtimeBaseUrl}${path}`, {
      ...init,
      credentials: "omit",
      cache: "no-store",
      headers: { Accept: "application/json", ...init?.headers },
    });
  } catch (error) {
    // 请求被取消时 abort 会抛 DOMException；统一归类为 network，
    // 调用方按需区分，不暴露底层错误名称给界面。
    if (error instanceof DOMException && error.name === "AbortError") {
      return { ok: false, reason: "network" };
    }
    return { ok: false, reason: "network" };
  }

  if (!response.ok) {
    // 当响应不是 application/json 时，大概率是代理/网关层返回的错误页面
    //（如 Vite 开发代理在 Runtime 未就绪时返回的 500 HTML），应归类为网络/
    // 连接错误，让界面提示用户检查 Runtime 状态并显式重试，而不是显示通用
    // 文件处理失败文案。Runtime 业务错误始终返回 JSON 的 code/message。
    const contentType = response.headers.get("content-type") ?? "";
    if (!contentType.includes("application/json")) {
      // 413 常由网关/Nginx 在请求进入 Runtime 之前返回，仍然属于明确的 HTTP
      // 失败，调用方可据此给出“上传体积超过限制”的准确提示，而不是误报为
      // Runtime 未连接。
      if (response.status === 413) {
        return { ok: false, reason: "http", status: 413 };
      }
      return { ok: false, reason: "network" };
    }
    const details = await parseErrorBody(response);
    return { ok: false, reason: "http", status: response.status, ...details };
  }

  try {
    const data = (await response.json()) as T;
    return { ok: true, data };
  } catch {
    return { ok: false, reason: "parse" };
  }
}

export function fetchRuntimeHealth(): Promise<RuntimeFetchResult<RuntimeHealthResponse>> {
  return fetchRuntimeJson<RuntimeHealthResponse>("/api/v1/health");
}

export function fetchRuntimeOcrStatus(): Promise<RuntimeFetchResult<RuntimeOcrStatusResponse>> {
  return fetchRuntimeJson<RuntimeOcrStatusResponse>("/api/v1/ocr/status");
}

export function fetchRuntimeRules(): Promise<RuntimeFetchResult<RuntimeRulesResponse>> {
  return fetchRuntimeJson<RuntimeRulesResponse>("/api/v1/rules");
}

export function fetchRuntimeBatch(batchId: string): Promise<RuntimeFetchResult<RuntimeBatchDetail>> {
  return fetchRuntimeJson<RuntimeBatchDetail>(`/api/v1/batches/${encodeURIComponent(batchId)}`);
}

/**
 * 批次列表：现有只读 `GET /api/v1/batches` 契约。只做一次性汇总读取，
 * 逐批详情由调用方按需单独请求，本函数不做任何 N+1 拉取。
 */
export function fetchRuntimeBatches(): Promise<RuntimeFetchResult<RuntimeBatchListResponse>> {
  return fetchRuntimeJson<RuntimeBatchListResponse>("/api/v1/batches");
}

/**
 * 创建批次：现有 `POST /api/v1/batches` 契约，multipart 字段 `files`（可重复）
 * 与 `rule_ids`（JSON 字符串数组），`credentials: "omit"`，不携带任何桌面
 * FileBay 凭据。只做 HTTP 适配，不在浏览器侧解析文件或计算脱敏结果。
 */
export function createRuntimeBatch(
  files: File[],
  ruleIds: string[]
): Promise<RuntimeFetchResult<RuntimeCreateBatchResponse>> {
  const form = new FormData();
  for (const file of files) {
    form.append("files", file, file.name);
  }
  form.append("rule_ids", JSON.stringify(ruleIds));
  return fetchRuntimeJson<RuntimeCreateBatchResponse>("/api/v1/batches", {
    method: "POST",
    body: form,
  });
}

export function retryRuntimeFile(fileId: string): Promise<RuntimeFetchResult<RuntimeRetryResponse>> {
  return fetchRuntimeJson<RuntimeRetryResponse>(`/api/v1/files/${encodeURIComponent(fileId)}/retry`, {
    method: "POST",
  });
}

export type RuntimeActionResult =
  | { ok: true }
  | { ok: false; reason: "network" }
  | { ok: false; reason: "http"; status: number; code?: string; message?: string; retryable?: boolean };

/**
 * 创建预览会话：`POST /api/v1/previews`，与 `createRuntimeBatch` 同样的
 * multipart 字段与校验语义，只是落地到临时预览会话而非正式批次——服务端
 * 在确认前不会写入任何正式 batch/artifact/mapping 记录。
 */
export function createRuntimePreview(
  files: File[],
  ruleIds: string[],
  signal?: AbortSignal
): Promise<RuntimeFetchResult<RuntimeCreatePreviewResponse>> {
  const form = new FormData();
  for (const file of files) {
    form.append("files", file, file.name);
  }
  form.append("rule_ids", JSON.stringify(ruleIds));
  return fetchRuntimeJson<RuntimeCreatePreviewResponse>("/api/v1/previews", {
    method: "POST",
    body: form,
    signal,
  });
}

/** 预览会话状态轮询：`GET /api/v1/previews/{preview_id}`，仅安全元数据。 */
export function fetchRuntimePreview(previewId: string): Promise<RuntimeFetchResult<RuntimePreviewDetail>> {
  return fetchRuntimeJson<RuntimePreviewDetail>(`/api/v1/previews/${encodeURIComponent(previewId)}`);
}

export type RuntimeTextFetchResult =
  | { ok: true; text: string }
  | { ok: false; reason: "network" }
  | { ok: false; reason: "http"; status: number; code?: string; message?: string; retryable?: boolean };

/**
 * 按需拉取单个预览文件的脱敏 Markdown 正文：`GET
 * /api/v1/previews/{preview_id}/files/{file_id}/content`。只在用户主动选中
 * 某个 Ready 文件时调用——调用方不得预取全部文件内容，也不得缓存/落盘/
 * 写入 console 或 URL。
 */
export async function fetchRuntimePreviewFileContent(
  previewId: string,
  fileId: string
): Promise<RuntimeTextFetchResult> {
  let response: Response;
  try {
    response = await fetch(
      `${runtimeBaseUrl}/api/v1/previews/${encodeURIComponent(previewId)}/files/${encodeURIComponent(fileId)}/content`,
      { credentials: "omit", cache: "no-store" }
    );
  } catch {
    return { ok: false, reason: "network" };
  }

  if (!response.ok) {
    const details = await parseErrorBody(response);
    return { ok: false, reason: "http", status: response.status, ...details };
  }

  const text = await response.text();
  return { ok: true, text };
}

/**
 * 确认预览：`POST /api/v1/previews/{preview_id}/confirm`。提交的是预览阶段
 * 已经算好的结果，不会触发服务端重新解析/OCR/脱敏。
 */
export function confirmRuntimePreview(
  previewId: string
): Promise<RuntimeFetchResult<RuntimeConfirmPreviewResponse>> {
  return fetchRuntimeJson<RuntimeConfirmPreviewResponse>(
    `/api/v1/previews/${encodeURIComponent(previewId)}/confirm`,
    { method: "POST" }
  );
}

/**
 * 取消预览：`DELETE /api/v1/previews/{preview_id}`，清空该会话全部临时数据。
 * 响应无正文（204），复用 `RuntimeActionResult`。
 */
export async function cancelRuntimePreview(previewId: string): Promise<RuntimeActionResult> {
  let response: Response;
  try {
    response = await fetch(`${runtimeBaseUrl}/api/v1/previews/${encodeURIComponent(previewId)}`, {
      method: "DELETE",
      credentials: "omit",
      cache: "no-store",
    });
  } catch {
    return { ok: false, reason: "network" };
  }

  if (!response.ok) {
    const details = await parseErrorBody(response);
    return { ok: false, reason: "http", status: response.status, ...details };
  }
  return { ok: true };
}

/**
 * 从文件名中提取一个安全的、不含路径分隔符或控制字符的“主干名”，
 * 用于拼装下载文件名——展示名视为不可信输入，不直接用作 HTML/路径/
 * `Content-Disposition` 指令的一部分。
 */
function safeStem(displayName: string): string {
  const withoutPath = displayName.replace(/^.*[\\/]/, "");
  // eslint-disable-next-line no-control-regex
  const withoutControlChars = withoutPath.replace(/[\x00-\x1f\x7f]/g, "");
  const withoutReservedChars = withoutControlChars.replace(/[<>:"/\\|?*]/g, "_");
  const dot = withoutReservedChars.lastIndexOf(".");
  const stem = dot > 0 ? withoutReservedChars.slice(0, dot) : withoutReservedChars;
  return stem.trim() || "artifact";
}

/**
 * 下载脱敏产物：`GET /api/v1/artifacts/{artifact_id}`，只使用编码后的
 * artifact ID，不接受或拼接任何服务器路径。下载后立即释放对象 URL。
 */
export async function downloadRuntimeArtifact(
  artifactId: string,
  displayName: string
): Promise<RuntimeActionResult> {
  let response: Response;
  try {
    response = await fetch(`${runtimeBaseUrl}/api/v1/artifacts/${encodeURIComponent(artifactId)}`, {
      credentials: "omit",
      cache: "no-store",
    });
  } catch {
    return { ok: false, reason: "network" };
  }

  if (!response.ok) {
    const details = await parseErrorBody(response);
    return { ok: false, reason: "http", status: response.status, ...details };
  }

  const blob = await response.blob();
  const objectUrl = URL.createObjectURL(blob);
  try {
    const anchor = document.createElement("a");
    anchor.href = objectUrl;
    anchor.download = `${safeStem(displayName)}.masked.md`;
    document.body.append(anchor);
    anchor.click();
    anchor.remove();
  } finally {
    URL.revokeObjectURL(objectUrl);
  }
  return { ok: true };
}

export type RuntimeRestoreResult =
  | { ok: true; count: number }
  | { ok: false; reason: "network" }
  | { ok: false; reason: "http"; status: number; code?: string; message?: string; retryable?: boolean }
  | { ok: false; reason: "invalid-count" };

/**
 * 服务器内部映射恢复：`POST /api/v1/artifacts/{artifact_id}/restore`，只使用
 * 编码后的 artifact ID，不接受、不拼接、不展示服务器 `.cmap`/mapping 路径或
 * 内容。成功响应不是 JSON，而是 `text/markdown` 正文 + `X-Restored-Entity-Count`
 * 响应头；该头必须是有限正整数才能触发下载，否则视为失败且不生成任何产物
 * （零恢复、非法计数头与网络/HTTP 错误一律不下载）。
 */
export async function restoreRuntimeArtifact(
  artifactId: string,
  displayName: string
): Promise<RuntimeRestoreResult> {
  let response: Response;
  try {
    response = await fetch(`${runtimeBaseUrl}/api/v1/artifacts/${encodeURIComponent(artifactId)}/restore`, {
      method: "POST",
      credentials: "omit",
      cache: "no-store",
    });
  } catch {
    return { ok: false, reason: "network" };
  }

  if (!response.ok) {
    const details = await parseErrorBody(response);
    return { ok: false, reason: "http", status: response.status, ...details };
  }

  const countHeader = response.headers.get("X-Restored-Entity-Count");
  const count = countHeader === null ? Number.NaN : Number(countHeader);
  if (!Number.isFinite(count) || !Number.isInteger(count) || count <= 0) {
    return { ok: false, reason: "invalid-count" };
  }

  const blob = await response.blob();
  const objectUrl = URL.createObjectURL(blob);
  try {
    const anchor = document.createElement("a");
    anchor.href = objectUrl;
    anchor.download = `${safeStem(displayName)}.restored.md`;
    document.body.append(anchor);
    anchor.click();
    anchor.remove();
  } finally {
    URL.revokeObjectURL(objectUrl);
  }
  return { ok: true, count };
}

/**
 * 敏感词库 CRUD/CSV 导入导出的浏览器 HTTP 适配。所有写操作使用
 * `credentials: "omit"`、`cache: "no-store"`；词条内容只经过这些函数原样
 * 转发，不在浏览器侧做任何额外的持久化、日志或缓存。
 */

export interface RuntimeSensitiveTermListParams {
  category?: string;
  query?: string;
  enabledOnly?: boolean;
}

export function fetchRuntimeSensitiveTerms(
  params: RuntimeSensitiveTermListParams = {}
): Promise<RuntimeFetchResult<RuntimeSensitiveTermsResponse>> {
  const search = new URLSearchParams();
  if (params.category) search.set("category", params.category);
  if (params.query) search.set("query", params.query);
  if (params.enabledOnly) search.set("enabled_only", "true");
  const queryString = search.toString();
  return fetchRuntimeJson<RuntimeSensitiveTermsResponse>(
    `/api/v1/sensitive-terms${queryString ? `?${queryString}` : ""}`
  );
}

export function createRuntimeSensitiveTerm(
  request: RuntimeCreateSensitiveTermRequest
): Promise<RuntimeFetchResult<RuntimeSensitiveTerm>> {
  return fetchRuntimeJson<RuntimeSensitiveTerm>("/api/v1/sensitive-terms", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(request),
  });
}

export function updateRuntimeSensitiveTerm(
  id: string,
  request: RuntimeUpdateSensitiveTermRequest
): Promise<RuntimeFetchResult<RuntimeSensitiveTerm>> {
  return fetchRuntimeJson<RuntimeSensitiveTerm>(`/api/v1/sensitive-terms/${encodeURIComponent(id)}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(request),
  });
}

export async function deleteRuntimeSensitiveTerm(id: string): Promise<RuntimeActionResult> {
  let response: Response;
  try {
    response = await fetch(`${runtimeBaseUrl}/api/v1/sensitive-terms/${encodeURIComponent(id)}`, {
      method: "DELETE",
      credentials: "omit",
      cache: "no-store",
    });
  } catch {
    return { ok: false, reason: "network" };
  }

  if (!response.ok) {
    const details = await parseErrorBody(response);
    return { ok: false, reason: "http", status: response.status, ...details };
  }
  return { ok: true };
}

export function fetchRuntimeSensitiveTermCategories(): Promise<
  RuntimeFetchResult<RuntimeSensitiveTermCategoriesResponse>
> {
  return fetchRuntimeJson<RuntimeSensitiveTermCategoriesResponse>("/api/v1/sensitive-terms/categories");
}

export function fetchRuntimeSensitiveTermsStats(): Promise<RuntimeFetchResult<RuntimeSensitiveTermsStats>> {
  return fetchRuntimeJson<RuntimeSensitiveTermsStats>("/api/v1/sensitive-terms/stats");
}

/**
 * CSV 导入：`multipart` 字段名固定为 `file`，只接受浏览器原生文件选择，不
 * 调用桌面对话框。
 */
export function importRuntimeSensitiveTermsCsv(
  file: File
): Promise<RuntimeFetchResult<RuntimeSensitiveTermsImportResponse>> {
  const form = new FormData();
  form.append("file", file, file.name);
  return fetchRuntimeJson<RuntimeSensitiveTermsImportResponse>("/api/v1/sensitive-terms/import", {
    method: "POST",
    body: form,
  });
}

/**
 * CSV 导出：`GET /api/v1/sensitive-terms/export` 返回的响应体直接触发浏览器
 * 下载，默认文件名 `sensitive_terms.csv`，不暴露服务器路径。
 */
export async function downloadRuntimeSensitiveTermsCsv(): Promise<RuntimeActionResult> {
  let response: Response;
  try {
    response = await fetch(`${runtimeBaseUrl}/api/v1/sensitive-terms/export`, {
      credentials: "omit",
      cache: "no-store",
    });
  } catch {
    return { ok: false, reason: "network" };
  }

  if (!response.ok) {
    const details = await parseErrorBody(response);
    return { ok: false, reason: "http", status: response.status, ...details };
  }

  const blob = await response.blob();
  const objectUrl = URL.createObjectURL(blob);
  try {
    const anchor = document.createElement("a");
    anchor.href = objectUrl;
    anchor.download = "sensitive_terms.csv";
    document.body.append(anchor);
    anchor.click();
    anchor.remove();
  } finally {
    URL.revokeObjectURL(objectUrl);
  }
  return { ok: true };
}

/**
 * 浏览器操作日志读取与清空。全部使用 `credentials: "omit"`、
 * `cache: "no-store"`；只读取 Runtime 已有的安全投影字段，不在浏览器侧
 * 拼接或推断服务器路径、SQL 或统计口径。
 */

export interface RuntimeOperationLogListParams {
  page?: number;
  pageSize?: number;
  level?: string;
  status?: string;
  batchId?: string;
}

export function fetchRuntimeOperationLogs(
  params: RuntimeOperationLogListParams = {}
): Promise<RuntimeFetchResult<RuntimeOperationLogListResponse>> {
  const search = new URLSearchParams();
  if (params.page) search.set("page", String(params.page));
  if (params.pageSize) search.set("page_size", String(params.pageSize));
  if (params.level) search.set("level", params.level);
  if (params.status) search.set("status", params.status);
  if (params.batchId) search.set("batch_id", params.batchId);
  const queryString = search.toString();
  return fetchRuntimeJson<RuntimeOperationLogListResponse>(
    `/api/v1/operation-logs${queryString ? `?${queryString}` : ""}`
  );
}

export function fetchRuntimeOperationLogStatistics(): Promise<
  RuntimeFetchResult<RuntimeOperationLogStatistics>
> {
  return fetchRuntimeJson<RuntimeOperationLogStatistics>("/api/v1/operation-logs/statistics");
}

export function fetchRuntimeOperationLogStorageStatus(): Promise<
  RuntimeFetchResult<RuntimeOperationLogStorageStatus>
> {
  return fetchRuntimeJson<RuntimeOperationLogStorageStatus>("/api/v1/operation-logs/storage-status");
}

/**
 * 清空事件日志：`DELETE /api/v1/operation-logs`。调用方必须在用户明确
 * 二次确认后才能调用本函数（8.8）——本函数本身不做任何确认逻辑，取消
 * 确认时调用方不得调用它。
 */
export function clearRuntimeOperationLogs(): Promise<
  RuntimeFetchResult<RuntimeClearOperationLogsResponse>
> {
  return fetchRuntimeJson<RuntimeClearOperationLogsResponse>("/api/v1/operation-logs", {
    method: "DELETE",
  });
}

/**
 * 浏览器沙箱/PIN 的 Runtime HTTP 适配。这是同一服务器系统用户共享的沙箱
 * 操作状态，不是账号登录、RBAC、多租户或 API 鉴权。全部使用
 * `credentials: "omit"`、`cache: "no-store"`；PIN 只存在于请求体的瞬时内存
 * 中，本模块不把 PIN 写入 URL/query、不缓存、不落盘、不写 console。
 */

export function fetchRuntimeSandboxStatus(): Promise<RuntimeFetchResult<RuntimeSandboxStatusResponse>> {
  return fetchRuntimeJson<RuntimeSandboxStatusResponse>("/api/v1/sandbox/status");
}

/**
 * 设置/重新设置 PIN：`PUT /api/v1/sandbox/pin`。首次设置省略 `current_pin`；
 * 已有 PIN 时必须提供正确的 `current_pin` 才能替换，服务端校验，浏览器侧
 * 不做旧 PIN 是否正确的预判断。
 */
export function setRuntimeSandboxPin(
  request: RuntimeSetSandboxPinRequest
): Promise<RuntimeFetchResult<RuntimeSandboxStatusResponse>> {
  return fetchRuntimeJson<RuntimeSandboxStatusResponse>("/api/v1/sandbox/pin", {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(request),
  });
}

/** 手动锁定：`POST /api/v1/sandbox/lock`，不需要提交 PIN。 */
export function lockRuntimeSandbox(): Promise<RuntimeFetchResult<RuntimeSandboxStatusResponse>> {
  return fetchRuntimeJson<RuntimeSandboxStatusResponse>("/api/v1/sandbox/lock", {
    method: "POST",
  });
}

/**
 * 验证解锁：`POST /api/v1/sandbox/unlock`。受 Runtime 全局尝试限制——错误
 * PIN 累计触发限速时，服务端返回 429 与安全的 `retryable`/`code`，本函数
 * 原样转发，不在浏览器侧重试或推断剩余等待时间。
 */
export function unlockRuntimeSandbox(
  request: RuntimeUnlockSandboxRequest
): Promise<RuntimeFetchResult<RuntimeSandboxStatusResponse>> {
  return fetchRuntimeJson<RuntimeSandboxStatusResponse>("/api/v1/sandbox/unlock", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(request),
  });
}

/**
 * 清除 PIN：`DELETE /api/v1/sandbox/pin`，必须提交当前 PIN 并受相同尝试
 * 限制；成功后服务端清除 hash 并回到 unlocked。
 */
export function clearRuntimeSandboxPin(
  request: RuntimeClearSandboxPinRequest
): Promise<RuntimeFetchResult<RuntimeSandboxStatusResponse>> {
  return fetchRuntimeJson<RuntimeSandboxStatusResponse>("/api/v1/sandbox/pin", {
    method: "DELETE",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(request),
  });
}

/**
 * FileBay 浏览器上传的 Runtime HTTP 适配。URL、Token、owner、repo 只由
 * 服务器管理员环境变量提供——本模块不提供任何设置/回显这些值的方法，
 * 也不在浏览器侧拼接远程路径。`credentials: "omit"`、`cache: "no-store"`；
 * 只有 `testRuntimeFileBayConnection`/`createRuntimeFileBayRepository`/
 * `confirmRuntimeFileBayUploads` 这三个显式、用户主动触发的动作会让服务端
 * 访问 FileBay；`fetchRuntimeFileBayStatus`/`fetchRuntimeFileBayCandidates`
 * 从不触发出站请求。
 */

export function fetchRuntimeFileBayStatus(): Promise<RuntimeFetchResult<RuntimeFileBayStatusResponse>> {
  return fetchRuntimeJson<RuntimeFileBayStatusResponse>("/api/v1/filebay/status");
}

/** 用户主动点击“测试连接”：`POST /api/v1/filebay/test`。 */
export function testRuntimeFileBayConnection(): Promise<RuntimeFetchResult<RuntimeFileBayTestResponse>> {
  return fetchRuntimeJson<RuntimeFileBayTestResponse>("/api/v1/filebay/test", {
    method: "POST",
  });
}

/**
 * 创建配置中指定名称的私有仓库：`POST /api/v1/filebay/repository`。请求体
 * 固定为空对象——浏览器不能提交仓库名、owner 或 `private` 参数。
 */
export function createRuntimeFileBayRepository(): Promise<
  RuntimeFetchResult<RuntimeFileBayRepositoryResponse>
> {
  return fetchRuntimeJson<RuntimeFileBayRepositoryResponse>("/api/v1/filebay/repository", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: "{}",
  });
}

/**
 * 查询某批次可上传候选：`GET /api/v1/filebay/batches/{batch_id}/candidates`。
 * 只返回该批次中 `Completed` 的脱敏 Markdown；不触发任何 FileBay 出站请求。
 */
export function fetchRuntimeFileBayCandidates(
  batchId: string
): Promise<RuntimeFetchResult<RuntimeFileBayCandidatesResponse>> {
  return fetchRuntimeJson<RuntimeFileBayCandidatesResponse>(
    `/api/v1/filebay/batches/${encodeURIComponent(batchId)}/candidates`
  );
}

/**
 * 确认上传：`POST /api/v1/filebay/uploads`。请求体只包含 `artifact_ids`
 * ——不提交远程路径、文件内容、URL、Token 或本地路径；远程路径由服务端
 * 生成。调用方必须已经过用户显式确认，本函数本身不做二次确认。
 */
export function confirmRuntimeFileBayUploads(
  request: RuntimeFileBayUploadRequest
): Promise<RuntimeFetchResult<RuntimeFileBayUploadResponse>> {
  return fetchRuntimeJson<RuntimeFileBayUploadResponse>("/api/v1/filebay/uploads", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(request),
  });
}
