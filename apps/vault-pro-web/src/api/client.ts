import type {
  BatchDetail,
  BatchListResponse,
  CreateBatchResponse,
  ErrorResponse,
  OcrStatusResponse,
  RetryResponse,
  RulesResponse,
} from "../types";
import { maskedArtifactFilename } from "../formatCatalog";

const configuredBaseUrl = import.meta.env.VITE_RUNTIME_API_URL ?? "http://127.0.0.1:8787";

function validateBaseUrl(value: string): string {
  const url = new URL(value);
  const loopbackHosts = new Set(["127.0.0.1", "localhost", "[::1]"]);
  if (url.protocol !== "http:" || !loopbackHosts.has(url.hostname)) {
    throw new Error("Runtime 地址必须是本机 HTTP loopback 地址");
  }
  return url.origin;
}

export const runtimeBaseUrl = validateBaseUrl(configuredBaseUrl);

export class RuntimeApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly retryable: boolean;

  constructor(status: number, response: ErrorResponse) {
    super(response.message);
    this.name = "RuntimeApiError";
    this.status = status;
    this.code = response.code;
    this.retryable = response.retryable;
  }
}

async function requestJson<T>(path: string, init?: RequestInit): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${runtimeBaseUrl}${path}`, {
      ...init,
      credentials: "omit",
      cache: "no-store",
      headers: {
        Accept: "application/json",
        ...init?.headers,
      },
    });
  } catch {
    throw new RuntimeApiError(0, {
      code: "RUNTIME_UNAVAILABLE",
      message: "无法连接本机 Runtime，请确认服务已启动后重试。",
      retryable: true,
    });
  }
  if (!response.ok) {
    let error: ErrorResponse = {
      code: `HTTP_${response.status}`,
      message: "Runtime 请求失败，请检查输入或稍后重试。",
      retryable: response.status >= 500,
    };
    try {
      const parsed = (await response.json()) as Partial<ErrorResponse>;
      if (typeof parsed.code === "string" && typeof parsed.message === "string") {
        error = {
          code: parsed.code,
          message: parsed.message,
          retryable: parsed.retryable === true,
        };
      }
    } catch {
      // Keep the safe generic error; never expose an untrusted response body.
    }
    throw new RuntimeApiError(response.status, error);
  }
  return (await response.json()) as T;
}

export const api = {
  rules: () => requestJson<RulesResponse>("/api/v1/rules"),
  ocrStatus: () => requestJson<OcrStatusResponse>("/api/v1/ocr/status"),
  batches: () => requestJson<BatchListResponse>("/api/v1/batches"),
  batch: (batchId: string) =>
    requestJson<BatchDetail>(`/api/v1/batches/${encodeURIComponent(batchId)}`),
  retry: (fileId: string) =>
    requestJson<RetryResponse>(`/api/v1/files/${encodeURIComponent(fileId)}/retry`, {
      method: "POST",
    }),
  createBatch: async (files: File[], ruleIds: string[]) => {
    const form = new FormData();
    for (const file of files) form.append("files", file, file.name);
    form.append("rule_ids", JSON.stringify(ruleIds));
    return requestJson<CreateBatchResponse>("/api/v1/batches", {
      method: "POST",
      body: form,
    });
  },

  restoreArtifact: async (artifactId: string) => {
    const response = await fetch(`${runtimeBaseUrl}/api/v1/artifacts/${encodeURIComponent(artifactId)}/restore`, {
      method: "POST", credentials: "omit",
    });
    if (!response.ok) {
      let error: ErrorResponse = { code: `HTTP_${response.status}`, message: "请求失败。", retryable: false };
      try { const parsed = (await response.json()) as Partial<ErrorResponse>; if (parsed.code) error.code = parsed.code; if (parsed.message) error.message = parsed.message; } catch {}
      throw new RuntimeApiError(response.status, error);
    }
    const blob = await response.blob();
    const count = parseInt(response.headers.get("X-Restored-Entity-Count") || "0", 10);
    const filename = response.headers.get("content-disposition")?.match(/filename="(.+)"/)?.[1] || "restored.md";
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a"); a.href = url; a.download = filename;
    document.body.append(a); a.click(); a.remove();
    URL.revokeObjectURL(url);
    return { count };
  },
};

export async function downloadArtifact(artifactId: string, displayName: string): Promise<void> {
  let response: Response;
  try {
    response = await fetch(
      `${runtimeBaseUrl}/api/v1/artifacts/${encodeURIComponent(artifactId)}`,
      { credentials: "omit", cache: "no-store" },
    );
  } catch {
    throw new RuntimeApiError(0, {
      code: "RUNTIME_UNAVAILABLE",
      message: "下载失败：无法连接本机 Runtime。",
      retryable: true,
    });
  }
  if (!response.ok) {
    throw new RuntimeApiError(response.status, {
      code: `HTTP_${response.status}`,
      message: response.status === 404 ? "产物不存在或尚不可下载。" : "产物下载失败。",
      retryable: response.status >= 500,
    });
  }
  const blob = await response.blob();
  const objectUrl = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = objectUrl;
  anchor.download = maskedArtifactFilename(displayName);
  document.body.append(anchor);
  anchor.click();
  anchor.remove();
  URL.revokeObjectURL(objectUrl);
}
