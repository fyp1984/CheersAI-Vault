// Copyright 2026 CheersAI. Licensed under Apache-2.0.
import { describe, it, expect } from "vitest";
import {
  classifyRuntimeFetchError,
  classifyRuntimeHttpResponse,
  parseRuntimeJsonResponse,
} from "@/lib/runtime/errorClassification";

function response(body: string | null, status: number, contentType?: string): Response {
  return new Response(body, {
    status,
    headers: contentType ? { "content-type": contentType } : undefined,
  });
}

describe("errorClassification util (vitest spec port)", () => {
  it("413 non-JSON maps to the fixed safe PAYLOAD_TOO_LARGE contract", async () => {
    await expect(
      classifyRuntimeHttpResponse(response("<html>fake upstream body</html>", 413, "text/html")),
    ).resolves.toEqual({
      ok: false,
      reason: "http",
      status: 413,
      code: "PAYLOAD_TOO_LARGE",
      message: "上传文件大小超过限制，请拆分或压缩后重试。",
    });
  });

  it("502 HTML maps to BAD_GATEWAY without leaking token, path, or stack text", async () => {
    const result = await classifyRuntimeHttpResponse(
      response(
        "token=FAKE_TOKEN_ONLY path=/fake/server/private stack=Error: fake\\n at fake.js:1:1",
        502,
        "text/html",
      ),
    );
    expect(result).toEqual({
      ok: false,
      reason: "http",
      status: 502,
      code: "BAD_GATEWAY",
      message: "本地 Runtime 网关不可达，请确认服务已启动后重试。",
    });
    const serialized = JSON.stringify(result);
    expect(serialized.includes("FAKE_TOKEN_ONLY")).toBe(false);
    expect(serialized.includes("/fake/server/private")).toBe(false);
    expect(serialized.includes("fake.js:1:1")).toBe(false);
  });

  it("503 plain text and empty body map to the fixed SERVICE_UNAVAILABLE contract", async () => {
    await expect(
      classifyRuntimeHttpResponse(response("upstream unavailable", 503, "text/plain")),
    ).resolves.toEqual({
      ok: false,
      reason: "http",
      status: 503,
      code: "SERVICE_UNAVAILABLE",
      message: "本地 Runtime 暂不可用，请确认服务已启动后重试。",
    });
    await expect(classifyRuntimeHttpResponse(response(null, 503))).resolves.toEqual({
      ok: false,
      reason: "http",
      status: 503,
      code: "SERVICE_UNAVAILABLE",
      message: "本地 Runtime 暂不可用，请确认服务已启动后重试。",
    });
  });

  it("complete JSON error preserves code/retryable and sanitizes English message", async () => {
    await expect(
      classifyRuntimeHttpResponse(
        response(
          JSON.stringify({
            code: "INVALID_QUERY",
            message: "safe message",
            retryable: false,
            extra: "ignored",
          }),
          400,
          "application/json",
        ),
      ),
    ).resolves.toEqual({
      ok: false,
      reason: "http",
      status: 400,
      code: "INVALID_QUERY",
      message: "请求参数或上传文件无效，请校验后重试。",
      retryable: false,
    });
  });

  it("classifyRuntimeFetchError covers network errors without leaking PII", () => {
    const r = classifyRuntimeFetchError(new TypeError("fetch failed for /users/13900000000/records"));
    expect(r.ok).toBe(false);
    expect(r.reason).toBe("network");
    expect(JSON.stringify(r).includes("13900000000")).toBe(false);
  });
});
