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

/**
 * Classify a response after `fetch` has received it. The body is consumed only
 * to recover a complete, type-checked Runtime error payload; malformed or
 * incomplete payloads become status-only failures.
 */
export async function classifyRuntimeHttpResponse(response: Response): Promise<RuntimeHttpFailure> {
  try {
    const payload: unknown = await response.json();
    if (isRuntimeErrorPayload(payload)) {
      return {
        ok: false,
        reason: "http",
        status: response.status,
        code: payload.code,
        message: payload.message,
        retryable: payload.retryable,
      };
    }
  } catch {
    // Non-JSON, empty, and malformed bodies deliberately remain status-only.
  }

  return { ok: false, reason: "http", status: response.status };
}

/** Keep fetch rejects, including AbortError before any response, in the existing network bucket. */
export function classifyRuntimeFetchError(_error: unknown): RuntimeNetworkFailure {
  return { ok: false, reason: "network" };
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
